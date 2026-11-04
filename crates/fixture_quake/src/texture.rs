//! Quake's palette and texture naming rules.

use engine_map::{MipLevel, TextureKind};

/// The first palette entry that ignores light, from `d_init.c`.
const FIRST_FULLBRIGHT: u8 = 224;

/// The palette entry for a hole in a cutout texture.
const CUTOUT_INDEX: u8 = 255;

/// The palette entry where the back layer of a sky shows through.
const SKY_CLEAR_INDEX: u8 = 0;

/// The 256 colours of `gfx/palette.lmp`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    /// The colours as RGB.
    colors: Vec<[u8; 3]>,
}

impl Palette {
    /// Reads a palette, or `None` if `bytes` isn't 768 long.
    #[must_use]
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let (colors, rest) = bytes.as_chunks::<3>();
        (colors.len() == 256 && rest.is_empty()).then(|| Self {
            colors: colors.to_vec(),
        })
    }

    /// The colour of `index`.
    fn color(&self, index: u8) -> [u8; 3] {
        self.colors[usize::from(index)]
    }

    /// Expands palette indices to one RGBA mip level. What alpha means
    /// depends on `kind`, as [`TextureKind`] sets out.
    pub(crate) fn expand(
        &self,
        indices: &[u8],
        width: u32,
        height: u32,
        kind: TextureKind,
    ) -> MipLevel {
        let mut rgba = Vec::with_capacity(indices.len() * 4);
        for &index in indices {
            let [r, g, b] = self.color(index);
            let opaque = match kind {
                TextureKind::Solid => index < FIRST_FULLBRIGHT,
                TextureKind::Cutout => index != CUTOUT_INDEX,
                TextureKind::Sky => index != SKY_CLEAR_INDEX,
                TextureKind::Liquid | TextureKind::Hidden => true,
            };
            rgba.extend_from_slice(&[r, g, b, if opaque { 255 } else { 0 }]);
        }
        MipLevel {
            width,
            height,
            rgba,
        }
    }
}

/// How Quake's tools mark a texture's behaviour by name.
#[must_use]
pub fn texture_kind(name: &str) -> TextureKind {
    let lower = name.to_ascii_lowercase();
    if lower.starts_with("sky") {
        TextureKind::Sky
    } else if lower.starts_with('*') {
        TextureKind::Liquid
    } else if lower.starts_with('{') {
        TextureKind::Cutout
    } else if matches!(
        lower.as_str(),
        "clip" | "skip" | "trigger" | "hint" | "hintskip" | "null"
    ) {
        TextureKind::Hidden
    } else {
        TextureKind::Solid
    }
}
