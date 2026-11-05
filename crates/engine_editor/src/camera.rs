//! The free camera, and turning between the world and the screen.

use engine_core::math::{Vec3, view_axes};

/// The horizontal field of view, at 4:3, in degrees.
pub const FOV: f32 = 90.0;

/// Units a second the camera flies.
const FLY_SPEED: f32 = 640.0;

/// Degrees turned per count of mouse movement.
const LOOK_SPEED: f32 = 0.15;

/// The free camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Where it is.
    pub origin: Vec3,
    /// Its yaw in degrees.
    pub yaw: f32,
    /// Its pitch in degrees. Positive looks down.
    pub pitch: f32,
}

/// Which movement keys are held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Keys {
    /// Forward.
    pub forward: bool,
    /// Back.
    pub back: bool,
    /// Left.
    pub left: bool,
    /// Right.
    pub right: bool,
    /// Up.
    pub up: bool,
    /// Down.
    pub down: bool,
    /// Faster.
    pub fast: bool,
}

impl Camera {
    /// Turns by mouse counts.
    pub fn look(&mut self, x: f64, y: f64) {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a look needs no more than f32"
        )]
        let (x, y) = (x as f32, y as f32);
        self.yaw -= x * LOOK_SPEED;
        self.pitch = (self.pitch + y * LOOK_SPEED).clamp(-89.0, 89.0);
    }

    /// Moves for `seconds` with `keys` held.
    pub fn fly(&mut self, keys: Keys, seconds: f32) {
        let [forward, right, _] = view_axes(self.yaw, self.pitch);
        let axis = |plus: bool, minus: bool| f32::from(u8::from(plus)) - f32::from(u8::from(minus));
        let along = forward * axis(keys.forward, keys.back)
            + right * axis(keys.right, keys.left)
            + Vec3::new(0.0, 0.0, axis(keys.up, keys.down));
        let speed = if keys.fast {
            FLY_SPEED * 3.0
        } else {
            FLY_SPEED
        };
        self.origin = self.origin + along * (speed * seconds);
    }

    /// The tangents of half the view's width and height, as the renderer's
    /// projection has them.
    fn half_view(size: (f32, f32)) -> (f32, f32) {
        let aspect = size.0 / size.1.max(1.0);
        let half = (FOV * 0.5).to_radians().tan();
        // The field of view is given at 4:3 and widens with the window.
        let high = half * 0.75;
        (high * aspect, high)
    }

    /// The direction through a pixel.
    #[must_use]
    pub fn ray(&self, pixel: (f32, f32), size: (f32, f32)) -> Vec3 {
        let [forward, right, up] = view_axes(self.yaw, self.pitch);
        let (wide, high) = Self::half_view(size);
        let x = (pixel.0 / size.0 * 2.0 - 1.0) * wide;
        let y = (1.0 - pixel.1 / size.1 * 2.0) * high;
        (forward + right * x + up * y).normalized()
    }

    /// Where `point` shows on screen, if it's in front of the camera.
    #[must_use]
    pub fn project(&self, point: Vec3, size: (f32, f32)) -> Option<(f32, f32)> {
        let [forward, right, up] = view_axes(self.yaw, self.pitch);
        let offset = point - self.origin;
        let depth = offset.dot(forward);
        if depth < 1.0 {
            return None;
        }
        let (wide, high) = Self::half_view(size);
        let x = offset.dot(right) / depth / wide;
        let y = offset.dot(up) / depth / high;
        Some(((x + 1.0) * 0.5 * size.0, (1.0 - y) * 0.5 * size.1))
    }
}
