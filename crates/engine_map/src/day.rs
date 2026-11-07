//! A day: where the sun is at an hour, and sets of light keyed through it (proposal 0148).

use engine_core::math::Vec3;

use crate::map::Map;
use crate::sky::{RISING, Sky};
use crate::zone::Zone;

/// Hours in a day.
pub const DAY_HOURS: f32 = 24.0;

/// The hour the sun comes up, and the one it goes down.
const SUNRISE: f32 = 6.0;
const SUNSET: f32 = 18.0;

/// How many angles along its arc a sun keeps each sample's shadow at: a bit each, in one number.
pub const ARC_ANGLES: usize = 32;

/// How far off its arc light can come from and still count as on it, as the sine of the angle.
const ARC_SLACK: f32 = 0.05;

/// The hours a sun's shadows are baked through, which it moves along with no new bake
/// (proposal 0148). The moon is always across the sky from the sun, so it goes along the same
/// arc, twelve hours on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SunArc {
    /// The hour it starts at.
    pub from: f32,
    /// The hour it ends at.
    pub to: f32,
}

impl Default for SunArc {
    /// All of a day, from sunrise to sunset.
    fn default() -> Self {
        Self {
            from: SUNRISE,
            to: SUNSET,
        }
    }
}

impl SunArc {
    /// The hour at `place` along it, from 0 at its start to one less than [`ARC_ANGLES`] at its
    /// end.
    #[must_use]
    #[expect(clippy::cast_precision_loss, reason = "a few angles")]
    pub fn hour_at(self, place: f32) -> f32 {
        self.from + (self.to - self.from) * place / (ARC_ANGLES - 1) as f32
    }
}

/// How a level's day goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DayCycle {
    /// The hour a level starts at.
    pub start: f32,
    /// How many seconds a whole day takes, with 0 for a day that stands still.
    pub length: f32,
    /// Which way the sun comes up, in degrees round from `+x`.
    pub heading: f32,
    /// How high the sun gets at noon, in degrees.
    pub height: f32,
}

impl Default for DayCycle {
    fn default() -> Self {
        Self {
            start: 12.0,
            length: 0.0,
            heading: 0.0,
            height: 60.0,
        }
    }
}

impl DayCycle {
    /// The hour, `seconds` after it was `hour`.
    #[must_use]
    pub fn hour_after(&self, hour: f32, seconds: f64) -> f32 {
        if self.length <= 0.0 {
            return hour.rem_euclid(DAY_HOURS);
        }
        #[expect(clippy::cast_possible_truncation, reason = "a day's worth of hours")]
        let gone = (seconds * f64::from(DAY_HOURS) / f64::from(self.length)) as f32;
        (hour + gone).rem_euclid(DAY_HOURS)
    }

    /// Where along `arc` light from `toward` comes from, as [`SunArc::hour_at`] takes it, or
    /// `None` if that's off it. Light that's below the horizon is off it.
    #[must_use]
    #[expect(clippy::cast_precision_loss, reason = "a few angles")]
    pub fn arc_place(&self, arc: SunArc, toward: Vec3) -> Option<f32> {
        let toward = toward.normalized();
        let rising = self.sun_toward(SUNRISE);
        let noon = self.sun_toward((SUNRISE + SUNSET) * 0.5);
        let across = rising.cross(noon);
        if toward.dot(across).abs() > ARC_SLACK {
            return None;
        }
        let turned = toward.dot(noon).atan2(toward.dot(rising));
        if turned < 0.0 {
            return None;
        }
        let hour = SUNRISE + turned / std::f32::consts::PI * (SUNSET - SUNRISE);
        let span = arc.to - arc.from;
        if span.abs() < 1e-3 {
            return None;
        }
        let last = (ARC_ANGLES - 1) as f32;
        let place = (hour - arc.from) / span * last;
        (-0.5..=last + 0.5)
            .contains(&place)
            .then(|| place.clamp(0.0, last))
    }

    /// Towards the sun at `hour`. It comes up at six, is highest at noon, goes down at six, and is
    /// under the ground through the night, which is when the moon's up.
    #[must_use]
    pub fn sun_toward(&self, hour: f32) -> Vec3 {
        let turned = (hour - SUNRISE) / (SUNSET - SUNRISE) * std::f32::consts::PI;
        let (rise, across) = turned.sin_cos();
        let (lift, lean) = self.height.to_radians().sin_cos();
        let flat = Vec3::new(across, -rise * lean, rise * lift);
        let (sin, cos) = self.heading.to_radians().sin_cos();
        Vec3::new(
            flat.x * cos - flat.y * sin,
            flat.x * sin + flat.y * cos,
            flat.z,
        )
    }
}

/// What a set of light says at one hour. What it leaves out comes from the keys that have it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LightKey {
    /// The hour.
    pub hour: f32,
    /// The sky overhead.
    pub zenith: Option<[f32; 3]>,
    /// The sky at the horizon.
    pub horizon: Option<[f32; 3]>,
    /// The sky below the horizon.
    pub ground: Option<[f32; 3]>,
    /// The sun's colour and light.
    pub sun: Option<[f32; 3]>,
    /// The moon's colour and light.
    pub moon: Option<[f32; 3]>,
    /// What the clouds' colours are multiplied by.
    pub clouds: Option<[f32; 3]>,
    /// Fog's colour.
    pub fog: Option<[f32; 3]>,
    /// What water's colour is multiplied by.
    pub water: Option<[f32; 3]>,
    /// Light added everywhere, on the scale of a sample of baked light.
    pub ambient: Option<[f32; 3]>,
    /// What highlights are multiplied by.
    pub shine: Option<[f32; 3]>,
}

/// A day's light: keys through it, blended between, going round.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LightSet {
    /// What it's called.
    pub name: String,
    /// Its keys, in order of hour.
    pub keys: Vec<LightKey>,
}

/// A day, now: how it looks, where the sun is, the sky that makes, and what the sun's baked light
/// is multiplied by.
#[derive(Clone, Debug, PartialEq)]
pub struct DayState {
    /// How it looks.
    pub look: DayLook,
    /// Towards the sun.
    pub toward: Vec3,
    /// The sky as it is.
    pub sky: Sky,
    /// What the sun's baked light is multiplied by, which goes down with it.
    pub sun_tint: [f32; 3],
}

impl Map {
    /// A day in this level at `hour`, seen from `eye`, with the sun towards `toward` if something's
    /// put it there. The level's set of light, or one made from its sky, with the set of each zone
    /// the eye is in blended over it by how far in, the one round the others first. `None` with no
    /// day, or no sky.
    #[must_use]
    pub fn day_at(&self, hour: f32, eye: Vec3, toward: Option<Vec3>) -> Option<DayState> {
        let (day, sky) = (self.day?, self.sky.as_ref()?);
        let toward = toward.unwrap_or_else(|| day.sun_toward(hour)).normalized();
        let named = |name: &str| self.light_sets.iter().find(|set| set.name == name);
        let made;
        let set = if let Some(set) = self.light_set.as_deref().and_then(named) {
            set
        } else {
            made = LightSet::from_sky(sky);
            &made
        };
        let mut look = set.at(hour, sky);
        let mut zones: Vec<&Zone> = self
            .zones
            .iter()
            .filter(|zone| zone.light_set.is_some())
            .collect();
        zones.sort_by(|a, b| b.size.total_cmp(&a.size));
        for zone in zones {
            let (Some(theirs), Some(depth)) = (
                zone.light_set.as_deref().and_then(named),
                zone.depth_of(eye),
            ) else {
                continue;
            };
            let share = (depth / zone.light_blend.max(1.0)).clamp(0.0, 1.0);
            look = look.towards(&theirs.at(hour, sky), share);
        }
        let up = (toward.z / RISING).clamp(0.0, 1.0);
        let sun_tint = std::array::from_fn(|part| {
            let baked = sky.sun_color[part];
            if baked > 0.0 {
                look.sun[part] / baked * up
            } else {
                0.0
            }
        });
        Some(DayState {
            sky: look.on(sky, toward),
            look,
            toward,
            sun_tint,
        })
    }
}

/// The light of a day at an hour, where all of it is known.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DayLook {
    /// The sky overhead.
    pub zenith: [f32; 3],
    /// The sky at the horizon.
    pub horizon: [f32; 3],
    /// The sky below the horizon.
    pub ground: [f32; 3],
    /// The sun's colour and light.
    pub sun: [f32; 3],
    /// The moon's colour and light.
    pub moon: [f32; 3],
    /// What the clouds' colours are multiplied by.
    pub clouds: [f32; 3],
    /// Fog's colour, if there's anything to say.
    pub fog: Option<[f32; 3]>,
    /// What water's colour is multiplied by.
    pub water: [f32; 3],
    /// Light added everywhere.
    pub ambient: [f32; 3],
    /// What highlights are multiplied by.
    pub shine: [f32; 3],
}

impl DayLook {
    /// What a sky says of itself, as at noon, with nothing changed.
    #[must_use]
    pub fn of(sky: &Sky) -> Self {
        Self {
            zenith: sky.zenith,
            horizon: sky.horizon,
            ground: sky.ground,
            sun: sky.sun_color,
            moon: sky.moon_color,
            clouds: [1.0; 3],
            fog: None,
            water: [1.0; 3],
            ambient: [0.0; 3],
            shine: [1.0; 3],
        }
    }

    /// `self` blended `share` of the way to `other`.
    #[must_use]
    pub fn towards(&self, other: &Self, share: f32) -> Self {
        let mix =
            |a: [f32; 3], b: [f32; 3]| std::array::from_fn(|at| a[at] + (b[at] - a[at]) * share);
        Self {
            zenith: mix(self.zenith, other.zenith),
            horizon: mix(self.horizon, other.horizon),
            ground: mix(self.ground, other.ground),
            sun: mix(self.sun, other.sun),
            moon: mix(self.moon, other.moon),
            clouds: mix(self.clouds, other.clouds),
            fog: match (self.fog, other.fog) {
                (Some(a), Some(b)) => Some(mix(a, b)),
                (a, b) => a.or(b),
            },
            water: mix(self.water, other.water),
            ambient: mix(self.ambient, other.ambient),
            shine: mix(self.shine, other.shine),
        }
    }

    /// `sky` as this says it is now.
    #[must_use]
    pub fn on(&self, sky: &Sky, toward: Vec3) -> Sky {
        let mut now = sky.clone();
        now.zenith = self.zenith;
        now.horizon = self.horizon;
        now.ground = self.ground;
        now.sun_color = self.sun;
        now.moon_color = self.moon;
        now.sun_direction = toward;
        for layer in &mut now.clouds {
            layer.color = std::array::from_fn(|at| layer.color[at] * self.clouds[at]);
        }
        now
    }
}

impl LightSet {
    /// A day made from a sky, for a level with a day and no set of its own: the sky as it is at
    /// noon, warm low down at dawn and dusk, and dark at night.
    #[must_use]
    pub fn from_sky(sky: &Sky) -> Self {
        let warm = [1.0, 0.55, 0.3];
        let times = |color: [f32; 3], by: [f32; 3]| std::array::from_fn(|at| color[at] * by[at]);
        let dawn = LightKey {
            hour: SUNRISE,
            zenith: Some(times(sky.zenith, [0.6, 0.55, 0.7])),
            horizon: Some(times(warm, [1.0, 0.9, 0.85])),
            ground: Some(times(sky.ground, [0.5; 3])),
            sun: Some(times(sky.sun_color, warm)),
            clouds: Some([1.0, 0.7, 0.55]),
            ..LightKey::default()
        };
        let noon = LightKey {
            hour: 12.0,
            zenith: Some(sky.zenith),
            horizon: Some(sky.horizon),
            ground: Some(sky.ground),
            sun: Some(sky.sun_color),
            clouds: Some([1.0; 3]),
            ..LightKey::default()
        };
        let night = LightKey {
            hour: 0.0,
            zenith: Some([0.01, 0.015, 0.04]),
            horizon: Some([0.03, 0.04, 0.08]),
            ground: Some([0.01, 0.01, 0.02]),
            sun: Some([0.0; 3]),
            clouds: Some([0.12, 0.13, 0.18]),
            ..LightKey::default()
        };
        Self {
            name: String::new(),
            keys: vec![
                night,
                dawn,
                LightKey {
                    hour: SUNRISE + 1.5,
                    ..noon
                },
                noon,
                LightKey {
                    hour: SUNSET - 1.5,
                    ..noon
                },
                LightKey {
                    hour: SUNSET,
                    ..dawn
                },
            ],
        }
    }

    /// What the set says at `hour`, with `sky` for what no key says.
    #[must_use]
    pub fn at(&self, hour: f32, sky: &Sky) -> DayLook {
        let base = DayLook::of(sky);
        let hour = hour.rem_euclid(DAY_HOURS);
        DayLook {
            zenith: self.field(hour, |key| key.zenith).unwrap_or(base.zenith),
            horizon: self.field(hour, |key| key.horizon).unwrap_or(base.horizon),
            ground: self.field(hour, |key| key.ground).unwrap_or(base.ground),
            sun: self.field(hour, |key| key.sun).unwrap_or(base.sun),
            moon: self.field(hour, |key| key.moon).unwrap_or(base.moon),
            clouds: self.field(hour, |key| key.clouds).unwrap_or(base.clouds),
            fog: self.field(hour, |key| key.fog),
            water: self.field(hour, |key| key.water).unwrap_or(base.water),
            ambient: self.field(hour, |key| key.ambient).unwrap_or(base.ambient),
            shine: self.field(hour, |key| key.shine).unwrap_or(base.shine),
        }
    }

    /// One of the set's values at `hour`, between the keys before and after that have it, going
    /// round midnight. `None` if no key has it.
    fn field(&self, hour: f32, value: impl Fn(&LightKey) -> Option<[f32; 3]>) -> Option<[f32; 3]> {
        let known: Vec<(f32, [f32; 3])> = self
            .keys
            .iter()
            .filter_map(|key| Some((key.hour.rem_euclid(DAY_HOURS), value(key)?)))
            .collect();
        let after = known
            .iter()
            .filter(|(at, _)| *at >= hour)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .or_else(|| known.iter().min_by(|a, b| a.0.total_cmp(&b.0)))?;
        let before = known
            .iter()
            .filter(|(at, _)| *at <= hour)
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .or_else(|| known.iter().max_by(|a, b| a.0.total_cmp(&b.0)))?;
        let span = (after.0 - before.0).rem_euclid(DAY_HOURS);
        if span <= f32::EPSILON {
            return Some(before.1);
        }
        let share = (hour - before.0).rem_euclid(DAY_HOURS) / span;
        Some(std::array::from_fn(|at| {
            before.1[at] + (after.1[at] - before.1[at]) * share
        }))
    }
}
