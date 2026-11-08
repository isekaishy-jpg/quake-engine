//! Timing what the GPU does. A clock on the CPU can't tell a frame's parts apart, and it picks
//! up everything else the machine is doing, so the GPU stamps the time at each end of a span.

use std::cell::Cell;

use vulkane::safe::{CommandBufferRecording, Device, PhysicalDevice, PipelineStage, QueryPool};

use super::FRAMES;

/// A part of a frame the clock times.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Span {
    /// Moving meshes' and models' corners being worked out.
    Deform,
    /// Shadow maps.
    Shadows,
    /// Moving meshes being drawn.
    Moving,
    /// The fixture's models being drawn.
    Models,
    /// All of the scene's pass.
    Scene,
    /// The final pass: water, fog and all that's done to a finished scene.
    Post,
    /// Light and fog in the air, which the final pass adds.
    Air,
    /// What glows, down the chain and back up.
    Glow,
}

/// How many spans there are.
const SPANS: usize = 8;

/// How long a frame's parts took on the GPU, in milliseconds, on average since this was last
/// asked. A part that wasn't drawn counts as nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GpuTimes {
    /// How many frames this is the average of.
    pub frames: u32,
    /// Moving meshes' and models' corners being worked out.
    pub deform: f64,
    /// Shadow maps.
    pub shadows: f64,
    /// Moving meshes being drawn.
    pub moving: f64,
    /// The fixture's models being drawn.
    pub models: f64,
    /// All of the scene's pass.
    pub scene: f64,
    /// The final pass: water, fog and all that's done to a finished scene.
    pub post: f64,
    /// Light and fog in the air, which the final pass adds.
    pub air: f64,
    /// What glows, down the chain and back up.
    pub glow: f64,
}

/// The GPU's clock. It's off until it's wanted, and costs nothing then.
pub(super) struct GpuClock {
    /// A pool for each frame in flight, or none if the queue can't tell the time.
    pools: Vec<QueryPool>,
    /// How long a tick of the GPU's clock is, in milliseconds.
    tick: f64,
    /// Whether it's running.
    on: bool,
    /// Which stamps each frame in flight has written.
    written: [Cell<u32>; FRAMES],
    /// Each span's time so far, in milliseconds.
    totals: [f64; SPANS],
    /// How many frames that's over.
    frames: u32,
}

impl GpuClock {
    /// A clock for `device`. A queue that has no clock leaves one that never runs.
    pub(super) fn new(device: &Device, physical: &PhysicalDevice, family: u32) -> Self {
        let bits = physical
            .queue_family_properties()
            .get(family as usize)
            .map_or(0, |family| family.timestamp_valid_bits());
        let pools = (0..FRAMES)
            .map(|_| QueryPool::timestamps(device, u32::try_from(SPANS * 2).unwrap_or(0)))
            .collect::<Result<Vec<_>, _>>()
            .ok()
            .filter(|_| bits > 0)
            .unwrap_or_default();
        Self {
            pools,
            tick: f64::from(physical.timestamp_period()) / 1_000_000.0,
            on: false,
            written: [Cell::new(0), Cell::new(0)],
            totals: [0.0; SPANS],
            frames: 0,
        }
    }

    /// Starts or stops it. Says if it can run at all.
    pub(super) fn set_on(&mut self, on: bool) -> bool {
        self.on = on && !self.pools.is_empty();
        self.on || !on
    }

    /// Readies frame `slot`'s stamps, outside any render pass.
    pub(super) fn begin(&self, rec: &mut CommandBufferRecording<'_>, slot: usize) {
        if self.on {
            let pool = &self.pools[slot];
            rec.reset_query_pool(pool, 0, pool.query_count());
            self.written[slot].set(0);
        }
    }

    /// Stamps the start of `span`. Everything recorded before it has finished by then.
    pub(super) fn start(&self, rec: &mut CommandBufferRecording<'_>, slot: usize, span: Span) {
        self.stamp(rec, slot, span as u32 * 2);
    }

    /// Stamps the end of `span`.
    pub(super) fn end(&self, rec: &mut CommandBufferRecording<'_>, slot: usize, span: Span) {
        self.stamp(rec, slot, span as u32 * 2 + 1);
    }

    /// Writes one stamp.
    fn stamp(&self, rec: &mut CommandBufferRecording<'_>, slot: usize, at: u32) {
        if self.on {
            rec.write_timestamp(PipelineStage::BOTTOM_OF_PIPE, &self.pools[slot], at);
            self.written[slot].set(self.written[slot].get() | 1 << at);
        }
    }

    /// Adds in what frame `slot` took. It has to have finished.
    pub(super) fn read(&mut self, slot: usize) {
        let written = self.written[slot].replace(0);
        if written == 0 {
            return;
        }
        for (span, total) in self.totals.iter_mut().enumerate() {
            let first = u32::try_from(span * 2).unwrap_or(0);
            if written >> first & 0b11 != 0b11 {
                continue;
            }
            if let Ok(stamps) = self.pools[slot].get_results_u64(first, 2) {
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "a span's ticks are far below 2^52"
                )]
                let ticks = stamps[1].saturating_sub(stamps[0]) as f64;
                *total += ticks * self.tick;
            }
        }
        self.frames += 1;
    }

    /// What's been timed since this was last asked.
    pub(super) fn take(&mut self) -> GpuTimes {
        let frames = std::mem::take(&mut self.frames);
        let [deform, shadows, moving, models, scene, post, air, glow] =
            std::mem::take(&mut self.totals).map(|total| total / f64::from(frames.max(1)));
        GpuTimes {
            frames,
            deform,
            shadows,
            moving,
            models,
            scene,
            post,
            air,
            glow,
        }
    }
}
