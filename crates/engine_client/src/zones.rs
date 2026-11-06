//! Following someone from zone to zone (proposal 0116).

use engine_core::math::Vec3;
use engine_map::Map;

/// How many seconds light takes to settle on going into a zone.
const LIGHT_SECONDS: f32 = 1.5;

/// What's changed on going from one zone to another.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ZoneChange {
    /// What to show, if anything.
    pub title: Option<String>,
    /// The music to play, if that's changed.
    pub music: Option<Option<String>>,
    /// The sound to go round, if that's changed.
    pub sound: Option<Option<String>>,
}

/// Where someone is, and how far the light has got.
#[derive(Clone, Debug)]
pub struct ZoneTracker {
    /// The zone they're in.
    zone: Option<usize>,
    /// The music that's on.
    music: Option<String>,
    /// The sound that's on.
    sound: Option<String>,
    /// The light as it is.
    light: [f32; 3],
    /// Whether anything's been looked at yet.
    started: bool,
}

impl Default for ZoneTracker {
    fn default() -> Self {
        Self {
            zone: None,
            music: None,
            sound: None,
            light: [1.0; 3],
            started: false,
        }
    }
}

impl ZoneTracker {
    /// The zone they're in.
    #[must_use]
    pub const fn zone(&self) -> Option<usize> {
        self.zone
    }

    /// How much light reaches the eye, as a share of each colour.
    #[must_use]
    pub const fn light(&self) -> [f32; 3] {
        self.light
    }

    /// Follows someone to `point`, `seconds` on. Returns what's changed, if they
    /// went into another zone. Leaving a zone for none shows nothing, and music
    /// goes on until something else is asked for, as stopping sounds wrong.
    pub fn update(&mut self, map: &Map, point: Vec3, seconds: f32) -> Option<ZoneChange> {
        let zone = map.zone_at(point);
        let settings = zone.map(|zone| map.zone_settings(zone));
        let wanted = settings
            .as_ref()
            .and_then(|settings| settings.light)
            .unwrap_or([1.0; 3]);
        let share = if self.started {
            (seconds / LIGHT_SECONDS).clamp(0.0, 1.0)
        } else {
            1.0
        };
        for (light, target) in self.light.iter_mut().zip(wanted) {
            *light += (target - *light) * share;
        }
        if self.started && zone == self.zone {
            return None;
        }
        self.started = true;
        let was_in = self.zone;
        self.zone = zone;
        let settings = settings.unwrap_or_default();
        let mut change = ZoneChange::default();
        if let Some(found) = zone.and_then(|zone| map.zones.get(zone)) {
            // Coming back out of a sub zone into what's round it says nothing.
            let line = was_in.map(|was_in| map.zone_line(was_in));
            let came_from_inside =
                line.is_some_and(|line| line.iter().any(|outer| outer.name == found.name));
            if !came_from_inside {
                change.title.clone_from(&found.title);
            }
        }
        if settings.music.is_some() && settings.music != self.music {
            self.music.clone_from(&settings.music);
            change.music = Some(settings.music);
        }
        if settings.sound != self.sound {
            self.sound.clone_from(&settings.sound);
            change.sound = Some(settings.sound);
        }
        Some(change)
    }
}
