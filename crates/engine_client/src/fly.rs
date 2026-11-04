//! A free camera, until real movement arrives (proposal 0019).

use engine_core::math::Vec3;

/// Units a second at normal speed.
const FLY_SPEED: f32 = 400.0;

/// How much `+speed` multiplies it.
const RUN_FACTOR: f32 = 2.5;

/// The movement buttons held down.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Buttons {
    /// `+forward`.
    pub forward: bool,
    /// `+back`.
    pub back: bool,
    /// `+moveleft`.
    pub left: bool,
    /// `+moveright`.
    pub right: bool,
    /// `+moveup`.
    pub up: bool,
    /// `+movedown`.
    pub down: bool,
    /// `+speed`.
    pub speed: bool,
}

impl Buttons {
    /// Sets the button called `name`, without its `+` or `-`. Returns whether
    /// the name was a button.
    pub fn set(&mut self, name: &str, held: bool) -> bool {
        let button = match name {
            "forward" => &mut self.forward,
            "back" => &mut self.back,
            "moveleft" => &mut self.left,
            "moveright" => &mut self.right,
            "moveup" => &mut self.up,
            "movedown" => &mut self.down,
            "speed" => &mut self.speed,
            _ => return false,
        };
        *button = held;
        true
    }
}

/// One axis from two opposing buttons.
fn axis(positive: bool, negative: bool) -> f32 {
    f32::from(u8::from(positive)) - f32::from(u8::from(negative))
}

/// A camera that moves on ticks, and interpolates between them for drawing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlyCamera {
    /// Where it was before the last tick.
    previous: Vec3,
    /// Where it is now.
    position: Vec3,
}

impl FlyCamera {
    /// A camera at `position`.
    #[must_use]
    pub fn new(position: Vec3) -> Self {
        Self {
            previous: position,
            position,
        }
    }

    /// Moves for one tick of `seconds`, along a view of `yaw` and `pitch`.
    pub fn tick(&mut self, buttons: Buttons, yaw: f32, pitch: f32, seconds: f32) {
        let (yaw_sin, yaw_cos) = yaw.to_radians().sin_cos();
        let (pitch_sin, pitch_cos) = pitch.to_radians().sin_cos();
        let forward = Vec3::new(yaw_cos * pitch_cos, yaw_sin * pitch_cos, -pitch_sin);
        let right = Vec3::new(yaw_sin, -yaw_cos, 0.0);
        let up = Vec3::new(0.0, 0.0, 1.0);
        let wish = forward * axis(buttons.forward, buttons.back)
            + right * axis(buttons.right, buttons.left)
            + up * axis(buttons.up, buttons.down);
        let speed = if buttons.speed {
            FLY_SPEED * RUN_FACTOR
        } else {
            FLY_SPEED
        };
        self.previous = self.position;
        self.position = self.position + wish.normalized() * (speed * seconds);
    }

    /// The position to draw, `fraction` of the way through the next tick.
    #[must_use]
    pub fn view_position(&self, fraction: f32) -> Vec3 {
        self.previous.lerp(self.position, fraction)
    }

    /// Where the last tick ended.
    #[must_use]
    pub fn position(&self) -> Vec3 {
        self.position
    }
}
