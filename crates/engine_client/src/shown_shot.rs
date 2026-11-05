//! A shot of your own, as it's shown before the server answers (proposal 0057).
//!
//! Nothing here decides a result. While it's active, the server's kick, weapon
//! frames and flash are the same shot arriving late, so they're left out.

use engine_core::entity::ShotFeel;
use engine_core::player::PUNCH_RECOVERY;
use engine_core::time::TICK_SECONDS;

/// How long each frame of a weapon lasts, in seconds.
const FRAME_SECONDS: f32 = 0.1;

/// How far late a shot can be and still follow on from the last, in seconds.
const CARRY_SECONDS: f64 = 2.0 * TICK_SECONDS;

/// When a weapon can next fire, as far as a client can tell.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShotClock {
    /// The game time of the next shot.
    next: f64,
}

impl ShotClock {
    /// If a shot can go at game time `time`, moves on by `refire` and says
    /// whether it carries on from the last. That counts from when the shot was
    /// due, as `refire` in the scripts does, so a held weapon keeps its pace.
    pub fn take(&mut self, time: f64, refire: f64) -> Option<bool> {
        if time < self.next {
            return None;
        }
        let carrying_on = time - self.next < CARRY_SECONDS;
        self.next = if carrying_on { self.next } else { time } + refire;
        Some(carrying_on)
    }

    /// Starts again, for a new level.
    pub fn reset(&mut self) {
        self.next = 0.0;
    }
}

/// What's being shown.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ShownShot {
    /// The kick now, in degrees.
    recoil: f32,
    /// The kick a tick ago.
    previous_recoil: f32,
    /// The frames it's running through.
    frames: Option<(u16, u16)>,
    /// Whether they go round.
    repeats: bool,
    /// How long they've been running, in seconds.
    age: f32,
    /// Whether the flash is yet to be shown.
    flash: bool,
    /// How much longer to wait for the server, in seconds.
    patience: f32,
    /// Whether the server has been seen firing.
    heard: bool,
    /// Whether the server's own show is being left out.
    active: bool,
}

impl ShownShot {
    /// Shows a shot, waiting `patience` seconds to hear of it.
    pub(crate) fn start(&mut self, feel: ShotFeel, patience: f32) {
        if feel.recoil == 0.0 && !feel.flash && feel.frames.is_none() {
            return;
        }
        self.recoil = feel.recoil;
        self.previous_recoil = feel.recoil;
        self.flash |= feel.flash;
        // A held weapon carries on through its frames.
        let same = self.active && self.repeats && self.frames == feel.frames;
        if !same {
            self.frames = feel.frames;
            self.age = 0.0;
        }
        self.repeats = feel.repeats;
        self.patience = patience;
        self.active = true;
    }

    /// Stops a held weapon's frames going round.
    pub(crate) fn let_go(&mut self) {
        if self.repeats {
            self.repeats = false;
            self.frames = None;
        }
    }

    /// Moves on by `seconds`. `busy` says whether the server's numbers show a
    /// shot going on.
    pub(crate) fn advance(&mut self, seconds: f32, busy: bool) {
        self.previous_recoil = self.recoil;
        self.recoil = (self.recoil - PUNCH_RECOVERY * seconds).max(0.0);
        self.age += seconds;
        self.patience -= seconds;
        if !self.active {
            return;
        }
        self.heard |= busy;
        let finished = self.recoil == 0.0 && self.frame().is_none_or(|frame| frame == 0);
        let answered = self.heard && !busy && finished;
        if answered || (!self.heard && self.patience <= 0.0) {
            *self = Self::default();
        }
    }

    /// Whether the server's own show is being left out.
    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    /// The kick, `fraction` of the way through the tick, in degrees.
    pub(crate) fn recoil(&self, fraction: f32) -> f32 {
        self.previous_recoil + (self.recoil - self.previous_recoil) * fraction
    }

    /// The weapon's frame, if it's being shown: 0 once it's stood down.
    pub(crate) fn frame(&self) -> Option<u16> {
        if !self.active {
            return None;
        }
        let Some((first, last)) = self.frames else {
            return Some(0);
        };
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a few frames"
        )]
        let step = (self.age / FRAME_SECONDS) as u16;
        let count = last - first + 1;
        if self.repeats {
            Some(first + step % count)
        } else if step < count {
            Some(first + step)
        } else {
            Some(0)
        }
    }

    /// Takes the flash, if it's yet to be shown.
    pub(crate) fn take_flash(&mut self) -> bool {
        std::mem::take(&mut self.flash)
    }
}
