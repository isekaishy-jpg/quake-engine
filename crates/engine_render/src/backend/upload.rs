//! Getting what's drawn onto the GPU.

use std::sync::Arc;

use engine_core::canvas::Quad;
use engine_core::math::Vec3;
use engine_map::{
    AliasModel, BOUNCE_STYLE, Fog, FogVolume, Look, MOST_CLOUDS, Map, Material, MipLevel,
    PixelForm, Pool, ProbeGrid, RayStencil, STENCIL_GRID, Sky, Sprite, Texture, TextureKind,
    ToneCurve, WaterSheet,
};
use vulkane::safe::{
    AccessFlags, Buffer, BufferCopy, BufferCreateInfo, BufferImageCopy, BufferUsage,
    DescriptorPool, DescriptorPoolSize, DescriptorSet, DescriptorType, Device, DeviceMemory,
    Format, Image, Image2dCreateInfo, ImageBarrier, ImageLayout, ImageUsage, ImageView,
    MemoryPropertyFlags, PhysicalDevice, PipelineStage,
};

use super::bloom::BloomStep;
use super::exposure::Exposure;
use super::post::render_scale;
use super::ripple::GpuRipples;
use super::shadow::{SUN_CASCADES, ShadowRange, SunShadow, shadow_figures};
use super::{
    Backend, DEPTH_FORMAT, EXTRA_FLOATS, FOG_LIGHTS, FRAME_SIZE, FRAMES, GpuAlias, GpuBuffer,
    GpuImage, GpuMap, GpuMesh, GpuPicture, GpuSprite, LIVE_BYTES, MAX_FOG_VOLUMES, MAX_POOLS,
    POST_BYTES, SKY_BYTES, Scene, ShadowInstance, TileWrite,
};
use super::{TAA_FRAMES, TAA_SETTING};
use crate::alias::{look_bytes, pose_bytes, rest_bytes, weld_corners};
use crate::canvas::{VERTEX_FLOATS, canvas_vertices};
use crate::error::RenderError;
use crate::instances::{
    INSTANCE_BYTES, LevelView, instance_batches, shared_looks, shared_places, view_planes,
};
use crate::live::{LiveLight, live_bytes};
use crate::material::{
    MATERIAL_PARAMS_BYTES, MATERIAL_PARAMS_STRIDE, material_params as material_params_of,
};
use crate::matrix::{inverse, view_projection};
use crate::renderer::{CapsuleDraw, PostSettings, SkyRayFloor, View};
use crate::sprite::{LightDraw, MAX_LIGHT_DRAWS, PARTICLE_BYTES, write_particle};
use crate::sprite::{MAX_PARTICLE_DRAWS, behind_first};
use crate::surface::built_in_for;
use crate::terrain::terrain_nodes;
use crate::world::{BatchKind, batch_kind, uses_material};
use crate::world::{ShadowGroup, TileImage, VertexPatch, shadow_groups};
use crate::world::{Vertex, WorldMesh};

/// A buffer the CPU can write.
/// Copies a square `size` across from `offset` in `staging` to `corner` of
/// `atlas`, which is read before and after.
fn copy_into_atlas(
    rec: &mut vulkane::safe::CommandBufferRecording<'_>,
    staging: &GpuBuffer,
    atlas: &GpuImage,
    offset: u64,
    size: u32,
    corner: [u32; 2],
) {
    rec.image_barrier(
        PipelineStage::ALL_GRAPHICS,
        PipelineStage::TRANSFER,
        ImageBarrier::color(
            &atlas.image,
            ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            ImageLayout::TRANSFER_DST_OPTIMAL,
            AccessFlags::SHADER_READ,
            AccessFlags::TRANSFER_WRITE,
        ),
    );
    let mut region = BufferImageCopy::full_2d(size, size);
    region.buffer_offset = offset;
    region.image_offset = [
        i32::try_from(corner[0]).unwrap_or(0),
        i32::try_from(corner[1]).unwrap_or(0),
        0,
    ];
    rec.copy_buffer_to_image(
        &staging.buffer,
        &atlas.image,
        ImageLayout::TRANSFER_DST_OPTIMAL,
        &[region],
    );
    rec.image_barrier(
        PipelineStage::TRANSFER,
        PipelineStage::FRAGMENT_SHADER,
        ImageBarrier::color(
            &atlas.image,
            ImageLayout::TRANSFER_DST_OPTIMAL,
            ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            AccessFlags::TRANSFER_WRITE,
            AccessFlags::SHADER_READ,
        ),
    );
}

/// Each texture of `map`'s cutoff, if it has holes: a masked material's, or half for a picture
/// with holes and no material.
fn shadow_cutoffs(map: &Map) -> Vec<Option<f32>> {
    map.textures
        .iter()
        .map(|texture| {
            (batch_kind(texture) == Some(BatchKind::Masked)).then(|| {
                texture
                    .material
                    .as_ref()
                    .filter(|_| uses_material(texture))
                    .map_or(0.5, |material| material.cutoff)
            })
        })
        .collect()
}

/// Whether a material of `map` casts shadows: what's solid, and not a liquid.
pub(super) fn casts_shadow(map: &Map, texture: u32) -> bool {
    map.textures.get(texture as usize).is_some_and(|texture| {
        texture.kind != TextureKind::Liquid
            && !matches!(
                batch_kind(texture),
                None | Some(BatchKind::Sky | BatchKind::Blended)
            )
    })
}

pub(super) fn host_buffer(
    device: &Device,
    physical: &PhysicalDevice,
    size: usize,
    usage: BufferUsage,
) -> Result<GpuBuffer, RenderError> {
    let (buffer, memory) = Buffer::new_bound(
        device,
        physical,
        BufferCreateInfo {
            size: size.max(4) as u64,
            usage,
        },
        MemoryPropertyFlags::HOST_VISIBLE | MemoryPropertyFlags::HOST_COHERENT,
    )?;
    Ok(GpuBuffer { buffer, memory })
}

/// The format of compiled `mips`, if that's what they are.
pub(super) fn compiled_format(mips: &[MipLevel]) -> Option<Format> {
    match mips.first()?.form {
        PixelForm::Rgba => None,
        PixelForm::Bc7Srgb => Some(Format::BC7_SRGB),
        PixelForm::Bc7 => Some(Format::BC7_UNORM),
        PixelForm::Bc5 => Some(Format::BC5_UNORM),
    }
}

/// How slot `number` of `material` is held: colours as sRGB, which reads back as
/// linear light, and what's a measure of something as it is.
fn slot_format(material: &Material, number: u8) -> Format {
    let image = material.slots.get(usize::from(number).wrapping_sub(1));
    if let Some(format) = image.and_then(|mips| compiled_format(mips)) {
        return format;
    }
    let [first, second] = material.shader_images;
    let metal_rough = material.metal_rough.and_then(|surface| surface.map);
    let data = [
        metal_rough,
        material.normal,
        material.specular,
        material.height,
        material.detail,
        material.flow,
        first,
        second,
    ];
    let mask = material
        .layers
        .iter()
        .any(|layer| layer.mask == number || layer.surface == Some(number));
    if mask || data.contains(&Some(number)) {
        Format::R8G8B8A8_UNORM
    } else {
        Format::R8G8B8A8_SRGB
    }
}

/// A uniform buffer that only the GPU writes, by copying.
pub(super) fn uniform_buffer(
    device: &Device,
    physical: &PhysicalDevice,
    size: usize,
) -> Result<GpuBuffer, RenderError> {
    let (buffer, memory) = Buffer::new_bound(
        device,
        physical,
        BufferCreateInfo {
            size: size.max(4) as u64,
            usage: BufferUsage::UNIFORM_BUFFER | BufferUsage::TRANSFER_DST,
        },
        MemoryPropertyFlags::DEVICE_LOCAL,
    )?;
    Ok(GpuBuffer { buffer, memory })
}

/// An image in device memory.
pub(super) fn device_image(
    device: &Device,
    physical: &PhysicalDevice,
    image: Image,
    depth: bool,
) -> Result<GpuImage, RenderError> {
    let requirements = image.memory_requirements();
    let memory_type = physical
        .find_memory_type(
            requirements.memory_type_bits,
            MemoryPropertyFlags::DEVICE_LOCAL,
        )
        .ok_or_else(|| RenderError("no memory for an image".to_owned()))?;
    let memory = DeviceMemory::allocate(device, requirements.size, memory_type)?;
    image.bind_memory(&memory, 0)?;
    let view = if depth {
        ImageView::new_2d_depth(&image)?
    } else {
        ImageView::new_2d_color(&image)?
    };
    Ok(GpuImage {
        view,
        image,
        _memory: memory,
    })
}

/// A depth image of `size`.
pub(super) fn depth_image(
    device: &Device,
    physical: &PhysicalDevice,
    size: (u32, u32),
) -> Result<GpuImage, RenderError> {
    let image = Image::new_2d(
        device,
        Image2dCreateInfo {
            format: DEPTH_FORMAT,
            width: size.0,
            height: size.1,
            usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT,
        },
    )?;
    device_image(device, physical, image, true)
}

/// `view`'s uniforms, with `lights`.
#[expect(
    clippy::cast_precision_loss,
    reason = "there are at most a few dozen lights"
)]
pub(super) fn frame_bytes(
    view: &View,
    aspect: f32,
    lights: &[LightDraw],
    sky: Option<&Sky>,
    fog: Option<&Fog>,
    post: &PostSettings,
    extra: &FrameExtras<'_>,
) -> Vec<u8> {
    let lights = lights.get(..MAX_LIGHT_DRAWS).unwrap_or(lights);
    let mut matrix = view_projection(
        view.origin,
        [view.yaw, view.pitch, view.roll],
        view.fov,
        aspect,
    );
    // TAA's moves the view a fraction of a pixel, by adding it in proportion to depth.
    let [across, down] = extra.jitter;
    for column in 0..4 {
        let w = matrix[column * 4 + 3];
        matrix[column * 4] += across * w;
        matrix[column * 4 + 1] += down * w;
    }
    let eye = [view.origin.x, view.origin.y, view.origin.z, 1.0];
    let params = [view.time, aspect, lights.len() as f32, view.gamma];
    let mut positions = [0.0; MAX_LIGHT_DRAWS * 4];
    let mut cuts = [0.0; MAX_LIGHT_DRAWS];
    for ((light, position), cut) in lights
        .iter()
        .zip(positions.as_chunks_mut::<4>().0)
        .zip(&mut cuts)
    {
        *position = [light.origin.x, light.origin.y, light.origin.z, light.radius];
        *cut = light.min_light;
    }
    let mut styles = view.styles;
    if let Some(bounce) = styles.get_mut(usize::from(BOUNCE_STYLE)) {
        *bounce = 1.0;
    }
    let mut out = Vec::with_capacity(FRAME_SIZE);
    for value in matrix
        .iter()
        .chain(&eye)
        .chain(&params)
        .chain(&styles)
        .chain(&positions)
        .chain(&cuts)
        .chain(&sky_values(sky, extra.sun_toward))
        .chain(&post_values(&matrix, post, fog))
        .chain(&extra_values(extra, post))
    {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// What a frame's stencils and capsules are.
#[derive(Clone, Copy)]
pub(super) struct FrameExtras<'a> {
    /// The stencils.
    stencils: &'a [RayStencil],
    /// Where their lights are now.
    lights: &'a [Vec3],
    /// How open their cells are.
    masks: &'a [[f32; STENCIL_GRID * STENCIL_GRID]],
    /// How far light gets past their cells.
    reaches: &'a [[f32; STENCIL_GRID * STENCIL_GRID]],
    /// The sky, for its rays.
    sky: Option<&'a Sky>,
    /// How low the sun's light gets near the view.
    sky_floor: Option<&'a SkyRayFloor>,
    /// What the sun's baked light is multiplied by.
    sun_tint: [f32; 3],
    /// Towards the sun, where a day has it.
    sun_toward: Option<Vec3>,
    /// What water's colour is multiplied by.
    water_tint: [f32; 3],
    /// Light added everywhere, and what highlights are multiplied by.
    day_light: [f32; 4],
    /// The characters shading light.
    capsules: &'a [CapsuleDraw],
    /// How the map's light comes out.
    look: Look,
    /// What exposure that goes by itself multiplies light by.
    exposure_scale: f32,
    /// What the final pass multiplies what glows by.
    bloom: f32,
    /// What baked light is multiplied by.
    light_scale: f32,
    /// What the final pass encodes for: which screen output, white in nits, and the brightest
    /// next to white.
    screen: [f32; 4],
    /// How far TAA moves the view this frame, as a share of the screen each way.
    jitter: [f32; 2],
    /// Whether there's TAA, whether there's a frame before to go on from, and which frame of
    /// its round this is.
    taa: [f32; 4],
    /// Where the view was last frame, with nothing off.
    last_view_projection: [f32; 16],
    /// Where the view is, with nothing off.
    steady_view_projection: [f32; 16],
    /// What a zone does to the light.
    light_shift: [f32; 3],
    /// Bodies of liquid.
    pools: &'a [Pool],
    /// Bodies of liquid with a shape.
    sheets: &'a [WaterSheet],
    /// Where the ripples are, and how wide, which is 0 with none.
    ripples: [f32; 3],
    /// Fog in places.
    fog_volumes: &'a [FogVolume],
    /// The lights that move, for those in fog.
    fog_lights: &'a [LiveLight],
    /// How many samples a pixel the scene has.
    samples: u32,
    /// Where the view is, to choose among them.
    eye: Vec3,
    /// Whether the view is under the open sky, so that mirrors can show it.
    under_sky: bool,
    /// The baked light in the air.
    probes: Option<&'a ProbeGrid>,
}

/// The stencil and capsule part of the frame uniforms.
fn extra_values(extra: &FrameExtras<'_>, post: &PostSettings) -> Vec<f32> {
    let FrameExtras {
        stencils,
        lights,
        masks,
        capsules,
        ..
    } = *extra;
    let mut out = vec![0.0; EXTRA_FLOATS];
    let count = if post.rays == 0 {
        0
    } else {
        stencils.len().min(4)
    };
    for (index, stencil) in stencils.iter().take(4).enumerate() {
        let at = index * 4;
        out[at..at + 4].copy_from_slice(&[
            stencil.corner.x,
            stencil.corner.y,
            stencil.corner.z,
            stencil.strength,
        ]);
        let at = 16 + index * 4;
        out[at..at + 3].copy_from_slice(&[stencil.across.x, stencil.across.y, stencil.across.z]);
        let at = 32 + index * 4;
        out[at..at + 4].copy_from_slice(&[
            stencil.down.x,
            stencil.down.y,
            stencil.down.z,
            stencil.reach,
        ]);
        let at = 48 + index * 4;
        let light = lights.get(index).copied().unwrap_or(stencil.light);
        out[at..at + 4].copy_from_slice(&[
            light.x,
            light.y,
            light.z,
            f32::from(u8::from(stencil.sun)),
        ]);
        let at = 68 + index * 256;
        match masks.get(index) {
            Some(mask) => out[at..at + 256].copy_from_slice(mask),
            None => out[at..at + 256].fill(1.0),
        }
    }
    #[expect(clippy::cast_precision_loss, reason = "four at most")]
    let stencil_count = count as f32;
    out[64] = stencil_count;
    // Steps along the view, and how blurred the stencil is read.
    let (steps, blur) = match post.rays {
        1 => (8.0, 8.0),
        2 => (12.0, 3.0),
        _ => (24.0, 1.5),
    };
    out[65] = steps;
    out[66] = blur;
    #[expect(clippy::cast_precision_loss, reason = "eight at most")]
    let samples = extra.samples as f32;
    out[67] = samples;
    let base = 68 + 1024;
    let shown = capsules.len().min(16);
    for (index, capsule) in capsules.iter().take(16).enumerate() {
        let at = base + index * 4;
        out[at..at + 4].copy_from_slice(&[
            capsule.origin.x,
            capsule.origin.y,
            capsule.origin.z,
            capsule.radius,
        ]);
        let at = base + 64 + index * 4;
        out[at..at + 4].copy_from_slice(&[
            capsule.toward_light.x,
            capsule.toward_light.y,
            capsule.toward_light.z,
            capsule.strength,
        ]);
    }
    #[expect(clippy::cast_precision_loss, reason = "sixteen at most")]
    let capsule_count = shown as f32;
    out[base + 128] = capsule_count;
    let base = base + 132;
    let bodies = nearest_bodies(extra.pools, extra.sheets, extra.eye);
    for (index, (pool, _)) in bodies.iter().enumerate() {
        let at = base + index * 4;
        out[at..at + 4].copy_from_slice(&[
            pool.low.x,
            pool.low.y,
            pool.low.z,
            pool.look.absorption,
        ]);
        let at = base + MAX_POOLS * 4 + index * 4;
        out[at..at + 4].copy_from_slice(&[
            pool.high.x,
            pool.high.y,
            pool.high.z,
            pool.look.caustics,
        ]);
        let at = base + MAX_POOLS * 8 + index * 4;
        let tinted: [f32; 3] =
            std::array::from_fn(|part| pool.look.color[part] * extra.water_tint[part]);
        out[at..at + 3].copy_from_slice(&tinted.map(linear));
        out[at + 3] = pool.look.reflect;
        let at = base + MAX_POOLS * 12 + index * 4;
        out[at] = pool.look.waves;
        out[at + 1] = pool.look.refraction;
        out[at + 2] = pool.look.deep;
        out[at + 3] = f32::from(u8::from(bodies[index].1.is_some()));
    }
    let base = base + MAX_POOLS * 16;
    #[expect(clippy::cast_precision_loss, reason = "eight at most")]
    let pool_count = bodies.len() as f32;
    out[base] = pool_count;
    out[base + 1] = post.detail_distance;
    out[base + 2] = f32::from(u8::from(extra.under_sky));
    let holding = bodies
        .iter()
        .position(|(_, sheet)| sheet.is_some_and(|sheet| extra.sheets[sheet].holds(extra.eye)));
    #[expect(clippy::cast_precision_loss, reason = "eight at most")]
    let holding = holding.map_or(0.0, |at| at as f32 + 1.0);
    out[base + 3] = holding;
    if let Some(probes) = extra.probes {
        out[base + 4..base + 8].copy_from_slice(&[
            probes.origin.x,
            probes.origin.y,
            probes.origin.z,
            probes.spacing,
        ]);
        #[expect(clippy::cast_precision_loss, reason = "grid sizes are small")]
        let size = probes.size.map(|axis| axis as f32);
        out[base + 8..base + 11].copy_from_slice(&size);
        out[base + 11] = f32::from(u8::from(post.fog_grid));
    }
    out[base + 12] = extra.look.light_curve;
    out[base + 13] = extra.look.exposure * extra.exposure_scale;
    out[base + 14] = match extra.look.tone {
        ToneCurve::Hold => 0.0,
        ToneCurve::Neutral => 1.0,
        ToneCurve::Film => 2.0,
    };
    let [red, green, blue] = extra.light_shift;
    out[base + 16..base + 19].copy_from_slice(&[1.0 + red, 1.0 + green, 1.0 + blue]);
    let base = base + 20;
    fog_values(&mut out[base..], extra);
    let base = base + MAX_FOG_VOLUMES * 16 + 4 + FOG_LIGHTS * 8;
    for (body, (pool, _)) in bodies.iter().enumerate() {
        let look = &pool.look;
        for (slot, overlay) in look.overlays.iter().chain([&look.foam_look]).enumerate() {
            let at = base + (body * 3 + slot) * 4;
            #[expect(clippy::cast_precision_loss, reason = "a few pictures")]
            let picture = overlay.picture as f32;
            let [x, y] = overlay.flow;
            // How much shows is what's left of how wide it is, in the fraction.
            let shows = overlay.size.round() + overlay.strength.clamp(0.0, 0.999);
            out[at..at + 4].copy_from_slice(&[picture, shows, x, y]);
        }
        let at = base + MAX_POOLS * 12 + body * 4;
        out[at..at + 2].copy_from_slice(&[look.foam, look.shore]);
    }
    let base = base + MAX_POOLS * 16;
    out[base..base + 3].copy_from_slice(&extra.ripples);
    let base = base + 4;
    for (index, stencil) in stencils.iter().take(4).enumerate() {
        let at = base + index * 256;
        match extra.reaches.get(index) {
            Some(reach) => out[at..at + 256].copy_from_slice(reach),
            None => out[at..at + 256].fill(stencil.reach),
        }
    }
    sky_ray_values(&mut out[base + 1024..], extra, post.rays != 0);
    let base = base + 1024 + 12 + 256;
    out[base..base + 3].copy_from_slice(&extra.sun_tint);
    out[base + 4..base + 8].copy_from_slice(&extra.day_light);
    // How much corners darken, how far that looks, and how many looks it takes.
    let looks = match post.ssao {
        0 => 0.0,
        1 => 8.0,
        _ => 16.0,
    };
    let strength = if looks > 0.0 { extra.look.ssao } else { 0.0 };
    out[base + 8..base + 11].copy_from_slice(&[strength, extra.look.ssao_radius, looks]);
    out[base + 12] = extra.bloom;
    out[base + 16] = extra.light_scale;
    out[base + 17] = extra.look.light_most.unwrap_or(0.0);
    out[base + 20..base + 24].copy_from_slice(&extra.screen);
    out[base + 24..base + 28].copy_from_slice(&extra.taa);
    out[base + 28..base + 44].copy_from_slice(&extra.last_view_projection);
    out[base + 44..base + 60].copy_from_slice(&extra.steady_view_projection);
    out[base + 60] = f32::from(u8::from(post.steady_shine));
    out
}

/// The `index`th of Halton's sequence in `base`, from 0 to 1, which spreads evenly however many
/// are taken.
fn halton(mut index: u32, base: u32) -> f32 {
    let mut fraction = 1.0;
    let mut out = 0.0;
    #[expect(clippy::cast_precision_loss, reason = "a small base")]
    let size = base as f32;
    while index > 0 {
        fraction /= size;
        #[expect(clippy::cast_precision_loss, reason = "a digit")]
        let digit = (index % base) as f32;
        out += fraction * digit;
        index /= base;
    }
    out
}

/// The bodies of liquid nearest `eye`, as pools, with which sheet each is, if it's one.
pub(super) fn nearest_bodies(
    pools: &[Pool],
    sheets: &[WaterSheet],
    eye: Vec3,
) -> Vec<(Pool, Option<usize>)> {
    let boxed = pools.iter().map(|pool| (*pool, None));
    let shaped = sheets.iter().enumerate().map(|(number, sheet)| {
        let (low, high) = sheet.bounds();
        let look = sheet.look;
        (Pool { low, high, look }, Some(number))
    });
    let mut bodies: Vec<(Pool, Option<usize>)> = boxed.chain(shaped).collect();
    let distance = |pool: &Pool| {
        let nearest = Vec3::new(
            eye.x.clamp(pool.low.x, pool.high.x),
            eye.y.clamp(pool.low.y, pool.high.y),
            eye.z.clamp(pool.low.z, pool.high.z),
        );
        (nearest - eye).length()
    };
    bodies.sort_by(|a, b| distance(&a.0).total_cmp(&distance(&b.0)));
    bodies.truncate(MAX_POOLS);
    bodies
}

/// Fog's part of the frame uniforms.
fn fog_values(out: &mut [f32], extra: &FrameExtras<'_>) {
    let mut volumes: Vec<&FogVolume> = extra.fog_volumes.iter().collect();
    // How far a volume is from the eye, which is nothing inside it.
    let away = |volume: &FogVolume| {
        let nearest = Vec3::new(
            extra.eye.x.clamp(volume.low.x, volume.high.x),
            extra.eye.y.clamp(volume.low.y, volume.high.y),
            extra.eye.z.clamp(volume.low.z, volume.high.z),
        );
        (nearest - extra.eye).length()
    };
    volumes.sort_by(|a, b| away(a).total_cmp(&away(b)));
    volumes.truncate(MAX_FOG_VOLUMES);
    let mut lit = 0_u32;
    for (index, volume) in (0_u32..).zip(&volumes) {
        let at = index as usize * 4;
        let (low, high, drift) = (volume.low, volume.high, volume.drift);
        out[at..at + 4].copy_from_slice(&[low.x, low.y, low.z, volume.edge]);
        let at = at + MAX_FOG_VOLUMES * 4;
        out[at..at + 4].copy_from_slice(&[high.x, high.y, high.z, volume.density]);
        let at = at + MAX_FOG_VOLUMES * 4;
        out[at..at + 3].copy_from_slice(&volume.color.map(linear));
        out[at + 3] = volume.wisps;
        let at = at + MAX_FOG_VOLUMES * 4;
        out[at..at + 4].copy_from_slice(&[drift.x, drift.y, drift.z, volume.wisp_size]);
        lit |= u32::from(volume.lit) << index;
    }
    let base = MAX_FOG_VOLUMES * 16;
    let mut lights: Vec<&LiveLight> = extra.fog_lights.iter().collect();
    let away = |light: &LiveLight| (light.origin - extra.eye).length() - light.radius;
    lights.sort_by(|a, b| away(a).total_cmp(&away(b)));
    lights.truncate(FOG_LIGHTS);
    #[expect(clippy::cast_precision_loss, reason = "eight at most")]
    let volume_count = volumes.len() as f32;
    #[expect(clippy::cast_precision_loss, reason = "four at most")]
    let light_count = lights.len() as f32;
    #[expect(clippy::cast_precision_loss, reason = "eight bits")]
    let lit_bits = lit as f32;
    let marched = volumes.iter().any(|volume| volume.marched());
    let marched = f32::from(u8::from(marched));
    out[base..base + 4].copy_from_slice(&[volume_count, light_count, lit_bits, marched]);
    for (index, light) in lights.iter().enumerate() {
        let at = base + 4 + index * 4;
        let place = light.origin;
        out[at..at + 4].copy_from_slice(&[place.x, place.y, place.z, light.radius]);
        let at = base + 4 + FOG_LIGHTS * 4 + index * 4;
        out[at..at + 3].copy_from_slice(&light.color);
    }
}

/// A colour as it's shown, in linear light, as `to_linear` does in the shaders.
fn linear(color: f32) -> f32 {
    if color <= 0.04045 {
        color / 12.92
    } else {
        ((color + 0.055) / 1.055).powf(2.4)
    }
}

/// The final pass's part of the frame uniforms.
fn post_values(
    matrix: &[f32; 16],
    post: &PostSettings,
    fog: Option<&Fog>,
) -> [f32; POST_BYTES / 4] {
    let mut out = [0.0; POST_BYTES / 4];
    out[..16].copy_from_slice(&inverse(matrix).unwrap_or([0.0; 16]));
    if let Some(fog) = fog {
        out[16..19].copy_from_slice(&fog.color.map(linear));
        out[19] = fog.density;
        out[20] = fog.height;
        out[21] = fog.falloff;
    }
    out[24..27].copy_from_slice(&post.outline_color.map(linear));
    out[27] = f32::from(post.outline);
    out[28] = f32::from(post.edge_smoothing);
    out[29] = post.outline_width;
    out[30] = post.outline_fade;
    out[31] = post.rounded_edges;
    out
}

/// What a frame says of rays from the sky: how bright and what from, how a picture goes, and
/// what's known of where light gets to.
fn sky_ray_values(out: &mut [f32], extra: &FrameExtras<'_>, wanted: bool) {
    let rays = extra
        .sky
        .and_then(|sky| sky.rays.as_ref())
        .filter(|_| wanted);
    let Some(rays) = rays else {
        return;
    };
    let lowest = extra.sky.iter().flat_map(|sky| &sky.clouds);
    let lowest = lowest
        .map(|clouds| clouds.height)
        .filter(|height| *height > 0.0);
    let top = lowest.fold(f32::INFINITY, f32::min);
    out[0] = rays.strength;
    out[1] = if rays.picture.is_some() { 2.0 } else { 1.0 };
    out[2] = f32::from(u8::from(rays.into_level));
    out[3] = if top.is_finite() { top } else { 0.0 };
    out[4] = rays.size;
    out[5..7].copy_from_slice(&rays.speed);
    out[7] = rays.flare;
    if let Some(floor) = extra.sky_floor {
        out[8..12].copy_from_slice(&floor.area);
        out[12..12 + 256].copy_from_slice(&floor.heights);
    }
}

/// The sky's part of the frame uniforms.
fn sky_values(sky: Option<&Sky>, sun_toward: Option<Vec3>) -> [f32; SKY_BYTES / 4] {
    let mut out = [0.0; SKY_BYTES / 4];
    let Some(sky) = sky else {
        return out;
    };
    out[..3].copy_from_slice(&sky.zenith);
    out[3] = sky.horizon_height;
    out[4..7].copy_from_slice(&sky.horizon);
    out[8..11].copy_from_slice(&sky.ground);
    out[12..15].copy_from_slice(&[
        sky.sun_direction.x,
        sky.sun_direction.y,
        sky.sun_direction.z,
    ]);
    out[15] = sky.sun_size;
    // A sun that moves is where it is now, not where the level put it.
    if let Some(toward) = sun_toward {
        let toward = toward.normalized();
        out[12..15].copy_from_slice(&[toward.x, toward.y, toward.z]);
    }
    let at = 20 + MOST_CLOUDS * 12;
    out[at + 3] = sky.moon_size;
    out[at + 4..at + 7].copy_from_slice(&sky.moon_color);
    out[at + 7] = sky.stars;
    out[16..19].copy_from_slice(&sky.sun_color);
    let mut picture = 0.0;
    for (index, clouds) in sky.clouds.iter().take(MOST_CLOUDS).enumerate() {
        let at = 20 + index * 4;
        out[at..at + 3].copy_from_slice(&clouds.color);
        out[at + 3] = clouds.coverage;
        let at = 20 + MOST_CLOUDS * 4 + index * 4;
        out[at..at + 2].copy_from_slice(&clouds.speed);
        out[at + 2] = clouds.scale;
        out[at + 3] = if clouds.image.is_some() { 2.0 } else { 1.0 };
        let at = 20 + MOST_CLOUDS * 8 + index * 4;
        out[at..at + 4].copy_from_slice(&[clouds.height, clouds.curve, clouds.lit, picture]);
        if clouds.image.is_some() {
            picture += 1.0;
        }
    }
    out
}

impl Backend {
    /// Uploads a flat picture of RGBA pixels, and returns its number.
    pub(crate) fn add_picture(
        &mut self,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<u32, RenderError> {
        let image = self.upload_image(&[(width, height, rgba)], Format::R8G8B8A8_UNORM)?;
        let pool = DescriptorPool::new(
            &self.device,
            1,
            &[
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLED_IMAGE,
                    descriptor_count: 1,
                },
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLER,
                    descriptor_count: 1,
                },
            ],
        )?;
        let canvas = self
            .canvas
            .as_mut()
            .ok_or_else(|| RenderError("no canvas".to_owned()))?;
        let set = pool.allocate(&canvas.set_layout)?;
        set.write_sampled_image(0, &image.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        set.write_sampler(1, &canvas.sampler);
        canvas.pictures.push(GpuPicture {
            set,
            _pool: pool,
            _image: image,
        });
        Ok(u32::try_from(canvas.pictures.len() - 1).unwrap_or(u32::MAX))
    }

    /// Writes this frame's canvas.
    pub(super) fn upload_canvas(
        &mut self,
        quads: &[Quad],
        size: (u32, u32),
    ) -> Result<(), RenderError> {
        let slot = self.slot;
        let Some(canvas) = &mut self.canvas else {
            return Ok(());
        };
        canvas.slot = slot;
        let (floats, batches) = canvas_vertices(quads, size);
        let mut mapped = canvas.vertices[slot].memory.map()?;
        let (target, _remainder) = mapped.as_slice_mut().as_chunks_mut::<4>();
        let ui_first = floats.len() / VERTEX_FLOATS;
        for (value, chunk) in floats.iter().chain(&canvas.ui.vertices).zip(target) {
            *chunk = value.to_le_bytes();
        }
        drop(mapped);
        canvas.batches = batches;
        canvas.ui_first = u32::try_from(ui_first).unwrap_or(0);
        Ok(())
    }

    /// Uploads `pictures`, as a width, a height and pixels, each with a set that
    /// draws it.
    fn upload_textures(
        &self,
        pictures: &[(u32, u32, &[u8])],
    ) -> Result<(DescriptorPool, Vec<DescriptorSet>, Vec<GpuImage>), RenderError> {
        let sets = u32::try_from(pictures.len().max(1)).unwrap_or(u32::MAX);
        let pool = DescriptorPool::new(
            &self.device,
            sets,
            &[
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLED_IMAGE,
                    descriptor_count: sets,
                },
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLER,
                    descriptor_count: sets,
                },
            ],
        )?;
        let mut images = Vec::with_capacity(pictures.len());
        let mut image_sets = Vec::with_capacity(pictures.len());
        for picture in pictures {
            let image = self.upload_image(&[*picture], Format::R8G8B8A8_SRGB)?;
            let set = pool.allocate(&self.texture_layout)?;
            set.write_sampled_image(0, &image.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            set.write_sampler(1, &self.texture_sampler);
            image_sets.push(set);
            images.push(image);
        }
        Ok((pool, image_sets, images))
    }

    /// Uploads a model, and returns its number.
    pub(crate) fn add_model(&mut self, model: Arc<AliasModel>) -> Result<u32, RenderError> {
        let welded = weld_corners(&model);
        let (poses, pose_set, pose_pool) =
            self.upload_poses(&pose_bytes(&model, &welded.corners))?;
        let rest = self.device_buffer(
            &rest_bytes(&model, &welded.corners),
            BufferUsage::VERTEX_BUFFER,
        )?;
        let uvs = self.device_buffer(&look_bytes(&welded.corners), BufferUsage::VERTEX_BUFFER)?;
        let index_bytes: Vec<u8> = welded
            .indices
            .iter()
            .flat_map(|index| index.to_le_bytes())
            .collect();
        let indices = self.device_buffer(&index_bytes, BufferUsage::INDEX_BUFFER)?;
        let size = (model.skin_width, model.skin_height);
        let pictures: Vec<_> = model
            .skins
            .iter()
            .map(|skin| (size.0, size.1, skin.as_slice()))
            .collect();
        let (pool, skin_sets, skins) = self.upload_textures(&pictures)?;
        self.models.push(GpuAlias {
            model,
            corners: u32::try_from(welded.corners.len()).unwrap_or(0),
            _poses: poses,
            pose_set,
            _pose_pool: pose_pool,
            rest,
            uvs,
            indices,
            index_count: u32::try_from(welded.indices.len()).unwrap_or(0),
            skin_sets,
            _skins: skins,
            _pool: pool,
        });
        Ok(u32::try_from(self.models.len() - 1).unwrap_or(u32::MAX))
    }

    /// Uploads a sprite, and returns its number.
    pub(crate) fn add_sprite(&mut self, sprite: Arc<Sprite>) -> Result<u32, RenderError> {
        let pictures: Vec<_> = sprite
            .images
            .iter()
            .map(|picture| (picture.width, picture.height, picture.rgba.as_slice()))
            .collect();
        let (pool, image_sets, images) = self.upload_textures(&pictures)?;
        self.sprites.push(GpuSprite {
            sprite,
            sets: image_sets,
            _images: images,
            _pool: pool,
        });
        Ok(u32::try_from(self.sprites.len() - 1).unwrap_or(u32::MAX))
    }

    /// Uploads `map`.
    pub(crate) fn load_map(&mut self, map: &Map, mesh: &WorldMesh) -> Result<(), RenderError> {
        self.wait_for_frame()?;
        self.device.wait_idle()?;
        self.map = None;
        // A new level starts where its own exposure should be, and TAA with nothing kept.
        self.exposure = Exposure::default();
        self.brightness = None;
        self.taa_kept = false;
        self.last_view_projection = None;
        self.last_brush_origins.clear();
        self.moved_brushes.clear();
        self.tile_writes.borrow_mut().clear();
        let mut vertex_bytes = Vec::with_capacity(mesh.vertices.len() * Vertex::SIZE);
        for vertex in &mesh.vertices {
            vertex.write(&mut vertex_bytes);
        }
        let vertices = self.device_buffer(&vertex_bytes, BufferUsage::VERTEX_BUFFER)?;
        let world_indices = mesh.world_index_count as usize;
        let index_buffer = || -> Result<GpuBuffer, RenderError> {
            let mut indices = host_buffer(
                &self.device,
                &self.physical,
                (world_indices + mesh.model_indices.len()) * 4,
                BufferUsage::INDEX_BUFFER,
            )?;
            {
                let mut mapped = indices.memory.map()?;
                let (target, _remainder) = mapped.as_slice_mut().as_chunks_mut::<4>();
                let models = &mut target[world_indices..];
                for (index, chunk) in mesh.model_indices.iter().zip(models) {
                    *chunk = index.to_le_bytes();
                }
            }
            Ok(indices)
        };
        let indices = [index_buffer()?, index_buffer()?];
        let mut shared_meshes = Vec::with_capacity(map.shared_meshes.len());
        for shared in &map.shared_meshes {
            let index_bytes: Vec<u8> = shared
                .indices
                .iter()
                .flat_map(|i| i.to_le_bytes())
                .collect();
            shared_meshes.push(GpuMesh {
                places: self.device_buffer(&shared_places(shared), BufferUsage::VERTEX_BUFFER)?,
                uvs: self.device_buffer(&shared_looks(shared), BufferUsage::VERTEX_BUFFER)?,
                indices: self.device_buffer(&index_bytes, BufferUsage::INDEX_BUFFER)?,
                levels: shared.levels.clone(),
                shadow_parts: shared
                    .levels
                    .first()
                    .into_iter()
                    .flatten()
                    .filter(|part| casts_shadow(map, part.texture))
                    .copied()
                    .collect(),
            });
        }
        let moving = self.upload_moving(map)?;
        let terrain = self.upload_terrain(map, mesh.chunk_lights.clone())?;
        let fields = self.upload_fields(map)?;
        let sims = self.upload_sims(map, &fields)?;
        let water = self.upload_water(map)?;
        self.upload_water_pictures(map)?;
        let held = self.water_pictures.take();
        let ripples = match &held {
            Some(image) => self.upload_ripples(map, &image.view)?,
            None => None,
        };
        self.water_pictures = held;
        let particle_pictures = self.upload_particle_pictures(map)?;
        let particle_shaders = self.upload_particle_shaders(&map.effect_shaders)?;
        let shadow_instances = map
            .mesh_instances
            .iter()
            .filter_map(|instance| {
                let shared = map.shared_meshes.get(instance.mesh as usize)?;
                let [x, y, z] = instance.rows;
                Some(ShadowInstance {
                    mesh: instance.mesh,
                    placed: [
                        x[0], y[0], z[0], 0.0, x[1], y[1], z[1], 0.0, x[2], y[2], z[2], 0.0, x[3],
                        y[3], z[3], 1.0,
                    ],
                    center: instance.place(shared.center),
                    radius: shared.radius * instance.stretch(),
                })
            })
            .collect();
        let instance_buffer = || {
            let size = map.mesh_instances.len().max(1) * INSTANCE_BYTES;
            host_buffer(
                &self.device,
                &self.physical,
                size,
                BufferUsage::VERTEX_BUFFER,
            )
        };
        let instances = [instance_buffer()?, instance_buffer()?];

        let sets = u32::try_from(map.textures.len() + 2).unwrap_or(u32::MAX);
        let material_count = map.textures.len();
        let images = sets.saturating_mul(12);
        let pool = DescriptorPool::new(
            &self.device,
            sets,
            &[
                // The frame set takes a few of each on top of one a set.
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::UNIFORM_BUFFER,
                    descriptor_count: sets.saturating_add(2),
                },
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLED_IMAGE,
                    descriptor_count: images,
                },
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLER,
                    descriptor_count: sets.saturating_add(2),
                },
            ],
        )?;
        let atlas = self.upload_image(
            &[(mesh.atlas_size, mesh.atlas_size, mesh.atlas.as_slice())],
            Format::R8G8B8A8_UNORM,
        )?;
        let direction_atlas = self.upload_image(
            &[(
                mesh.atlas_size,
                mesh.atlas_size,
                mesh.direction_atlas.as_slice(),
            )],
            Format::R8G8B8A8_UNORM,
        )?;
        let paint_atlas = self.upload_image(
            &[(
                mesh.atlas_size,
                mesh.atlas_size,
                mesh.paint_atlas.as_slice(),
            )],
            Format::R8G8B8A8_UNORM,
        )?;
        // Every opaque face of the world, then every moving model, for shadows. What has holes
        // goes a picture at a time, as light goes through them.
        let shadow_cutoffs = shadow_cutoffs(map);
        let opaque_batch = |batch: &Option<u32>| {
            batch.is_some_and(|batch| {
                let batch = &mesh.batches[batch as usize];
                let liquid = map.textures.get(batch.texture as usize);
                let liquid = liquid.is_some_and(|texture| texture.kind == TextureKind::Liquid);
                !liquid
                    && !matches!(
                        batch.kind,
                        BatchKind::Sky | BatchKind::Blended | BatchKind::Masked
                    )
            })
        };
        let mut cut_textures: Vec<u32> = mesh
            .batches
            .iter()
            .filter(|batch| batch.kind == BatchKind::Masked)
            .filter(|batch| casts_shadow(map, batch.texture))
            .map(|batch| batch.texture)
            .collect();
        cut_textures.sort_unstable();
        cut_textures.dedup();
        let mut shadow_world = Vec::new();
        let mut shadow_parts = Vec::new();
        let mut add_groups = |groups: Vec<ShadowGroup>, cut: Option<u32>| {
            for group in groups {
                shadow_parts.push(ShadowRange {
                    first: u32::try_from(shadow_world.len()).unwrap_or(0),
                    count: u32::try_from(group.indices.len()).unwrap_or(0),
                    offset: Vec3::ZERO,
                    bounds: group.bounds,
                    cut,
                });
                shadow_world.extend(group.indices);
            }
        };
        add_groups(shadow_groups(map, mesh, opaque_batch), None);
        for texture in cut_textures {
            let of_it = |batch: &Option<u32>| {
                batch.is_some_and(|batch| {
                    let batch = &mesh.batches[batch as usize];
                    batch.kind == BatchKind::Masked && batch.texture == texture
                })
            };
            add_groups(shadow_groups(map, mesh, of_it), Some(texture));
        }
        let shadow_world_count = u32::try_from(shadow_world.len()).unwrap_or(0);
        let mut shadow_indices = host_buffer(
            &self.device,
            &self.physical,
            ((shadow_world.len() + mesh.model_indices.len()).max(1)) * 4,
            BufferUsage::INDEX_BUFFER,
        )?;
        {
            let mut mapped = shadow_indices.memory.map()?;
            let all = shadow_world.iter().chain(&mesh.model_indices);
            for (index, chunk) in all.zip(mapped.as_slice_mut().as_chunks_mut::<4>().0) {
                *chunk = index.to_le_bytes();
            }
        }
        self.shadows.forget();
        let frame_set = pool.allocate(&self.frame_layout)?;
        frame_set.write_buffer(
            0,
            DescriptorType::UNIFORM_BUFFER,
            &self.uniforms.buffer,
            0,
            FRAME_SIZE as u64,
        );
        frame_set.write_sampled_image(1, &atlas.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        frame_set.write_sampler(2, &self.light_sampler);
        frame_set.write_buffer(
            4,
            DescriptorType::UNIFORM_BUFFER,
            &self.live_uniforms.buffer,
            0,
            LIVE_BYTES as u64,
        );
        frame_set.write_sampled_image(
            5,
            &self.shadows.atlas.view,
            ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        );
        frame_set.write_sampler(6, &self.shadow_sampler);
        frame_set.write_sampled_image(
            8,
            &self.shadows.sun_atlas.view,
            ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        );
        frame_set.write_sampled_image(7, &paint_atlas.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        // How open pixels are comes with the scene's target, and till there is one, they're open.
        if self.sky_blank.is_none() {
            let white = [255_u8; 4];
            self.sky_blank = Some(self.upload_image(&[(1, 1, &white)], Format::R8G8B8A8_UNORM)?);
        }
        let corners = self
            .scene_target
            .as_ref()
            .map(|target| &target.corners.image);
        if let Some(corners) = corners.or(self.sky_blank.as_ref()) {
            frame_set.write_sampled_image(9, &corners.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        }
        frame_set.write_sampled_image(
            3,
            &direction_atlas.view,
            ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        );
        let live_bounce = self.upload_live_bounce(map)?;
        if self.bounce_blank.is_none() {
            self.bounce_blank =
                Some(self.upload_image(&[(1, 1, &[0; 8][..])], Format::R16G16B16A16_SFLOAT)?);
        }
        let bounce_image = live_bounce
            .as_ref()
            .map(|bounce| &bounce.image)
            .or(self.bounce_blank.as_ref());
        if let Some(image) = bounce_image {
            frame_set.write_sampled_image(10, &image.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        }

        let blank =
            self.upload_image(&[(1, 1, &[128, 128, 255, 255][..])], Format::R8G8B8A8_UNORM)?;
        let mut material_params = (material_count > 0)
            .then(|| {
                host_buffer(
                    &self.device,
                    &self.physical,
                    material_count * MATERIAL_PARAMS_STRIDE,
                    BufferUsage::UNIFORM_BUFFER,
                )
            })
            .transpose()?;
        let mut next_params = 0;
        let mut textures = Vec::with_capacity(map.textures.len());
        let mut material_sets = Vec::with_capacity(map.textures.len());
        let mut texture_sets = Vec::with_capacity(map.textures.len());
        let mut texture_shaders = Vec::with_capacity(map.textures.len());
        let plain_material = Material::default();
        // A picture with holes and no material is solid where it's more than half.
        let plain_cutout = Material {
            cutoff: 0.5,
            ..Material::default()
        };
        for texture in &map.textures {
            if texture.kind == TextureKind::Hidden {
                material_sets.push(false);
                texture_sets.push(None);
                texture_shaders.push(0);
                continue;
            }
            let levels: Vec<(u32, u32, &[u8])> = texture
                .mips
                .iter()
                .map(|mip| (mip.width, mip.height, mip.rgba.as_slice()))
                .collect();
            let format = compiled_format(&texture.mips).unwrap_or(Format::R8G8B8A8_SRGB);
            let image = self.upload_image(&levels, format)?;
            let own = texture.material.as_ref().filter(|_| uses_material(texture));
            let plain = if texture.kind == TextureKind::Cutout {
                &plain_cutout
            } else {
                &plain_material
            };
            let material = own.map_or(plain, |material| material.as_ref());
            texture_shaders.push(self.shader_of(texture, material)?);
            let set = pool.allocate(&self.material_layout)?;
            set.write_sampled_image(0, &image.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            let sampler = if own.is_some() {
                &self.material_sampler
            } else {
                &self.texture_sampler
            };
            set.write_sampler(1, sampler);
            let mut uploaded = Vec::with_capacity(material.slots.len());
            for (number, slot) in (1_u8..).zip(&material.slots) {
                let levels: Vec<(u32, u32, &[u8])> = slot
                    .iter()
                    .map(|mip| (mip.width, mip.height, mip.rgba.as_slice()))
                    .collect();
                uploaded.push(self.upload_image(&levels, slot_format(material, number))?);
            }
            // The first seven follow the sampler, and the rest the uniforms.
            for slot in 0..11_u32 {
                let view = uploaded
                    .get(slot as usize)
                    .map_or(&blank.view, |image| &image.view);
                let binding = if slot < 7 { slot + 2 } else { slot + 3 };
                let number = u8::try_from(slot + 1).unwrap_or(0);
                let name = material.field_images.iter().find(|(at, _)| *at == number);
                let field = name.and_then(|(_, name)| fields.iter().find(|f| f.name == *name));
                if let Some(field) = field {
                    set.write_sampled_image(binding, &field.shown.view, ImageLayout::GENERAL);
                } else {
                    set.write_sampled_image(binding, view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                }
            }
            if let Some(params) = &mut material_params {
                let offset = next_params * MATERIAL_PARAMS_STRIDE;
                let bytes = material_params_of(material);
                params.memory.map()?.as_slice_mut()[offset..offset + bytes.len()]
                    .copy_from_slice(&bytes);
                set.write_buffer(
                    9,
                    DescriptorType::UNIFORM_BUFFER,
                    &params.buffer,
                    offset as u64,
                    MATERIAL_PARAMS_BYTES as u64,
                );
                next_params += 1;
            }
            material_sets.push(own.is_some());
            texture_sets.push(Some(set));
            textures.push(image);
            textures.extend(uploaded);
        }
        self.upload_sky_pictures(map)?;
        let sky_set = match &map.sky {
            Some(_) => {
                let set = pool.allocate(&self.material_layout)?;
                // The set's first picture, then its first three slots, which are the same for
                // every sky, so that one with fewer has something there.
                for (index, binding) in [0, 2, 3, 4].into_iter().enumerate() {
                    let image = self.cloud_pictures.get(index);
                    let view = image.map_or(&blank.view, |image| &image.view);
                    set.write_sampled_image(binding, view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                }
                set.write_sampler(1, &self.material_sampler);
                Some(set)
            }
            None => None,
        };
        self.load_stencils(map)?;
        self.use_post_shader(map.post_shader.as_deref())?;
        self.map = Some(GpuMap {
            shared_meshes,
            instances,
            instance_batches: std::array::from_fn(|_| Vec::new()),
            instance_centers: std::array::from_fn(|_| Vec::new()),
            sims,
            particle_pictures,
            particle_shaders,
            particle_codes: map.effect_shaders.clone(),
            water,
            ripples,
            fields,
            terrain,
            instance_levels: vec![0; map.mesh_instances.len()],
            moving,
            shadow_instances,
            sky_set,
            texture_shaders,
            material_sets,
            vertices,
            indices,
            indices_stale: [true; FRAMES],
            shadow_indices,
            shadow_world_count,
            shadow_cutoffs,
            live_bounce,
            shadow_parts,
            texture_sets,
            _material_params: material_params,
            _textures: {
                let mut all = textures;
                all.push(blank);
                all
            },
            _pool: pool,
            frame_set,
            atlas,
            direction_atlas,
            paint_atlas,
        });
        // The last level's are gone, and what the final pass reads has to be told.
        if let Some(target) = &self.scene_target {
            self.write_ripples(&target.set);
        }
        Ok(())
    }

    /// The place of `texture`'s shader, with `material` being what draws it.
    fn shader_of(&mut self, texture: &Texture, material: &Material) -> Result<usize, RenderError> {
        if let Some(shader) = &material.shader {
            return self.surface_for(&shader.name, &shader.code);
        }
        let Some(name) = built_in_for(texture) else {
            return Ok(0);
        };
        let code = self.built_in_shaders.get(name).cloned();
        let code = code.unwrap_or_default();
        self.surface_for(name, &code)
    }

    /// A buffer only the GPU touches.
    pub(super) fn scratch_buffer(
        &self,
        size: usize,
        usage: BufferUsage,
    ) -> Result<GpuBuffer, RenderError> {
        let (buffer, memory) = Buffer::new_bound(
            &self.device,
            &self.physical,
            BufferCreateInfo {
                size: size.max(4) as u64,
                usage,
            },
            MemoryPropertyFlags::DEVICE_LOCAL,
        )?;
        Ok(GpuBuffer { buffer, memory })
    }

    /// A device-local buffer holding `bytes`.
    pub(super) fn device_buffer(
        &self,
        bytes: &[u8],
        usage: BufferUsage,
    ) -> Result<GpuBuffer, RenderError> {
        let mut staging = host_buffer(
            &self.device,
            &self.physical,
            bytes.len(),
            BufferUsage::TRANSFER_SRC,
        )?;
        staging.memory.map()?.as_slice_mut()[..bytes.len()].copy_from_slice(bytes);
        let (buffer, memory) = Buffer::new_bound(
            &self.device,
            &self.physical,
            BufferCreateInfo {
                size: bytes.len().max(4) as u64,
                usage: usage | BufferUsage::TRANSFER_DST,
            },
            MemoryPropertyFlags::DEVICE_LOCAL,
        )?;
        self.queue
            .one_shot(&self.device, self.queue_family, |rec| {
                rec.copy_buffer(
                    &staging.buffer,
                    &buffer,
                    &[BufferCopy {
                        src_offset: 0,
                        dst_offset: 0,
                        size: bytes.len().max(4) as u64,
                    }],
                );
                Ok(())
            })?;
        Ok(GpuBuffer { buffer, memory })
    }

    /// An image holding `levels`, largest first, ready to sample.
    /// Points vertices at where their light is now.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the device fails.
    pub(crate) fn write_vertices(&self, patches: &[VertexPatch]) -> Result<(), RenderError> {
        self.write_tile(None, patches)
    }

    /// Copies a tile's light into the atlas at `corner`, and points vertices at
    /// it.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the device fails.
    pub(crate) fn write_tile_light(
        &self,
        image: &TileImage,
        corner: [u32; 2],
        patches: &[VertexPatch],
    ) -> Result<(), RenderError> {
        self.write_tile(Some((image, corner)), patches)
    }

    /// Queues what's given for the next frame to write, first thing, so nothing
    /// has to stop and wait for the GPU while the view moves through a level.
    /// Barriers wait for any frame still using what's written.
    fn write_tile(
        &self,
        image: Option<(&TileImage, [u32; 2])>,
        patches: &[VertexPatch],
    ) -> Result<(), RenderError> {
        if self.map.is_none() {
            return Ok(());
        }
        // Without paint, a tile has nothing to say in that atlas.
        let parts = image.iter().flat_map(|(image, _)| {
            let paint = image.paint.iter().any(|byte| *byte != 0);
            let paint = paint.then_some(image.paint.as_slice());
            [
                Some(image.bytes.as_slice()),
                Some(image.directions.as_slice()),
                paint,
            ]
            .into_iter()
            .flatten()
        });
        let mut pieces: Vec<&[u8]> = Vec::with_capacity(patches.len() + 3);
        let mut offsets = Vec::new();
        let mut total = 0;
        for part in parts {
            offsets.push(total as u64);
            total += part.len();
            pieces.push(part);
        }
        let mut copies = Vec::with_capacity(patches.len());
        for (first, patch) in patches {
            copies.push(BufferCopy {
                src_offset: total as u64,
                dst_offset: u64::from(*first) * Vertex::SIZE as u64,
                size: patch.len() as u64,
            });
            total += patch.len();
            pieces.push(patch);
        }
        if total == 0 {
            return Ok(());
        }
        let mut staging = host_buffer(
            &self.device,
            &self.physical,
            total,
            BufferUsage::TRANSFER_SRC,
        )?;
        {
            let mut mapped = staging.memory.map()?;
            let mut rest = mapped.as_slice_mut();
            for piece in pieces {
                let (here, after) = rest.split_at_mut(piece.len());
                here.copy_from_slice(piece);
                rest = after;
            }
        }
        self.tile_writes.borrow_mut().push(TileWrite {
            staging,
            picture: image.map(|(image, corner)| (image.size, corner)),
            offsets,
            copies,
        });
        Ok(())
    }

    /// Records the tile writes that are waiting, and holds on to what they read
    /// until this frame's done.
    pub(super) fn record_tile_writes(&self, rec: &mut vulkane::safe::CommandBufferRecording<'_>) {
        let Some(map) = &self.map else {
            return;
        };
        let atlases = [&map.atlas, &map.direction_atlas, &map.paint_atlas];
        let mut writes = self.tile_writes.borrow_mut();
        for write in writes.iter() {
            if let Some((size, corner)) = write.picture {
                for (atlas, offset) in atlases.iter().zip(&write.offsets) {
                    copy_into_atlas(rec, &write.staging, atlas, *offset, size, corner);
                }
            }
            if !write.copies.is_empty() {
                rec.memory_barrier(
                    PipelineStage::ALL_GRAPHICS,
                    PipelineStage::TRANSFER,
                    AccessFlags::VERTEX_ATTRIBUTE_READ,
                    AccessFlags::TRANSFER_WRITE,
                );
                rec.copy_buffer(&write.staging.buffer, &map.vertices.buffer, &write.copies);
                rec.memory_barrier(
                    PipelineStage::TRANSFER,
                    PipelineStage::ALL_GRAPHICS,
                    AccessFlags::TRANSFER_WRITE,
                    AccessFlags::VERTEX_ATTRIBUTE_READ,
                );
            }
        }
        self.written_tiles[self.slot]
            .borrow_mut()
            .extend(writes.drain(..));
    }

    pub(super) fn upload_image(
        &self,
        levels: &[(u32, u32, &[u8])],
        format: Format,
    ) -> Result<GpuImage, RenderError> {
        let &(width, height, _) = levels
            .first()
            .ok_or_else(|| RenderError("an image with no pixels".to_owned()))?;
        let image = Image::new_2d_array(
            &self.device,
            Image2dCreateInfo {
                format,
                width,
                height,
                usage: ImageUsage::SAMPLED | ImageUsage::TRANSFER_DST,
            },
            1,
            u32::try_from(levels.len()).unwrap_or(1),
        )?;
        let image = device_image(&self.device, &self.physical, image, false)?;
        let mut pixels = Vec::new();
        let mut regions = Vec::with_capacity(levels.len());
        for (level, &(width, height, bytes)) in levels.iter().enumerate() {
            let mut copy = BufferImageCopy::full_2d(width, height);
            copy.buffer_offset = pixels.len() as u64;
            copy.mip_level = u32::try_from(level).unwrap_or(0);
            regions.push(copy);
            pixels.extend_from_slice(bytes);
        }
        let mut staging = host_buffer(
            &self.device,
            &self.physical,
            pixels.len(),
            BufferUsage::TRANSFER_SRC,
        )?;
        staging.memory.map()?.as_slice_mut()[..pixels.len()].copy_from_slice(&pixels);
        self.queue
            .one_shot(&self.device, self.queue_family, |rec| {
                rec.image_barrier(
                    PipelineStage::TOP_OF_PIPE,
                    PipelineStage::TRANSFER,
                    ImageBarrier::color(
                        &image.image,
                        ImageLayout::UNDEFINED,
                        ImageLayout::TRANSFER_DST_OPTIMAL,
                        AccessFlags::NONE,
                        AccessFlags::TRANSFER_WRITE,
                    ),
                );
                rec.copy_buffer_to_image(
                    &staging.buffer,
                    &image.image,
                    ImageLayout::TRANSFER_DST_OPTIMAL,
                    &regions,
                );
                rec.image_barrier(
                    PipelineStage::TRANSFER,
                    PipelineStage::FRAGMENT_SHADER,
                    ImageBarrier::color(
                        &image.image,
                        ImageLayout::TRANSFER_DST_OPTIMAL,
                        ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        AccessFlags::TRANSFER_WRITE,
                        AccessFlags::SHADER_READ,
                    ),
                );
                Ok(())
            })?;
        Ok(image)
    }

    /// Writes the uniforms, and the indices if they changed.
    pub(super) fn upload_frame(
        &mut self,
        view: &View,
        scene: Option<&Scene<'_>>,
        aspect: f32,
    ) -> Result<(), RenderError> {
        self.plan_fields(view.time);
        let lights = scene.map_or(&[][..], |scene| scene.drawables.lights);
        let sky = scene.and_then(|scene| scene.drawables.sky_now.or(scene.map.sky.as_ref()));
        // A day changes fog's colour, and nothing else of it.
        let day_fog = scene.and_then(|scene| {
            let color = scene.drawables.fog_color?;
            scene.map.fog.map(|fog| Fog { color, ..fog })
        });
        let fog = day_fog
            .as_ref()
            .or_else(|| scene.and_then(|scene| scene.map.fog.as_ref()));
        let extra = FrameExtras {
            stencils: scene.map_or(&[][..], |scene| scene.map.stencils.as_slice()),
            lights: scene.map_or(&[][..], |scene| scene.drawables.stencil_lights),
            masks: scene.map_or(&[][..], |scene| scene.drawables.stencil_masks),
            reaches: scene.map_or(&[][..], |scene| scene.drawables.stencil_reaches),
            sky,
            sky_floor: scene.and_then(|scene| scene.drawables.sky_ray_floor),
            sun_toward: scene.and_then(|scene| scene.drawables.sun_toward),
            water_tint: scene
                .and_then(|scene| scene.drawables.water_tint)
                .unwrap_or([1.0; 3]),
            day_light: scene
                .and_then(|scene| scene.drawables.day_light)
                .unwrap_or([0.0, 0.0, 0.0, 1.0]),
            sun_tint: scene
                .and_then(|scene| scene.drawables.sun_tint)
                .unwrap_or([1.0; 3]),
            capsules: scene.map_or(&[][..], |scene| scene.drawables.capsules),
            pools: scene.map_or(&[][..], |scene| scene.map.pools.as_slice()),
            sheets: scene.map_or(&[][..], |scene| scene.map.water_sheets.as_slice()),
            ripples: self
                .map
                .as_ref()
                .and_then(|map| map.ripples.as_ref())
                .map_or([0.0; 3], GpuRipples::place),
            fog_volumes: scene.map_or(&[][..], |scene| scene.map.fog_volumes.as_slice()),
            fog_lights: scene.map_or(&[][..], |scene| scene.drawables.live_lights),
            eye: view.origin,
            under_sky: scene.is_some_and(|scene| scene.map.open_above(view.origin)),
            samples: self.samples,
            probes: scene.and_then(|scene| scene.map.probes.as_ref()),
            look: scene.map_or_else(Look::default, |scene| scene.map.look),
            exposure_scale: 1.0,
            bloom: 0.0,
            light_scale: scene.map_or(1.0, |scene| scene.map.light_scale()),
            screen: self.screen_values(self.encoding),
            jitter: [0.0; 2],
            taa: [0.0; 4],
            last_view_projection: [0.0; 16],
            steady_view_projection: [0.0; 16],
            light_shift: view.light_shift,
        };
        // Exposure goes by itself as far as a zone, or else the level, lets it, from how bright
        // what was last measured was.
        let range = scene
            .and_then(|scene| scene.drawables.exposure_range)
            .unwrap_or(extra.look.exposure_range);
        self.metering = self.post.auto_exposure && scene.is_some() && range != [0.0; 2];
        if self.metering {
            let speed = extra.look.exposure_speed;
            self.exposure.step(self.brightness, range, speed, view.time);
        } else {
            self.exposure = Exposure::default();
            self.brightness = None;
        }
        // What glows is what's past the threshold once exposed, and each image of the chain adds
        // its share.
        let exposure = extra.look.exposure * self.exposure.scale();
        self.bloom_shown = self.post.bloom && extra.look.bloom > 0.0 && scene.is_some();
        self.bloom_step = BloomStep {
            threshold: extra.look.bloom_threshold,
            knee: extra.look.bloom_threshold * 0.5,
            exposure,
        };
        let levels = self
            .scene_target
            .as_ref()
            .map_or(1, |target| target.bloom.count());
        #[expect(clippy::cast_precision_loss, reason = "a few levels")]
        let bloom = if self.bloom_shown {
            extra.look.bloom / levels as f32
        } else {
            0.0
        };
        // TAA moves the view a little each frame, round a pattern that covers a pixel evenly,
        // and finds where each pixel was last frame from where the view was.
        self.taa_on = self.post.edge_smoothing == TAA_SETTING && scene.is_some();
        let steady = view_projection(
            view.origin,
            [view.yaw, view.pitch, view.roll],
            view.fov,
            aspect,
        );
        // Drawn smaller than the screen, more moves of the view go round, so that more places in
        // each of its pixels are seen.
        let scale = render_scale(self.post.render_scale);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a short round"
        )]
        let frames = (TAA_FRAMES as f32 / scale.min(1.0).powi(2)).ceil() as u32;
        let (jitter, taa) = if self.taa_on {
            let round = self.taa_frame % frames + 1;
            let (width, height) = self.scene_size();
            #[expect(
                clippy::cast_precision_loss,
                reason = "window sizes are far below 2^24"
            )]
            let jitter = [
                (halton(round, 2) - 0.5) * 2.0 / width.max(1) as f32,
                (halton(round, 3) - 0.5) * 2.0 / height.max(1) as f32,
            ];
            let kept = self.taa_kept && self.last_view_projection.is_some();
            #[expect(clippy::cast_precision_loss, reason = "a short round")]
            let which = round as f32;
            (jitter, [1.0, f32::from(u8::from(kept)), which, scale])
        } else {
            ([0.0; 2], [0.0, 0.0, 0.0, scale])
        };
        let last_view_projection = self.last_view_projection.unwrap_or(steady);
        if self.taa_on {
            self.taa_frame = self.taa_frame.wrapping_add(1);
            self.last_view_projection = Some(steady);
            self.taa_kept = true;
        } else {
            self.last_view_projection = None;
            self.taa_kept = false;
        }
        let extra = FrameExtras {
            exposure_scale: self.exposure.scale(),
            bloom,
            jitter,
            taa,
            last_view_projection,
            steady_view_projection: steady,
            ..extra
        };
        self.note_moved_brushes(scene);
        self.plan_live_bounce(scene)?;
        self.sky_rays_shown = self.post.rays != 0 && sky.is_some_and(|sky| sky.rays.is_some());
        self.ssao_shown = self.post.ssao != 0 && extra.look.ssao > 0.0;
        self.fog_marched = extra.fog_volumes.iter().any(FogVolume::marched);
        let pushes = scene.map_or(&[][..], |scene| scene.drawables.water_pushes);
        self.plan_ripples(view.time, view.origin, pushes, self.slot)?;
        self.plan_water(extra.pools, extra.sheets, view.origin);
        let bytes = frame_bytes(view, aspect, lights, sky, fog, &self.post, &extra);
        let slot = self.slot;
        let values = &mut self.frame_values[slot];
        values.frame.memory.map()?.as_slice_mut()[..bytes.len()].copy_from_slice(&bytes);
        let live = scene.map_or(&[][..], |scene| scene.drawables.live_lights);
        let slots = match scene.filter(|_| self.post.live_shadows) {
            Some(scene) => {
                let figures = shadow_figures(&self.models, scene.drawables.entities, &[]);
                let instances = scene.drawables.instances;
                let moving: Vec<(Vec3, f32)> = scene
                    .drawables
                    .moving
                    .iter()
                    .map(|draw| (draw.center, draw.radius))
                    .collect();
                self.shadows
                    .plan(live, instances, (&figures, &moving), scene.map, view.origin)
            }
            None => {
                self.shadows.rest();
                vec![None; live.len()]
            }
        };
        let size = self.scene_size();
        let capsules = scene.map_or(&[][..], |scene| scene.drawables.capsules);
        let sun = scene.and_then(|scene| scene.drawables.sun);
        let arc = scene.and_then(|scene| scene.drawables.sun_arc);
        // A sun on an arc has its shadows baked, so there are no maps of them to draw.
        let drawn = sun.filter(|_| arc.is_none());
        self.shadows
            .plan_sun(drawn.map(|sun| sun.toward), view.origin);
        let shadow = match arc {
            Some(_) => Some(SunShadow {
                matrices: [[0.0; 16]; SUN_CASCADES],
            }),
            None => self.shadows.sun_shadow(),
        };
        let sun = sun.zip(shadow).map(|(sun, shadow)| (sun, shadow, arc));
        let bytes = live_bytes(view, aspect, size, live, capsules, &slots, sun);
        let values = &mut self.frame_values[slot];
        values.live.memory.map()?.as_slice_mut()[..bytes.len()].copy_from_slice(&bytes);
        if let Some(scene) = scene
            && !scene.drawables.particles.is_empty()
        {
            let mut mapped = self.particles[slot].memory.map()?;
            let (target, _remainder) = mapped.as_slice_mut().as_chunks_mut::<PARTICLE_BYTES>();
            let listed = &scene.drawables.particles;
            let listed = &listed[..listed.len().min(MAX_PARTICLE_DRAWS)];
            let (ordered, behind) =
                behind_first(listed, view.origin, &scene.mesh.see_through_boxes);
            for (particle, chunk) in ordered.iter().zip(target) {
                write_particle(particle, chunk);
            }
            self.particle_order = ordered.iter().map(|particle| particle.shader).collect();
            self.particles_behind = behind;
        } else {
            self.particle_order.clear();
            self.particles_behind = 0;
        }
        if let Some(scene) = scene {
            let angles = [view.yaw, view.pitch, view.roll];
            let seen = view_projection(view.origin, angles, view.fov, aspect);
            let back = inverse(&seen).unwrap_or(seen);
            self.plan_sims(scene.drawables.particle_sims, (&seen, &back), slot)?;
        }
        if let (Some(scene), Some(map)) = (scene, &mut self.map) {
            let angles = [view.yaw, view.pitch, view.roll];
            let seen = view_projection(view.origin, angles, view.fov, aspect);
            let planes = view_planes(&seen);
            let drawables = &scene.drawables;
            self.moving_overflow |= map.moving.prepare(
                slot,
                drawables.moving,
                (drawables.palette, drawables.morph_weights),
                &planes,
            )?;
            self.moving_overflow |=
                map.moving
                    .prepare_figures(slot, drawables.entities, &self.models, &planes)?;
            if let Some(land) = &mut map.terrain {
                let mut mapped = land.nodes[slot].memory.map()?;
                land.batches[slot] = terrain_nodes(
                    scene.map,
                    (&scene.visible.chunks, &land.lights, &land.ranges),
                    (view.origin, &planes),
                    mapped.as_slice_mut(),
                );
            }
            if !scene.map.mesh_instances.is_empty() {
                let levels = LevelView {
                    eye: view.origin,
                    height_scale: Vec3::new(seen[1], seen[5], seen[9]).length(),
                };
                let mut mapped = map.instances[slot].memory.map()?;
                map.instance_batches[slot] = instance_batches(
                    scene.map,
                    &scene.mesh.instance_lights,
                    &scene.visible.instances,
                    &planes,
                    (&levels, &mut map.instance_levels),
                    (mapped.as_slice_mut(), &mut map.instance_centers[slot]),
                );
            }
            if scene.changed {
                map.indices_stale = [true; FRAMES];
            }
            if map.indices_stale[slot] {
                let mut mapped = map.indices[slot].memory.map()?;
                let (target, _remainder) = mapped.as_slice_mut().as_chunks_mut::<4>();
                for (index, chunk) in scene.visible.indices.iter().zip(target) {
                    *chunk = index.to_le_bytes();
                }
                map.indices_stale[slot] = false;
            }
        }
        Ok(())
    }
}
