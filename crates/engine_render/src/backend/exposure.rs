//! Exposure that goes by itself, as eyes do (proposal 0150): how bright a scene is, measured on
//! the GPU and read back once its frame is done, and how far exposure has got towards it.

use std::cell::Cell;

use vulkane::safe::{
    AccessFlags, AttachmentDescription, AttachmentLoadOp, AttachmentStoreOp, BufferImageCopy,
    BufferUsage, DescriptorSet, Device, Format, Framebuffer, GraphicsPipeline, Image,
    Image2dCreateInfo, ImageLayout, ImageUsage, PhysicalDevice, PipelineLayout, PipelineStage,
    RenderPass, RenderPassCreateInfo,
};

use super::upload::{device_image, host_buffer};
use super::{FRAMES, GpuBuffer, GpuImage};
use crate::error::RenderError;

/// How many cells across and down a scene is measured in.
pub(super) const METER_CELLS: u32 = 16;

/// What's kept of each cell: how bright it is on average, as a power of two.
const METER_FORMAT: Format = Format::R32_SFLOAT;

/// How bright, as a power of two, the middle of what's seen is brought to: middle grey.
const MIDDLE_GREY: f32 = -2.47;

/// The share of what's measured left out at each end, the darkest and the brightest, so that a
/// lamp in view or a black corner doesn't sway it.
const LEFT_OUT: f32 = 0.1;

/// How long can go between two frames before exposure stops going by degrees and starts again
/// where it should be, as after a level's loaded or the game's been paused.
const LONGEST_STEP: f32 = 1.0;

/// The render pass of how bright a scene is, left ready to copy.
pub(super) fn make_meter_pass(device: &Device) -> Result<RenderPass, RenderError> {
    Ok(RenderPass::new(
        device,
        RenderPassCreateInfo {
            color_attachments: &[AttachmentDescription {
                format: METER_FORMAT,
                load_op: AttachmentLoadOp::DONT_CARE,
                store_op: AttachmentStoreOp::STORE,
                initial_layout: ImageLayout::UNDEFINED,
                final_layout: ImageLayout::TRANSFER_SRC_OPTIMAL,
            }],
            depth_attachment: None,
        },
    )?)
}

/// Where how bright a scene is gets measured, and read back from, a frame at a time.
pub(super) struct Meter {
    /// Where it's drawn.
    framebuffer: Framebuffer,
    /// Kept alive for the framebuffer.
    image: GpuImage,
    /// Where each frame's is copied, to read once it's done.
    readbacks: [GpuBuffer; FRAMES],
    /// Whether each frame measured.
    written: [Cell<bool>; FRAMES],
}

impl Meter {
    /// A meter, drawn with `pass`.
    pub(super) fn new(
        device: &Device,
        physical: &PhysicalDevice,
        pass: &RenderPass,
    ) -> Result<Self, RenderError> {
        let image = Image::new_2d(
            device,
            Image2dCreateInfo {
                format: METER_FORMAT,
                width: METER_CELLS,
                height: METER_CELLS,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_SRC,
            },
        )?;
        let image = device_image(device, physical, image, false)?;
        let framebuffer = Framebuffer::new(device, pass, &[&image.view], METER_CELLS, METER_CELLS)?;
        let size = (METER_CELLS * METER_CELLS * 4) as usize;
        let readback = || host_buffer(device, physical, size, BufferUsage::TRANSFER_DST);
        Ok(Self {
            framebuffer,
            image,
            readbacks: [readback()?, readback()?],
            written: [Cell::new(false), Cell::new(false)],
        })
    }

    /// Records measuring the scene `set` reads, in frame `slot`.
    pub(super) fn record(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        (pass, pipeline, layout): (&RenderPass, &GraphicsPipeline, &PipelineLayout),
        set: &DescriptorSet,
        slot: usize,
    ) {
        rec.begin_render_pass_ext(pass, &self.framebuffer, &[]);
        #[expect(clippy::cast_precision_loss, reason = "a few cells")]
        let cells = METER_CELLS as f32;
        rec.set_viewport(0.0, 0.0, cells, cells);
        rec.set_scissor(0, 0, METER_CELLS, METER_CELLS);
        rec.bind_graphics_pipeline(pipeline);
        rec.bind_graphics_descriptor_sets(layout, 0, &[set]);
        rec.draw(3, 1, 0, 0);
        rec.end_render_pass();
        rec.copy_image_to_buffer(
            &self.image.image,
            ImageLayout::TRANSFER_SRC_OPTIMAL,
            &self.readbacks[slot].buffer,
            &[BufferImageCopy::full_2d(METER_CELLS, METER_CELLS)],
        );
        rec.memory_barrier(
            PipelineStage::TRANSFER,
            PipelineStage::HOST,
            AccessFlags::TRANSFER_WRITE,
            AccessFlags::HOST_READ,
        );
        self.written[slot].set(true);
    }

    /// How bright frame `slot` measured its scene, if it did. It has to have finished.
    pub(super) fn read(&mut self, slot: usize) -> Option<f32> {
        if !self.written[slot].replace(false) {
            return None;
        }
        let mapped = self.readbacks[slot].memory.map().ok()?;
        let bytes = mapped.as_slice();
        let (cells, _) = bytes.as_chunks::<4>();
        let cells: Vec<f32> = cells.iter().map(|cell| f32::from_le_bytes(*cell)).collect();
        metered(&cells)
    }
}

/// How bright a scene is, as a power of two, from its cells', row by row. Those near the middle
/// count most, as that's where a player looks, and the darkest and brightest are left out.
pub(crate) fn metered(cells: &[f32]) -> Option<f32> {
    let side = METER_CELLS as usize;
    let mut weighted: Vec<(f32, f32)> = cells
        .iter()
        .enumerate()
        .filter(|(_, value)| value.is_finite())
        .map(|(at, value)| {
            #[expect(clippy::cast_precision_loss, reason = "a few cells")]
            let away = |place: usize| ((place as f32 + 0.5) / side as f32 - 0.5).abs() * 2.0;
            let (x, y) = (away(at % side), away(at / side));
            let weight = 1.0 - 0.75 * (x * x + y * y).min(1.0);
            (*value, weight)
        })
        .collect();
    weighted.sort_by(|a, b| a.0.total_cmp(&b.0));
    let whole: f32 = weighted.iter().map(|(_, weight)| weight).sum();
    if whole <= 0.0 {
        return None;
    }
    let (low, high) = (whole * LEFT_OUT, whole * (1.0 - LEFT_OUT));
    let (mut before, mut total, mut counted) = (0.0, 0.0, 0.0);
    for (value, weight) in weighted {
        // How much of this cell's weight is inside what's kept.
        let kept = ((before + weight).min(high) - before.max(low)).max(0.0);
        total += value * kept;
        counted += kept;
        before += weight;
    }
    (counted > 0.0).then(|| total / counted)
}

/// How far exposure should be from the level's, in stops, for a scene `brightness` bright, held
/// to `range`.
pub(crate) fn wanted_stops(brightness: f32, range: [f32; 2]) -> f32 {
    (MIDDLE_GREY - brightness).clamp(range[0], range[1])
}

/// Exposure `seconds` on from `now`, towards `wanted`, going `speed` stops a second: the first
/// into light, the second into the dark.
pub(crate) fn adapted(now: f32, wanted: f32, seconds: f32, speed: [f32; 2]) -> f32 {
    let rate = if wanted < now { speed[0] } else { speed[1] };
    let step = rate * seconds;
    now + (wanted - now).clamp(-step, step)
}

/// How far exposure has got by itself.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Exposure {
    /// How far from the level's it is, in stops.
    pub(crate) stops: f32,
    /// When it was last moved, if it's been set going.
    pub(crate) at: Option<f32>,
}

impl Exposure {
    /// Moves it on to `time`, towards what a scene `brightness` bright wants, if one's been
    /// measured. The first time, or after a long gap, it's set where it should be.
    pub(crate) fn step(
        &mut self,
        brightness: Option<f32>,
        range: [f32; 2],
        speed: [f32; 2],
        time: f32,
    ) {
        let Some(brightness) = brightness else {
            return;
        };
        let wanted = wanted_stops(brightness, range);
        let gap = self.at.map(|at| time - at);
        self.stops = match gap {
            Some(gap) if (0.0..=LONGEST_STEP).contains(&gap) => {
                adapted(self.stops, wanted, gap, speed).clamp(range[0], range[1])
            }
            _ => wanted,
        };
        self.at = Some(time);
    }

    /// What it multiplies light by.
    pub(crate) fn scale(self) -> f32 {
        self.stops.exp2()
    }
}
