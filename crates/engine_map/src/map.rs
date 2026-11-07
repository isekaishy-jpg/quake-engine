//! Geometry, textures, lighting and entities.

use std::sync::Arc;

use engine_core::math::Vec3;

use crate::emitter::Emitter;
use crate::live::LiveLightSource;
use crate::material::Material;
use crate::nav::{NavGrid, NavStep, Spline};
use crate::nav_chunks::ChunkedNav;
use crate::occluder::ModelOccluder;
use crate::pool::Pool;
use crate::probes::{LightTrack, ProbeGrid};
use crate::sky::{Fog, Sky};
use crate::stencil::RayStencil;
use crate::tile_light::LightTiles;

use crate::tree::{ClipChild, Contents, NodeChild, Tree};

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

/// What a mip's bytes are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PixelForm {
    /// Four bytes a pixel.
    #[default]
    Rgba,
    /// Compressed colour, in sRGB.
    Bc7Srgb,
    /// Compressed numbers.
    Bc7,
    /// Two compressed channels, for a normal map.
    Bc5,
}

/// One level of a texture's mip chain, in RGBA bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MipLevel {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Its pixels, four bytes each, unless `form` says otherwise.
    pub rgba: Vec<u8>,
    /// What those bytes are.
    pub form: PixelForm,
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

impl Texture {
    /// How many texels one repeat covers, across and down, if it has a size at
    /// all. A texel is a world unit on a surface at its usual scale, so this is
    /// what sets how big a surface looks, not how many pixels it has.
    #[must_use]
    #[expect(clippy::cast_precision_loss, reason = "picture sizes are small")]
    pub fn tile_size(&self) -> Option<[f32; 2]> {
        let set = self.material.as_ref().and_then(|m| m.world_size);
        let picture = self
            .mips
            .first()
            .map(|mip| [mip.width as f32, mip.height as f32]);
        set.or(picture)
    }
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
    /// Which tile's light holds its samples, or [`crate::RESIDENT`] for
    /// [`Map::light`].
    pub tile: u32,
    /// Its average colour, for when its tile's light isn't held.
    pub average: [u8; 3],
}

impl FaceLight {
    /// How many styles this face has.
    #[must_use]
    pub fn style_count(&self) -> u32 {
        let count = self.styles.iter().take_while(|&&s| s != Face::NO_STYLE);
        u32::try_from(count.count()).unwrap_or(0)
    }
}

/// One placed mesh's faces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MeshGroup {
    /// The first face.
    pub first_face: u32,
    /// How many there are.
    pub face_count: u32,
    /// Whether it's lit from one place, as a small prop is.
    pub by_probe: bool,
    /// Which placement of the level it came from, if it's drawn as an instance.
    pub placement: Option<u32>,
}

/// A corner of a [`SharedMesh`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeshCorner {
    /// Where it is, in the mesh's own space.
    pub position: [f32; 3],
    /// Its place on the material, where 1 is the texture's width.
    pub uv: [f32; 2],
    /// Which way it faces, in steps of 1/127.
    pub normal: [i8; 3],
}

/// The triangles of a [`SharedMesh`] with one material.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SharedPart {
    /// Its material in [`Map::textures`].
    pub texture: u32,
    /// The first of its indices.
    pub first: u32,
    /// How many indices it has.
    pub count: u32,
}

/// A mesh kept once, however many times it's placed (proposal 0140).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SharedMesh {
    /// Its corners.
    pub corners: Vec<MeshCorner>,
    /// Its triangles, three to each.
    pub indices: Vec<u32>,
    /// Its parts by material, for each level of detail, the finest first.
    pub levels: Vec<Vec<SharedPart>>,
    /// The share of the view's height it has to fall below to go to each coarser level.
    pub level_sizes: Vec<f32>,
    /// The middle of what it covers.
    pub center: Vec3,
    /// How far it reaches from there.
    pub radius: f32,
}

/// A placement of a [`SharedMesh`]. Its faces in [`Map::faces`] hold its light, and are found
/// by leaves, but aren't drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeshInstance {
    /// Which of [`Map::shared_meshes`] it is.
    pub mesh: u32,
    /// The three rows of what places it.
    pub rows: [[f32; 4]; 3],
    /// The first of its faces.
    pub first_face: u32,
    /// How many faces it has.
    pub face_count: u32,
}

impl MeshInstance {
    /// Where `point`, in the mesh's own space, ends up.
    #[must_use]
    pub fn place(&self, point: Vec3) -> Vec3 {
        let row = |row: [f32; 4]| row[0] * point.x + row[1] * point.y + row[2] * point.z + row[3];
        Vec3::new(row(self.rows[0]), row(self.rows[1]), row(self.rows[2]))
    }

    /// How much it stretches at most.
    #[must_use]
    pub fn stretch(&self) -> f32 {
        let column =
            |at: usize| Vec3::new(self.rows[0][at], self.rows[1][at], self.rows[2][at]).length();
        column(0).max(column(1)).max(column(2))
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
    /// How a place becomes a place on its light map, where that isn't by `axes`.
    /// A mesh's triangles share light maps laid out for them (proposal 0110).
    pub light_axes: Option<[TexelAxis; 2]>,
}

impl Face {
    /// What places its light map.
    #[must_use]
    pub fn light_space(&self) -> &[TexelAxis; 2] {
        self.light_axes.as_ref().unwrap_or(&self.axes)
    }

    /// Whether it's lit from one place, as a probe is, and so has no shape to its
    /// light until it's drawn.
    #[must_use]
    pub fn lit_by_probe(&self) -> bool {
        self.light_axes
            .is_some_and(|axes| axes[0].axis == Vec3::ZERO && axes[1].axis == Vec3::ZERO)
    }

    /// The style number marking an unused lighting layer.
    pub const NO_STYLE: u8 = 255;
}

/// How a map's light comes out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// What baked light is raised to. 1 is true linear light, which looks washed
    /// out with the soft bounced light levels have, and higher makes dim light
    /// dimmer: 2.2 is the murky look of light on a picture in gamma space.
    pub light_curve: f32,
    /// How much light reaches the eye.
    pub exposure: f32,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            light_curve: 1.8,
            exposure: 1.0,
        }
    }
}

/// A map's mega texture, as it's kept in a bake.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MegaImage {
    /// A face and how far its coordinates moved, three numbers each.
    pub faces: Vec<i32>,
    /// Its pixels, in RGBA bytes.
    pub rgba: Vec<u8>,
    /// The images laid out the same way, for what its surfaces use.
    pub maps: MegaMaps,
}

/// The other images of a mega texture, each only if a surface has one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MegaMaps {
    /// Normals, flat where a surface has none.
    pub normal: Option<Vec<u8>>,
    /// Specular colour, with gloss in alpha, already scaled by each material.
    pub specular: Option<Vec<u8>>,
    /// Emissive colour, already scaled by each material.
    pub emissive: Option<Vec<u8>>,
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
    /// Which way points face, in steps of 1/127, by their place in `points`. It
    /// stops at the last that has one, and all zeros means the face is flat.
    pub normals: Vec<[i8; 3]>,
    /// The faces.
    pub faces: Vec<Face>,
    /// Each placed mesh's faces.
    pub mesh_groups: Vec<MeshGroup>,
    /// Meshes kept once, for placements to share.
    pub shared_meshes: Vec<SharedMesh>,
    /// What's drawn from them.
    pub mesh_instances: Vec<MeshInstance>,
    /// The named parts of the map.
    pub zones: Vec<crate::Zone>,
    /// The effects that can be started.
    pub effects: Vec<Arc<crate::EffectDef>>,
    /// The effects in the level from the start.
    pub effect_points: Vec<crate::EffectPoint>,
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
    /// Where large monsters can walk, from the larger clipping hull.
    pub nav_large: Option<NavGrid>,
    /// The same two, for a tiled map, read a chunk at a time.
    pub nav_chunks: [Option<ChunkedNav>; 2],
    /// Where a tiled map's light is, for faces not in [`Map::light`].
    pub light_tiles: Option<LightTiles>,
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
    /// Code for the final pass, if it has its own.
    pub post_shader: Option<Arc<crate::ShaderCode>>,
    /// How its light comes out.
    pub look: Look,
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
    /// Whether nothing solid stands over `point`, all the way to the top of the
    /// world, so that a mirror there could see the sky.
    #[must_use]
    pub fn open_above(&self, point: Vec3) -> bool {
        /// How finely it looks: as thin as a thin ceiling.
        const STEP: f32 = 8.0;
        let top = self.models.first().map_or(point.z, |world| world.maxs.z);
        let mut at = point;
        while at.z < top {
            let leaf = self.tree.leaf(self.tree.leaf_at(at));
            if leaf.is_some_and(|leaf| leaf.contents == Contents::Solid) {
                return false;
            }
            at.z += STEP;
        }
        true
    }

    /// Where something `width` across can walk: the grid of the hull that
    /// holds it, as collision chooses one.
    #[must_use]
    pub fn nav_for(&self, width: f32) -> Option<&NavGrid> {
        match Model::hull_for(width) {
            Some((1, _)) => self.nav_large.as_ref(),
            _ => self.nav.as_ref(),
        }
    }

    /// The way for something `width` across, wherever this map keeps its grids.
    #[must_use]
    pub fn find_path(&self, width: f32, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        if let Some(nav) = self.nav_for(width) {
            return nav.find_path(from, to);
        }
        let hull = usize::from(matches!(Model::hull_for(width), Some((1, _))));
        self.nav_chunks.get(hull)?.as_ref()?.find_path(from, to)
    }

    /// As [`Self::find_path`], saying how each point is reached.
    #[must_use]
    pub fn find_route(&self, width: f32, from: Vec3, to: Vec3) -> Option<Vec<NavStep>> {
        if let Some(nav) = self.nav_for(width) {
            return nav.find_route(from, to);
        }
        let hull = usize::from(matches!(Model::hull_for(width), Some((1, _))));
        self.nav_chunks.get(hull)?.as_ref()?.find_route(from, to)
    }

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
