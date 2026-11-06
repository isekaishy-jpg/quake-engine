//! Recording a frame's draw calls.

use engine_core::math::Vec3;
use engine_map::{Map, Shading};
use vulkane::safe::ShaderStageFlags;

use super::{
    ANIMATION_RATE, Backend, CanvasPass, GpuAlias, GpuBuffer, GpuMap, GpuSprite, INDEX_TYPE_U32,
    Layouts, MODEL_OFFSET_SIZE, Pipelines, Scene,
};
use crate::alias::{POSE_CORNER_BYTES, push_bytes};
use crate::renderer::View;
use crate::sprite::{
    MAX_GPU_PARTICLE_DRAWS, MAX_PARTICLE_DRAWS, ViewAxes, gpu_particle_bytes, sprite_axes,
    sprite_push_bytes,
};
use crate::world::{Batch, BatchKind};

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
    /// The map.
    map: &'a GpuMap,
    /// Its visible indices, for this frame.
    indices: &'a GpuBuffer,
    /// Whether models get hull outlines.
    outline_hull: bool,
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
        rec.bind_graphics_pipeline(&canvas.pipeline);
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
            map: self.map.as_ref()?,
            indices: &self.map.as_ref()?.indices[self.slot],
            outline_hull: self.post.outline_hull,
        })
    }
}

impl Recorder<'_> {
    /// Whether `batch` goes through the depth pass: what's solid all over. A detail
    /// that fades can discard pixels, so its depth can't be known ahead of it, and
    /// nor can that of a shader that moves its surface.
    fn depth_first(&self, map: &Map, batch: &Batch) -> bool {
        let fades = map
            .textures
            .get(batch.texture as usize)
            .and_then(|texture| texture.material.as_ref())
            .is_some_and(|material| material.detail_fade);
        let shader = self.map.texture_shaders.get(batch.texture as usize);
        let surface = shader.and_then(|shader| self.pipelines.surfaces.get(*shader));
        batch.kind == BatchKind::Material
            && !fades
            && surface.is_some_and(|surface| surface.after_depth_safe)
    }

    /// Records everything in `scene`.
    pub(super) fn record(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: &Scene<'_>,
        view: &View,
        size: (u32, u32),
    ) {
        self.record_sky(rec, scene, size);
        self.record_depth(rec, scene, size);
        self.record_world(rec, scene, view, size, false);
        self.record_models(rec, scene);
        self.record_world(rec, scene, view, size, true);
        self.record_sprites(rec, scene, view);
        self.record_particles(rec, scene);
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
        rec.bind_graphics_descriptor_sets(&self.layouts.world, 0, &[&self.map.frame_set, set]);
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
        let mut bound = None;
        let mut draw = |rec: &mut vulkane::safe::CommandBufferRecording<'_>,
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
            if bound != Some((batch.kind, after_depth, plain, shader)) {
                if window {
                    rec.bind_graphics_pipeline(&self.pipelines.sky_window);
                } else if plain {
                    rec.bind_graphics_pipeline(&self.pipelines.plain_after_depth);
                } else if after_depth {
                    rec.bind_graphics_pipeline(&surface.after_depth);
                } else {
                    rec.bind_graphics_pipeline(surface.get(batch.kind));
                }
                bound = Some((batch.kind, after_depth, plain, shader));
            }
            let set_layout = if window {
                layout
            } else {
                &self.layouts.material
            };
            let set = match (&map.sky_set, batch.kind) {
                (Some(sky), BatchKind::Sky) => sky,
                _ => set,
            };
            rec.bind_graphics_descriptor_sets(set_layout, 0, &[&map.frame_set, set]);
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
                    order.push((distance, Vec3::ZERO, batch, first, count, 0));
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
                    order.push((
                        distance,
                        instance.origin,
                        range.batch,
                        first,
                        range.count,
                        instance.frame,
                    ));
                }
            }
            order.sort_by(|a, b| b.0.total_cmp(&a.0));
            for (_, origin, batch, first, count, frame) in order {
                let bytes = offset_bytes(origin);
                rec.push_constants(layout, ShaderStageFlags::VERTEX, 0, &bytes);
                draw(rec, batch, first, count, frame);
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

    /// Records the models.
    pub(super) fn record_models(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: &Scene<'_>,
    ) {
        if scene.drawables.entities.is_empty() {
            return;
        }
        if self.outline_hull {
            rec.bind_graphics_pipeline(&self.pipelines.alias_hull);
            self.record_model_draws(rec, scene, true);
        }
        rec.bind_graphics_pipeline(&self.pipelines.alias);
        self.record_model_draws(rec, scene, false);
    }

    /// Records each model with the pipeline that's bound. A hull leaves out
    /// what's kept in front, which is the view's own weapon.
    fn record_model_draws(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: &Scene<'_>,
        hull: bool,
    ) {
        let layout = &self.layouts.alias;
        for draw in scene.drawables.entities {
            if hull && draw.in_front {
                continue;
            }
            let Some(gpu) = self.models.get(draw.model as usize) else {
                continue;
            };
            let skin = gpu
                .skin_sets
                .get(draw.skin as usize)
                .or(gpu.skin_sets.first());
            let Some(skin) = skin else {
                continue;
            };
            let corners = gpu.model.corners.len();
            let pose_size = (corners * POSE_CORNER_BYTES) as u64;
            rec.bind_vertex_buffers(
                0,
                &[
                    (&gpu.poses.buffer, u64::from(draw.pose) * pose_size),
                    (&gpu.poses.buffer, u64::from(draw.next_pose) * pose_size),
                    (&gpu.uvs.buffer, 0),
                ],
            );
            rec.bind_graphics_descriptor_sets(layout, 0, &[&self.map.frame_set, skin]);
            rec.push_constants(
                layout,
                ShaderStageFlags::VERTEX,
                0,
                &push_bytes(draw, &gpu.model),
            );
            rec.draw(u32::try_from(corners).unwrap_or(0), 1, 0, 0);
        }
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

    /// Records the particles.
    pub(super) fn record_particles(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: &Scene<'_>,
    ) {
        let count = scene.drawables.particles.len().min(MAX_PARTICLE_DRAWS);
        if count > 0 {
            // What covers comes first, as what adds doesn't care what's behind it.
            let list = &scene.drawables.particles[..count];
            let over = list.iter().take_while(|particle| !particle.adds).count();
            let over = u32::try_from(over).unwrap_or(0);
            let all = u32::try_from(count).unwrap_or(0);
            rec.bind_graphics_descriptor_sets(&self.layouts.particle, 0, &[&self.map.frame_set]);
            rec.bind_vertex_buffers(0, &[(&self.particles.buffer, 0)]);
            if over > 0 {
                rec.bind_graphics_pipeline(&self.pipelines.particle.over);
                rec.draw(6, over, 0, 0);
            }
            if all > over {
                rec.bind_graphics_pipeline(&self.pipelines.particle.add);
                rec.draw(6, all - over, 0, over);
            }
        }
        let batches = scene
            .drawables
            .gpu_particles
            .iter()
            .take(MAX_GPU_PARTICLE_DRAWS);
        for batch in batches.filter(|batch| batch.count > 0) {
            let layout = &self.layouts.gpu_particle;
            rec.bind_graphics_descriptor_sets(layout, 0, &[&self.map.frame_set]);
            let pipelines = &self.pipelines.particle;
            rec.bind_graphics_pipeline(if batch.adds {
                &pipelines.gpu_add
            } else {
                &pipelines.gpu_over
            });
            rec.push_constants(
                layout,
                ShaderStageFlags::VERTEX,
                0,
                &gpu_particle_bytes(batch),
            );
            rec.draw(6, batch.count, 0, 0);
        }
    }
}
