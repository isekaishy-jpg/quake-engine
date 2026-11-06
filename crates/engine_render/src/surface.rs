//! Which of the engine's own shaders a texture uses.

use engine_map::{Texture, TextureKind};

use crate::world::uses_material;

/// The name of the engine's own shader for `texture`, if it has no shader of
/// its own and no material to draw it.
pub(crate) fn built_in_for(texture: &Texture) -> Option<&'static str> {
    if uses_material(texture) {
        return None;
    }
    match texture.kind {
        TextureKind::Solid => Some("classic"),
        TextureKind::Cutout => Some("classic_cutout"),
        TextureKind::Liquid => Some("classic_liquid"),
        TextureKind::Sky => Some("classic_sky"),
        TextureKind::Hidden => None,
    }
}
