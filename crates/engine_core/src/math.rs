//! Small value types for geometry.

use std::ops::{Add, Mul, Sub};

use serde::{Deserialize, Serialize};

/// A point or direction. Z is up, as in the map data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    /// Forward at yaw 0.
    pub x: f32,
    /// Left at yaw 0.
    pub y: f32,
    /// Up.
    pub z: f32,
}

/// The forward, right and up directions of a view, in degrees, from
/// `AngleVectors` with no roll.
#[must_use]
pub fn view_axes(yaw: f32, pitch: f32) -> [Vec3; 3] {
    let (yaw_sin, yaw_cos) = yaw.to_radians().sin_cos();
    let (pitch_sin, pitch_cos) = pitch.to_radians().sin_cos();
    [
        Vec3::new(yaw_cos * pitch_cos, yaw_sin * pitch_cos, -pitch_sin),
        Vec3::new(yaw_sin, -yaw_cos, 0.0),
        Vec3::new(yaw_cos * pitch_sin, yaw_sin * pitch_sin, pitch_cos),
    ]
}

impl Vec3 {
    /// The origin.
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    /// A vector from its parts.
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// The dot product.
    #[must_use]
    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// The cross product.
    #[must_use]
    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    /// The length.
    #[must_use]
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    /// The same direction with length 1, or zero if it has no length.
    #[must_use]
    pub fn normalized(self) -> Self {
        let length = self.length();
        if length > 0.0 {
            self * (1.0 / length)
        } else {
            Self::ZERO
        }
    }

    /// The point `fraction` of the way to `other`.
    #[must_use]
    pub fn lerp(self, other: Self, fraction: f32) -> Self {
        self + (other - self) * fraction
    }
}

impl Add for Vec3 {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }
}

impl Sub for Vec3 {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Self;

    fn mul(self, scale: f32) -> Self {
        Self::new(self.x * scale, self.y * scale, self.z * scale)
    }
}
