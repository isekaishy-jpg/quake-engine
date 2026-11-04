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

/// The volume in the left and right ears. The fade with distance is
/// `SND_Spatialize`'s, but where that adds to the near ear, which doubles a
/// sound beside the listener, this shares the same power between the ears. So
/// a sound is as loud whichever way the listener turns.
#[must_use]
pub fn stereo_volumes(
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
    let fade = volume * (1.0 - distance * attenuation / NOMINAL_CLIP_DISTANCE);
    let fade = fade.max(0.0);
    (
        fade * (1.0 - dot).max(0.0).sqrt(),
        fade * (1.0 + dot).max(0.0).sqrt(),
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
