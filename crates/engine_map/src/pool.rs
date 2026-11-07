//! Bodies of liquid, for the light in them (proposal 0077).

use engine_core::math::Vec3;

use crate::tree::Contents;

/// Something pushing the water near the view down.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WaterPush {
    /// Where it is.
    pub place: Vec3,
    /// How far it reaches.
    pub reach: f32,
    /// How hard it pushes.
    pub strength: f32,
    /// Which way it's going, in radians.
    pub turn: f32,
    /// Which of the level's water pictures it pushes as, plus one, or 0 for a ring.
    pub picture: u32,
}

/// A picture over a surface, scrolling.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WaterOverlay {
    /// Which of [`crate::Map::water_pictures`] it is, plus one, or 0 for none.
    pub picture: u32,
    /// How wide it is, in world units.
    pub size: f32,
    /// How fast it goes, and which way, in world units a second.
    pub flow: [f32; 2],
    /// How much of it shows, from 0 to 1.
    pub strength: f32,
}

impl WaterOverlay {
    /// None at all.
    pub const NONE: Self = Self {
        picture: 0,
        size: 256.0,
        flow: [0.0; 2],
        strength: 1.0,
    };
}

/// How a body of liquid looks from inside.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PoolLook {
    /// The colour depth takes things towards.
    pub color: [f32; 3],
    /// How fast depth tints, per unit.
    pub absorption: f32,
    /// How strong caustics and shafts are.
    pub caustics: f32,
    /// How much its surface mirrors, at a glance.
    pub reflect: f32,
    /// How tall its waves look.
    pub waves: f32,
    /// How far its surface bends what's seen through it.
    pub refraction: f32,
    /// The colour over the view with the eye in it: red, green, blue and strength.
    pub tint: [f32; 4],
    /// How much thicker it gets with each unit of depth.
    pub deep: f32,
    /// The effect that goes with the view while the eye's in it, by its id, or 0.
    pub view_effect: u32,
    /// What's over its surface.
    pub overlays: [WaterOverlay; 2],
    /// How far from the shore foam reaches, by depth, or 0 for none.
    pub foam: f32,
    /// What foam looks like, or a noise if it says no picture.
    pub foam_look: WaterOverlay,
    /// How deep it has to get before it stops fading into what's under it, or 0 for a hard edge.
    pub shore: f32,
    /// The effect something going in starts, by its id, or 0.
    pub splash: u32,
    /// The effect something going fast through it starts, by its id, or 0.
    pub spray: u32,
    /// Which of [`crate::Map::water_sounds`] something going in makes, plus one, or 0.
    pub splash_sound: u32,
    /// What something going through it leaves, of [`crate::Map::water_pictures`], plus one, or 0
    /// for a ring.
    pub trail: u32,
    /// How far that reaches.
    pub trail_size: f32,
    /// How many rings a second rain makes on it, near the view.
    pub rain_rings: f32,
}

impl PoolLook {
    /// The usual look of `contents`, if it's a liquid.
    #[must_use]
    pub const fn of(contents: Contents) -> Option<Self> {
        match contents {
            Contents::Water => Some(Self {
                color: [0.05, 0.22, 0.28],
                absorption: 0.006,
                caustics: 0.6,
                reflect: 0.6,
                waves: 1.0,
                refraction: 1.0,
                tint: [130.0 / 255.0, 80.0 / 255.0, 50.0 / 255.0, 128.0 / 255.0],
                deep: 0.0,
                view_effect: 0,
                overlays: [WaterOverlay::NONE; 2],
                foam: 0.0,
                foam_look: WaterOverlay::NONE,
                shore: 0.0,
                splash: 0,
                spray: 0,
                splash_sound: 0,
                trail: 0,
                trail_size: 28.0,
                rain_rings: 0.0,
            }),
            Contents::Slime => Some(Self {
                color: [0.12, 0.3, 0.05],
                absorption: 0.02,
                caustics: 0.3,
                reflect: 0.25,
                waves: 0.5,
                refraction: 0.4,
                tint: [0.0, 25.0 / 255.0, 5.0 / 255.0, 150.0 / 255.0],
                deep: 0.0,
                view_effect: 0,
                overlays: [WaterOverlay::NONE; 2],
                foam: 0.0,
                foam_look: WaterOverlay::NONE,
                shore: 0.0,
                splash: 0,
                spray: 0,
                splash_sound: 0,
                trail: 0,
                trail_size: 28.0,
                rain_rings: 0.0,
            }),
            Contents::Lava => Some(Self {
                color: [0.75, 0.2, 0.03],
                absorption: 0.05,
                caustics: 0.0,
                reflect: 0.0,
                waves: 0.0,
                refraction: 0.0,
                tint: [1.0, 80.0 / 255.0, 0.0, 150.0 / 255.0],
                deep: 0.0,
                view_effect: 0,
                overlays: [WaterOverlay::NONE; 2],
                foam: 0.0,
                foam_look: WaterOverlay::NONE,
                shore: 0.0,
                splash: 0,
                spray: 0,
                splash_sound: 0,
                trail: 0,
                trail_size: 28.0,
                rain_rings: 0.0,
            }),
            _ => None,
        }
    }
}

/// A body of liquid, as a box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pool {
    /// Its lowest corner.
    pub low: Vec3,
    /// Its highest corner.
    pub high: Vec3,
    /// How it looks.
    pub look: PoolLook,
}

/// The look of what's around `eye`, which is in `contents`: the pool it's in, or failing that
/// what such a liquid usually looks like, as original levels have no pools.
#[must_use]
pub fn look_around(pools: &[Pool], eye: Vec3, contents: Contents) -> Option<PoolLook> {
    let inside = |pool: &&Pool| {
        (pool.low.x..=pool.high.x).contains(&eye.x)
            && (pool.low.y..=pool.high.y).contains(&eye.y)
            && (pool.low.z..=pool.high.z).contains(&eye.z)
    };
    let liquid = match contents {
        // Quake colours the view the same inside a sky.
        Contents::Sky => Contents::Water,
        other => other,
    };
    let usual = PoolLook::of(liquid)?;
    Some(pools.iter().find(inside).map_or(usual, |pool| pool.look))
}

/// What a sample of a [`WaterSheet`] holds where there's no water.
pub const NO_WATER: f32 = f32::MIN;

/// A body of liquid of any shape: a grid of heights, and what's under it. It can't overhang.
#[derive(Clone, Debug, PartialEq)]
pub struct WaterSheet {
    /// Where its first sample is.
    pub origin: [f32; 2],
    /// How far apart its samples are.
    pub cell: f32,
    /// How many samples it has each way.
    pub size: [u32; 2],
    /// Each sample's height, row by row, or [`NO_WATER`].
    pub heights: Vec<f32>,
    /// How far down it goes from its surface.
    pub depth: f32,
    /// What it is.
    pub contents: Contents,
    /// How it looks.
    pub look: PoolLook,
}

impl WaterSheet {
    /// The height of sample `at`, if there's water there.
    fn sample(&self, at: [u32; 2]) -> Option<f32> {
        if at[0] >= self.size[0] || at[1] >= self.size[1] {
            return None;
        }
        let number = at[1] as usize * self.size[0] as usize + at[0] as usize;
        self.heights.get(number).copied().filter(|h| *h > NO_WATER)
    }

    /// How high its surface is at `x` and `y`, if it's there. Where only some of a cell has water,
    /// it goes by those that have.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "held to the grid"
    )]
    pub fn height_at(&self, x: f32, y: f32) -> Option<f32> {
        let across = (x - self.origin[0]) / self.cell;
        let down = (y - self.origin[1]) / self.cell;
        if across < 0.0 || down < 0.0 {
            return None;
        }
        let (first, second) = (across.floor(), down.floor());
        let (part_x, part_y) = (across - first, down - second);
        let at = [first as u32, second as u32];
        let mut sum = 0.0;
        let mut weight = 0.0;
        for (dx, dy, share) in [
            (0, 0, (1.0 - part_x) * (1.0 - part_y)),
            (1, 0, part_x * (1.0 - part_y)),
            (0, 1, (1.0 - part_x) * part_y),
            (1, 1, part_x * part_y),
        ] {
            if let Some(height) = self.sample([at[0] + dx, at[1] + dy]) {
                sum += height * share;
                weight += share;
            }
        }
        (weight > 0.001).then(|| sum / weight)
    }

    /// Whether `point` is in it.
    #[must_use]
    pub fn holds(&self, point: Vec3) -> bool {
        self.height_at(point.x, point.y)
            .is_some_and(|top| point.z <= top && point.z >= top - self.depth)
    }

    /// Its lowest and highest corners.
    #[must_use]
    #[expect(clippy::cast_precision_loss, reason = "a grid is well below 2^24")]
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let there = self.heights.iter().copied().filter(|h| *h > NO_WATER);
        let (low, high) = there.fold((f32::MAX, f32::MIN), |(low, high), height| {
            (low.min(height), high.max(height))
        });
        let far = [
            self.origin[0] + self.size[0].saturating_sub(1) as f32 * self.cell,
            self.origin[1] + self.size[1].saturating_sub(1) as f32 * self.cell,
        ];
        (
            Vec3::new(self.origin[0], self.origin[1], low - self.depth),
            Vec3::new(far[0], far[1], high),
        )
    }

    /// Its surface as triangles, three corners each. A cell needs water at all four corners.
    #[must_use]
    #[expect(clippy::cast_precision_loss, reason = "a grid is well below 2^24")]
    pub fn triangles(&self) -> Vec<Vec3> {
        let mut out = Vec::new();
        let place = |x: u32, y: u32, height: f32| {
            Vec3::new(
                self.origin[0] + x as f32 * self.cell,
                self.origin[1] + y as f32 * self.cell,
                height,
            )
        };
        for y in 0..self.size[1].saturating_sub(1) {
            for x in 0..self.size[0].saturating_sub(1) {
                let corners = [[x, y], [x + 1, y], [x + 1, y + 1], [x, y + 1]];
                let [Some(a), Some(b), Some(c), Some(d)] = corners.map(|at| self.sample(at)) else {
                    continue;
                };
                let (a, b) = (place(x, y, a), place(x + 1, y, b));
                let (c, d) = (place(x + 1, y + 1, c), place(x, y + 1, d));
                out.extend([a, b, c, a, c, d]);
            }
        }
        out
    }
}
