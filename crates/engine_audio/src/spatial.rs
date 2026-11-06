//! How a sound's place becomes the volume in each ear.

use engine_core::math::Vec3;

/// `sound_nominal_clip_dist`: where a sound of attenuation 1 falls silent.
const NOMINAL_CLIP_DISTANCE: f32 = 1000.0;

/// Where sounds are heard from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Listener {
    /// Where the ears are.
    pub origin: Vec3,
    /// The direction of the right ear, of length 1.
    pub right: Vec3,
}

impl Default for Listener {
    fn default() -> Self {
        Self {
            origin: Vec3::ZERO,
            right: Vec3::new(0.0, -1.0, 0.0),
        }
    }
}

/// How a kind of sound fades with distance, up to where it falls silent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fade {
    /// Falls away quickly and then slowly, as sound does in air. Halfway out it's about
    /// a quarter as loud, where a straight line would leave half.
    #[default]
    Natural,
    /// A straight line, from `SND_Spatialize`, which keeps far off weapons and the loops
    /// that mark a place in earshot.
    Straight,
}

impl Fade {
    /// The kind of fade for the sound called `name`. Gunfire is what a player has to hear
    /// from across a level.
    #[must_use]
    pub fn of(name: &str) -> Self {
        if name.starts_with("weapons/") {
            Self::Straight
        } else {
            Self::Natural
        }
    }

    /// How much is left at `share` of the way out to silence.
    #[must_use]
    pub fn at(self, share: f32) -> f32 {
        let left = (1.0 - share).max(0.0);
        match self {
            Self::Straight => left,
            Self::Natural => left * left.sqrt() / (1.0 + share.max(0.0)),
        }
    }
}

/// How a room sounds, from how far it is to its surfaces.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Room {
    /// How big it is, from 0 for a cupboard to 1 for a cathedral.
    pub size: f32,
    /// How much of it is open to the sky, from 0 to 1, which sends nothing back.
    pub open: f32,
}

/// The distance past which a room counts as big as it gets.
const BIGGEST_ROOM: f32 = 1024.0;

impl Room {
    /// The room round a listener, from how far rays go in each direction. `None` is a ray
    /// that met nothing, or the sky.
    #[must_use]
    pub fn from_rays(rays: &[Option<f32>]) -> Self {
        let hits: Vec<f32> = rays.iter().flatten().copied().collect();
        if rays.is_empty() {
            return Self::default();
        }
        #[expect(clippy::cast_precision_loss, reason = "a handful of rays")]
        let open = 1.0 - hits.len() as f32 / rays.len() as f32;
        #[expect(clippy::cast_precision_loss, reason = "a handful of rays")]
        let mean = hits.iter().sum::<f32>() / hits.len().max(1) as f32;
        Self {
            size: (mean / BIGGEST_ROOM).clamp(0.0, 1.0),
            open,
        }
    }

    /// How long the room rings, as the reverb's feedback, from 0 to 1.
    #[must_use]
    pub fn feedback(self) -> f32 {
        0.45 + 0.42 * self.size
    }

    /// How much of what's heard comes back from the room, as an amplitude. Hard walls close
    /// by give a little, a hall gives more, and the open air gives nothing.
    #[must_use]
    pub fn send(self) -> f32 {
        (0.12 + 0.28 * self.size) * (1.0 - self.open)
    }
}

/// The volume in the left and right ears, on a straight line. See [`volumes_by`].
#[must_use]
pub fn stereo_volumes(
    source: Vec3,
    listener: &Listener,
    volume: f32,
    attenuation: f32,
) -> (f32, f32) {
    volumes_by(Fade::Straight, source, listener, volume, attenuation)
}

/// The volume in the left and right ears. The range is `SND_Spatialize`'s, but where that
/// adds to the near ear, which doubles a sound beside the listener, this shares the same
/// power between the ears. So a sound is as loud whichever way the listener turns.
#[must_use]
pub fn volumes_by(
    fade: Fade,
    source: Vec3,
    listener: &Listener,
    volume: f32,
    attenuation: f32,
) -> (f32, f32) {
    let offset = source - listener.origin;
    let distance = offset.length();
    let dot = if distance > 0.0 {
        listener.right.dot(offset.normalized())
    } else {
        0.0
    };
    let left = volume * fade.at(distance * attenuation / NOMINAL_CLIP_DISTANCE);
    (
        left * (1.0 - dot).max(0.0).sqrt(),
        left * (1.0 + dot).max(0.0).sqrt(),
    )
}

/// The amplitude and pan from -1 to 1 that give these volumes through an
/// equal power panner, which is what the mixer has.
#[must_use]
pub fn amplitude_and_pan(left: f32, right: f32) -> (f32, f32) {
    let power = left * left + right * right;
    if power <= 0.0 {
        return (0.0, 0.0);
    }
    ((power / 2.0).sqrt(), 2.0 * right * right / power - 1.0)
}
