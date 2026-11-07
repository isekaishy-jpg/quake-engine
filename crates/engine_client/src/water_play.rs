//! What going through water does to it: rings, wakes, splashes and spray (proposal 0147).

use std::collections::HashMap;

use engine_core::math::Vec3;
use engine_map::{Map, WaterPush};

/// How far over a surface something can be and still be touching it.
const TOUCH_OVER: f32 = 24.0;

/// How far under a surface it can be and still be touching it.
const TOUCH_UNDER: f32 = 56.0;

/// How fast something has to go in to splash, in units a second.
const SPLASH_SPEED: f32 = 60.0;

/// How fast it has to go to leave a wake.
const WAKE_SPEED: f32 = 40.0;

/// How fast it has to go through to throw up spray.
const SPRAY_SPEED: f32 = 280.0;

/// How long between sprays from one thing, in seconds.
const SPRAY_GAP: f32 = 0.15;

/// How hard a wake pushes, at its strongest.
const WAKE_STRENGTH: f32 = 0.12;

/// How many frames a second a wake's full strength is for.
const WAKE_STEPS: f32 = 60.0;

/// The speed at which a wake is as strong as it gets.
const FULL_WAKE: f32 = 320.0;

/// How far from the view rain's rings go.
const RAIN_REACH: f32 = 440.0;

/// How far from where it's looked for a surface can be, for rain.
const RAIN_DEPTH: f32 = 4000.0;

/// How far from a surface something that hits it can be.
const IMPACT_REACH: f32 = 16.0;

/// The most pushes a frame has.
const MOST_PUSHES: usize = 64;

/// What going through water came to this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WaterEvents {
    /// What pushes the water.
    pub pushes: Vec<WaterPush>,
    /// The effects it starts, by id, and where.
    pub effects: Vec<(u32, Vec3)>,
    /// The sounds it makes, by which of the map's, and where.
    pub sounds: Vec<(usize, Vec3)>,
}

/// What's known of something in or near water.
#[derive(Clone, Copy, Debug)]
struct Seen {
    /// Where it was.
    place: Vec3,
    /// Whether it was touching a surface.
    touching: bool,
    /// How long until it can throw up spray again.
    spray_wait: f32,
}

/// Follows what moves through water.
#[derive(Clone, Debug)]
pub struct WaterPlay {
    /// What's been seen, by entity.
    seen: HashMap<u64, Seen>,
    /// Rain's rings that haven't fallen, as parts of one.
    rain_owed: f32,
    /// What makes each ring land somewhere else.
    seed: u32,
}

impl Default for WaterPlay {
    fn default() -> Self {
        Self {
            seen: HashMap::new(),
            rain_owed: 0.0,
            seed: 0x2545_f491,
        }
    }
}

impl WaterPlay {
    /// A number from 0 to 1.
    #[expect(clippy::cast_precision_loss, reason = "it's only a share")]
    fn random(&mut self) -> f32 {
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / 16_777_216.0
    }

    /// Goes on by `seconds`, with `movers` as each thing's id and where it is, `impacts` as what's
    /// just hit something, and the view at `eye`.
    pub fn follow(
        &mut self,
        map: &Map,
        movers: &[(u64, Vec3)],
        impacts: &[Vec3],
        (eye, seconds): (Vec3, f32),
    ) -> WaterEvents {
        let mut out = WaterEvents::default();
        if map.pools.is_empty() && map.water_sheets.is_empty() {
            return out;
        }
        let was = std::mem::take(&mut self.seen);
        for (id, place) in movers {
            let seen = Self::follow_one(map, *place, was.get(id).copied(), seconds, &mut out);
            let _previous = self.seen.insert(*id, seen);
        }
        for impact in impacts {
            if let Some((top, _)) = map.surface_near(*impact, IMPACT_REACH) {
                out.pushes.push(WaterPush {
                    place: Vec3::new(impact.x, impact.y, top),
                    reach: 14.0,
                    strength: 1.5,
                    ..WaterPush::default()
                });
            }
        }
        self.rain(map, eye, seconds, &mut out);
        out.pushes.truncate(MOST_PUSHES);
        out
    }

    /// What something at `place` does, having been as `was`.
    fn follow_one(
        map: &Map,
        place: Vec3,
        was: Option<Seen>,
        seconds: f32,
        out: &mut WaterEvents,
    ) -> Seen {
        let surface = map.surface_near(place, TOUCH_UNDER);
        let surface = surface.filter(|(top, _)| place.z - top <= TOUCH_OVER);
        let spray_wait = was.map_or(0.0, |seen| (seen.spray_wait - seconds).max(0.0));
        let mut seen = Seen {
            place,
            touching: surface.is_some(),
            spray_wait,
        };
        let (Some((top, look)), Some(before)) = (surface, was) else {
            return seen;
        };
        if seconds <= 0.0 {
            return seen;
        }
        let moved = (place - before.place) * (1.0 / seconds);
        let at = Vec3::new(place.x, place.y, top);
        let speed = moved.length();
        if !before.touching && speed > SPLASH_SPEED {
            out.pushes.push(WaterPush {
                place: at,
                reach: 40.0,
                strength: 3.0,
                ..WaterPush::default()
            });
            if look.splash != 0 {
                out.effects.push((look.splash, at));
            }
            if let Some(sound) = (look.splash_sound as usize).checked_sub(1) {
                out.sounds.push((sound, at));
            }
        }
        let flat = moved.x.hypot(moved.y);
        if flat > WAKE_SPEED {
            out.pushes.push(WaterPush {
                place: at,
                reach: look.trail_size,
                // By how long this frame is, so that frame rate doesn't make a wake stronger.
                strength: (flat / FULL_WAKE).min(1.0)
                    * WAKE_STRENGTH
                    * (seconds * WAKE_STEPS).min(1.0),
                turn: moved.y.atan2(moved.x),
                picture: look.trail,
            });
        }
        if flat > SPRAY_SPEED && look.spray != 0 && spray_wait <= 0.0 {
            out.effects.push((look.spray, at));
            seen.spray_wait = SPRAY_GAP;
        }
        seen
    }

    /// Rain's rings, on whatever's near the view that has rain.
    fn rain(&mut self, map: &Map, eye: Vec3, seconds: f32, out: &mut WaterEvents) {
        let pools = map.pools.iter().map(|pool| pool.look.rain_rings);
        let sheets = map.water_sheets.iter().map(|sheet| sheet.look.rain_rings);
        let rate = pools.chain(sheets).fold(0.0, f32::max);
        if rate <= 0.0 {
            return;
        }
        self.rain_owed += rate * seconds;
        while self.rain_owed >= 1.0 {
            self.rain_owed -= 1.0;
            let x = eye.x + (self.random() * 2.0 - 1.0) * RAIN_REACH;
            let y = eye.y + (self.random() * 2.0 - 1.0) * RAIN_REACH;
            let found = map.surface_near(Vec3::new(x, y, eye.z), RAIN_DEPTH);
            if let Some((top, look)) = found
                && self.random() * rate < look.rain_rings
            {
                out.pushes.push(WaterPush {
                    place: Vec3::new(x, y, top),
                    reach: 7.0,
                    strength: 0.5,
                    ..WaterPush::default()
                });
            }
        }
    }
}
