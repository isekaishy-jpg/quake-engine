//! Moving lights, as a map places them (proposal 0084).

use engine_core::math::Vec3;

/// Where something moving along `path` is at `time`, going round it every
/// `period` seconds, or `still` with no path.
#[must_use]
pub fn along_path(path: &[Vec3], period: f32, time: f32, still: Vec3) -> Vec3 {
    if path.len() < 2 || period <= 0.0 {
        return still;
    }
    #[expect(clippy::cast_precision_loss, reason = "paths are short")]
    let legs = path.len() as f32;
    let along = (time / period).rem_euclid(1.0) * legs;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "within the path"
    )]
    let leg = along as usize % path.len();
    let from = path[leg];
    let to = path[(leg + 1) % path.len()];
    from + (to - from) * along.fract()
}

/// A light that's worked out each frame, not baked.
#[derive(Clone, Debug, PartialEq)]
pub struct LiveLightSource {
    /// Where it is.
    pub origin: Vec3,
    /// How far it reaches.
    pub radius: f32,
    /// Its colour, where 1 is full.
    pub color: [f32; 3],
    /// Which way a spot points, or zero for all round.
    pub spot: Vec3,
    /// The cosine of a spot's half angle.
    pub spot_cosine: f32,
    /// Whether it casts shadows.
    pub shadows: bool,
    /// Points it moves through in turn, if it moves.
    pub path: Vec<Vec3>,
    /// How many seconds a trip round `path` takes.
    pub period: f32,
}

impl LiveLightSource {
    /// Where it is at `time`.
    #[must_use]
    pub fn position(&self, time: f32) -> Vec3 {
        along_path(&self.path, self.period, time, self.origin)
    }
}
