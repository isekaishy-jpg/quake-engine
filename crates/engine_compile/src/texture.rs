//! A stand-in for materials nothing supplies.

use engine_map::{MipLevel, Texture, TextureKind};

/// The placeholder's side, in texels.
const SIZE: u32 = 64;

/// How many mips it has.
const MIP_COUNT: u32 = 4;

/// The size of a check, in texels.
const CHECK: u32 = 16;

/// A colour from `name`, so each material is told apart.
fn color_of(name: &str) -> [u8; 3] {
    let mut hash: u32 = 2_166_136_261;
    for byte in name.bytes() {
        hash = (hash ^ u32::from(byte)).wrapping_mul(16_777_619);
    }
    let [a, b, c, _] = hash.to_le_bytes();
    [a / 2 + 96, b / 2 + 96, c / 2 + 96]
}

/// A checked texture in a colour taken from `name`, for a material that can't
/// be found. A map then still plays, and the missing piece is plain to see.
#[must_use]
pub fn placeholder_texture(name: &str) -> Texture {
    let [red, green, blue] = color_of(name);
    let mips = (0..MIP_COUNT)
        .map(|level| {
            let size = SIZE >> level;
            let check = (CHECK >> level).max(1);
            let mut rgba = Vec::with_capacity((size * size * 4) as usize);
            for y in 0..size {
                for x in 0..size {
                    let light = (x / check + y / check).is_multiple_of(2);
                    let shade = |part: u8| if light { part } else { part / 4 * 3 };
                    rgba.extend_from_slice(&[shade(red), shade(green), shade(blue), 255]);
                }
            }
            MipLevel {
                width: size,
                height: size,
                rgba,
                form: engine_map::PixelForm::Rgba,
            }
        })
        .collect();
    Texture {
        name: name.to_owned(),
        kind: TextureKind::Solid,
        mips,
        animation: None,
        alternate: None,
        material: None,
    }
}
