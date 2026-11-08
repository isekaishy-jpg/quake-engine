//! What's sent to a screen: standard, or HDR as scRGB or HDR10 (proposal 0150). The standard
//! passes and pipelines are always there, for pictures and screens without HDR. An HDR output's
//! are made when it's wanted.

use engine_core::canvas::Quad;
use vulkane::safe::{
    AccessFlags, BufferImageCopy, BufferUsage, ClearValue, Format, Framebuffer, GraphicsPipeline,
    Image, Image2dCreateInfo, ImageLayout, ImageUsage, PipelineStage, RenderPass,
};

use super::pipelines::{canvas_pipeline, make_pass};
use super::upload::{device_image, host_buffer};
use super::{Attachments, Backend, HDR10_BLUE_FIRST, SCRGB_FORMAT, Scene, depth_image};
use crate::error::RenderError;
use crate::renderer::{ScreenCapture, ScreenOutput, USUAL_PEAK_NITS, USUAL_WHITE_NITS, View};

/// An HDR output's passes and pipelines.
pub(super) struct ScreenPipes {
    /// Which output they're for.
    pub(super) output: ScreenOutput,
    /// What it's sent as.
    format: Format,
    /// Drawing to a window.
    pub(super) window_pass: RenderPass,
    /// Drawing to be read back.
    capture_pass: RenderPass,
    /// The final pass.
    pub(super) post: GraphicsPipeline,
    /// Flat pictures over the view.
    pub(super) canvas: Option<GraphicsPipeline>,
}

impl Backend {
    /// What `output` is sent as.
    pub(super) fn output_format(&self, output: ScreenOutput) -> Format {
        match output {
            ScreenOutput::Standard => super::COLOR_FORMAT,
            ScreenOutput::ScRgb => SCRGB_FORMAT,
            ScreenOutput::Hdr10 => self.hdr10,
        }
    }

    /// Makes sure there are passes and pipelines for `output`, if it isn't standard.
    pub(super) fn ensure_screen_pipes(&mut self, output: ScreenOutput) -> Result<(), RenderError> {
        let format = self.output_format(output);
        if output == ScreenOutput::Standard
            || self
                .screen_pipes
                .as_ref()
                .is_some_and(|pipes| pipes.output == output && pipes.format == format)
        {
            return Ok(());
        }
        let window_pass = make_pass(&self.device, format, ImageLayout::PRESENT_SRC_KHR)?;
        let capture_pass = make_pass(&self.device, format, ImageLayout::TRANSFER_SRC_OPTIMAL)?;
        let post = Self::make_post_pipeline(
            &self.device,
            &self.post_layout,
            &window_pass,
            self.samples,
            "fs_post",
            &self.post_code,
        )?;
        let canvas = self
            .canvas
            .as_ref()
            .map(|canvas| canvas_pipeline(&self.device, &canvas.layout, &window_pass))
            .transpose()?;
        self.screen_pipes = Some(ScreenPipes {
            output,
            format,
            window_pass,
            capture_pass,
            post,
            canvas,
        });
        Ok(())
    }

    /// The final pass's pipeline and the canvas's for `output`, which has to have been made sure
    /// of. The canvas's is `None` for the standard one.
    pub(super) fn screen_pipelines(
        &self,
        output: ScreenOutput,
    ) -> (&GraphicsPipeline, Option<&GraphicsPipeline>) {
        match self
            .screen_pipes
            .as_ref()
            .filter(|pipes| pipes.output == output)
        {
            Some(pipes) => (&pipes.post, pipes.canvas.as_ref()),
            None => (&self.post_pipeline, None),
        }
    }

    /// What the final pass and the canvas are told of `output`: which it is, how bright white is
    /// in nits, and how many times brighter than white the brightest is.
    pub(super) fn screen_values(&self, output: ScreenOutput) -> [f32; 4] {
        let positive = |value: f32, usual: f32| if value > 0.0 { value } else { usual };
        let white = positive(self.post.white_nits, USUAL_WHITE_NITS);
        let peak = positive(self.post.peak_nits, USUAL_PEAK_NITS).max(white);
        let mode = match output {
            ScreenOutput::Standard => 0.0,
            ScreenOutput::ScRgb => 1.0,
            ScreenOutput::Hdr10 => 2.0,
        };
        [mode, white, peak / white, 0.0]
    }

    /// Draws off-screen as `output` would be sent, and reads it back, each colour from 0 to 1 as
    /// it's stored: scRGB as linear light, HDR10 on its curve.
    pub(crate) fn capture_as(
        &mut self,
        view: &View,
        scene: Option<&Scene<'_>>,
        quads: &[Quad],
        (width, height): (u32, u32),
        output: ScreenOutput,
    ) -> Result<ScreenCapture, RenderError> {
        if output == ScreenOutput::Standard {
            let capture = self.capture(view, scene, quads, width, height)?;
            let values = capture
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .map(|pixel| pixel.map(|part| f32::from(part) / 255.0))
                .collect();
            return Ok(ScreenCapture {
                width,
                height,
                values,
            });
        }
        self.wait_for_frame()?;
        self.device.wait_idle()?;
        self.ensure_screen_pipes(output)?;
        let format = self.output_format(output);
        let blue_first = format == HDR10_BLUE_FIRST;
        let color = Image::new_2d(
            &self.device,
            Image2dCreateInfo {
                format,
                width,
                height,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_SRC,
            },
        )?;
        let color = device_image(&self.device, &self.physical, color, false)?;
        let depth = depth_image(&self.device, &self.physical, (width, height))?;
        let Some(pipes) = self.screen_pipes.as_ref() else {
            return Err(RenderError("no HDR passes".to_owned()));
        };
        let attachments = Attachments {
            framebuffer: Framebuffer::new(
                &self.device,
                &pipes.capture_pass,
                &[&color.view, &depth.view],
                width,
                height,
            )?,
            _depth: depth,
        };
        let pixel_bytes = if output == ScreenOutput::ScRgb { 8 } else { 4 };
        let size = (width as usize) * (height as usize) * pixel_bytes;
        let mut readback = host_buffer(
            &self.device,
            &self.physical,
            size,
            BufferUsage::TRANSFER_DST,
        )?;
        let (corner, shown) = self.view_area((width, height));
        #[expect(
            clippy::cast_precision_loss,
            reason = "capture sizes are far below 2^24"
        )]
        let aspect = shown.0 as f32 / shown.1.max(1) as f32;
        self.encoding = output;
        self.upload_frame(view, scene, aspect)?;
        self.upload_canvas(quads, (width, height))?;
        self.ensure_scene_target(shown)?;
        let screen = self.screen_values(output);
        let Some(pipes) = self.screen_pipes.as_ref() else {
            return Err(RenderError("no HDR passes".to_owned()));
        };
        let recorded = self.queue.one_shot(&self.device, self.queue_family, |rec| {
            self.record_scene(rec, scene, view, shown);
            rec.begin_render_pass_ext(
                &pipes.capture_pass,
                &attachments.framebuffer,
                &[
                    ClearValue::Color([0.0; 4]),
                    ClearValue::DepthStencil {
                        depth: 1.0,
                        stencil: 0,
                    },
                ],
            );
            self.record_post(rec, corner, shown, &pipes.post);
            Self::record_canvas(
                rec,
                self.canvas.as_ref(),
                (width, height),
                (pipes.canvas.as_ref(), screen),
            );
            rec.end_render_pass();
            rec.copy_image_to_buffer(
                &color.image,
                ImageLayout::TRANSFER_SRC_OPTIMAL,
                &readback.buffer,
                &[BufferImageCopy::full_2d(width, height)],
            );
            rec.memory_barrier(
                PipelineStage::TRANSFER,
                PipelineStage::HOST,
                AccessFlags::TRANSFER_WRITE,
                AccessFlags::HOST_READ,
            );
            Ok(())
        });
        // Every frame and picture says what it encodes for, so nothing needs putting back.
        recorded?;
        self.read_frame(self.slot);
        let mapped = readback.memory.map()?;
        let bytes = &mapped.as_slice()[..size];
        let values = match output {
            ScreenOutput::ScRgb => bytes
                .as_chunks::<8>()
                .0
                .iter()
                .map(|pixel| {
                    let half =
                        |at: usize| half_to_f32(u16::from_le_bytes([pixel[at], pixel[at + 1]]));
                    [half(0), half(2), half(4), half(6)]
                })
                .collect(),
            _ => bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|pixel| {
                    let packed = u32::from_le_bytes(*pixel);
                    let part = |shift: u32| {
                        f32::from(u16::try_from(packed >> shift & 0x3ff).unwrap_or(0)) / 1023.0
                    };
                    let alpha = f32::from(u8::try_from(packed >> 30).unwrap_or(0)) / 3.0;
                    if blue_first {
                        [part(20), part(10), part(0), alpha]
                    } else {
                        [part(0), part(10), part(20), alpha]
                    }
                })
                .collect(),
        };
        Ok(ScreenCapture {
            width,
            height,
            values,
        })
    }
}

/// A 16 bit float as an `f32`.
fn half_to_f32(half: u16) -> f32 {
    let sign = if half & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exponent = i32::from(half >> 10 & 0x1f);
    let fraction = f32::from(half & 0x3ff);
    match exponent {
        0 => sign * fraction * (-24.0_f32).exp2(),
        31 => {
            if fraction == 0.0 {
                sign * f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => {
            sign * (1.0 + fraction / 1024.0)
                * f32::from(i16::try_from(exponent - 15).unwrap_or(0)).exp2()
        }
    }
}
