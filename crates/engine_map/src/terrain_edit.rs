//! Shaping land, by a circle with a soft edge (proposal 0144). The tools and the editor's brush both
//! come here, so that land shapes the same whichever does it.

use crate::TerrainData;

/// A circle on the land, in world units from a sheet's low corner.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TerrainBrush {
    /// Its middle.
    pub middle: [f32; 2],
    /// How far it reaches.
    pub radius: f32,
    /// How much of it fades, from 0 for a hard edge to 1 for all of it.
    pub soft: f32,
}

impl TerrainBrush {
    /// How much of the brush is on a place, from 0 to 1.
    #[must_use]
    pub fn weight(&self, x: f32, y: f32) -> f32 {
        let (across, up) = (x - self.middle[0], y - self.middle[1]);
        let away = across.hypot(up);
        let hard = self.radius * (1.0 - self.soft.clamp(0.0, 1.0));
        if away <= hard {
            return 1.0;
        }
        if away >= self.radius {
            return 0.0;
        }
        let share = (self.radius - away) / (self.radius - hard).max(f32::EPSILON);
        share * share * (3.0 - 2.0 * share)
    }
}

/// What a brush does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TerrainStroke {
    /// Lifts the land by this much at the brush's middle, and lowers it if that's negative.
    Raise(f32),
    /// Moves the land to this height, by how much of the brush is there.
    Flatten(f32),
    /// Moves each height towards what's round it, by this share.
    Smooth(f32),
    /// Cuts a hole, or fills one with `false`.
    Hole(bool),
}

/// The samples within reach of `brush`, as low and high columns and rows.
fn reach_of(data: &TerrainData, brush: &TerrainBrush) -> [u32; 4] {
    let cell = data.cell.max(f32::EPSILON);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "kept to the sheet"
    )]
    let at = |value: f32, last: u32| {
        #[expect(clippy::cast_precision_loss, reason = "a land's size")]
        let last = last as f32;
        (value / cell).clamp(0.0, last) as u32
    };
    let [x, y] = brush.middle;
    let [across, down] = data.cells;
    [
        at(x - brush.radius, across),
        at(y - brush.radius, down),
        (at(x + brush.radius, across) + 1).min(across),
        (at(y + brush.radius, down) + 1).min(down),
    ]
}

/// The weight of `brush` at sample `x`, `y`.
#[expect(clippy::cast_precision_loss, reason = "a land's size")]
fn weight_at(data: &TerrainData, brush: &TerrainBrush, x: u32, y: u32) -> f32 {
    brush.weight(x as f32 * data.cell, y as f32 * data.cell)
}

/// Applies `stroke` to `data`.
pub fn stroke_land(data: &mut TerrainData, brush: &TerrainBrush, stroke: TerrainStroke) {
    let [low_x, low_y, high_x, high_y] = reach_of(data, brush);
    let across = data.across() as usize;
    let before = matches!(stroke, TerrainStroke::Smooth(_)).then(|| data.heights.clone());
    for y in low_y..=high_y {
        for x in low_x..=high_x {
            let weight = weight_at(data, brush, x, y);
            if weight <= 0.0 {
                continue;
            }
            let at = y as usize * across + x as usize;
            match stroke {
                TerrainStroke::Raise(amount) => data.heights[at] += amount * weight,
                TerrainStroke::Flatten(height) => {
                    let was = data.heights[at];
                    data.heights[at] = was + (height - was) * weight;
                }
                TerrainStroke::Smooth(share) => {
                    if let Some(before) = &before {
                        let round = neighbours(before, data, x, y);
                        let was = before[at];
                        data.heights[at] = was + (round - was) * share.clamp(0.0, 1.0) * weight;
                    }
                }
                TerrainStroke::Hole(hole) => {
                    if weight > 0.5 && x < data.cells[0] && y < data.cells[1] {
                        data.set_hole(x, y, hole);
                    }
                }
            }
        }
    }
}

/// The mean of the heights round sample `x`, `y`, in `heights`.
fn neighbours(heights: &[f32], data: &TerrainData, x: u32, y: u32) -> f32 {
    let across = data.across() as usize;
    let mut sum = 0.0;
    let mut count = 0.0;
    for row in y.saturating_sub(1)..=(y + 1).min(data.cells[1]) {
        for column in x.saturating_sub(1)..=(x + 1).min(data.cells[0]) {
            sum += heights[row as usize * across + column as usize];
            count += 1.0;
        }
    }
    sum / count
}

/// Presses `stamp`'s heights into the land, with its middle at `brush`'s, scaled by `strength`.
/// It's added, and fades out by the brush.
pub fn stamp_land(
    data: &mut TerrainData,
    stamp: &TerrainData,
    brush: &TerrainBrush,
    strength: f32,
) {
    let [low_x, low_y, high_x, high_y] = reach_of(data, brush);
    let across = data.across() as usize;
    let cell = data.cell;
    let [stamp_x, stamp_y] = stamp.cells;
    #[expect(clippy::cast_precision_loss, reason = "a land's size")]
    let (wide, deep) = (stamp_x as f32 * stamp.cell, stamp_y as f32 * stamp.cell);
    for y in low_y..=high_y {
        for x in low_x..=high_x {
            let weight = weight_at(data, brush, x, y);
            if weight <= 0.0 {
                continue;
            }
            #[expect(clippy::cast_precision_loss, reason = "a land's size")]
            let (place_x, place_y) = (x as f32 * cell, y as f32 * cell);
            let from_x = (place_x - brush.middle[0]) / (brush.radius * 2.0) + 0.5;
            let from_y = (place_y - brush.middle[1]) / (brush.radius * 2.0) + 0.5;
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "kept to the stamp"
            )]
            let height = stamp.height(
                (from_x.clamp(0.0, 1.0) * wide / stamp.cell) as u32,
                (from_y.clamp(0.0, 1.0) * deep / stamp.cell) as u32,
            );
            data.heights[y as usize * across + x as usize] += height * strength * weight;
        }
    }
}

/// Scatters `count` stamps of `stamp` over the land within `brush`, each a little different, by a
/// number that makes the same scatter again.
pub fn splat_land(
    data: &mut TerrainData,
    stamp: &TerrainData,
    brush: &TerrainBrush,
    (count, strength, seed): (u32, f32, u32),
) {
    let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        #[expect(clippy::cast_precision_loss, reason = "a share")]
        let share = (state >> 8) as f32 / (1_u32 << 24) as f32;
        share
    };
    for _ in 0..count {
        let (turn, reach, size, height) = (next(), next(), next(), next());
        let away = brush.radius * reach.sqrt();
        let (sine, cosine) = (turn * std::f32::consts::TAU).sin_cos();
        let one = TerrainBrush {
            middle: [
                brush.middle[0] + cosine * away,
                brush.middle[1] + sine * away,
            ],
            radius: brush.radius * (0.1 + size * 0.2),
            soft: 1.0,
        };
        stamp_land(data, stamp, &one, strength * (0.5 + height));
    }
}

/// Copies what's round `from` in `brush`'s circle to `brush`'s own place, fading into what's there.
pub fn copy_land(data: &mut TerrainData, from: [f32; 2], brush: &TerrainBrush) {
    let source = data.clone();
    let [low_x, low_y, high_x, high_y] = reach_of(data, brush);
    let across = data.across() as usize;
    let cell = data.cell.max(f32::EPSILON);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "how many cells it moves by, which is small"
    )]
    let (shift_x, shift_y) = (
        ((from[0] - brush.middle[0]) / cell).round() as i64,
        ((from[1] - brush.middle[1]) / cell).round() as i64,
    );
    for y in low_y..=high_y {
        for x in low_x..=high_x {
            let weight = weight_at(data, brush, x, y);
            if weight <= 0.0 {
                continue;
            }
            let there_x = u32::try_from((i64::from(x) + shift_x).max(0)).unwrap_or(0);
            let there_y = u32::try_from((i64::from(y) + shift_y).max(0)).unwrap_or(0);
            let at = y as usize * across + x as usize;
            let was = data.heights[at];
            data.heights[at] = was + (source.height(there_x, there_y) - was) * weight;
        }
    }
}
