//! Shadow maps for moving lights: six tiles of an atlas each, kept until
//! something changes (proposal 0084).

use std::cell::Cell;

use engine_core::entity::ModelInstance;
use engine_core::math::Vec3;
use engine_map::Map;
use engine_shaders::{Module, spirv};
use vulkane::safe::{
    AttachmentDescription, AttachmentLoadOp, AttachmentStoreOp, CompareOp, CullMode, Device,
    Format, Framebuffer, GraphicsPipeline, GraphicsPipelineBuilder, GraphicsShaderStage, Image,
    Image2dCreateInfo, ImageLayout, ImageUsage, InputRate, PhysicalDevice, PipelineLayout,
    PushConstantRange, RenderPass, RenderPassCreateInfo, ShaderModule, ShaderStageFlags,
    VertexInputAttribute, VertexInputBinding,
};

use super::deform::bind_moving;
use super::upload::device_image;
use super::{DEPTH_FORMAT, GpuAlias, GpuImage, GpuMap, INDEX_TYPE_U32, ShadowInstance};
use crate::alias::{EntityDraw, POSE_CORNER_BYTES, model_matrix};
use crate::error::RenderError;
use crate::instances::PLACE_BYTES;
use crate::live::{LiveLight, MAX_SHADOW_LIGHTS, cube_face};
use crate::matrix::multiply;
use crate::world::Vertex;

/// The atlas's side.
const ATLAS_SIZE: u32 = 4096;

/// A tile's side.
const TILE_SIZE: u32 = 512;

/// Tiles along a side of the atlas.
const TILES_ACROSS: u32 = ATLAS_SIZE / TILE_SIZE;

/// How many lights' shadows are redrawn in a frame. The rest wait their turn.
const REDRAWS_PER_FRAME: usize = 2;

/// Bytes of what a tile's draws push: a matrix, an offset and a scale.
const PUSH_SIZE: u32 = 96;

/// The most an alias model's stored position can be along an axis.
const POSE_RANGE: f32 = 255.0;

/// How far a figure's shadow reaches at least, for one that's left.
const GONE_REACH: f32 = 64.0;

/// Some of what shadows are drawn from.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct ShadowRange {
    /// The first index.
    pub(super) first: u32,
    /// How many indices.
    pub(super) count: u32,
    /// Where it's moved to.
    pub(super) offset: Vec3,
    /// The middle and size of it, if a light can miss it.
    pub(super) bounds: Option<(Vec3, f32)>,
}

/// What a shadow slot was last drawn for.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Kept {
    /// Which light.
    id: u32,
    /// Where it was.
    origin: Vec3,
    /// How far it reached.
    radius: f32,
}

/// What about a figure changes its shadow.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Stance {
    /// Which model.
    model: u32,
    /// The pose it's blending from.
    pose: u32,
    /// The pose it's blending to.
    next_pose: u32,
    /// How far between them.
    blend: f32,
    /// Where it is.
    origin: Vec3,
    /// How it's turned.
    angles: Vec3,
}

impl Stance {
    /// A figure's stance. Nothing else about it, such as how it's lit, moves
    /// its shadow.
    const fn of(draw: &EntityDraw) -> Self {
        Self {
            model: draw.model,
            pose: draw.pose,
            next_pose: draw.next_pose,
            blend: draw.blend,
            origin: draw.origin,
            angles: draw.angles,
        }
    }
}

/// A figure to draw into shadows.
pub(super) type Figure<'a> = (&'a GpuAlias, &'a EntityDraw);

/// The figures of `entities`. What's kept in front is the view's own weapon,
/// which casts nothing.
pub(super) fn shadow_figures<'a>(
    models: &'a [GpuAlias],
    entities: &'a [EntityDraw],
) -> Vec<Figure<'a>> {
    entities
        .iter()
        .filter(|draw| !draw.in_front)
        .filter_map(|draw| Some((models.get(draw.model as usize)?, draw)))
        .collect()
}

/// How far from its origin any part of `model` can reach.
fn figure_reach(model: &GpuAlias) -> f32 {
    let (scale, offset) = (model.model.scale, model.model.offset);
    let corner = |x: f32, y: f32, z: f32| {
        (offset + Vec3::new(scale.x * x, scale.y * y, scale.z * z)).length()
    };
    let far = POSE_RANGE;
    [0.0, far]
        .into_iter()
        .flat_map(|x| [(x, 0.0), (x, far)])
        .flat_map(|(x, y)| [corner(x, y, 0.0), corner(x, y, far)])
        .fold(0.0, f32::max)
}

/// Shadow maps, and what's in them.
pub(super) struct Shadows {
    /// The atlas, kept alive for its framebuffer.
    pub(super) atlas: GpuImage,
    /// Starts the atlas off, once.
    first_pass: RenderPass,
    /// Draws into the atlas, keeping what's there.
    pass: RenderPass,
    /// The atlas as a target.
    framebuffer: Framebuffer,
    /// The layout of its draws.
    layout: PipelineLayout,
    /// Draws geometry.
    pipeline: GraphicsPipeline,
    /// Draws alias models.
    figure_pipeline: GraphicsPipeline,
    /// The same as `pipeline`, for meshes, whose corners are smaller.
    mesh_pipeline: GraphicsPipeline,
    /// Empties a tile.
    clear: GraphicsPipeline,
    /// Whether the atlas has been started off.
    ready: Cell<bool>,
    /// What each slot holds.
    kept: Vec<Option<Kept>>,
    /// Where moving models were last frame.
    models: Vec<(u32, Vec3)>,
    /// How figures stood last frame.
    stances: Vec<Stance>,
    /// How many frames each slot has waited for a redraw it needs.
    waiting: [u32; MAX_SHADOW_LIGHTS],
    /// The lights to redraw this frame, by slot.
    redraw: Vec<(usize, LiveLight)>,
}

/// A pass over the atlas's depth.
fn atlas_pass(device: &Device, first: bool) -> Result<RenderPass, RenderError> {
    Ok(RenderPass::new(
        device,
        RenderPassCreateInfo {
            color_attachments: &[],
            depth_attachment: Some(AttachmentDescription {
                format: DEPTH_FORMAT,
                load_op: if first {
                    AttachmentLoadOp::CLEAR
                } else {
                    AttachmentLoadOp::LOAD
                },
                store_op: AttachmentStoreOp::STORE,
                initial_layout: if first {
                    ImageLayout::UNDEFINED
                } else {
                    ImageLayout::SHADER_READ_ONLY_OPTIMAL
                },
                final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }),
        },
    )?)
}

impl Shadows {
    /// Makes the atlas and what draws into it.
    pub(super) fn new(device: &Device, physical: &PhysicalDevice) -> Result<Self, RenderError> {
        let image = Image::new_2d(
            device,
            Image2dCreateInfo {
                format: DEPTH_FORMAT,
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT | ImageUsage::SAMPLED,
            },
        )?;
        let atlas = device_image(device, physical, image, true)?;
        let first_pass = atlas_pass(device, true)?;
        let pass = atlas_pass(device, false)?;
        let framebuffer = Framebuffer::new(device, &pass, &[&atlas.view], ATLAS_SIZE, ATLAS_SIZE)?;
        let layout = PipelineLayout::with_push_constants(
            device,
            &[],
            &[PushConstantRange {
                stage_flags: ShaderStageFlags::VERTEX,
                offset: 0,
                size: PUSH_SIZE,
            }],
        )?;
        let spirv = spirv(&Module::Shadow).map_err(RenderError)?;
        let shader = ShaderModule::from_spirv(device, &spirv)?;
        let bindings = [VertexInputBinding {
            binding: 0,
            stride: u32::try_from(Vertex::SIZE).unwrap_or(0),
            input_rate: InputRate::VERTEX,
        }];
        let attributes = [VertexInputAttribute {
            location: 0,
            binding: 0,
            format: Format::R32G32B32_SFLOAT,
            offset: 0,
        }];
        let pipeline = GraphicsPipelineBuilder::new(&layout, &pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_shadow")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_shadow")
            .color_attachment_count(0)
            .vertex_input(&bindings, &attributes)
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(true, true)
            .depth_bias(1.5, 2.0, 0.0)
            .build(device)?;
        let mesh_bindings = [VertexInputBinding {
            stride: u32::try_from(PLACE_BYTES).unwrap_or(0),
            ..bindings[0]
        }];
        let mesh_pipeline = GraphicsPipelineBuilder::new(&layout, &pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_shadow")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_shadow")
            .color_attachment_count(0)
            .vertex_input(&mesh_bindings, &attributes)
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(true, true)
            .depth_bias(1.5, 2.0, 0.0)
            .build(device)?;
        let pose_stride = u32::try_from(POSE_CORNER_BYTES).unwrap_or(0);
        let pose_binding = |binding| VertexInputBinding {
            binding,
            stride: pose_stride,
            input_rate: InputRate::VERTEX,
        };
        let pose_attribute = |location| VertexInputAttribute {
            location,
            binding: location,
            format: Format::R8G8B8A8_UINT,
            offset: 0,
        };
        let figure_pipeline = GraphicsPipelineBuilder::new(&layout, &pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_figure")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_shadow")
            .color_attachment_count(0)
            .vertex_input(
                &[pose_binding(0), pose_binding(1)],
                &[pose_attribute(0), pose_attribute(1)],
            )
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(true, true)
            .depth_bias(1.5, 2.0, 0.0)
            .build(device)?;
        let clear = GraphicsPipelineBuilder::new(&layout, &pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_clear")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_shadow")
            .color_attachment_count(0)
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(true, true)
            .depth_compare_op(CompareOp::ALWAYS)
            .build(device)?;
        Ok(Self {
            atlas,
            first_pass,
            pass,
            framebuffer,
            layout,
            pipeline,
            figure_pipeline,
            mesh_pipeline,
            clear,
            ready: Cell::new(false),
            kept: vec![None; MAX_SHADOW_LIGHTS],
            models: Vec::new(),
            stances: Vec::new(),
            waiting: [0; MAX_SHADOW_LIGHTS],
            redraw: Vec::new(),
        })
    }

    /// Redraws nothing this frame, for when shadows are off.
    pub(super) fn rest(&mut self) {
        self.redraw.clear();
    }

    /// How many lights' shadows this frame redraws.
    pub(super) fn redraws(&self) -> usize {
        self.redraw.len()
    }

    /// Forgets what's drawn, for a new map.
    pub(super) fn forget(&mut self) {
        self.kept.fill(None);
        self.models.clear();
        self.stances.clear();
        self.waiting.fill(0);
    }

    /// Gives each shadowed light a slot, keeping those that have one, and
    /// works out which slots to redraw this frame. Returns each light's slot.
    pub(super) fn plan(
        &mut self,
        lights: &[LiveLight],
        instances: &[ModelInstance],
        (figures, moving): (&[Figure<'_>], &[(Vec3, f32)]),
        map: &Map,
        eye: Vec3,
    ) -> Vec<Option<usize>> {
        let mut moved = self.moved_models(instances, map);
        moved.extend(self.moved_figures(figures));
        // A moving mesh changes shape where it stands, so there's no telling it's still.
        moved.extend_from_slice(moving);
        let mut slots = vec![None; lights.len()];
        let shadowed: Vec<usize> = (0..lights.len()).filter(|at| lights[*at].shadows).collect();
        // Lights that held a slot keep it, so their tiles stay good.
        for at in &shadowed {
            slots[*at] = self
                .kept
                .iter()
                .position(|kept| kept.is_some_and(|kept| kept.id == lights[*at].id));
        }
        for at in &shadowed {
            if slots[*at].is_some() {
                continue;
            }
            let free = (0..MAX_SHADOW_LIGHTS).find(|slot| !slots.contains(&Some(*slot)));
            slots[*at] = free;
            if let Some(slot) = free {
                self.kept[slot] = None;
            }
        }
        let mut stale: Vec<(u32, f32, usize, LiveLight)> = Vec::new();
        for (at, light) in lights.iter().enumerate() {
            let Some(slot) = slots[at] else {
                continue;
            };
            let same = self.kept[slot]
                .is_some_and(|kept| kept.origin == light.origin && kept.radius == light.radius);
            let touched = moved
                .iter()
                .any(|(middle, size)| (*middle - light.origin).length() < light.radius + size);
            if !same || touched || self.waiting[slot] > 0 {
                let waited = self.waiting[slot];
                stale.push((waited, (light.origin - eye).length(), slot, *light));
            }
        }
        // What's waited longest goes first, then what's nearest, so figures
        // that never stop can't keep a far light from its turn.
        stale.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.total_cmp(&b.1)));
        for (_, _, slot, _) in stale.iter().skip(REDRAWS_PER_FRAME) {
            self.waiting[*slot] += 1;
        }
        stale.truncate(REDRAWS_PER_FRAME);
        self.redraw = stale
            .iter()
            .map(|(_, _, slot, light)| (*slot, *light))
            .collect();
        for (slot, light) in &self.redraw {
            self.waiting[*slot] = 0;
            self.kept[*slot] = Some(Kept {
                id: light.id,
                origin: light.origin,
                radius: light.radius,
            });
        }
        slots
    }

    /// The middle and size of each model that's moved since last frame.
    fn moved_models(&mut self, instances: &[ModelInstance], map: &Map) -> Vec<(Vec3, f32)> {
        let now: Vec<(u32, Vec3)> = instances
            .iter()
            .filter(|i| i.visible)
            .map(|i| (i.model, i.origin))
            .collect();
        let mut moved = Vec::new();
        // What's gone has moved as much as what's arrived.
        let arrived = now.iter().filter(|at| !self.models.contains(at));
        let gone = self.models.iter().filter(|at| !now.contains(at));
        for (model, origin) in arrived.chain(gone) {
            if let Some(found) = map.models.get(*model as usize) {
                let middle = (found.mins + found.maxs) * 0.5 + *origin;
                moved.push((middle, (found.maxs - found.mins).length() * 0.5));
            }
        }
        self.models = now;
        moved
    }

    /// The middle and size of each figure that's moved or changed pose since
    /// last frame.
    fn moved_figures(&mut self, figures: &[Figure<'_>]) -> Vec<(Vec3, f32)> {
        let now: Vec<Stance> = figures.iter().map(|(_, draw)| Stance::of(draw)).collect();
        let mut moved: Vec<(Vec3, f32)> = figures
            .iter()
            .filter(|(_, draw)| !self.stances.contains(&Stance::of(draw)))
            .map(|(model, draw)| (draw.origin, figure_reach(model)))
            .collect();
        // Where one has gone from, its shadow has to go too. Its model may not
        // be in this frame, so this takes the size of the largest that is.
        let reach = figures
            .iter()
            .map(|(model, _)| figure_reach(model))
            .fold(GONE_REACH, f32::max);
        moved.extend(
            self.stances
                .iter()
                .filter(|stance| !now.contains(stance))
                .map(|stance| (stance.origin, reach)),
        );
        self.stances = now;
        moved
    }

    /// Draws what's stale.
    pub(super) fn record(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        map: Option<&GpuMap>,
        mesh_ranges: &[ShadowRange],
        figures: &[Figure<'_>],
        frame: usize,
    ) {
        if !self.ready.replace(true) {
            rec.begin_render_pass_ext(
                &self.first_pass,
                &self.framebuffer,
                &[vulkane::safe::ClearValue::DepthStencil {
                    depth: 1.0,
                    stencil: 0,
                }],
            );
            rec.end_render_pass();
        }
        let Some(map) = map.filter(|_| !self.redraw.is_empty()) else {
            return;
        };
        rec.begin_render_pass_ext(&self.pass, &self.framebuffer, &[]);
        for (slot, light) in &self.redraw {
            for face in 0..6 {
                self.record_tile(rec, map, mesh_ranges, *slot * 6 + face, face, light);
                self.record_moving(rec, map, frame, face, light);
                self.record_figures(rec, figures, face, light);
            }
        }
        rec.end_render_pass();
    }

    /// Draws one tile.
    fn record_tile(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        map: &GpuMap,
        ranges: &[ShadowRange],
        tile: usize,
        face: usize,
        light: &LiveLight,
    ) {
        let tile = u32::try_from(tile).unwrap_or(0);
        let (x, y) = (
            tile % TILES_ACROSS * TILE_SIZE,
            tile / TILES_ACROSS * TILE_SIZE,
        );
        #[expect(clippy::cast_precision_loss, reason = "atlas sizes are small")]
        rec.set_viewport(x as f32, y as f32, TILE_SIZE as f32, TILE_SIZE as f32);
        rec.set_scissor(
            i32::try_from(x).unwrap_or(0),
            i32::try_from(y).unwrap_or(0),
            TILE_SIZE,
            TILE_SIZE,
        );
        rec.bind_graphics_pipeline(&self.clear);
        rec.draw(3, 1, 0, 0);
        rec.bind_graphics_pipeline(&self.pipeline);
        rec.bind_vertex_buffers(0, &[(&map.vertices.buffer, 0)]);
        rec.bind_index_buffer(&map.shadow_indices.buffer, 0, INDEX_TYPE_U32);
        let matrix = cube_face(light.origin, face, light.radius);
        let reached = |range: &&ShadowRange| {
            range
                .bounds
                .is_none_or(|(middle, size)| (middle - light.origin).length() < light.radius + size)
        };
        for range in ranges.iter().filter(reached) {
            let offset = range.offset;
            let tail = [offset.x, offset.y, offset.z, 0.0, 0.0, 0.0, 0.0, 0.0];
            self.push(rec, &matrix, tail);
            rec.draw_indexed(range.count, 1, range.first, 0, 0);
        }
        self.record_instances(rec, map, &matrix, light);
    }

    /// Draws the moving meshes a light reaches into the tile that's set up.
    fn record_moving(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        map: &GpuMap,
        frame: usize,
        face: usize,
        light: &LiveLight,
    ) {
        let Some(moving) = &map.moving else {
            return;
        };
        let matrix = cube_face(light.origin, face, light.radius);
        rec.bind_graphics_pipeline(&self.mesh_pipeline);
        for job in moving.jobs(frame) {
            if (job.center - light.origin).length() >= light.radius + job.radius {
                continue;
            }
            if bind_moving(rec, moving, job, frame, false).is_none() {
                continue;
            }
            let Some(mesh) = moving.meshes.get(job.mesh as usize) else {
                continue;
            };
            self.push(rec, &multiply(&matrix, &job.placed), [0.0; 8]);
            for part in &mesh.shadow_parts {
                rec.draw_indexed(part.count, 1, part.first, 0, 0);
            }
        }
    }

    /// Draws the copies of shared meshes a light reaches into the tile that's set up.
    fn record_instances(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        map: &GpuMap,
        matrix: &[f32; 16],
        light: &LiveLight,
    ) {
        let reached = |caster: &&ShadowInstance| {
            (caster.center - light.origin).length() < light.radius + caster.radius
        };
        rec.bind_graphics_pipeline(&self.mesh_pipeline);
        for caster in map.shadow_instances.iter().filter(reached) {
            let Some(mesh) = map.shared_meshes.get(caster.mesh as usize) else {
                continue;
            };
            rec.bind_vertex_buffers(0, &[(&mesh.places.buffer, 0)]);
            rec.bind_index_buffer(&mesh.indices.buffer, 0, INDEX_TYPE_U32);
            self.push(rec, &multiply(matrix, &caster.placed), [0.0; 8]);
            for part in &mesh.shadow_parts {
                rec.draw_indexed(part.count, 1, part.first, 0, 0);
            }
        }
    }

    /// Pushes what a draw needs.
    fn push(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        matrix: &[f32; 16],
        tail: [f32; 8],
    ) {
        let mut bytes = [0; PUSH_SIZE as usize];
        let values = matrix.iter().copied().chain(tail);
        for (chunk, value) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(values) {
            *chunk = value.to_le_bytes();
        }
        rec.push_constants(&self.layout, ShaderStageFlags::VERTEX, 0, &bytes);
    }

    /// Draws the figures a light reaches into the tile that's set up.
    fn record_figures(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        figures: &[Figure<'_>],
        face: usize,
        light: &LiveLight,
    ) {
        let reached = |(model, draw): &&Figure<'_>| {
            (draw.origin - light.origin).length() < light.radius + figure_reach(model)
        };
        let matrix = cube_face(light.origin, face, light.radius);
        let mut bound = false;
        for (gpu, draw) in figures.iter().filter(reached) {
            if !bound {
                rec.bind_graphics_pipeline(&self.figure_pipeline);
                bound = true;
            }
            let corners = gpu.model.corners.len();
            let pose_size = (corners * POSE_CORNER_BYTES) as u64;
            rec.bind_vertex_buffers(
                0,
                &[
                    (&gpu.poses.buffer, u64::from(draw.pose) * pose_size),
                    (&gpu.poses.buffer, u64::from(draw.next_pose) * pose_size),
                ],
            );
            let placed = multiply(&matrix, &model_matrix(draw.origin, draw.angles));
            let (scale, offset) = (gpu.model.scale, gpu.model.offset);
            let tail = [
                offset.x, offset.y, offset.z, 0.0, scale.x, scale.y, scale.z, draw.blend,
            ];
            self.push(rec, &placed, tail);
            rec.draw(u32::try_from(corners).unwrap_or(0), 1, 0, 0);
        }
    }
}
