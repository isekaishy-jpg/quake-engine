//! Sounds as the game describes them, on their way from scripts to the mixer.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::math::Vec3;

/// A sound starting on an entity's channel.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundStart {
    /// The entity it comes from.
    pub entity: u64,
    /// The entity's channel, up to 7. A sound replaces another on the same
    /// channel of the same entity, except on channel 0.
    pub channel: u8,
    /// Where it comes from.
    pub origin: Vec3,
    /// From 0 to 1.
    pub volume: f32,
    /// How fast it fades with distance, from 0 for not at all up to 4.
    pub attenuation: f32,
    /// Whether it comes from the listener's own entity.
    pub own: bool,
    /// The sound's name, as `misc/talk.wav`.
    pub name: String,
}

/// A sound that loops at a fixed place for the whole level.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StaticSound {
    /// Where it comes from.
    pub origin: Vec3,
    /// From 0 to 1.
    pub volume: f32,
    /// How fast it fades with distance.
    pub attenuation: f32,
    /// The sound's name.
    pub name: String,
}

/// Decoded mono audio.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    /// Samples a second.
    pub sample_rate: u32,
    /// Samples from -1 to 1.
    pub samples: Arc<[f32]>,
    /// The sample a loop goes back to, if it loops.
    pub loop_start: Option<usize>,
}
