//! What the material shader needs, as bytes.

use engine_map::{AlphaMode, Material, Shading};

/// The bytes of one material's parameters, matching `MaterialParams`.
pub const MATERIAL_PARAMS_BYTES: usize = 224;

/// How far apart parameters sit in their buffer, the widest alignment Vulkan
/// allows for a uniform buffer offset.
pub const MATERIAL_PARAMS_STRIDE: usize = 256;

/// A slot as the shader reads it, with -1 for none.
fn slot(slot: Option<u8>) -> i32 {
    slot.map_or(-1, i32::from)
}

/// `material`'s parameters.
#[must_use]
pub fn material_params(material: &Material) -> [u8; MATERIAL_PARAMS_BYTES] {
    let mut out = [0; MATERIAL_PARAMS_BYTES];
    let shading = match material.shading {
        Shading::Lit => 0,
        Shading::Cel => 1,
        Shading::Matcap => 2,
        Shading::Unlit => 3,
    };
    let alpha = match material.alpha {
        AlphaMode::Opaque => 0,
        AlphaMode::Mask => 1,
        AlphaMode::Blend => 2,
    };
    let features = i32::from(material.triplanar)
        | (i32::from(material.hex_tiling) << 1)
        | (i32::from(material.detail_fade) << 2);
    let layer_count = i32::try_from(material.layers.len()).unwrap_or(0);
    let mut words = [0_i32; 24];
    words[..4].copy_from_slice(&[shading, alpha, features, layer_count]);
    words[4..8].copy_from_slice(&[
        slot(material.normal),
        slot(material.specular),
        slot(material.emissive),
        slot(material.height),
    ]);
    words[8..12].copy_from_slice(&[
        slot(material.tint),
        slot(material.matcap),
        slot(material.ramp),
        slot(material.detail),
    ]);
    for (index, layer) in material.layers.iter().take(3).enumerate() {
        words[12 + index * 4] = i32::from(layer.base);
        words[13 + index * 4] = i32::from(layer.mask);
    }
    for (chunk, word) in out.as_chunks_mut::<4>().0.iter_mut().zip(&words) {
        *chunk = word.to_le_bytes();
    }
    let floats = [
        material.scroll[0],
        material.scroll[1],
        material.flipbook[0],
        material.flipbook[1],
        material.specular_strength,
        material.gloss,
        material.parallax,
        material.flipbook[2],
        material.detail_scale,
        material.emissive_strength,
        material.cutoff,
        0.0,
        material.reflect,
        material.flow_strength,
        material.flow_speed,
        material.flow.map_or(-1.0, f32::from),
    ];
    let shader_floats = floats.iter().chain(&material.shader_params);
    for (chunk, float) in out[96..]
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(shader_floats)
    {
        *chunk = float.to_le_bytes();
    }
    let [first, second] = material.shader_images;
    let shader_slots = [slot(first), slot(second), -1, -1];
    for (chunk, word) in out[192..]
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(shader_slots)
    {
        *chunk = word.to_le_bytes();
    }
    let metal_rough = material.metal_rough.map_or([0.0; 4], |surface| {
        let slot = surface.map.map_or(-1.0, f32::from);
        [surface.metallic, surface.roughness, 1.0, slot]
    });
    for (chunk, float) in out[208..]
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(metal_rough)
    {
        *chunk = float.to_le_bytes();
    }
    out
}
