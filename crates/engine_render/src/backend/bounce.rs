//! What live lights bounce, on the GPU (proposal 0153): the probes' grid, in slices stacked down an
//! image as the probes are, of light in half floats, written as lights change.

use std::cell::Cell;
use std::collections::HashMap;

use engine_map::Map;
use vulkane::safe::{
    AccessFlags, BufferImageCopy, BufferUsage, Format, ImageBarrier, ImageLayout, PipelineStage,
};

use super::deform::half_bits;
use super::upload::host_buffer;
use super::{Backend, FRAMES, GpuBuffer, GpuImage, Scene};
use crate::error::RenderError;
use crate::live::LiveLight;
use crate::live_bounce::bounce_of_light;

/// How many lights' bounce is worked out again in a frame. The rest keep what they had, so a
/// moving light's bounce follows a frame or two behind, and many don't stall one.
const UPDATES_A_FRAME: usize = 2;

/// What a light's bounce is worked out from, as bits, so a light that hasn't changed keeps it.
type Key = [u32; 8];

/// What a light bounces onto each probe it reaches, by index.
type Bounced = Vec<(u32, [f32; 3])>;

/// The bits of what `light`'s bounce comes from: where it is, to a couple of units, how far it
/// reaches, its colour and how much it bounces.
fn key_of(light: &LiveLight) -> Key {
    // Where it is, to a couple of units, as bits of a whole number of them.
    #[expect(clippy::cast_possible_truncation, reason = "a place to the unit")]
    let at = |value: f32| ((value * 0.5).round() as i32).cast_unsigned();
    [
        at(light.origin.x),
        at(light.origin.y),
        at(light.origin.z),
        light.radius.to_bits(),
        light.color[0].to_bits(),
        light.color[1].to_bits(),
        light.color[2].to_bits(),
        light.bounce.to_bits(),
    ]
}

/// The live bounce of a map with probes.
pub(super) struct LiveBounce {
    /// It, as the shaders read it.
    pub(super) image: GpuImage,
    /// What's copied into it, a frame in flight each.
    staging: Vec<GpuBuffer>,
    /// The image's size.
    size: (u32, u32),
    /// Each light's bounce, by its number, and what it was worked out from.
    kept: HashMap<u32, (Key, Bounced)>,
    /// What the image holds now.
    shown: Vec<[f32; 3]>,
    /// Whether each frame in flight's staging has to be copied in.
    copy: [Cell<bool>; FRAMES],
}

impl Backend {
    /// The live bounce of `map`, if it has probes for it to be at.
    pub(super) fn upload_live_bounce(&self, map: &Map) -> Result<Option<LiveBounce>, RenderError> {
        let Some(probes) = map
            .probes
            .as_ref()
            .filter(|probes| !probes.colors.is_empty())
        else {
            return Ok(None);
        };
        let [across, down, high] = probes.size;
        let size = (across, down * high);
        let count = (across * down * high) as usize;
        let bytes = vec![0_u8; count * 8];
        let image = self.upload_image(&[(size.0, size.1, &bytes)], Format::R16G16B16A16_SFLOAT)?;
        let staging = (0..FRAMES)
            .map(|_| {
                host_buffer(
                    &self.device,
                    &self.physical,
                    count * 8,
                    BufferUsage::TRANSFER_SRC,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(LiveBounce {
            image,
            staging,
            size,
            kept: HashMap::new(),
            shown: vec![[0.0; 3]; count],
            copy: std::array::from_fn(|_| Cell::new(false)),
        }))
    }

    /// Works out what the live lights bounce this frame, a few of them again, and readies it
    /// to be copied in if it's changed.
    pub(super) fn plan_live_bounce(
        &mut self,
        scene: Option<&Scene<'_>>,
    ) -> Result<(), RenderError> {
        let slot = self.slot;
        let Some(bounce) = self.map.as_mut().and_then(|map| map.live_bounce.as_mut()) else {
            return Ok(());
        };
        let lights = scene.map_or(&[][..], |scene| scene.drawables.live_lights);
        let probes = scene.and_then(|scene| scene.map.probes.as_ref());
        let mut updates = 0;
        let mut kept = HashMap::new();
        for light in lights.iter().filter(|light| light.bounce > 0.0) {
            let key = key_of(light);
            let before = bounce.kept.remove(&light.id);
            let fresh = before.as_ref().is_some_and(|(known, _)| *known == key);
            let entry = match (scene, probes) {
                (Some(scene), Some(probes)) if !fresh && updates < UPDATES_A_FRAME => {
                    updates += 1;
                    Some((key, bounce_of_light(scene.map, probes, light)))
                }
                _ => before,
            };
            if let Some(entry) = entry {
                kept.insert(light.id, entry);
            }
        }
        bounce.kept = kept;
        let mut volume = vec![[0.0_f32; 3]; bounce.shown.len()];
        for (_, entries) in bounce.kept.values() {
            for (index, light) in entries {
                if let Some(sum) = volume.get_mut(*index as usize) {
                    for part in 0..3 {
                        sum[part] += light[part];
                    }
                }
            }
        }
        if volume == bounce.shown && !bounce.copy.iter().any(Cell::get) {
            return Ok(());
        }
        let changed = volume != bounce.shown;
        bounce.shown = volume;
        if changed {
            // Each frame in flight copies it in once, from its own staging.
            for copy in &bounce.copy {
                copy.set(true);
            }
        }
        if bounce.copy[slot].get() {
            let mut mapped = bounce.staging[slot].memory.map()?;
            let bytes = mapped.as_slice_mut();
            for (texel, light) in bytes.as_chunks_mut::<8>().0.iter_mut().zip(&bounce.shown) {
                let [r, g, b] = light.map(half_bits);
                for (chunk, half) in texel.as_chunks_mut::<2>().0.iter_mut().zip([r, g, b, 0]) {
                    *chunk = half.to_le_bytes();
                }
            }
        }
        Ok(())
    }

    /// Copies what the live lights bounce in, if this frame's has to be.
    pub(super) fn record_live_bounce(&self, rec: &mut vulkane::safe::CommandBufferRecording<'_>) {
        let Some(bounce) = self.map.as_ref().and_then(|map| map.live_bounce.as_ref()) else {
            return;
        };
        if !bounce.copy[self.slot].replace(false) {
            return;
        }
        rec.image_barrier(
            PipelineStage::ALL_GRAPHICS,
            PipelineStage::TRANSFER,
            ImageBarrier::color(
                &bounce.image.image,
                ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                ImageLayout::TRANSFER_DST_OPTIMAL,
                AccessFlags::SHADER_READ,
                AccessFlags::TRANSFER_WRITE,
            ),
        );
        let region = BufferImageCopy::full_2d(bounce.size.0, bounce.size.1);
        rec.copy_buffer_to_image(
            &bounce.staging[self.slot].buffer,
            &bounce.image.image,
            ImageLayout::TRANSFER_DST_OPTIMAL,
            &[region],
        );
        rec.image_barrier(
            PipelineStage::TRANSFER,
            PipelineStage::ALL_GRAPHICS,
            ImageBarrier::color(
                &bounce.image.image,
                ImageLayout::TRANSFER_DST_OPTIMAL,
                ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                AccessFlags::TRANSFER_WRITE,
                AccessFlags::SHADER_READ,
            ),
        );
    }
}
