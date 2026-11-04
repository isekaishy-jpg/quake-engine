//! Where the eye goes.

/// The eye's height above the middle of the player's box, from
/// `DEFAULT_VIEWHEIGHT`.
pub const VIEW_HEIGHT: f32 = 22.0;

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
