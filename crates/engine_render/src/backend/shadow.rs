//! Shadow maps for moving lights: six tiles of an atlas each, kept until
//! something changes (proposal 0084).

use std::cell::Cell;

use engine_core::entity::ModelInstance;
use engine_core::math::Vec3;
use engine_map::Map;
use vulkane::safe::{
    AttachmentDescription, AttachmentLoadOp, AttachmentStoreOp, CompareOp, CullMode, Device,
    Format, Framebuffer, GraphicsPipeline, GraphicsPipelineBuilder, GraphicsShaderStage, Image,
    Image2dCreateInfo, ImageLayout, ImageUsage, InputRate, PhysicalDevice, PipelineLayout,
    PushConstantRange, RenderPass, RenderPassCreateInfo, ShaderModule, ShaderStageFlags,
    VertexInputAttribute, VertexInputBinding,
};

use super::upload::device_image;
use super::{DEPTH_FORMAT, GpuImage, GpuMap, INDEX_TYPE_U32};
use crate::error::RenderError;
use crate::live::{LiveLight, MAX_SHADOW_LIGHTS, cube_face};
use crate::world::Vertex;

/// The atlas's side.
const ATLAS_SIZE: u32 = 2048;

/// A tile's side.
const TILE_SIZE: u32 = 256;

/// Tiles along a side of the atlas.
const TILES_ACROSS: u32 = ATLAS_SIZE / TILE_SIZE;

/// How many lights' shadows are redrawn in a frame. The rest wait their turn.
const REDRAWS_PER_FRAME: usize = 2;

/// Bytes of what a tile's draws push: a matrix and an offset.
const PUSH_SIZE: u32 = 80;

/// The shaders.
const SHADOW_SHADER: &str = include_str!("../shaders/shadow.wgsl");

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
    /// Empties a tile.
    clear: GraphicsPipeline,
    /// Whether the atlas has been started off.
    ready: Cell<bool>,
    /// What each slot holds.
    kept: Vec<Option<Kept>>,
    /// Where moving models were last frame.
    models: Vec<(u32, Vec3)>,
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
        let spirv = vulkane::safe::naga::compile_wgsl(SHADOW_SHADER)
            .map_err(|error| RenderError(error.to_string()))?;
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
            clear,
            ready: Cell::new(false),
            kept: vec![None; MAX_SHADOW_LIGHTS],
            models: Vec::new(),
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
    }

    /// Gives each shadowed light a slot, keeping those that have one, and
    /// works out which slots to redraw this frame. Returns each light's slot.
    pub(super) fn plan(
        &mut self,
        lights: &[LiveLight],
        instances: &[ModelInstance],
        map: &Map,
        eye: Vec3,
    ) -> Vec<Option<usize>> {
        let moved = self.moved_models(instances, map);
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
        let mut stale: Vec<(f32, usize, LiveLight)> = Vec::new();
        for (at, light) in lights.iter().enumerate() {
            let Some(slot) = slots[at] else {
                continue;
            };
            let same = self.kept[slot]
                .is_some_and(|kept| kept.origin == light.origin && kept.radius == light.radius);
            let touched = moved
                .iter()
                .any(|(middle, size)| (*middle - light.origin).length() < light.radius + size);
            if !same || touched {
                stale.push(((light.origin - eye).length(), slot, *light));
            }
        }
        stale.sort_by(|a, b| a.0.total_cmp(&b.0));
        stale.truncate(REDRAWS_PER_FRAME);
        self.redraw = stale
            .iter()
            .map(|(_, slot, light)| (*slot, *light))
            .collect();
        for (slot, light) in &self.redraw {
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
        for (model, origin) in &now {
            if self.models.contains(&(*model, *origin)) {
                continue;
            }
            if let Some(found) = map.models.get(*model as usize) {
                let middle = (found.mins + found.maxs) * 0.5 + *origin;
                moved.push((middle, (found.maxs - found.mins).length() * 0.5));
            }
        }
        self.models = now;
        moved
    }

    /// Draws what's stale.
    pub(super) fn record(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        map: Option<&GpuMap>,
        mesh_ranges: &[(u32, u32, Vec3)],
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
            }
        }
        rec.end_render_pass();
    }

    /// Draws one tile.
    fn record_tile(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        map: &GpuMap,
        ranges: &[(u32, u32, Vec3)],
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
        for (first, count, offset) in ranges {
            let mut bytes = [0; PUSH_SIZE as usize];
            let values = matrix
                .iter()
                .copied()
                .chain([offset.x, offset.y, offset.z, 0.0]);
            for (chunk, value) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(values) {
                *chunk = value.to_le_bytes();
            }
            rec.push_constants(&self.layout, ShaderStageFlags::VERTEX, 0, &bytes);
            rec.draw_indexed(*count, 1, *first, 0, 0);
        }
    }
}
