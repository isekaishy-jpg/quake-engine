//! Light that live lights bounce, kept at the map's probes (proposal 0153). A baked light's bounce
//! is baked; a live light moves, so what it bounces is worked out as it goes, coarsely, at the
//! probes round it, for surfaces to read between them. Light off what it lights lights the room
//! it's in, its shadows included, and not through a wall: a probe takes it only if it can see a
//! place in that room.

use engine_core::math::Vec3;
use engine_map::{Map, ProbeGrid};

use crate::live::LiveLight;

/// How much of what a light lights bounces back into the room, as a baked bounce's gain is.
pub const LIVE_BOUNCE_GAIN: f32 = 0.25;

/// How far out from a light the places in its room are looked for, as a share of its reach.
const ROOM_REACH: f32 = 0.5;

/// How far short of a wall a place in a light's room stops.
const WALL_GAP: f32 = 8.0;

/// A corner's way, as a share of each axis.
const CORNER: f32 = 0.577_350_3;

/// The ways out from a light that its room's places are found along: along each axis, and to
/// each corner.
const ROOM_WAYS: [Vec3; 14] = [
    Vec3::new(1.0, 0.0, 0.0),
    Vec3::new(-1.0, 0.0, 0.0),
    Vec3::new(0.0, 1.0, 0.0),
    Vec3::new(0.0, -1.0, 0.0),
    Vec3::new(0.0, 0.0, 1.0),
    Vec3::new(0.0, 0.0, -1.0),
    Vec3::new(CORNER, CORNER, CORNER),
    Vec3::new(CORNER, CORNER, -CORNER),
    Vec3::new(CORNER, -CORNER, CORNER),
    Vec3::new(CORNER, -CORNER, -CORNER),
    Vec3::new(-CORNER, CORNER, CORNER),
    Vec3::new(-CORNER, CORNER, -CORNER),
    Vec3::new(-CORNER, -CORNER, CORNER),
    Vec3::new(-CORNER, -CORNER, -CORNER),
];

/// Places in the room `light` is in of `map`: where it is, and out along each way, short of what
/// stops it.
fn room_of(map: &Map, light: &LiveLight) -> Vec<Vec3> {
    let mut places = vec![light.origin];
    let reach = light.radius * ROOM_REACH;
    for way in ROOM_WAYS {
        let (mut low, mut high) = (0.0_f32, reach);
        // How far is clear, found by halves, as a line test says only whether it's clear.
        if map.tree.line_clear(light.origin, light.origin + way * high) {
            low = high;
        } else {
            for _ in 0..8 {
                let middle = (low + high) * 0.5;
                if map
                    .tree
                    .line_clear(light.origin, light.origin + way * middle)
                {
                    low = middle;
                } else {
                    high = middle;
                }
            }
        }
        let out = (low - WALL_GAP).max(0.0);
        if out > 0.0 {
            places.push(light.origin + way * out);
        }
    }
    places
}

/// What `light` bounces onto each of `probes` it reaches in `map`, by index: its colour, by how
/// much it bounces, as it falls off with distance as its own light does, by how much of its room
/// the probe sees.
#[must_use]
pub fn bounce_of_light(map: &Map, probes: &ProbeGrid, light: &LiveLight) -> Vec<(u32, [f32; 3])> {
    if light.bounce <= 0.0 || light.radius <= 0.0 || probes.spacing <= 0.0 {
        return Vec::new();
    }
    let room = room_of(map, light);
    #[expect(clippy::cast_precision_loss, reason = "a few places")]
    let each = 1.0 / room.len() as f32;
    let reach = light.radius;
    let local = |value: f32, origin: f32| (value - origin) / probes.spacing;
    let span = |axis: usize, middle: f32, origin: f32| {
        let size = probes.size[axis];
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped into the grid"
        )]
        let at = |value: f32| (local(value, origin).max(0.0) as u32).min(size.saturating_sub(1));
        at(middle - reach)..=at(middle + reach + probes.spacing)
    };
    let origin = probes.origin;
    let mut out = Vec::new();
    for z in span(2, light.origin.z, origin.z) {
        for y in span(1, light.origin.y, origin.y) {
            for x in span(0, light.origin.x, origin.x) {
                #[expect(clippy::cast_precision_loss, reason = "grid sizes are small")]
                let place = origin + Vec3::new(x as f32, y as f32, z as f32) * probes.spacing;
                let distance = (place - light.origin).length();
                if distance >= reach {
                    continue;
                }
                let seen = room
                    .iter()
                    .filter(|there| map.tree.line_clear(place, **there))
                    .count();
                if seen == 0 {
                    continue;
                }
                let fall = 1.0 - distance * distance / (reach * reach);
                #[expect(clippy::cast_precision_loss, reason = "a few places")]
                let strength = fall * fall * light.bounce * LIVE_BOUNCE_GAIN * seen as f32 * each;
                let [sx, sy, _] = probes.size;
                out.push((
                    (z * sy + y) * sx + x,
                    light.color.map(|part| part * strength),
                ));
            }
        }
    }
    out
}
