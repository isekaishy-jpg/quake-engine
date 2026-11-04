//! Where the eye goes.

/// The eye's height above the middle of the player's box, from
/// `DEFAULT_VIEWHEIGHT`.
pub const VIEW_HEIGHT: f32 = 22.0;

/// How fast the view catches up after a stair, in units a second.
const STEP_SPEED: f32 = 80.0;

/// The furthest the view falls behind on a stair.
const MAX_STEP_LAG: f32 = 12.0;

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
