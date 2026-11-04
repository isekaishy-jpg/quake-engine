//! Geometry, textures, lighting and entities.

use engine_core::math::Vec3;

use crate::tree::Tree;

/// How a texture is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureKind {
    /// An ordinary lit surface. Alpha is how much light affects a pixel, so
    /// zero is fullbright.
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Texture {
    /// Its name, for display.
    pub name: String,
    /// How it's drawn.
    pub kind: TextureKind,
    /// Its mip chain, largest first, never empty.
    pub mips: Vec<MipLevel>,
    /// Its animation, if it has one.
    pub animation: Option<Animation>,
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

/// A group of faces: the world, or a brush model.
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    /// The first face in [`Map::faces`].
    pub first_face: u32,
    /// How many faces it has.
    pub face_count: u32,
    /// Where it's placed.
    pub origin: Vec3,
    /// Whether it's drawn.
    pub visible: bool,
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
}

impl Map {
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
