//! How much water and sky can be heard from a point.
//!
//! `CalcAmbientSounds` in `vis` meant leaves to store this by distance, but it
//! overwrites the distance before using it, so shipped maps only hold silent
//! or full. This measures what it meant to, from the listener instead of from
//! the leaf, and treats what a map stores as a limit (proposal 0031).

use std::sync::Arc;

use engine_core::math::Vec3;
use engine_map::Map;

/// Closer than this to a surface is full volume.
const FULL_DISTANCE: f32 = 100.0;

/// From here on a surface can't be heard, from `1.0 - dist * 0.002`.
const SILENT_DISTANCE: f32 = 500.0;

/// The water sound's place in a set of levels.
const WATER: usize = 0;

/// The sky sound's place in a set of levels.
const SKY: usize = 1;

/// Which sound a texture makes, as `vis` decides. Slime sounds like water, and
/// lava has no sound.
fn ambient_of(name: &str) -> Option<usize> {
    let name = name.to_ascii_lowercase();
    if name.starts_with("sky") {
        Some(SKY)
    } else if ["*water", "*slime", "*04water"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
    {
        Some(WATER)
    } else {
        None
    }
}

/// A box around a face.
#[derive(Clone, Copy, Debug)]
struct Bounds {
    /// The smallest corner.
    mins: Vec3,
    /// The largest corner.
    maxs: Vec3,
}

impl Bounds {
    /// How far `point` is from the box.
    fn distance(&self, point: Vec3) -> f32 {
        let outside = |low: f32, value: f32, high: f32| (low - value).max(value - high).max(0.0);
        Vec3::new(
            outside(self.mins.x, point.x, self.maxs.x),
            outside(self.mins.y, point.y, self.maxs.y),
            outside(self.mins.z, point.z, self.maxs.z),
        )
        .length()
    }
}

/// The box around the points of a face, if it has any.
fn face_bounds(map: &Map, first: u32, count: u32) -> Option<Bounds> {
    let first = usize::try_from(first).ok()?;
    let count = usize::try_from(count).ok()?;
    let points = map.points.get(first..first + count)?;
    let (head, rest) = points.split_first()?;
    let mut bounds = Bounds {
        mins: *head,
        maxs: *head,
    };
    for point in rest {
        bounds.mins = Vec3::new(
            bounds.mins.x.min(point.x),
            bounds.mins.y.min(point.y),
            bounds.mins.z.min(point.z),
        );
        bounds.maxs = Vec3::new(
            bounds.maxs.x.max(point.x),
            bounds.maxs.y.max(point.y),
            bounds.maxs.z.max(point.z),
        );
    }
    Some(bounds)
}

/// The volume of something `distance` away, from 0 to 1.
fn falloff(distance: f32) -> f32 {
    ((SILENT_DISTANCE - distance) / (SILENT_DISTANCE - FULL_DISTANCE)).clamp(0.0, 1.0)
}

/// Finds how much of each ambient sound a point hears.
pub struct AmbientSounds {
    /// The map.
    map: Arc<Map>,
    /// Each face's sound and box, for faces that make one.
    faces: Vec<Option<(usize, Bounds)>>,
    /// The leaf `audible` was gathered for.
    leaf: Option<u32>,
    /// The boxes of each sound's faces in sight of that leaf.
    audible: [Vec<Bounds>; 2],
}

impl AmbientSounds {
    /// Finds the faces in `map` that make a sound.
    #[must_use]
    pub fn new(map: Arc<Map>) -> Self {
        let faces = map
            .faces
            .iter()
            .map(|face| {
                let texture = map.textures.get(usize::try_from(face.texture).ok()?)?;
                let sound = ambient_of(&texture.name)?;
                let bounds = face_bounds(&map, face.first_point, face.point_count)?;
                Some((sound, bounds))
            })
            .collect();
        Self {
            map,
            faces,
            leaf: None,
            audible: [Vec::new(), Vec::new()],
        }
    }

    /// Gathers the faces in sight of `leaf`.
    fn gather(&mut self, leaf: u32) {
        let tree = &self.map.tree;
        let mut visible = vec![false; tree.leaves.len()];
        tree.mark_visible(leaf, &mut visible);
        let mut seen = vec![false; self.faces.len()];
        self.audible = [Vec::new(), Vec::new()];
        for (in_sight, leaf) in visible.iter().zip(&tree.leaves) {
            if !in_sight {
                continue;
            }
            let first = usize::try_from(leaf.first_face).unwrap_or(usize::MAX);
            let count = usize::try_from(leaf.face_count).unwrap_or(0);
            let faces = tree.leaf_faces.iter().skip(first).take(count);
            for face in faces.filter_map(|face| usize::try_from(*face).ok()) {
                if let Some(Some((sound, bounds))) = self.faces.get(face)
                    && let Some(flag) = seen.get_mut(face)
                    && !*flag
                {
                    *flag = true;
                    self.audible[*sound].push(*bounds);
                }
            }
        }
        self.leaf = Some(leaf);
    }

    /// The levels of water and sky at `point`, out of 255.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a level is rounded to a byte, and is from 0 to 255"
    )]
    pub fn levels(&mut self, point: Vec3) -> [u8; 2] {
        let leaf = self.map.tree.leaf_at(point);
        if self.leaf != Some(leaf) {
            self.gather(leaf);
        }
        let stored = self.map.tree.leaf(leaf).map_or([0; 4], |leaf| leaf.ambient);
        [WATER, SKY].map(|sound| {
            let nearest = self.audible[sound]
                .iter()
                .map(|bounds| bounds.distance(point))
                .fold(f32::INFINITY, f32::min);
            let level = (falloff(nearest) * 255.0).round() as u8;
            level.min(stored[sound])
        })
    }
}
