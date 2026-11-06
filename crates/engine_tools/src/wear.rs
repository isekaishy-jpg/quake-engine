//! Wear: making a level look used (proposal 0120).
//!
//! Damp creeps up from where walls meet floors and down from where they meet
//! ceilings, dirt gathers on open floors, and stains and cracks mark walls. Placing
//! thousands of those by hand isn't work anyone should do, so this reads the level's
//! real surfaces and puts down ordinary `paint` and `decal` blocks. They can be changed
//! or removed like any others, and running it again replaces what it made.

use std::collections::HashSet;

use engine_core::math::Vec3;
use engine_format::{Block, Value};
use engine_map::{Contents, Face, Map};
use serde_json::{Value as Json, json};

use crate::json::numbers;
use crate::operations::built_map;
use crate::session::Session;

/// What marks what wear made, so it can be made again.
const MADE: &str = "from_wear";

/// The painted layer that's damp, as materials list their layers.
const DAMP: f64 = 0.0;

/// The painted layer that's dirt.
const DIRT: f64 = 1.0;

/// How long a stroke along an edge is, so that gaps can be left in the line.
const PIECE: f32 = 192.0;

/// How far patches of one kind of wear reach, so that it comes in places and isn't
/// mixed evenly.
const PATCH: f32 = 640.0;

/// How much wall gets one decal, at full wear.
const WALL_PER_DECAL: f32 = 420.0 * 420.0;

/// How much floor gets one dab of dirt, at full wear.
const FLOOR_PER_DAB: f32 = 320.0 * 320.0;

/// The most decals a level gets, as each is more to draw.
const MOST_DECALS: usize = 800;

/// The materials of decals, and how large each may be.
const STAIN: &str = "decal_stain";
/// See [`STAIN`].
const CRACKS: &str = "decal_cracks";
/// See [`STAIN`].
const GRIME: &str = "decal_grime";

/// How wide a cell of the search for open space is. Walls are at least this thick, so
/// that it can't step through one.
const SPACE_CELL: f32 = 32.0;

/// How far out of a surface to look for open space.
const LOOK_OUT: f32 = 24.0;

/// A number that only depends on what it's given.
fn mixed(seed: u32, a: u32, b: u32) -> u32 {
    let mut value = seed
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(a.wrapping_mul(0x85EB_CA6B))
        .wrapping_add(b.wrapping_mul(0xC2B2_AE35));
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^ (value >> 16)
}

/// A number from 0 to 1 that only depends on what it's given.
fn roll(seed: u32, a: u32, b: u32) -> f32 {
    #[expect(clippy::cast_precision_loss, reason = "a rough share is enough")]
    let share = mixed(seed, a, b) as f32 / u32::MAX as f32;
    share
}

/// A polygon of the level.
struct Surface<'a> {
    /// Its number, which seeds what happens on it.
    number: u32,
    /// Its corners.
    corners: &'a [Vec3],
    /// The way out of it.
    normal: Vec3,
    /// Its area.
    area: f32,
}

/// The way out of a polygon and its area, by Newell's method.
fn shape_of(corners: &[Vec3]) -> (Vec3, f32) {
    let mut sum = Vec3::ZERO;
    for (index, corner) in corners.iter().enumerate() {
        let next = corners[(index + 1) % corners.len()];
        sum = sum
            + Vec3::new(
                (corner.y - next.y) * (corner.z + next.z),
                (corner.z - next.z) * (corner.x + next.x),
                (corner.x - next.x) * (corner.y + next.y),
            );
    }
    let length = sum.length();
    if length < 1e-3 {
        return (Vec3::ZERO, 0.0);
    }
    (sum * (1.0 / length), length * 0.5)
}

/// The cell `point` is in.
fn space_cell(point: Vec3) -> [i32; 3] {
    #[expect(clippy::cast_possible_truncation, reason = "levels fit an i32")]
    let cell = |value: f32| (value / SPACE_CELL).floor() as i32;
    [cell(point.x), cell(point.y), cell(point.z)]
}

/// The middle of a cell.
fn cell_middle(cell: [i32; 3]) -> Vec3 {
    #[expect(clippy::cast_precision_loss, reason = "levels are far smaller")]
    let middle = |value: i32| (value as f32 + 0.5) * SPACE_CELL;
    Vec3::new(middle(cell[0]), middle(cell[1]), middle(cell[2]))
}

/// Whether someone could be at `point`.
fn is_open(map: &Map, point: Vec3) -> bool {
    let leaf = map.tree.leaves.get(map.tree.leaf_at(point) as usize);
    leaf.is_some_and(|leaf| !matches!(leaf.contents, Contents::Solid | Contents::Sky))
}

/// Where each entity stands, for those that say.
fn entity_places(map: &Map) -> Vec<Vec3> {
    map.entities
        .iter()
        .filter_map(|entity| {
            let words = entity.get("origin")?;
            let parts: Vec<f32> = words.split(' ').filter_map(|w| w.parse().ok()).collect();
            match parts.as_slice() {
                [x, y, z] => Some(Vec3::new(*x, *y, *z)),
                _ => None,
            }
        })
        .collect()
}

/// The open space that can be reached from the level's entities, which is where anyone
/// can see. Faces on the outside of a level are real faces too, and wear there would be
/// wasted. With no entities to start from, it's `None`, and everything counts.
fn reached_space(map: &Map) -> Option<HashSet<[i32; 3]>> {
    let world = map.models.first()?;
    let (low, high) = (space_cell(world.mins), space_cell(world.maxs));
    let mut waiting: Vec<[i32; 3]> = entity_places(map)
        .into_iter()
        .filter(|place| is_open(map, *place))
        .map(space_cell)
        .collect();
    if waiting.is_empty() {
        return None;
    }
    let mut reached: HashSet<[i32; 3]> = waiting.iter().copied().collect();
    while let Some(cell) = waiting.pop() {
        for (axis, step) in [(0, -1), (0, 1), (1, -1), (1, 1), (2, -1), (2, 1)] {
            let mut next = cell;
            next[axis] += step;
            let inside = (0..3).all(|axis| (low[axis]..=high[axis]).contains(&next[axis]));
            if inside && !reached.contains(&next) && is_open(map, cell_middle(next)) {
                let _new = reached.insert(next);
                waiting.push(next);
            }
        }
    }
    Some(reached)
}

/// Whether anyone can see `surface`. A big face can be mostly behind something, so it
/// looks out from its middle and from halfway to each of its corners.
fn can_be_seen(surface: &Surface<'_>, reached: &HashSet<[i32; 3]>) -> bool {
    let corners = surface.corners;
    #[expect(clippy::cast_precision_loss, reason = "a face has few corners")]
    let share = 1.0 / corners.len() as f32;
    let middle = corners.iter().fold(Vec3::ZERO, |sum, corner| sum + *corner) * share;
    let out = surface.normal * LOOK_OUT;
    std::iter::once(middle)
        .chain(corners.iter().map(|corner| (middle + *corner) * 0.5))
        .any(|place| reached.contains(&space_cell(place + out)))
}

/// Whether wear shows on a material. Metal, wood, glass and liquids stay clean.
fn wears(material: &str) -> bool {
    ["_wall_", "_rock", "_floor_", "_ceiling_"]
        .iter()
        .any(|part| material.contains(part))
}

/// The surfaces of the level's world that can wear.
fn surfaces(map: &Map) -> Vec<Surface<'_>> {
    let Some(world) = map.models.first() else {
        return Vec::new();
    };
    let first = world.first_face as usize;
    let faces: &[Face] = map
        .faces
        .get(first..first + world.face_count as usize)
        .unwrap_or_default();
    (0_u32..)
        .zip(faces)
        .filter_map(|(number, face)| {
            let material = &map.textures.get(face.texture as usize)?.name;
            let start = face.first_point as usize;
            let corners = map.points.get(start..start + face.point_count as usize)?;
            let (normal, area) = shape_of(corners);
            (wears(material) && area > 1.0).then_some(Surface {
                number,
                corners,
                normal,
                area,
            })
        })
        .collect()
}

/// A flat list of `points`' numbers.
fn flat(points: &[Vec3]) -> Value {
    let list: Vec<f64> = points
        .iter()
        .flat_map(|point| [point.x, point.y, point.z].map(f64::from))
        .collect();
    numbers(&list)
}

/// A stroke of `layer`.
fn stroke(layer: f64, radius: f32, strength: f32, points: &[Vec3]) -> Block {
    let mut block = Block::new("paint");
    block.set("layer", Value::Number(layer));
    block.set("radius", Value::Number(f64::from(radius)));
    block.set("strength", Value::Number(f64::from(strength)));
    block.set("points", flat(points));
    block.set(MADE, Value::Flag(true));
    block
}

/// A decal of `material` at `origin` on a surface facing `normal`.
fn decal(material: &str, origin: Vec3, normal: Vec3, size: f32, angle: f32) -> Block {
    let mut block = Block::new("decal");
    block.set("material", Value::Text(material.to_owned()));
    block.set("origin", flat(&[origin]));
    block.set("normal", flat(&[normal]));
    block.set("size", Value::Number(f64::from(size)));
    block.set("angle", Value::Number(f64::from(angle)));
    block.set(MADE, Value::Flag(true));
    block
}

/// The ends of the edge along the top or bottom of a wall, if it has a level one.
fn level_edge(corners: &[Vec3], top: bool) -> Option<(Vec3, Vec3)> {
    let height = |corner: &Vec3| if top { -corner.z } else { corner.z };
    let lowest = corners.iter().map(height).fold(f32::MAX, f32::min);
    let mut along = corners.iter().filter(|c| height(c) < lowest + 1.0);
    let first = *along.next()?;
    let last = along
        .max_by(|a, b| {
            let (a, b) = ((**a - first).length(), (**b - first).length());
            a.total_cmp(&b)
        })
        .copied()?;
    Some((first, last))
}

/// Which layer a place has, in patches.
fn layer_at(seed: u32, at: Vec3) -> f64 {
    #[expect(clippy::cast_possible_truncation, reason = "levels fit an i32")]
    let cell = |value: f32| (value / PATCH).floor() as i32;
    if roll(seed, cell(at.x).cast_unsigned(), cell(at.y).cast_unsigned()) < 0.7 {
        DAMP
    } else {
        DIRT
    }
}

/// Strokes along an edge of a wall, rising or falling `reach` away from it at their
/// middles, with gaps. `chance` is how much of the edge gets one.
fn along_edge(
    (from, to): (Vec3, Vec3),
    (seed, number): (u32, u32),
    (chance, reach): (f32, f32),
    out: &mut Vec<Block>,
) {
    let length = (to - from).length();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "an edge has few pieces"
    )]
    let pieces = (length / PIECE).ceil().max(1.0) as u32;
    for piece in 0..pieces {
        let roll_for = |what: u32| roll(seed ^ what, number, piece);
        if roll_for(1) >= chance {
            continue;
        }
        #[expect(clippy::cast_precision_loss, reason = "an edge has few pieces")]
        let (start, end) = (
            piece as f32 / pieces as f32,
            (piece + 1) as f32 / pieces as f32,
        );
        let a = from + (to - from) * start;
        let b = from + (to - from) * end;
        let rise = Vec3::new(0.0, 0.0, reach * roll_for(2));
        let middle = (a + b) * 0.5 + rise;
        let radius = 32.0 + 64.0 * roll_for(3);
        let strength = 0.7 + 0.3 * roll_for(4);
        let layer = layer_at(seed, middle);
        out.push(stroke(layer, radius, strength, &[a, middle, b]));
    }
}

/// A place on a surface, in the fan of triangles from its first corner.
fn place_on(surface: &Surface<'_>, seed: u32, try_number: u32) -> Vec3 {
    let corners = surface.corners;
    let triangles = corners.len().saturating_sub(2).max(1);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a face has few corners"
    )]
    let index = ((roll(seed ^ 5, surface.number, try_number) * triangles as f32) as usize)
        .min(triangles - 1);
    let (a, b, c) = (
        corners[0],
        corners[(index + 1).min(corners.len() - 1)],
        corners[(index + 2).min(corners.len() - 1)],
    );
    let (mut u, mut v) = (
        roll(seed ^ 6, surface.number, try_number),
        roll(seed ^ 7, surface.number, try_number),
    );
    if u + v > 1.0 {
        (u, v) = (1.0 - u, 1.0 - v);
    }
    a + (b - a) * u + (c - a) * v
}

/// How many of something `area` gets at `per` each, with what's left over rolled.
fn share_of(area: f32, per: f32, amount: f32, chance: f32) -> u32 {
    let wanted = area / per * amount;
    let whole = wanted.floor();
    let extra = u32::from(chance < wanted - whole);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "counts are small"
    )]
    let count = whole as u32;
    count + extra
}

/// What wear a wall gets.
fn wear_wall(surface: &Surface<'_>, seed: u32, amount: f32, out: &mut Worn) {
    let number = surface.number;
    if let Some(edge) = level_edge(surface.corners, false) {
        along_edge(
            edge,
            (seed, number),
            (0.65 * amount, 40.0),
            &mut out.strokes,
        );
    }
    if let Some(edge) = level_edge(surface.corners, true) {
        along_edge(
            edge,
            (seed ^ 11, number),
            (0.3 * amount, -24.0),
            &mut out.strokes,
        );
    }
    let count = share_of(
        surface.area,
        WALL_PER_DECAL,
        amount,
        roll(seed ^ 8, number, 0),
    );
    for try_number in 0..count {
        let place = place_on(surface, seed, try_number);
        let kind = roll(seed ^ 9, number, try_number);
        let size = 96.0 + 160.0 * roll(seed ^ 10, number, try_number);
        let turn = 360.0 * roll(seed ^ 12, number, try_number);
        out.decals.push(if kind < 0.6 {
            decal(STAIN, place, surface.normal, size, 0.0)
        } else {
            decal(CRACKS, place, surface.normal, size, turn)
        });
    }
}

/// What wear a floor gets.
fn wear_floor(surface: &Surface<'_>, seed: u32, amount: f32, out: &mut Worn) {
    let number = surface.number;
    let chance = roll(seed ^ 13, number, 0);
    for try_number in 0..share_of(surface.area, FLOOR_PER_DAB, amount, chance) {
        let place = place_on(surface, seed ^ 14, try_number);
        let radius = 48.0 + 96.0 * roll(seed ^ 15, number, try_number);
        let strength = 0.5 + 0.4 * roll(seed ^ 16, number, try_number);
        out.strokes.push(stroke(DIRT, radius, strength, &[place]));
        if roll(seed ^ 17, number, try_number) < 0.4 {
            let turn = 360.0 * roll(seed ^ 18, number, try_number);
            let size = 128.0 + 128.0 * roll(seed ^ 19, number, try_number);
            out.decals
                .push(decal(GRIME, place, surface.normal, size, turn));
        }
    }
}

/// What wear makes.
#[derive(Default)]
struct Worn {
    /// Strokes of paint.
    strokes: Vec<Block>,
    /// Decals.
    decals: Vec<Block>,
}

/// `wear`: replaces the wear on the level with new wear. Optional amount, from 0 to 1,
/// and seed.
pub(crate) fn wear(session: &mut Session, args: &Json) -> Result<Json, String> {
    let amount = args.get("amount").and_then(Json::as_f64).unwrap_or(1.0);
    #[expect(clippy::cast_possible_truncation, reason = "a share fits f32")]
    let amount = amount.clamp(0.0, 1.0) as f32;
    let seed = args.get("seed").and_then(Json::as_u64).unwrap_or(1);
    #[expect(clippy::cast_possible_truncation, reason = "any bits will do")]
    let seed = seed as u32;
    session.checkpoint();
    // What's there is worked out without the old wear, which would be in it as decals.
    let before = session.document.blocks.clone();
    session.document.blocks.retain(|b| b.get(MADE).is_none());
    let map = match built_map(session) {
        Ok((map, ..)) => map,
        Err(error) => {
            session.document.blocks = before;
            return Err(error);
        }
    };
    let reached = reached_space(&map);
    let mut worn = Worn::default();
    for surface in surfaces(&map) {
        if reached
            .as_ref()
            .is_some_and(|reached| !can_be_seen(&surface, reached))
        {
            continue;
        }
        if surface.normal.z.abs() < 0.3 {
            wear_wall(&surface, seed, amount, &mut worn);
        } else if surface.normal.z > 0.7 {
            wear_floor(&surface, seed, amount, &mut worn);
        }
    }
    // Keep an even share of them, and not whichever came first.
    let mut number = 0;
    worn.decals.sort_by_cached_key(|_| {
        number += 1;
        mixed(seed ^ 20, number, 0)
    });
    worn.decals.truncate(MOST_DECALS);
    let answer = json!({ "strokes": worn.strokes.len(), "decals": worn.decals.len() });
    for mut block in worn.strokes.into_iter().chain(worn.decals) {
        let _id = session.give_id(&mut block);
        session.document.blocks.push(block);
    }
    Ok(answer)
}
