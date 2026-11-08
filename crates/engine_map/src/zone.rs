//! Zones: named parts of a map that things hang from (proposal 0116).

use engine_core::math::Vec3;

use crate::map::Map;

/// A plane round part of a zone, facing out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZonePlane {
    /// Which way it faces.
    pub normal: Vec3,
    /// How far along that it is.
    pub distance: f32,
}

/// A named part of a map.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Zone {
    /// What it's called in the map.
    pub name: String,
    /// What's shown on going in, if anything.
    pub title: Option<String>,
    /// The zone it's part of, by name.
    pub parent: Option<String>,
    /// Which matters most, where zones overlap and neither is inside the other.
    pub priority: i32,
    /// The convex shapes it's made of.
    pub shapes: Vec<Vec<ZonePlane>>,
    /// How much room it takes, which says which is the inner of two.
    pub size: f32,
    /// A music track or file.
    pub music: Option<String>,
    /// A sound that goes round and round in it.
    pub sound: Option<String>,
    /// A set of light of its own, by name, for a day in it.
    pub light_set: Option<String>,
    /// How far in it takes for its set to be all there is.
    pub light_blend: f32,
    /// How much light reaches the eye in it, as a share of each colour.
    pub light: Option<[f32; 3]>,
    /// How far exposure can go by itself in it, down and up in stops, in place of the level's.
    pub exposure_range: Option<[f32; 2]>,
}

impl Zone {
    /// How far into it `point` is, from its nearest side, or `None` outside it.
    #[must_use]
    pub fn depth_of(&self, point: Vec3) -> Option<f32> {
        let depths = self.shapes.iter().map(|shape| {
            shape
                .iter()
                .map(|plane| plane.distance - plane.normal.dot(point))
                .fold(f32::INFINITY, f32::min)
        });
        let deepest = depths.fold(f32::NEG_INFINITY, f32::max);
        (deepest >= 0.0).then_some(deepest)
    }

    /// Whether `point` is in it.
    #[must_use]
    pub fn holds(&self, point: Vec3) -> bool {
        self.shapes.iter().any(|shape| {
            shape
                .iter()
                .all(|plane| plane.normal.dot(point) <= plane.distance)
        })
    }
}

impl Map {
    /// The number of the zone `point` is most in: the one that matters most, then
    /// the smallest, which is how a sub zone wins over the zone around it.
    #[must_use]
    pub fn zone_at(&self, point: Vec3) -> Option<usize> {
        self.zones
            .iter()
            .enumerate()
            .filter(|(_, zone)| zone.holds(point))
            .max_by(|(_, one), (_, other)| {
                one.priority
                    .cmp(&other.priority)
                    .then(other.size.total_cmp(&one.size))
            })
            .map(|(number, _)| number)
    }

    /// `zone`, then the zones it's part of, as far as they go.
    #[must_use]
    pub fn zone_line(&self, zone: usize) -> Vec<&Zone> {
        let mut line = Vec::new();
        let mut next = self.zones.get(zone);
        while let Some(found) = next {
            if line.len() >= self.zones.len() {
                break;
            }
            line.push(found);
            let parent = found.parent.as_deref();
            next = parent.and_then(|name| self.zones.iter().find(|zone| zone.name == name));
        }
        line
    }
}

/// What holds where someone is, with what isn't set taken from the zones round it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ZoneSettings {
    /// A music track or file.
    pub music: Option<String>,
    /// A sound that goes round and round.
    pub sound: Option<String>,
    /// How much light reaches the eye.
    pub light: Option<[f32; 3]>,
    /// How far exposure can go by itself.
    pub exposure_range: Option<[f32; 2]>,
}

impl Map {
    /// What holds in `zone`.
    #[must_use]
    pub fn zone_settings(&self, zone: usize) -> ZoneSettings {
        let mut out = ZoneSettings::default();
        for found in self.zone_line(zone) {
            out.music = out.music.or_else(|| found.music.clone());
            out.sound = out.sound.or_else(|| found.sound.clone());
            out.light = out.light.or(found.light);
            out.exposure_range = out.exposure_range.or(found.exposure_range);
        }
        out
    }
}
