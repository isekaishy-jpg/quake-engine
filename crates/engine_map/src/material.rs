//! What a surface is made of, beyond its base colour (proposal 0069).

use std::sync::Arc;

use crate::map::MipLevel;

/// The most images a material can use, base colour included. Layers and extra
/// maps share them, because each is at least one read per pixel.
pub const MATERIAL_SLOTS: usize = 12;

/// The most blended layers a material can have over its base.
pub const MAX_LAYERS: usize = 3;

/// How a surface answers light.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shading {
    /// Ordinary light.
    #[default]
    Lit,
    /// Light turned into hard bands.
    Cel,
    /// Lit, with a painted sphere of shine over it.
    Matcap,
    /// No light at all.
    Unlit,
}

/// How a surface uses alpha.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlphaMode {
    /// It's solid.
    #[default]
    Opaque,
    /// Pixels are there or they aren't.
    Mask,
    /// It's see-through.
    Blend,
}

/// A layer blended over the base.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaterialLayer {
    /// The slot of its colour.
    pub base: u8,
    /// The slot of the mask saying where it shows.
    pub mask: u8,
    /// The slot of its own bumps, in red and green, and shine, in alpha, if it has them. Without
    /// them, it keeps what's under it.
    pub surface: Option<u8>,
}

/// The mask of a layer that reads the map's paint, not an image.
pub const PAINT_MASK: u8 = 15;

/// A surface described by metal and roughness, as glTF does (proposal 0108).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetalRough {
    /// How metal it is, from 0 to 1. Its map is scaled by this.
    pub metallic: f32,
    /// How rough it is, from 0 to 1. Its map is scaled by this.
    pub roughness: f32,
    /// The slot of its map: occlusion in red, roughness in green, metal in blue.
    pub map: Option<u8>,
}

/// A shader, as a surface or a level names it (proposal 0107).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShaderCode {
    /// Its name.
    pub name: String,
    /// Its code, which is empty if it's missing.
    pub code: String,
}

/// A picture worked out a step at a time, by a compute shader of the level's own.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldDef {
    /// What it's called, which is how a material asks for it.
    pub name: String,
    /// Its shader.
    pub shader: Arc<ShaderCode>,
    /// How wide it is, each way.
    pub size: u32,
    /// How many steps a second it takes.
    pub rate: f32,
    /// What it's given, which its shader reads as it would a material's.
    pub params: [f32; SHADER_PARAMS],
}

/// The most numbers a material can give its shader.
pub const SHADER_PARAMS: usize = 8;

/// The most images a material can give its shader.
pub const SHADER_IMAGES: usize = 2;

/// One image: its mips, largest first.
pub type SlotImage = Arc<Vec<MipLevel>>;

/// Everything about a material except its base colour, which is the texture's
/// own image, in slot 0.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Material {
    /// Its metal and roughness, if it's described that way. It can have a
    /// specular map as well, which adds to them.
    pub metal_rough: Option<MetalRough>,
    /// Its shader, if it has one of its own.
    pub shader: Option<Arc<ShaderCode>>,
    /// The slots that show a field and not a picture, with the field's name.
    pub field_images: Vec<(u8, String)>,
    /// The numbers it gives its shader.
    pub shader_params: [f32; SHADER_PARAMS],
    /// The slots of the images it gives its shader.
    pub shader_images: [Option<u8>; SHADER_IMAGES],
    /// How it answers light.
    pub shading: Shading,
    /// How it uses alpha.
    pub alpha: AlphaMode,
    /// How many world units one repeat covers, across and down, if it's set.
    /// Without it that's one unit to a pixel.
    pub world_size: Option<[f32; 2]>,
    /// The images of slots 1 onwards.
    pub slots: Vec<SlotImage>,
    /// The slot of its normal map.
    pub normal: Option<u8>,
    /// The slot of its specular map: colour, with gloss in alpha.
    pub specular: Option<u8>,
    /// The slot of its glow.
    pub emissive: Option<u8>,
    /// The slot of its height map.
    pub height: Option<u8>,
    /// The slot of what light it lets through, as colour and alpha.
    pub tint: Option<u8>,
    /// The slot of its matcap.
    pub matcap: Option<u8>,
    /// The slot of its cel ramp.
    pub ramp: Option<u8>,
    /// The slot of its detail texture.
    pub detail: Option<u8>,
    /// Its blended layers.
    pub layers: Vec<MaterialLayer>,
    /// Whether it fades out with distance, as a detail.
    pub detail_fade: bool,
    /// Whether its surfaces go in the map's mega texture when baked.
    pub mega: bool,
    /// Whether to project textures from the world, ignoring face mapping.
    pub triplanar: bool,
    /// Whether to break up repetition with hexagon tiles.
    pub hex_tiling: bool,
    /// Whether it only ever lies on solid surfaces, as a decal does. What's see-through
    /// is drawn one face at a time, far to near, so that each shows through the next.
    /// This doesn't need that, so all of it can be drawn at once, first.
    pub decal: bool,
    /// Texture movement, in repeats a second.
    pub scroll: [f32; 2],
    /// Flipbook columns, rows and frames a second, or zeros for none.
    pub flipbook: [f32; 3],
    /// How strong specular light is.
    pub specular_strength: f32,
    /// How tight specular highlights are.
    pub gloss: f32,
    /// How deep parallax reaches, as a share of a repeat.
    pub parallax: f32,
    /// How many detail repeats fit in one texture repeat.
    pub detail_scale: f32,
    /// How bright its glow is.
    pub emissive_strength: f32,
    /// Below this alpha, a masked pixel goes.
    pub cutoff: f32,
    /// The slot of its flow map, with direction in red and green.
    pub flow: Option<u8>,
    /// How far a flow map moves its texture, as a share of a repeat.
    pub flow_strength: f32,
    /// How many flow cycles a second.
    pub flow_speed: f32,
    /// How much it mirrors what's around it, from 0 to 1.
    pub reflect: f32,
    /// Areas of its image that faces fit themselves to, as left, top, right
    /// and bottom from 0 to 1.
    pub hotspots: Vec<[f32; 4]>,
}

impl Material {
    /// How many images it uses, with the base.
    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.slots.len() + 1
    }
}

/// Which way a normal map's texel faces, as a unit vector: from its red and green, as a shader has
/// it, since its blue may not be kept.
#[must_use]
pub fn normal_of_texel(red: u8, green: u8) -> [f32; 3] {
    let x = f32::from(red) / 127.5 - 1.0;
    let y = f32::from(green) / 127.5 - 1.0;
    [x, y, (1.0 - x * x - y * y).max(0.0).sqrt()]
}

/// A normal map's texel for `average`, the average of the unit normals under it, and `alpha`: which
/// way they face on average in red and green, and how much they agree in blue. That's the
/// average's length, which is 1 where they all face one way and less as they spread.
#[must_use]
pub fn texel_of_normals(average: [f32; 3], alpha: u8) -> [u8; 4] {
    let length = average.iter().map(|part| part * part).sum::<f32>().sqrt();
    let [x, y, _] = if length > 1e-4 {
        average.map(|part| part / length)
    } else {
        [0.0, 0.0, 1.0]
    };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to a byte"
    )]
    let byte = |value: f32| (value * 255.0).round().clamp(0.0, 255.0) as u8;
    [
        byte(x * 0.5 + 0.5),
        byte(y * 0.5 + 0.5),
        byte(length.min(1.0)),
        alpha,
    ]
}

/// A normal map's chain of mips, largest first. Each level keeps which way its texels' bumps face
/// on average, and in blue how much they agree (see [`texel_of_normals`]), from all the texels of
/// the first level under it. A bump too small to see is gone from the direction, but not from how
/// rough a highlight on it is (proposal 0151). The first level keeps its red and green as they
/// are, with blue at the top, as each texel is one bump. An odd last row or column is left out, as
/// halving goes, as [`mip_chain`] leaves it.
#[must_use]
pub fn normal_chain(width: u32, height: u32, rgba: &[u8]) -> Vec<MipLevel> {
    let texels = rgba.as_chunks::<4>().0;
    let mut sums: Vec<([f32; 3], f32)> = texels
        .iter()
        .map(|[red, green, _, alpha]| (normal_of_texel(*red, *green), f32::from(*alpha)))
        .collect();
    let (mut across, mut down) = (width.max(1) as usize, height.max(1) as usize);
    let first = texels
        .iter()
        .flat_map(|[red, green, _, alpha]| [*red, *green, u8::MAX, *alpha])
        .collect();
    let mut levels = vec![MipLevel {
        width: u32::try_from(across).unwrap_or(1),
        height: u32::try_from(down).unwrap_or(1),
        rgba: first,
        form: crate::PixelForm::Rgba,
    }];
    while across > 1 || down > 1 {
        let (half_across, half_down) = ((across / 2).max(1), (down / 2).max(1));
        let mut next = Vec::with_capacity(half_across * half_down);
        for y in 0..half_down {
            for x in 0..half_across {
                let mut normal = [0.0; 3];
                let mut alpha = 0.0;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (from_x, from_y) =
                        ((x * 2 + dx).min(across - 1), (y * 2 + dy).min(down - 1));
                    let (under, under_alpha) = sums[from_y * across + from_x];
                    for (sum, part) in normal.iter_mut().zip(under) {
                        *sum += part * 0.25;
                    }
                    alpha += under_alpha * 0.25;
                }
                next.push((normal, alpha));
            }
        }
        sums = next;
        (across, down) = (half_across, half_down);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "an average of bytes"
        )]
        let rgba = sums
            .iter()
            .flat_map(|(normal, alpha)| texel_of_normals(*normal, alpha.round() as u8))
            .collect();
        levels.push(MipLevel {
            width: u32::try_from(across).unwrap_or(1),
            height: u32::try_from(down).unwrap_or(1),
            rgba,
            form: crate::PixelForm::Rgba,
        });
    }
    levels
}

/// A full chain of mips from `rgba`, box filtered, largest first.
#[must_use]
pub fn mip_chain(width: u32, height: u32, rgba: Vec<u8>) -> Vec<MipLevel> {
    let mut levels = vec![MipLevel {
        width,
        height,
        rgba,
        form: crate::PixelForm::Rgba,
    }];
    while let Some(last) = levels.last() {
        if last.width <= 1 && last.height <= 1 {
            break;
        }
        let (old_width, old_height) = (last.width as usize, last.height as usize);
        let (new_width, new_height) = ((old_width / 2).max(1), (old_height / 2).max(1));
        let mut pixels = Vec::with_capacity(new_width * new_height * 4);
        for y in 0..new_height {
            for x in 0..new_width {
                for channel in 0..4 {
                    let mut total = 0_u32;
                    for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                        let sy = (y * 2 + dy).min(old_height - 1);
                        let sx = (x * 2 + dx).min(old_width - 1);
                        total += u32::from(last.rgba[(sy * old_width + sx) * 4 + channel]);
                    }
                    pixels.push(u8::try_from(total / 4).unwrap_or(u8::MAX));
                }
            }
        }
        levels.push(MipLevel {
            width: u32::try_from(new_width).unwrap_or(1),
            height: u32::try_from(new_height).unwrap_or(1),
            rgba: pixels,
            form: crate::PixelForm::Rgba,
        });
    }
    levels
}
