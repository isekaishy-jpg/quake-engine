//! The colour over the view: being in a liquid, being hurt, picking things up, and whatever
//! else a game adds (proposal 0134). This says what colour, and what draws it doesn't care why.

/// How fast a hurt fades, out of 1 a second.
const DAMAGE_FADE: f32 = 150.0 / 255.0;

/// How fast a bonus fades, out of 1 a second.
const BONUS_FADE: f32 = 100.0 / 255.0;

/// The most a hurt shows.
const MOST_DAMAGE: f32 = 150.0 / 255.0;

/// How strong a bonus flash starts.
const BONUS_STRENGTH: f32 = 50.0 / 255.0;

/// The colour of a bonus flash.
const BONUS_COLOR: [f32; 3] = [215.0 / 255.0, 186.0 / 255.0, 69.0 / 255.0];

/// A colour over the view: red, green and blue, then how strong it is, each from 0 to 1.
pub type Shift = [f32; 4];

/// What's colouring the view.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewTint {
    /// What the eye is in.
    surroundings: Shift,
    /// Being hurt, fading.
    damage: Shift,
    /// Picking something up, fading.
    bonus: Shift,
}

impl ViewTint {
    /// Says what the eye is in, by the tint of its liquid, if any.
    pub const fn set_surroundings(&mut self, tint: Shift) {
        self.surroundings = tint;
    }

    /// Reddens the view, from `V_ParseDamage`.
    pub fn hurt(&mut self, armor: u8, blood: u8) {
        let count = (f32::from(blood) * 0.5 + f32::from(armor) * 0.5).max(10.0);
        let strength = (self.damage[3] + 3.0 * count / 255.0).clamp(0.0, MOST_DAMAGE);
        let [red, green, blue] = if armor > blood {
            [200.0, 100.0, 100.0]
        } else if armor != 0 {
            [220.0, 50.0, 50.0]
        } else {
            [255.0, 0.0, 0.0]
        };
        self.damage = [red / 255.0, green / 255.0, blue / 255.0, strength];
    }

    /// Flashes the view, from `V_BonusFlash_f`.
    pub const fn bonus_flash(&mut self) {
        let [red, green, blue] = BONUS_COLOR;
        self.bonus = [red, green, blue, BONUS_STRENGTH];
    }

    /// Lets flashes fade by `seconds`.
    pub fn pass(&mut self, seconds: f32) {
        self.damage[3] = (self.damage[3] - seconds * DAMAGE_FADE).max(0.0);
        self.bonus[3] = (self.bonus[3] - seconds * BONUS_FADE).max(0.0);
    }

    /// The colour over the view, from `V_CalcBlend`: each in turn, over what's before it.
    #[must_use]
    pub fn color(&self) -> Shift {
        let mut blended = [0.0_f32; 3];
        let mut alpha = 0.0_f32;
        for [red, green, blue, strength] in [self.surroundings, self.damage, self.bonus] {
            if strength == 0.0 {
                continue;
            }
            alpha += strength * (1.0 - alpha);
            let share = strength / alpha;
            for (channel, value) in blended.iter_mut().zip([red, green, blue]) {
                *channel = *channel * (1.0 - share) + value * share;
            }
        }
        let [red, green, blue] = blended;
        [red, green, blue, alpha.clamp(0.0, 1.0)]
    }
}
