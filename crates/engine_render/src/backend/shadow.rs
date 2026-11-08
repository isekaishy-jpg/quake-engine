//! Shadow maps for moving lights: six tiles of an atlas each, kept until
//! something changes (proposal 0084).

use std::cell::Cell;

use engine_core::entity::ModelInstance;
use engine_core::math::Vec3;
use engine_map::{Map, SharedPart};
use engine_shaders::{Module, spirv};
use vulkane::safe::{
    AttachmentDescription, AttachmentLoadOp, AttachmentStoreOp, CompareOp, CullMode,
    DescriptorSetLayout, Device, Format, Framebuffer, GraphicsPipeline, GraphicsPipelineBuilder,
    GraphicsShaderStage, Image, Image2dCreateInfo, ImageLayout, ImageUsage, InputRate,
    PhysicalDevice, PipelineLayout, PushConstantRange, RenderPass, RenderPassCreateInfo,
    ShaderModule, ShaderStageFlags, VertexInputAttribute, VertexInputBinding,
};

use super::deform::GpuMoving;
use super::deform::bind_moving;
use super::pipelines::land_vertices;
use super::upload::device_image;
use super::{DEPTH_FORMAT, GpuAlias, GpuBuffer, GpuImage, GpuMap, INDEX_TYPE_U32, ShadowInstance};
use crate::alias::{EntityDraw, model_matrix};
use crate::error::RenderError;
use crate::instances::{LOOK_BYTES, PLACE_BYTES};
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

/// Bytes of what a tile's draws push: a matrix and an offset.
const PUSH_SIZE: u32 = 80;

/// What reads it: where things are, and a cut-out's cutoff.
fn push_stages() -> ShaderStageFlags {
    ShaderStageFlags::VERTEX | ShaderStageFlags::FRAGMENT
}

/// Where a world vertex's place on its picture is.
const UV_OFFSET: u32 = 12;

/// The most an alias model's stored position can be along an axis.
const POSE_RANGE: f32 = 255.0;

/// How far a figure's shadow reaches at least, for one that's left.
const GONE_REACH: f32 = 64.0;

/// How many maps of the sun's shadow there are, each further out and coarser than the last.
pub(super) const SUN_CASCADES: usize = 3;

/// A sun map's side.
const SUN_TILE: u32 = 2048;

/// How far each of the sun's maps reaches from the view, each way.
pub(crate) const SUN_REACHES: [f32; SUN_CASCADES] = [400.0, 1500.0, 5000.0];

/// How deep a sun map is, through the middle of the view.
const SUN_DEPTH: f32 = 24_000.0;

/// What a view of shadows reaches: a ball round where it's from.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Reach {
    /// Its middle.
    center: Vec3,
    /// How far it goes.
    radius: f32,
}

impl Reach {
    /// Whether something round `center`, `size` across, is in it.
    fn touches(self, center: Vec3, size: f32) -> bool {
        (center - self.center).length() < self.radius + size
    }
}

/// One of the sun's maps, as it was last drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SunView {
    /// From the world to the map.
    pub(super) matrix: [f32; 16],
    /// What it covers.
    reach: Reach,
}

/// The sun's maps of the view, as the shading wants them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SunShadow {
    /// Each map's matrix.
    pub(crate) matrices: [[f32; 16]; SUN_CASCADES],
}

/// A map looking along `toward`, from the sun down, round `eye`, `half` across each way. It's
/// kept to whole pixels of it, or its edges would crawl as the view moves.
fn sun_view(toward: Vec3, eye: Vec3, half: f32) -> SunView {
    let down = toward.normalized() * -1.0;
    let helper = if down.z.abs() > 0.99 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 0.0, 1.0)
    };
    let cross = |a: Vec3, b: Vec3| {
        Vec3::new(
            a.y * b.z - a.z * b.y,
            a.z * b.x - a.x * b.z,
            a.x * b.y - a.y * b.x,
        )
    };
    let right = cross(helper, down).normalized();
    let up = cross(down, right);
    #[expect(clippy::cast_precision_loss, reason = "a small size")]
    let pixel = 2.0 * half / SUN_TILE as f32;
    let snap = |axis: Vec3| (axis.dot(eye) / pixel).round() * pixel;
    let center = right * snap(right) + up * snap(up) + down * down.dot(eye);
    let row = |axis: Vec3, scale: f32, add: f32| {
        [
            axis.x * scale,
            axis.y * scale,
            axis.z * scale,
            -axis.dot(center) * scale + add,
        ]
    };
    let x = row(right, 1.0 / half, 0.0);
    let y = row(up, 1.0 / half, 0.0);
    let z = row(down, 1.0 / SUN_DEPTH, 0.5);
    let w = [0.0, 0.0, 0.0, 1.0];
    let matrix = [
        x[0], y[0], z[0], w[0], x[1], y[1], z[1], w[1], x[2], y[2], z[2], w[2], x[3], y[3], z[3],
        w[3],
    ];
    // What can shade the map: up to the top of it, towards the sun.
    let reach = Reach {
        center,
        radius: (half * half * 2.0 + (SUN_DEPTH * 0.5) * (SUN_DEPTH * 0.5)).sqrt(),
    };
    SunView { matrix, reach }
}

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
    /// The texture whose holes it has, if it's a cut-out.
    pub(super) cut: Option<u32>,
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

/// A figure to draw into shadows, with where its moved corners went.
pub(super) type Figure<'a> = (&'a GpuAlias, &'a EntityDraw, Option<u32>);

/// The figures of `entities`. What's kept in front is the view's own weapon,
/// which casts nothing.
pub(super) fn shadow_figures<'a>(
    models: &'a [GpuAlias],
    entities: &'a [EntityDraw],
    places: &[Option<u32>],
) -> Vec<Figure<'a>> {
    entities
        .iter()
        .enumerate()
        .filter(|(_, draw)| !draw.in_front)
        .filter_map(|(at, draw)| {
            let place = places.get(at).copied().flatten();
            Some((models.get(draw.model as usize)?, draw, place))
        })
        .collect()
}

impl GpuAlias {
    /// How far from its origin any part of it can reach.
    pub(super) fn reach(&self) -> f32 {
        let (scale, offset) = (self.model.scale, self.model.offset);
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
    /// The same as `pipeline`, for meshes, whose corners are smaller.
    mesh_pipeline: GraphicsPipeline,
    /// Land.
    land_pipeline: GraphicsPipeline,
    /// What has holes, which light goes through.
    cut_pipeline: GraphicsPipeline,
    /// The same, for meshes.
    mesh_cut_pipeline: GraphicsPipeline,
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
    /// The sun's maps, side by side.
    pub(super) sun_atlas: GpuImage,
    /// Them as a target.
    sun_framebuffer: Framebuffer,
    /// Whether they've been started off.
    sun_ready: Cell<bool>,
    /// Each map as it was last drawn, with no sun none of them.
    sun_views: [Option<SunView>; SUN_CASCADES],
    /// Which maps to draw this frame.
    sun_redraw: Vec<usize>,
    /// Frames the sun's been out, which takes turns at the far maps.
    sun_frames: u64,
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
    pub(super) fn new(
        device: &Device,
        physical: &PhysicalDevice,
        (land_layout, material_layout): (&DescriptorSetLayout, &DescriptorSetLayout),
    ) -> Result<Self, RenderError> {
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
        #[expect(clippy::cast_possible_truncation, reason = "three maps")]
        let sun_width = SUN_TILE * SUN_CASCADES as u32;
        let sun_image = Image::new_2d(
            device,
            Image2dCreateInfo {
                format: DEPTH_FORMAT,
                width: sun_width,
                height: SUN_TILE,
                usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT | ImageUsage::SAMPLED,
            },
        )?;
        let sun_atlas = device_image(device, physical, sun_image, true)?;
        let sun_framebuffer =
            Framebuffer::new(device, &pass, &[&sun_atlas.view], sun_width, SUN_TILE)?;
        let layout = PipelineLayout::with_push_constants(
            device,
            &[land_layout, material_layout],
            &[PushConstantRange {
                stage_flags: push_stages(),
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
        let uv = |binding, offset| VertexInputAttribute {
            location: 1,
            binding,
            format: Format::R32G32_SFLOAT,
            offset,
        };
        let cut_pipeline = GraphicsPipelineBuilder::new(&layout, &pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_cut")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_cut")
            .color_attachment_count(0)
            .vertex_input(&bindings, &[attributes[0], uv(0, UV_OFFSET)])
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(true, true)
            .depth_bias(1.5, 2.0, 0.0)
            .build(device)?;
        let looks = VertexInputBinding {
            binding: 1,
            stride: u32::try_from(LOOK_BYTES).unwrap_or(0),
            input_rate: InputRate::VERTEX,
        };
        let mesh_cut_pipeline = GraphicsPipelineBuilder::new(&layout, &pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_cut")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_cut")
            .color_attachment_count(0)
            .vertex_input(&[mesh_bindings[0], looks], &[attributes[0], uv(1, 0)])
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(true, true)
            .depth_bias(1.5, 2.0, 0.0)
            .build(device)?;
        let (land_bindings, land_attributes) = land_vertices();
        let land_pipeline = GraphicsPipelineBuilder::new(&layout, &pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_land")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_land")
            .color_attachment_count(0)
            .vertex_input(&land_bindings, &land_attributes)
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
            mesh_pipeline,
            land_pipeline,
            cut_pipeline,
            mesh_cut_pipeline,
            clear,
            ready: Cell::new(false),
            kept: vec![None; MAX_SHADOW_LIGHTS],
            models: Vec::new(),
            stances: Vec::new(),
            waiting: [0; MAX_SHADOW_LIGHTS],
            redraw: Vec::new(),
            sun_atlas,
            sun_framebuffer,
            sun_ready: Cell::new(false),
            sun_views: [None; SUN_CASCADES],
            sun_redraw: Vec::new(),
            sun_frames: 0,
        })
    }

    /// Works out which of the sun's maps to draw this frame, round `eye`. The nearest is drawn
    /// every frame, and the far ones take turns, as a sun moves slowly and they're coarse.
    pub(super) fn plan_sun(&mut self, toward: Option<Vec3>, eye: Vec3) {
        self.sun_redraw.clear();
        let Some(toward) = toward.filter(|toward| toward.z > 0.0) else {
            self.sun_views = [None; SUN_CASCADES];
            return;
        };
        self.sun_frames += 1;
        #[expect(clippy::cast_possible_truncation, reason = "a small count")]
        let turn = 1 + (self.sun_frames % (SUN_CASCADES as u64 - 1)) as usize;
        for (cascade, (view, reach)) in self.sun_views.iter_mut().zip(SUN_REACHES).enumerate() {
            if cascade == 0 || cascade == turn || view.is_none() {
                *view = Some(sun_view(toward, eye, reach));
                self.sun_redraw.push(cascade);
            }
        }
    }

    /// The sun's maps as they're drawn, if the sun's out.
    pub(super) fn sun_shadow(&self) -> Option<SunShadow> {
        let mut matrices = [[0.0; 16]; SUN_CASCADES];
        for (matrix, view) in matrices.iter_mut().zip(&self.sun_views) {
            *matrix = view.as_ref()?.matrix;
        }
        Some(SunShadow { matrices })
    }

    /// Whether the sun's maps want drawing this frame.
    pub(super) fn sun_draws(&self) -> bool {
        !self.sun_redraw.is_empty()
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
        let now: Vec<Stance> = figures
            .iter()
            .map(|(_, draw, _)| Stance::of(draw))
            .collect();
        let mut moved: Vec<(Vec3, f32)> = figures
            .iter()
            .filter(|(_, draw, _)| !self.stances.contains(&Stance::of(draw)))
            .map(|(model, draw, _)| (draw.origin, model.reach()))
            .collect();
        // Where one has gone from, its shadow has to go too. Its model may not
        // be in this frame, so this takes the size of the largest that is.
        let reach = figures
            .iter()
            .map(|(model, _, _)| model.reach())
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
        if !self.sun_ready.replace(true) {
            rec.begin_render_pass_ext(
                &self.first_pass,
                &self.sun_framebuffer,
                &[vulkane::safe::ClearValue::DepthStencil {
                    depth: 1.0,
                    stencil: 0,
                }],
            );
            rec.end_render_pass();
        }
        let Some(map) = map else {
            return;
        };
        let casters = (mesh_ranges, figures, frame);
        if !self.redraw.is_empty() {
            rec.begin_render_pass_ext(&self.pass, &self.framebuffer, &[]);
            for (slot, light) in &self.redraw {
                let reach = Reach {
                    center: light.origin,
                    radius: light.radius,
                };
                for face in 0..6 {
                    let tile = u32::try_from(*slot * 6 + face).unwrap_or(0);
                    let at = (
                        tile % TILES_ACROSS * TILE_SIZE,
                        tile / TILES_ACROSS * TILE_SIZE,
                    );
                    self.clear_tile(rec, at, TILE_SIZE);
                    let matrix = cube_face(light.origin, face, light.radius);
                    self.record_casters(rec, map, casters, &matrix, reach);
                }
            }
            rec.end_render_pass();
        }
        if !self.sun_redraw.is_empty() {
            rec.begin_render_pass_ext(&self.pass, &self.sun_framebuffer, &[]);
            for cascade in &self.sun_redraw {
                let Some(view) = self.sun_views[*cascade] else {
                    continue;
                };
                let x = u32::try_from(*cascade).unwrap_or(0) * SUN_TILE;
                self.clear_tile(rec, (x, 0), SUN_TILE);
                self.record_casters(rec, map, casters, &view.matrix, view.reach);
            }
            rec.end_render_pass();
        }
    }

    /// Sets a tile at `at`, `size` across, as what's drawn into, and empties it.
    fn clear_tile(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        (x, y): (u32, u32),
        size: u32,
    ) {
        #[expect(clippy::cast_precision_loss, reason = "atlas sizes are small")]
        rec.set_viewport(x as f32, y as f32, size as f32, size as f32);
        rec.set_scissor(
            i32::try_from(x).unwrap_or(0),
            i32::try_from(y).unwrap_or(0),
            size,
            size,
        );
        rec.bind_graphics_pipeline(&self.clear);
        rec.draw(3, 1, 0, 0);
    }

    /// Draws everything that casts a shadow into the tile that's set up, as `matrix` sees it.
    fn record_casters(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        map: &GpuMap,
        (ranges, figures, frame): (&[ShadowRange], &[Figure<'_>], usize),
        matrix: &[f32; 16],
        reach: Reach,
    ) {
        self.record_world(rec, map, ranges, matrix, reach);
        self.record_moving(rec, map, frame, matrix, reach);
        self.record_land(rec, map, matrix, reach);
        self.record_figures(rec, figures, (&map.moving, frame), matrix, reach);
    }

    /// Draws the world's faces and the copies of meshes `reach` gets to.
    fn record_world(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        map: &GpuMap,
        ranges: &[ShadowRange],
        matrix: &[f32; 16],
        reach: Reach,
    ) {
        rec.bind_graphics_pipeline(&self.pipeline);
        rec.bind_vertex_buffers(0, &[(&map.vertices.buffer, 0)]);
        rec.bind_index_buffer(&map.shadow_indices.buffer, 0, INDEX_TYPE_U32);
        let reached = |range: &&ShadowRange| {
            range
                .bounds
                .is_none_or(|(middle, size)| reach.touches(middle, size))
        };
        for range in ranges
            .iter()
            .filter(reached)
            .filter(|range| range.cut.is_none())
        {
            let offset = range.offset;
            let tail = [offset.x, offset.y, offset.z, 0.0];
            self.push(rec, matrix, tail);
            rec.draw_indexed(range.count, 1, range.first, 0, 0);
        }
        rec.bind_graphics_pipeline(&self.cut_pipeline);
        for range in ranges.iter().filter(reached) {
            let Some(cutoff) = range
                .cut
                .and_then(|texture| self.bind_cut(rec, map, texture))
            else {
                continue;
            };
            let offset = range.offset;
            self.push(rec, matrix, [offset.x, offset.y, offset.z, cutoff]);
            rec.draw_indexed(range.count, 1, range.first, 0, 0);
        }
        self.record_instances(rec, map, matrix, reach);
    }

    /// Binds the picture of `texture`, a cut-out, and says its cutoff, if it is one.
    fn bind_cut(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        map: &GpuMap,
        texture: u32,
    ) -> Option<f32> {
        let cutoff = map
            .shadow_cutoffs
            .get(texture as usize)
            .copied()
            .flatten()?;
        let set = map.texture_sets.get(texture as usize)?.as_ref()?;
        rec.bind_graphics_descriptor_sets(&self.layout, 1, &[set]);
        Some(cutoff)
    }

    /// Draws a mesh's `parts` that's bound, as `matrix` places it: what's solid, then what has
    /// holes, which needs where it is on its picture, from `looks`.
    fn record_parts(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        map: &GpuMap,
        (parts, looks): (&[SharedPart], (&GpuBuffer, u64)),
        matrix: &[f32; 16],
    ) {
        rec.bind_graphics_pipeline(&self.mesh_pipeline);
        self.push(rec, matrix, [0.0; 4]);
        let cut = |part: &&SharedPart| {
            map.shadow_cutoffs
                .get(part.texture as usize)
                .copied()
                .flatten()
                .is_some()
        };
        for part in parts.iter().filter(|part| !cut(part)) {
            rec.draw_indexed(part.count, 1, part.first, 0, 0);
        }
        let mut bound = false;
        for part in parts.iter().filter(cut) {
            if !bound {
                rec.bind_graphics_pipeline(&self.mesh_cut_pipeline);
                rec.bind_vertex_buffers(1, &[(&looks.0.buffer, looks.1)]);
                bound = true;
            }
            let Some(cutoff) = self.bind_cut(rec, map, part.texture) else {
                continue;
            };
            self.push(rec, matrix, [0.0, 0.0, 0.0, cutoff]);
            rec.draw_indexed(part.count, 1, part.first, 0, 0);
        }
    }

    /// Draws the moving meshes a light reaches into the tile that's set up.
    fn record_moving(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        map: &GpuMap,
        frame: usize,
        matrix: &[f32; 16],
        reach: Reach,
    ) {
        let moving = &map.moving;
        for job in moving.jobs(frame) {
            if !reach.touches(job.center, job.radius) {
                continue;
            }
            if bind_moving(rec, moving, job, frame, false).is_none() {
                continue;
            }
            let Some(mesh) = moving.meshes.get(job.mesh as usize) else {
                continue;
            };
            let parts = (mesh.shadow_parts(job.level), moving.looks_of(job));
            self.record_parts(rec, map, parts, &multiply(matrix, &job.placed));
        }
    }

    /// Draws the copies of shared meshes a light reaches into the tile that's set up.
    fn record_instances(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        map: &GpuMap,
        matrix: &[f32; 16],
        reach: Reach,
    ) {
        let reached = |caster: &&ShadowInstance| reach.touches(caster.center, caster.radius);
        for caster in map.shadow_instances.iter().filter(reached) {
            let Some(mesh) = map.shared_meshes.get(caster.mesh as usize) else {
                continue;
            };
            rec.bind_vertex_buffers(0, &[(&mesh.places.buffer, 0)]);
            rec.bind_index_buffer(&mesh.indices.buffer, 0, INDEX_TYPE_U32);
            let parts = (&mesh.shadow_parts[..], (&mesh.uvs, 0));
            self.record_parts(rec, map, parts, &multiply(matrix, &caster.placed));
        }
    }

    /// Pushes what a draw needs.
    fn push(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        matrix: &[f32; 16],
        tail: [f32; 4],
    ) {
        let mut bytes = [0; PUSH_SIZE as usize];
        let values = matrix.iter().copied().chain(tail);
        for (chunk, value) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(values) {
            *chunk = value.to_le_bytes();
        }
        rec.push_constants(&self.layout, push_stages(), 0, &bytes);
    }

    /// Draws the land a light reaches into the tile that's set up.
    fn record_land(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        map: &GpuMap,
        matrix: &[f32; 16],
        reach: Reach,
    ) {
        let Some(land) = &map.terrain else {
            return;
        };
        // A chunk at the level shadows use is the second size of grid.
        let Some(&(first, count)) = land.grids.get(1) else {
            return;
        };
        rec.bind_graphics_pipeline(&self.land_pipeline);
        rec.bind_graphics_descriptor_sets(&self.layout, 0, &[&land.set]);
        rec.bind_vertex_buffers(0, &[(&land.whole.buffer, 0)]);
        rec.bind_index_buffer(&land.indices.buffer, 0, INDEX_TYPE_U32);
        self.push(rec, matrix, [0.0; 4]);
        for (chunk, (middle, size)) in (0_u32..).zip(&land.whole_bounds) {
            if reach.touches(*middle, *size) {
                rec.draw_indexed(count, 1, first, 0, chunk);
            }
        }
    }

    /// Draws the figures a light reaches into the tile that's set up.
    fn record_figures(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        figures: &[Figure<'_>],
        (moving, slot): (&GpuMoving, usize),
        matrix: &[f32; 16],
        reach: Reach,
    ) {
        let reached = |(model, draw, _): &&Figure<'_>| reach.touches(draw.origin, model.reach());
        rec.bind_graphics_pipeline(&self.mesh_pipeline);
        for (gpu, draw, moved_to) in figures.iter().filter(reached) {
            let (corners, offset) = match moved_to {
                Some(first) => (moving.moved(slot), u64::from(*first) * PLACE_BYTES as u64),
                None => (&gpu.rest, 0),
            };
            rec.bind_vertex_buffers(0, &[(&corners.buffer, offset)]);
            rec.bind_index_buffer(&gpu.indices.buffer, 0, INDEX_TYPE_U32);
            let placed = multiply(matrix, &model_matrix(draw.origin, draw.angles));
            self.push(rec, &placed, [0.0; 4]);
            rec.draw_indexed(gpu.index_count, 1, 0, 0, 0);
        }
    }
}
