//! Recording a frame's draw calls.

use std::cell::Cell;

use engine_core::math::Vec3;
use engine_map::{Map, Shading, SharedPart};
use vulkane::safe::{DescriptorSet, GraphicsPipeline, ShaderStageFlags};

use super::clock::{GpuClock, Span};
use super::deform::{MovingJob, bind_figure, bind_moving};
use super::pipelines::CANVAS_SCREEN_BYTES;
use super::{
    ANIMATION_RATE, Backend, CanvasPass, GpuAlias, GpuBuffer, GpuMap, GpuMesh, GpuSprite,
    INDEX_TYPE_U32, Layouts, MODEL_OFFSET_SIZE, ParticlePipelines, Pipelines, Scene,
    SurfacePipelines,
};
use crate::instances::InstanceBatch;
use crate::renderer::View;
use crate::sprite::{
    MAX_GPU_PARTICLE_DRAWS, ViewAxes, gpu_particle_bytes, sprite_axes, sprite_push_bytes,
};
use crate::world::{Batch, BatchKind, batch_kind};

/// The corners of the cube the sky sphere is drawn on.
const SKY_SPHERE_CORNERS: u32 = 36;

/// What recording a frame needs.
pub(super) struct Recorder<'a> {
    /// The layouts.
    layouts: &'a Layouts,
    /// The pipelines.
    pipelines: &'a Pipelines,
    /// Every model.
    models: &'a [GpuAlias],
    /// Every sprite.
    sprites: &'a [GpuSprite],
    /// The particles to draw.
    particles: &'a GpuBuffer,
    /// What draws particles with no shader of their own.
    default_particles: &'a ParticlePipelines,
    /// The shader of each particle, in the order they're drawn.
    particle_order: &'a [u32],
    /// The map.
    map: &'a GpuMap,
    /// Its visible indices, for this frame.
    indices: &'a GpuBuffer,
    /// The copies of shared meshes to draw, for this frame.
    instances: &'a GpuBuffer,
    /// Where each mesh's copies are.
    instance_batches: &'a [InstanceBatch],
    /// The middle of each copy.
    instance_centers: &'a [Vec3],
    /// Which frame in flight this is.
    slot: usize,
    /// Times what the GPU does.
    clock: &'a GpuClock,
    /// Whether models get hull outlines.
    outline_hull: bool,
}

/// Something see-through, to be drawn far to near with everything else that is.
enum SeeThrough<'a> {
    /// Faces of the world, or of a brush model at `origin`.
    Faces {
        /// Where the model has moved to.
        origin: Vec3,
        /// Which batch.
        batch: u32,
        /// The first index.
        first: u32,
        /// How many.
        count: u32,
        /// The model's frame.
        frame: u16,
    },
    /// A part of one copy of a shared mesh.
    Copy(&'a GpuMesh, SharedPart, u32),
    /// A part of something that moves.
    Moving(&'a MovingJob, SharedPart),
}

/// Whether `batch`'s material does nothing the plain shader leaves out.
fn plain_material(map: &Map, batch: &Batch) -> bool {
    let material = map
        .textures
        .get(batch.texture as usize)
        .and_then(|texture| texture.material.as_ref());
    material.is_some_and(|material| {
        material.shader.is_none()
            && material.metal_rough.is_none()
            && material.shading == Shading::Lit
            && !material.triplanar
            && !material.hex_tiling
            && material.flow.is_none()
            && material.parallax == 0.0
            && material.height.is_none()
            && material.layers.is_empty()
            && material.detail.is_none()
            && material.emissive.is_none()
            && material.scroll == [0.0; 2]
            && material.flipbook == [0.0; 3]
    })
}

/// A model's position, as push constant bytes.
pub(super) fn offset_bytes(origin: Vec3) -> [u8; MODEL_OFFSET_SIZE as usize] {
    let mut bytes = [0; MODEL_OFFSET_SIZE as usize];
    for (chunk, value) in bytes
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip([origin.x, origin.y, origin.z, 0.0])
    {
        *chunk = value.to_le_bytes();
    }
    bytes
}

/// The texture to show for `texture` at animation `step`, on something showing
/// `frame`, from `R_TextureAnimation`.
pub(super) fn animated_texture(map: &Map, texture: u32, step: u32, frame: u16) -> u32 {
    let base = map.textures.get(texture as usize);
    let texture = base
        .filter(|_| frame != 0)
        .and_then(|base| base.alternate)
        .unwrap_or(texture);
    map.textures
        .get(texture as usize)
        .and_then(|texture| texture.animation.as_ref())
        .and_then(|animation| {
            let count = u32::try_from(animation.frames.len()).ok()?;
            let at = (animation.position + step) % count;
            animation.frames.get(at as usize).copied()
        })
        .unwrap_or(texture)
}

impl Backend {
    /// Records the canvas.
    pub(super) fn record_canvas(
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        canvas: Option<&CanvasPass>,
        size: (u32, u32),
        (pipeline, screen): (Option<&GraphicsPipeline>, [f32; 4]),
    ) {
        let Some(canvas) =
            canvas.filter(|canvas| !canvas.batches.is_empty() || !canvas.ui.batches.is_empty())
        else {
            return;
        };
        #[expect(
            clippy::cast_precision_loss,
            reason = "window sizes are far below 2^24"
        )]
        rec.set_viewport(0.0, 0.0, size.0 as f32, size.1 as f32);
        rec.set_scissor(0, 0, size.0, size.1);
        rec.bind_graphics_pipeline(pipeline.unwrap_or(&canvas.pipeline));
        let mut told = [0_u8; CANVAS_SCREEN_BYTES as usize];
        for (chunk, value) in told.as_chunks_mut::<4>().0.iter_mut().zip(screen) {
            *chunk = value.to_le_bytes();
        }
        rec.push_constants(&canvas.layout, ShaderStageFlags::FRAGMENT, 0, &told);
        rec.bind_vertex_buffers(0, &[(&canvas.vertices[canvas.slot].buffer, 0)]);
        for batch in &canvas.batches {
            let Some(picture) = canvas.pictures.get(batch.picture as usize) else {
                continue;
            };
            rec.bind_graphics_descriptor_sets(&canvas.layout, 0, &[&picture.set]);
            rec.draw(batch.count, 1, batch.first, 0);
        }
        for batch in &canvas.ui.batches {
            let Some(picture) = canvas.pictures.get(batch.picture as usize) else {
                continue;
            };
            let [left, top, right, bottom] = batch.clip;
            if right <= left || bottom <= top {
                continue;
            }
            rec.set_scissor(
                i32::try_from(left).unwrap_or(0),
                i32::try_from(top).unwrap_or(0),
                right - left,
                bottom - top,
            );
            rec.bind_graphics_descriptor_sets(&canvas.layout, 0, &[&picture.set]);
            rec.draw(batch.count, 1, canvas.ui_first + batch.first, 0);
        }
    }

    /// What recording a frame needs, once there's a map.
    pub(super) fn recorder(&self) -> Option<Recorder<'_>> {
        Some(Recorder {
            layouts: &self.layouts,
            pipelines: &self.pipelines,
            models: &self.models,
            sprites: &self.sprites,
            particles: &self.particles[self.slot],
            default_particles: &self.particle_pipelines,
            particle_order: &self.particle_order,
            map: self.map.as_ref()?,
            indices: &self.map.as_ref()?.indices[self.slot],
            instances: &self.map.as_ref()?.instances[self.slot],
            instance_batches: &self.map.as_ref()?.instance_batches[self.slot],
            instance_centers: &self.map.as_ref()?.instance_centers[self.slot],
            slot: self.slot,
            clock: &self.clock,
            outline_hull: self.post.outline_hull,
        })
    }
}

impl Recorder<'_> {
    /// Whether `batch` goes through the depth pass: what's solid all over. A detail
    /// that fades can discard pixels, so its depth can't be known ahead of it, and
    /// nor can that of a shader that moves its surface.
    fn depth_first(&self, map: &Map, batch: &Batch) -> bool {
        batch.kind == BatchKind::Material && self.texture_depth_first(map, batch.texture)
    }

    /// Whether a mesh's part goes through the depth pass, by the same rule as a face.
    fn part_depth_first(&self, map: &Map, texture: u32) -> bool {
        let kind = map.textures.get(texture as usize).and_then(batch_kind);
        kind == Some(BatchKind::Material) && self.texture_depth_first(map, texture)
    }

    /// Whether what's solid with `texture` can have its depth drawn ahead of it.
    fn texture_depth_first(&self, map: &Map, texture: u32) -> bool {
        let fades = map
            .textures
            .get(texture as usize)
            .and_then(|texture| texture.material.as_ref())
            .is_some_and(|material| material.detail_fade);
        let shader = self.map.texture_shaders.get(texture as usize);
        let surface = shader.and_then(|shader| self.pipelines.surfaces.get(*shader));
        !fades && surface.is_some_and(|surface| surface.after_depth_safe)
    }

    /// Records the depth of meshes. Their pixels cost as much as a face's, and they overlap more.
    fn record_mesh_depth(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: &Scene<'_>,
    ) {
        let map = self.map;
        rec.bind_graphics_pipeline(&self.pipelines.mesh_depth);
        rec.bind_graphics_descriptor_sets(&self.layouts.material, 0, &[&map.frame_set]);
        for batch in self.instance_batches {
            let Some(mesh) = map.shared_meshes.get(batch.mesh as usize) else {
                continue;
            };
            let Some(parts) = mesh.levels.get(batch.level as usize) else {
                continue;
            };
            rec.bind_vertex_buffers(
                0,
                &[
                    (&mesh.places.buffer, 0),
                    (&mesh.uvs.buffer, 0),
                    (&self.instances.buffer, 0),
                ],
            );
            rec.bind_index_buffer(&mesh.indices.buffer, 0, INDEX_TYPE_U32);
            for part in parts {
                if self.part_depth_first(scene.map, part.texture) {
                    rec.draw_indexed(part.count, batch.count, part.first, 0, batch.first);
                }
            }
        }
        let moving = &map.moving;
        for job in moving.jobs(self.slot).iter().filter(|job| job.in_view) {
            let Some(mesh) = moving.meshes.get(job.mesh as usize) else {
                continue;
            };
            if bind_moving(rec, moving, job, self.slot, true).is_none() {
                continue;
            }
            for part in mesh.parts(job.level) {
                if self.part_depth_first(scene.map, part.texture) {
                    rec.draw_indexed(part.count, 1, part.first, 0, job.instance);
                }
            }
        }
    }

    /// Records all that's solid in `scene`.
    pub(super) fn record_solid(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: &Scene<'_>,
        view: &View,
        size: (u32, u32),
    ) {
        self.record_ahead(rec, scene, size);
        self.record_shaded(rec, scene, view, size);
    }

    /// Records what comes before anything solid is shaded: the sky, and depth.
    pub(super) fn record_ahead(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: &Scene<'_>,
        size: (u32, u32),
    ) {
        self.record_sky(rec, scene, size);
        self.record_depth(rec, scene, size);
    }

    /// Records what's solid, shaded, over the depth that's there.
    pub(super) fn record_shaded(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: &Scene<'_>,
        view: &View,
        size: (u32, u32),
    ) {
        self.record_world(rec, scene, view, size, false);
        self.record_meshes(rec, scene, view);
        self.record_land(rec, scene, false);
        self.record_models(rec);
    }

    /// Records what's see-through in `scene`, and sprites.
    pub(super) fn record_see_through(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: &Scene<'_>,
        view: &View,
        size: (u32, u32),
    ) {
        self.record_world(rec, scene, view, size, true);
        self.record_sprites(rec, scene, view);
    }

    /// Records the sky sphere, if there is one.
    fn record_sky(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: &Scene<'_>,
        size: (u32, u32),
    ) {
        let Some(set) = self
            .map
            .sky_set
            .as_ref()
            .filter(|_| scene.map.sky.is_some())
        else {
            return;
        };
        #[expect(
            clippy::cast_precision_loss,
            reason = "window sizes are far below 2^24"
        )]
        rec.set_viewport(0.0, 0.0, size.0 as f32, size.1 as f32);
        rec.set_scissor(0, 0, size.0, size.1);
        rec.bind_graphics_pipeline(&self.pipelines.sky_sphere);
        rec.bind_graphics_descriptor_sets(&self.layouts.material, 0, &[&self.map.frame_set, set]);
        rec.draw(SKY_SPHERE_CORNERS, 1, 0, 0);
    }

    /// Records the depth of what's solid. Shading is most of a frame's cost, and
    /// surfaces are drawn by texture, not by distance, so without this whatever
    /// lies behind what's in view gets shaded too.
    fn record_depth(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: &Scene<'_>,
        size: (u32, u32),
    ) {
        let map = self.map;
        let layout = &self.layouts.world;
        #[expect(
            clippy::cast_precision_loss,
            reason = "window sizes are far below 2^24"
        )]
        rec.set_viewport(0.0, 0.0, size.0 as f32, size.1 as f32);
        rec.set_scissor(0, 0, size.0, size.1);
        rec.bind_graphics_pipeline(&self.pipelines.depth_only);
        rec.bind_graphics_descriptor_sets(layout, 0, &[&map.frame_set]);
        rec.bind_vertex_buffers(0, &[(&map.vertices.buffer, 0)]);
        rec.bind_index_buffer(&self.indices.buffer, 0, INDEX_TYPE_U32);
        let wanted = |batch: u32| {
            let found = scene.mesh.batches.get(batch as usize);
            found.is_some_and(|batch| self.depth_first(scene.map, batch))
        };
        rec.push_constants(
            layout,
            ShaderStageFlags::VERTEX,
            0,
            &offset_bytes(Vec3::ZERO),
        );
        for (batch, &(first, count)) in (0_u32..).zip(&scene.visible.ranges) {
            if count > 0 && wanted(batch) {
                rec.draw_indexed(count, 1, first, 0, 0);
            }
        }
        for instance in scene.drawables.instances.iter().filter(|i| i.visible) {
            let Some(ranges) = scene.mesh.model_ranges.get(instance.model as usize) else {
                continue;
            };
            let bytes = offset_bytes(instance.origin);
            rec.push_constants(layout, ShaderStageFlags::VERTEX, 0, &bytes);
            for range in ranges.iter().filter(|range| wanted(range.batch)) {
                let first = scene.mesh.world_index_count + range.first;
                rec.draw_indexed(range.count, 1, first, 0, 0);
            }
        }
        self.record_mesh_depth(rec, scene);
        self.record_land(rec, scene, true);
    }

    /// Records the land, or its depth.
    fn record_land(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: &Scene<'_>,
        depth: bool,
    ) {
        let map = self.map;
        let Some(land) = &map.terrain else {
            return;
        };
        let layout = &self.layouts.terrain;
        rec.bind_vertex_buffers(0, &[(&land.nodes[self.slot].buffer, 0)]);
        rec.bind_index_buffer(&land.indices.buffer, 0, INDEX_TYPE_U32);
        if depth {
            rec.bind_graphics_pipeline(&self.pipelines.land_depth);
            rec.bind_graphics_descriptor_sets(layout, 0, &[&map.frame_set]);
        }
        rec.bind_graphics_descriptor_sets(layout, 2, &[&land.set]);
        for batch in &land.batches[self.slot] {
            let Some(sheet) = scene.map.terrain.get(batch.sheet as usize) else {
                continue;
            };
            let Some(&(first, count)) = land.grids.get(batch.grid as usize) else {
                continue;
            };
            if !depth {
                let texture = sheet.texture as usize;
                let Some(Some(set)) = map.texture_sets.get(texture) else {
                    continue;
                };
                let shader = map.texture_shaders.get(texture).copied().unwrap_or(0);
                let Some(surface) = self.pipelines.surfaces.get(shader) else {
                    continue;
                };
                rec.bind_graphics_pipeline(if surface.after_depth_safe {
                    &surface.land_after_depth
                } else {
                    &surface.land
                });
                rec.bind_graphics_descriptor_sets(layout, 0, &[&map.frame_set, set]);
                rec.bind_graphics_descriptor_sets(layout, 2, &[&land.set]);
            }
            rec.draw_indexed(count, batch.count, first, 0, batch.first);
        }
    }

    /// Records the world and brush models: what's see-through, or the rest.
    pub(super) fn record_world(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: &Scene<'_>,
        view: &View,
        size: (u32, u32),
        see_through: bool,
    ) {
        let map = self.map;
        let layout = &self.layouts.world;
        #[expect(
            clippy::cast_precision_loss,
            reason = "window sizes are far below 2^24"
        )]
        rec.set_viewport(0.0, 0.0, size.0 as f32, size.1 as f32);
        rec.set_scissor(0, 0, size.0, size.1);
        rec.bind_vertex_buffers(0, &[(&map.vertices.buffer, 0)]);
        rec.bind_index_buffer(&self.indices.buffer, 0, INDEX_TYPE_U32);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "time is never negative, and the frame count wraps by design"
        )]
        let step = (view.time * ANIMATION_RATE) as u32;
        let bound = Cell::new(None);
        let draw = |rec: &mut vulkane::safe::CommandBufferRecording<'_>,
                    batch: u32,
                    first: u32,
                    count: u32,
                    frame: u16| {
            let Some(batch) = scene.mesh.batches.get(batch as usize) else {
                return;
            };
            if (batch.kind == BatchKind::Blended) != see_through {
                return;
            }
            let texture = animated_texture(scene.map, batch.texture, step, frame);
            let Some(Some(set)) = map.texture_sets.get(texture as usize) else {
                return;
            };
            let after_depth = self.depth_first(scene.map, batch);
            let plain = after_depth && plain_material(scene.map, batch);
            let shader = map.texture_shaders.get(texture as usize).copied();
            let shader = shader.unwrap_or(0);
            let Some(surface) = self.pipelines.surfaces.get(shader) else {
                return;
            };
            let window = batch.kind == BatchKind::Sky && map.sky_set.is_some();
            if bound.get() != Some((batch.kind, after_depth, plain, shader)) {
                if window {
                    rec.bind_graphics_pipeline(&self.pipelines.sky_window);
                } else if plain {
                    rec.bind_graphics_pipeline(&self.pipelines.plain_after_depth);
                } else if after_depth {
                    rec.bind_graphics_pipeline(&surface.after_depth);
                } else {
                    rec.bind_graphics_pipeline(surface.get(batch.kind));
                }
                bound.set(Some((batch.kind, after_depth, plain, shader)));
            }
            let set = match (&map.sky_set, batch.kind) {
                (Some(sky), BatchKind::Sky) => sky,
                _ => set,
            };
            rec.bind_graphics_descriptor_sets(&self.layouts.material, 0, &[&map.frame_set, set]);
            rec.draw_indexed(count, 1, first, 0, 0);
        };
        if see_through {
            // Far to near, so each shows through the next.
            let mut order = Vec::new();
            let center = |batch: u32| {
                let place = scene.mesh.batch_centers.get(batch as usize);
                place.copied().unwrap_or(Vec3::ZERO)
            };
            for (batch, &(first, count)) in (0_u32..).zip(&scene.visible.ranges) {
                if count > 0 {
                    let distance = (center(batch) - view.origin).length();
                    let faces = SeeThrough::Faces {
                        origin: Vec3::ZERO,
                        batch,
                        first,
                        count,
                        frame: 0,
                    };
                    order.push((distance, faces));
                }
            }
            for instance in scene.drawables.instances.iter().filter(|i| i.visible) {
                let Some(ranges) = scene.mesh.model_ranges.get(instance.model as usize) else {
                    continue;
                };
                for range in ranges {
                    let first = scene.mesh.world_index_count + range.first;
                    let at = center(range.batch) + instance.origin;
                    let distance = (at - view.origin).length();
                    let faces = SeeThrough::Faces {
                        origin: instance.origin,
                        batch: range.batch,
                        first,
                        count: range.count,
                        frame: instance.frame,
                    };
                    order.push((distance, faces));
                }
            }
            self.see_through_meshes(scene, view, &mut order);
            order.sort_by(|a, b| b.0.total_cmp(&a.0));
            let mut on_faces = true;
            for (_, item) in order {
                match item {
                    SeeThrough::Faces {
                        origin,
                        batch,
                        first,
                        count,
                        frame,
                    } => {
                        if !on_faces {
                            rec.bind_vertex_buffers(0, &[(&map.vertices.buffer, 0)]);
                            rec.bind_index_buffer(&self.indices.buffer, 0, INDEX_TYPE_U32);
                            bound.set(None);
                            on_faces = true;
                        }
                        let bytes = offset_bytes(origin);
                        rec.push_constants(layout, ShaderStageFlags::VERTEX, 0, &bytes);
                        draw(rec, batch, first, count, frame);
                    }
                    SeeThrough::Copy(mesh, part, instance) => {
                        on_faces = false;
                        self.bind_copies(rec, mesh);
                        self.draw_part(rec, scene, step, &part, instance);
                    }
                    SeeThrough::Moving(job, part) => {
                        on_faces = false;
                        let moving = &map.moving;
                        if bind_moving(rec, moving, job, self.slot, true).is_some() {
                            self.draw_part(rec, scene, step, &part, job.instance);
                        }
                    }
                }
            }
            return;
        }
        rec.push_constants(
            layout,
            ShaderStageFlags::VERTEX,
            0,
            &offset_bytes(Vec3::ZERO),
        );
        for (batch, &(first, count)) in (0_u32..).zip(&scene.visible.ranges) {
            if count > 0 {
                draw(rec, batch, first, count, 0);
            }
        }
        for instance in scene
            .drawables
            .instances
            .iter()
            .filter(|instance| instance.visible)
        {
            let Some(ranges) = scene.mesh.model_ranges.get(instance.model as usize) else {
                continue;
            };
            let bytes = offset_bytes(instance.origin);
            rec.push_constants(layout, ShaderStageFlags::VERTEX, 0, &bytes);
            for range in ranges {
                let first = scene.mesh.world_index_count + range.first;
                draw(rec, range.batch, first, range.count, instance.frame);
            }
        }
    }

    /// Adds what's see-through in copies and what moves to `order`. A copy's drawn by itself, as
    /// what's see-through has to go in its place among the rest.
    fn see_through_meshes<'s>(
        &'s self,
        scene: &Scene<'_>,
        view: &View,
        order: &mut Vec<(f32, SeeThrough<'s>)>,
    ) {
        let map = self.map;
        let blended = |part: &&SharedPart| {
            let texture = scene.map.textures.get(part.texture as usize);
            texture.and_then(batch_kind) == Some(BatchKind::Blended)
        };
        for batch in self.instance_batches {
            let Some(mesh) = map.shared_meshes.get(batch.mesh as usize) else {
                continue;
            };
            let Some(parts) = mesh.levels.get(batch.level as usize) else {
                continue;
            };
            for part in parts.iter().filter(blended) {
                for instance in batch.first..batch.first + batch.count {
                    let center = self.instance_centers.get(instance as usize);
                    let center = center.copied().unwrap_or(Vec3::ZERO);
                    let distance = (center - view.origin).length();
                    order.push((distance, SeeThrough::Copy(mesh, *part, instance)));
                }
            }
        }
        let moving = &map.moving;
        for job in moving.jobs(self.slot).iter().filter(|job| job.in_view) {
            let Some(mesh) = moving.meshes.get(job.mesh as usize) else {
                continue;
            };
            let distance = (job.center - view.origin).length();
            for part in mesh.parts(job.level).iter().filter(blended) {
                order.push((distance, SeeThrough::Moving(job, *part)));
            }
        }
    }

    /// Binds what a shared mesh's copies draw from.
    fn bind_copies(&self, rec: &mut vulkane::safe::CommandBufferRecording<'_>, mesh: &GpuMesh) {
        rec.bind_vertex_buffers(
            0,
            &[
                (&mesh.places.buffer, 0),
                (&mesh.uvs.buffer, 0),
                (&self.instances.buffer, 0),
            ],
        );
        rec.bind_index_buffer(&mesh.indices.buffer, 0, INDEX_TYPE_U32);
    }

    /// Draws one part of a see-through mesh that's bound.
    fn draw_part(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: &Scene<'_>,
        step: u32,
        part: &SharedPart,
        instance: u32,
    ) {
        let map = self.map;
        let texture = animated_texture(scene.map, part.texture, step, 0);
        let Some(Some(set)) = map.texture_sets.get(texture as usize) else {
            return;
        };
        let shader = map.texture_shaders.get(texture as usize).copied();
        let Some(surface) = self.pipelines.surfaces.get(shader.unwrap_or(0)) else {
            return;
        };
        rec.bind_graphics_pipeline(surface.mesh(BatchKind::Blended));
        rec.bind_graphics_descriptor_sets(&self.layouts.material, 0, &[&map.frame_set, set]);
        rec.draw_indexed(part.count, 1, part.first, 0, instance);
    }

    /// Records what's solid or has holes in the copies of shared meshes. Each part of a mesh is
    /// one draw of all its copies. What's see-through is sorted with the world's, instead.
    fn record_meshes(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: &Scene<'_>,
        view: &View,
    ) {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "time is never negative, and the frame count wraps by design"
        )]
        let step = (view.time * ANIMATION_RATE) as u32;
        let map = self.map;
        for batch in self.instance_batches {
            let Some(mesh) = map.shared_meshes.get(batch.mesh as usize) else {
                continue;
            };
            self.bind_copies(rec, mesh);
            let Some(parts) = mesh.levels.get(batch.level as usize) else {
                continue;
            };
            for part in parts {
                let Some(kind) = scene
                    .map
                    .textures
                    .get(part.texture as usize)
                    .and_then(batch_kind)
                else {
                    continue;
                };
                if kind == BatchKind::Blended {
                    continue;
                }
                let texture = animated_texture(scene.map, part.texture, step, 0);
                let Some(Some(set)) = map.texture_sets.get(texture as usize) else {
                    continue;
                };
                let shader = map
                    .texture_shaders
                    .get(texture as usize)
                    .copied()
                    .unwrap_or(0);
                let Some(surface) = self.pipelines.surfaces.get(shader) else {
                    continue;
                };
                rec.bind_graphics_pipeline(self.mesh_pipeline(scene, surface, kind, part.texture));
                rec.bind_graphics_descriptor_sets(
                    &self.layouts.material,
                    0,
                    &[&map.frame_set, set],
                );
                rec.draw_indexed(part.count, batch.count, part.first, 0, batch.first);
            }
        }
        self.clock.start(rec, self.slot, Span::Moving);
        self.record_moving(rec, scene, step);
        self.clock.end(rec, self.slot, Span::Moving);
    }

    /// The pipeline for a mesh's part, which needn't write depth if that's been drawn.
    fn mesh_pipeline<'p>(
        &self,
        scene: &Scene<'_>,
        surface: &'p SurfacePipelines,
        kind: BatchKind,
        texture: u32,
    ) -> &'p GraphicsPipeline {
        if self.part_depth_first(scene.map, texture) {
            &surface.mesh_after_depth
        } else {
            surface.mesh(kind)
        }
    }

    /// Records what moved this frame, which is drawn as any other mesh is from here.
    fn record_moving(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: &Scene<'_>,
        step: u32,
    ) {
        let map = self.map;
        let moving = &map.moving;
        for job in moving.jobs(self.slot).iter().filter(|job| job.in_view) {
            if bind_moving(rec, moving, job, self.slot, true).is_none() {
                continue;
            }
            let Some(mesh) = moving.meshes.get(job.mesh as usize) else {
                continue;
            };
            if self.outline_hull {
                rec.bind_graphics_pipeline(&self.pipelines.mesh_hull);
                rec.bind_graphics_descriptor_sets(&self.layouts.material, 0, &[&map.frame_set]);
                for part in mesh.shadow_parts(job.level) {
                    rec.draw_indexed(part.count, 1, part.first, 0, job.instance);
                }
            }
            for part in mesh.parts(job.level) {
                let texture = scene.map.textures.get(part.texture as usize);
                let Some(kind) = texture.and_then(batch_kind) else {
                    continue;
                };
                if kind == BatchKind::Blended {
                    continue;
                }
                let texture = animated_texture(scene.map, part.texture, step, 0);
                let Some(Some(set)) = map.texture_sets.get(texture as usize) else {
                    continue;
                };
                let shader = map.texture_shaders.get(texture as usize).copied();
                let Some(surface) = self.pipelines.surfaces.get(shader.unwrap_or(0)) else {
                    continue;
                };
                rec.bind_graphics_pipeline(self.mesh_pipeline(scene, surface, kind, part.texture));
                rec.bind_graphics_descriptor_sets(
                    &self.layouts.material,
                    0,
                    &[&map.frame_set, set],
                );
                rec.draw_indexed(part.count, 1, part.first, 0, job.instance);
            }
        }
    }

    /// Records the fixture's models, whose corners were moved with everything else's.
    pub(super) fn record_models(&self, rec: &mut vulkane::safe::CommandBufferRecording<'_>) {
        let moving = &self.map.moving;
        let figures = moving.figures(self.slot);
        if figures.is_empty() {
            return;
        }
        self.clock.start(rec, self.slot, Span::Models);
        if self.outline_hull {
            rec.bind_graphics_pipeline(&self.pipelines.mesh_hull);
            rec.bind_graphics_descriptor_sets(&self.layouts.material, 0, &[&self.map.frame_set]);
            for job in figures.iter().filter(|job| !job.in_front) {
                let Some(gpu) = self.models.get(job.model as usize) else {
                    continue;
                };
                bind_figure(rec, moving, gpu, job.moved_to, self.slot);
                rec.draw_indexed(gpu.index_count, 1, 0, 0, job.instance);
            }
        }
        rec.bind_graphics_pipeline(&self.pipelines.alias);
        for job in figures {
            let Some(gpu) = self.models.get(job.model as usize) else {
                continue;
            };
            let skin = gpu
                .skin_sets
                .get(job.skin as usize)
                .or(gpu.skin_sets.first());
            let Some(skin) = skin else {
                continue;
            };
            bind_figure(rec, moving, gpu, job.moved_to, self.slot);
            rec.bind_graphics_descriptor_sets(&self.layouts.alias, 0, &[&self.map.frame_set, skin]);
            rec.draw_indexed(gpu.index_count, 1, 0, 0, job.instance);
        }
        self.clock.end(rec, self.slot, Span::Models);
    }

    /// Records the sprites.
    pub(super) fn record_sprites(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: &Scene<'_>,
        view: &View,
    ) {
        if scene.drawables.sprites.is_empty() {
            return;
        }
        let layout = &self.layouts.sprite;
        let axes = ViewAxes::new(view.pitch, view.yaw, 0.0);
        rec.bind_graphics_pipeline(&self.pipelines.sprite);
        for draw in scene.drawables.sprites {
            let Some(gpu) = self.sprites.get(draw.sprite as usize) else {
                continue;
            };
            let at = draw.image as usize;
            let (Some(picture), Some(set)) = (gpu.sprite.images.get(at), gpu.sets.get(at)) else {
                continue;
            };
            let facing = gpu.sprite.facing;
            let to_sprite = draw.origin - view.origin;
            let Some(turned) = sprite_axes(facing, &axes, to_sprite, draw.angles) else {
                continue;
            };
            rec.bind_graphics_descriptor_sets(layout, 0, &[&self.map.frame_set, set]);
            rec.push_constants(
                layout,
                ShaderStageFlags::VERTEX,
                0,
                &sprite_push_bytes(draw, picture, turned),
            );
            rec.draw(6, 1, 0, 0);
        }
    }

    /// Whether there are particles to draw.
    pub(super) fn has_particles(&self, scene: &Scene<'_>) -> bool {
        !scene.drawables.particles.is_empty()
            || self
                .map
                .sims
                .as_ref()
                .is_some_and(|sims| !sims.runs[self.slot].is_empty())
            || scene
                .drawables
                .gpu_particles
                .iter()
                .any(|batch| batch.count > 0)
    }

    /// Records `range` of the particles, in their own pass. They're in order, so each run of those
    /// that have one shader is a draw.
    pub(super) fn record_particles(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        _scene: &Scene<'_>,
        size: (u32, u32),
        depth: &DescriptorSet,
        range: std::ops::Range<usize>,
    ) {
        #[expect(
            clippy::cast_precision_loss,
            reason = "window sizes are far below 2^24"
        )]
        rec.set_viewport(0.0, 0.0, size.0 as f32, size.1 as f32);
        rec.set_scissor(0, 0, size.0, size.1);
        if range.is_empty() {
            return;
        }
        let sets = [&self.map.frame_set, depth, &self.map.particle_pictures.set];
        rec.bind_graphics_descriptor_sets(&self.layouts.particle, 0, &sets);
        rec.bind_vertex_buffers(0, &[(&self.particles.buffer, 0)]);
        let mut first = range.start;
        while first < range.end {
            let shader = self.particle_order[first];
            let run = self.particle_order[first..range.end]
                .iter()
                .take_while(|own| **own == shader)
                .count();
            let own = (shader as usize).checked_sub(1);
            let pipelines = own.and_then(|at| self.map.particle_shaders.get(at));
            let pipelines = pipelines.unwrap_or(self.default_particles);
            rec.bind_graphics_pipeline(&pipelines.listed);
            let (many, from) = (
                u32::try_from(run).unwrap_or(0),
                u32::try_from(first).unwrap_or(0),
            );
            rec.draw(6, many, 0, from);
            first += run;
        }
    }

    /// Records the batches of particles the GPU works out.
    pub(super) fn record_worked_out(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: &Scene<'_>,
        depth: &DescriptorSet,
    ) {
        let sets = [&self.map.frame_set, depth, &self.map.particle_pictures.set];
        let batches = scene
            .drawables
            .gpu_particles
            .iter()
            .take(MAX_GPU_PARTICLE_DRAWS);
        for batch in batches.filter(|batch| batch.count > 0) {
            let layout = &self.layouts.gpu_particle;
            rec.bind_graphics_descriptor_sets(layout, 0, &sets);
            rec.bind_graphics_pipeline(&self.default_particles.worked_out);
            rec.push_constants(
                layout,
                ShaderStageFlags::VERTEX,
                0,
                &gpu_particle_bytes(batch),
            );
            rec.draw(6, batch.count, 0, 0);
        }
        if let Some(sims) = &self.map.sims {
            let layout = &self.layouts.sim_particle;
            let sets = [
                &self.map.frame_set,
                depth,
                &self.map.particle_pictures.set,
                &sims.sets[self.slot],
            ];
            for run in &sims.runs[self.slot] {
                rec.bind_graphics_descriptor_sets(layout, 0, &sets);
                let own = (run.shader as usize).checked_sub(1);
                let pipelines = own.and_then(|at| self.map.particle_shaders.get(at));
                let pipelines = pipelines.unwrap_or(self.default_particles);
                rec.bind_graphics_pipeline(&pipelines.simulated);
                let mut push = [0; super::sim::SIM_PUSH as usize];
                push[..4].copy_from_slice(&run.emitter.to_le_bytes());
                rec.push_constants(layout, ShaderStageFlags::VERTEX, 0, &push);
                rec.draw(6, run.count, 0, 0);
            }
        }
    }
}
