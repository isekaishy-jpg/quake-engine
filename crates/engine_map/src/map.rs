//! Geometry, textures, lighting and entities.

use std::sync::Arc;

use engine_core::math::Vec3;

use crate::emitter::Emitter;
use crate::live::LiveLightSource;
use crate::material::Material;
use crate::nav::{NavGrid, Spline};
use crate::occluder::ModelOccluder;
use crate::pool::Pool;
use crate::probes::{LightTrack, ProbeGrid};
use crate::sky::{Fog, Sky};
use crate::stencil::RayStencil;

use crate::tree::{ClipChild, NodeChild, Tree};

/// How a texture is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextureKind {
    /// An ordinary lit surface. Alpha is how much light affects a pixel, so
    /// zero is fullbright.
    #[default]
    Solid,
    /// A lit surface with holes, where alpha zero is a hole.
    Cutout,
    /// A moving liquid, not lit.
    Liquid,
    /// Two scrolling layers side by side: the left half is in front, with
    /// alpha zero where the right half shows through.
    Sky,
    /// Never drawn.
    Hidden,
}

/// One level of a texture's mip chain, in RGBA bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MipLevel {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Four bytes a pixel.
    pub rgba: Vec<u8>,
}

/// Frames a texture cycles through.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Animation {
    /// Texture indices, in order.
    pub frames: Vec<u32>,
    /// Where this texture sits in `frames`.
    pub position: u32,
}

/// A texture.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Texture {
    /// Its name, for display.
    pub name: String,
    /// How it's drawn.
    pub kind: TextureKind,
    /// Its mip chain, largest first, never empty.
    pub mips: Vec<MipLevel>,
    /// Its animation, if it has one.
    pub animation: Option<Animation>,
    /// The texture to show instead on a brush entity with a frame set, as a
    /// used button does.
    pub alternate: Option<u32>,
    /// What else it's made of. Without one, it draws by its kind alone.
    pub material: Option<Arc<Material>>,
}

/// One texture axis: a texel coordinate is `dot(point, axis) + offset`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TexelAxis {
    /// The direction and scale.
    pub axis: Vec3,
    /// The offset in texels.
    pub offset: f32,
}

impl TexelAxis {
    /// The texel coordinate of `point`.
    #[must_use]
    pub fn project(&self, point: Vec3) -> f32 {
        self.axis.dot(point) + self.offset
    }
}

/// A face's baked light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaceLight {
    /// The first sample in [`Map::light`]. Each style's samples follow on.
    pub first_sample: u32,
    /// Samples across.
    pub width: u32,
    /// Samples down.
    pub height: u32,
    /// The texel coordinates of the first sample.
    pub texel_origin: [i32; 2],
    /// Texels between samples.
    pub texels_per_sample: u32,
    /// The light style of each layer. Unused layers hold [`Face::NO_STYLE`].
    pub styles: [u8; 4],
}

impl FaceLight {
    /// How many styles this face has.
    #[must_use]
    pub fn style_count(&self) -> u32 {
        let count = self.styles.iter().take_while(|&&s| s != Face::NO_STYLE);
        u32::try_from(count.count()).unwrap_or(0)
    }
}

/// A convex polygon.
#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    /// The first point in [`Map::points`].
    pub first_point: u32,
    /// How many points it has.
    pub point_count: u32,
    /// Its texture in [`Map::textures`].
    pub texture: u32,
    /// The texture's horizontal and vertical axes.
    pub axes: [TexelAxis; 2],
    /// Its baked light, if any.
    pub light: Option<FaceLight>,
}

impl Face {
    /// The style number marking an unused lighting layer.
    pub const NO_STYLE: u8 = 255;
}

/// A map's mega texture, as it's kept in a bake.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MegaImage {
    /// A face and how far its coordinates moved, three numbers each.
    pub faces: Vec<i32>,
    /// Its pixels, in RGBA bytes.
    pub rgba: Vec<u8>,
}

/// A group of faces: the world, or a brush model.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    /// The first face in [`Map::faces`].
    pub first_face: u32,
    /// How many faces it has.
    pub face_count: u32,
    /// The smallest corner of its bounds.
    pub mins: Vec3,
    /// The largest corner of its bounds.
    pub maxs: Vec3,
    /// The root of its drawing tree, which points move through.
    pub root: Option<NodeChild>,
    /// The roots of its clipping hulls, for players and for large monsters.
    pub hulls: [ClipChild; 2],
}

/// An entity from the map, as key-value text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entity {
    /// Its pairs, in file order.
    pub fields: Vec<(String, String)>,
}

impl Entity {
    /// The value of `key`, if present.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }
}

/// A complete map.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Map {
    /// The textures.
    pub textures: Vec<Texture>,
    /// Every face's points.
    pub points: Vec<Vec3>,
    /// The faces.
    pub faces: Vec<Face>,
    /// The models. The first is the world.
    pub models: Vec<Model>,
    /// Baked light samples, as RGB.
    pub light: Vec<[u8; 3]>,
    /// Whether the map has baked light at all. Without it, everything is
    /// drawn at full brightness.
    pub lit: bool,
    /// The spatial tree.
    pub tree: Tree,
    /// The entities.
    pub entities: Vec<Entity>,
    /// The sky sphere, if the map has one.
    pub sky: Option<Sky>,
    /// How far each light sample is from a shadow's edge, for crisp styles:
    /// 128 is the edge, and above it is lit. Empty if nothing is crisp.
    pub light_edges: Vec<u8>,
    /// Baked lights that move between keyframes.
    pub light_tracks: Vec<LightTrack>,
    /// Where things can walk, if it's been baked.
    pub nav: Option<NavGrid>,
    /// Lines for movement set out by hand.
    pub splines: Vec<Spline>,
    /// How wide a tile is, or 0 for a map with no tiles.
    pub tile_size: f32,
    /// How far from every player an entity sleeps, or 0 if none ever do.
    pub wake_reach: f32,
    /// The mega texture as baked, if there is one, to keep beside the map.
    pub mega: Option<MegaImage>,
    /// Faces lit from elsewhere, with how far to lift their light samples, as
    /// stamped faces are, since they sit inside a solid.
    pub light_lifts: Vec<(u32, Vec3)>,
    /// How much of each material layer is painted at each light sample, beside
    /// `light`. Empty if nothing is painted.
    pub paint: Vec<[u8; 4]>,
    /// Lights worked out each frame.
    pub live_lights: Vec<LiveLightSource>,
    /// Small fixed fires.
    pub emitters: Vec<Emitter>,
    /// The shade moving models cast.
    pub occluders: Vec<ModelOccluder>,
    /// Bodies of liquid.
    pub pools: Vec<Pool>,
    /// Surfaces that shape visible rays.
    pub stencils: Vec<RayStencil>,
    /// Fog, if the map has any.
    pub fog: Option<Fog>,
    /// Where each sample's light comes from, as bytes. Empty unless the map
    /// was baked with directions.
    pub light_directions: Vec<[u8; 3]>,
    /// Light through the map's space, if it was baked.
    pub probes: Option<ProbeGrid>,
}

impl Model {
    /// Which of the clipping hulls fits a box `width` across, and that hull's
    /// low corner, or `None` for something small enough to be a point, as
    /// `SV_HullForEntity` chooses.
    #[must_use]
    pub fn hull_for(width: f32) -> Option<(usize, Vec3)> {
        if width < 3.0 {
            None
        } else if width <= 32.0 {
            Some((0, Vec3::new(-16.0, -16.0, -24.0)))
        } else {
            Some((1, Vec3::new(-32.0, -32.0, -24.0)))
        }
    }
}

impl Map {
    /// The tile `point` is in, and where it is in that tile. With no tiles it's
    /// all tile 0.
    #[must_use]
    pub fn tile_of(&self, point: Vec3) -> ([i32; 2], Vec3) {
        if self.tile_size <= 0.0 {
            return ([0, 0], point);
        }
        let (x, y) = (
            (point.x / self.tile_size).floor(),
            (point.y / self.tile_size).floor(),
        );
        #[expect(clippy::cast_possible_truncation, reason = "grid places are small")]
        let tile = [x as i32, y as i32];
        let local = Vec3::new(
            point.x - x * self.tile_size,
            point.y - y * self.tile_size,
            point.z,
        );
        (tile, local)
    }

    /// The first entity of `class`, if any.
    #[must_use]
    pub fn entity_of_class(&self, class: &str) -> Option<&Entity> {
        self.entities
            .iter()
            .find(|entity| entity.get("classname") == Some(class))
    }

    /// The points of `face`.
    #[must_use]
    pub fn face_points(&self, face: &Face) -> &[Vec3] {
        let first = usize::try_from(face.first_point).unwrap_or(usize::MAX);
        let count = usize::try_from(face.point_count).unwrap_or(0);
        self.points.get(first..first + count).unwrap_or_default()
    }
}
