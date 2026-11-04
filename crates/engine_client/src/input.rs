//! Turning held buttons into a command.

use engine_core::player::UserCommand;

/// The speed asked for by a held key when running. It's above the fastest a
/// player can go (proposal 0023).
const RUN_SPEED: f32 = 400.0;

/// The speed asked for when walking.
const WALK_SPEED: f32 = 200.0;

/// `cl_yawspeed`, in degrees a second.
const YAW_SPEED: f32 = 140.0;

/// `cl_pitchspeed`, in degrees a second.
const PITCH_SPEED: f32 = 150.0;

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
    /// `+jump`.
    pub jump: bool,
    /// `+attack`.
    pub attack: bool,
    /// `+left`.
    pub turn_left: bool,
    /// `+right`.
    pub turn_right: bool,
    /// `+lookup`.
    pub look_up: bool,
    /// `+lookdown`.
    pub look_down: bool,
    /// `+strafe`.
    pub strafe: bool,
    /// The last `impulse`, until a command takes it.
    pub impulse: u8,
    /// Whether running is the default, so `+speed` walks.
    pub always_run: bool,
}

/// One axis from two opposing buttons.
fn axis(positive: bool, negative: bool) -> f32 {
    f32::from(u8::from(positive)) - f32::from(u8::from(negative))
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
            "jump" => &mut self.jump,
            "attack" => &mut self.attack,
            "left" => &mut self.turn_left,
            "right" => &mut self.turn_right,
            "lookup" => &mut self.look_up,
            "lookdown" => &mut self.look_down,
            "strafe" => &mut self.strafe,
            _ => return false,
        };
        *button = held;
        true
    }

    /// How far the turning keys move the view over `seconds`, as yaw and
    /// pitch, from `CL_AdjustAngles`.
    #[must_use]
    pub fn turn(&self, seconds: f32) -> (f32, f32) {
        let yaw = if self.strafe {
            0.0
        } else {
            axis(self.turn_left, self.turn_right) * YAW_SPEED * seconds
        };
        let pitch = axis(self.look_down, self.look_up) * PITCH_SPEED * seconds;
        (yaw, pitch)
    }

    /// Lets go of everything held, keeping what isn't a button.
    pub fn release(&mut self) {
        *self = Self {
            always_run: self.always_run,
            ..Self::default()
        };
    }

    /// A command for this input. The client fills in the number and time.
    #[must_use]
    pub fn command(&self, yaw: f32, pitch: f32) -> UserCommand {
        let speed = if self.speed == self.always_run {
            WALK_SPEED
        } else {
            RUN_SPEED
        };
        let sideways = if self.strafe {
            axis(self.right || self.turn_right, self.left || self.turn_left)
        } else {
            axis(self.right, self.left)
        };
        UserCommand {
            yaw,
            pitch,
            forward: axis(self.forward, self.back) * speed,
            side: sideways * speed,
            up: axis(self.up || self.jump, self.down) * speed,
            jump: self.jump,
            attack: self.attack,
            impulse: self.impulse,
            ..UserCommand::default()
        }
    }
}
