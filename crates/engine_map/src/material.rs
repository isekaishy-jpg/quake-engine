//! What a surface is made of, beyond its base colour (proposal 0069).

use std::sync::Arc;

use crate::map::MipLevel;

/// The most images a material can use, base colour included. Layers and extra
/// maps share them, because each is at least one read per pixel.
pub const MATERIAL_SLOTS: usize = 8;

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
}

/// One image: its mips, largest first.
pub type SlotImage = Arc<Vec<MipLevel>>;

/// Everything about a material except its base colour, which is the texture's
/// own image, in slot 0.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Material {
    /// How it answers light.
    pub shading: Shading,
    /// How it uses alpha.
    pub alpha: AlphaMode,
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
    /// Whether to project textures from the world, ignoring face mapping.
    pub triplanar: bool,
    /// Whether to break up repetition with hexagon tiles.
    pub hex_tiling: bool,
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

/// A full chain of mips from `rgba`, box filtered, largest first.
#[must_use]
pub fn mip_chain(width: u32, height: u32, rgba: Vec<u8>) -> Vec<MipLevel> {
    let mut levels = vec![MipLevel {
        width,
        height,
        rgba,
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
        });
    }
    levels
}
