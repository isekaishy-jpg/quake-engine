//! Where the eye goes.

pub use engine_core::player::VIEW_HEIGHT;

/// How fast the view catches up after a stair, in units a second.
const STEP_SPEED: f32 = 80.0;

/// The furthest the view falls behind on a stair.
const MAX_STEP_LAG: f32 = 12.0;

/// `cl_bob`: how far speed moves the view.
const BOB: f32 = 0.02;

/// `cl_bobcycle`: how long a bob takes, in seconds.
const BOB_CYCLE: f32 = 0.6;

/// `cl_bobup`: how much of a cycle goes up.
const BOB_UP: f32 = 0.5;

/// `cl_rollspeed`: the sideways speed of the most roll.
const ROLL_SPEED: f32 = 200.0;

/// `cl_rollangle`: the most roll from moving, in degrees.
const ROLL_ANGLE: f32 = 2.0;

/// `v_kicktime`: how long a hit knocks the view, in seconds.
const KICK_SECONDS: f32 = 0.5;

/// `v_kickroll`.
const KICK_ROLL: f32 = 0.6;

/// `v_kickpitch`.
const KICK_PITCH: f32 = 0.6;

/// The roll of a dead player's view.
const DEAD_ROLL: f32 = 80.0;

/// The roll of a view for moving sideways at `side`, from `V_CalcRoll`.
#[must_use]
pub fn view_roll(side: f32) -> f32 {
    let amount = side.abs();
    let roll = if amount < ROLL_SPEED {
        amount * ROLL_ANGLE / ROLL_SPEED
    } else {
        ROLL_ANGLE
    };
    roll * side.signum()
}

/// The small sway of a view with nothing to do, as pitch, yaw and roll, from
/// `V_AddIdle`.
#[must_use]
pub fn view_idle(time: f32) -> (f32, f32, f32) {
    (
        (time * 1.0).sin() * 0.3,
        (time * 2.0).sin() * 0.3,
        (time * 0.5).sin() * 0.1,
    )
}

/// What being hit does to the view, from `V_ParseDamage` and `V_CalcViewRoll`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewKick {
    /// The roll at the start of the kick.
    roll: f32,
    /// The pitch at the start of the kick.
    pitch: f32,
    /// Seconds left.
    left: f32,
}

impl ViewKick {
    /// Knocks the view. `count` is how hard, and `side` and `ahead` say how
    /// much of the blow came from the right and from in front.
    pub fn hit(&mut self, count: f32, side: f32, ahead: f32) {
        self.roll = count * side * KICK_ROLL;
        self.pitch = count * ahead * KICK_PITCH;
        self.left = KICK_SECONDS;
    }

    /// The view's pitch and roll now, and moves on by `seconds`.
    pub fn update(&mut self, seconds: f32) -> (f32, f32) {
        if self.left <= 0.0 {
            return (0.0, 0.0);
        }
        let share = self.left / KICK_SECONDS;
        self.left -= seconds;
        (self.pitch * share, self.roll * share)
    }
}

/// The roll of a dead player's view.
#[must_use]
pub const fn dead_roll() -> f32 {
    DEAD_ROLL
}

/// How far the view bobs at `time`, for a player moving at `speed` along the
/// ground, from `V_CalcBob`.
#[must_use]
pub fn view_bob(time: f32, speed: f32) -> f32 {
    use std::f32::consts::PI;
    let cycle = time.rem_euclid(BOB_CYCLE) / BOB_CYCLE;
    let angle = if cycle < BOB_UP {
        PI * cycle / BOB_UP
    } else {
        PI + PI * (cycle - BOB_UP) / (1.0 - BOB_UP)
    };
    let bob = speed * BOB;
    (bob * 0.3 + bob * 0.7 * angle.sin()).clamp(-7.0, 4.0)
}

/// Smooths the view going up stairs, from `V_CalcRefdef` in `view.c`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepSmoother {
    /// The smoothed height.
    height: f32,
}

impl StepSmoother {
    /// Follows a player at `height`, and returns the offset for the view,
    /// which is zero or below.
    pub fn update(&mut self, height: f32, on_ground: bool, seconds: f32) -> f32 {
        if on_ground && height > self.height {
            self.height = (self.height + seconds.max(0.0) * STEP_SPEED)
                .min(height)
                .max(height - MAX_STEP_LAG);
        } else {
            self.height = height;
        }
        self.height - height
    }
}
