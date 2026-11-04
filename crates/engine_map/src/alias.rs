//! Animated models, knowing nothing about any file format.

use engine_core::math::Vec3;

/// A model's vertex in one pose.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AliasVertex {
    /// A position, which [`AliasModel::scale`] and `offset` place.
    pub position: [u8; 3],
    /// A direction of length 127.
    pub normal: [i8; 3],
}

/// One corner of a triangle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AliasCorner {
    /// Which vertex it is.
    pub vertex: u32,
    /// Where it is on the skin, from 0 to 1.
    pub uv: [f32; 2],
}

/// A frame: one pose, or a run of them that cycles by itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AliasFrame {
    /// The first pose in [`AliasModel::poses`].
    pub first_pose: u32,
    /// How many poses it has.
    pub pose_count: u32,
    /// When each pose ends, in seconds from the start of the cycle.
    pub pose_ends: Vec<f32>,
}

impl AliasFrame {
    /// The pose to show `time` seconds in.
    #[must_use]
    pub fn pose_at(&self, time: f32) -> u32 {
        let Some(&cycle) = self.pose_ends.last().filter(|cycle| **cycle > 0.0) else {
            return self.first_pose;
        };
        let into = time.rem_euclid(cycle);
        let step = self
            .pose_ends
            .iter()
            .take_while(|end| **end <= into)
            .count();
        self.first_pose + u32::try_from(step).unwrap_or(0).min(self.pose_count - 1)
    }
}

/// A model that animates by moving its vertices.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AliasModel {
    /// What a vertex's position is multiplied by.
    pub scale: Vec3,
    /// What's then added.
    pub offset: Vec3,
    /// The skins' width in pixels.
    pub skin_width: u32,
    /// The skins' height in pixels.
    pub skin_height: u32,
    /// Each skin in RGBA, where alpha zero is fullbright.
    pub skins: Vec<Vec<u8>>,
    /// Three corners to a triangle.
    pub corners: Vec<AliasCorner>,
    /// Every pose, each with one vertex for each of the model's vertices.
    pub poses: Vec<Vec<AliasVertex>>,
    /// The frames a game asks for by number.
    pub frames: Vec<AliasFrame>,
    /// Whether it spins, as bonus items do.
    pub spins: bool,
    /// The trail it leaves, by the original's flag bits.
    pub flags: u32,
}
