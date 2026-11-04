//! Turning held buttons into a command.

use engine_core::player::UserCommand;

/// The speed asked for by a held key. It's above the fastest a player can
/// go, so running is the default (proposal 0023).
const RUN_SPEED: f32 = 400.0;

/// The speed asked for with `+speed` held, which walks.
const WALK_SPEED: f32 = 200.0;

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
            _ => return false,
        };
        *button = held;
        true
    }

    /// A command for this input. The client fills in the number and time.
    #[must_use]
    pub fn command(&self, yaw: f32, pitch: f32) -> UserCommand {
        let speed = if self.speed { WALK_SPEED } else { RUN_SPEED };
        UserCommand {
            yaw,
            pitch,
            forward: axis(self.forward, self.back) * speed,
            side: axis(self.right, self.left) * speed,
            up: axis(self.up || self.jump, self.down) * speed,
            jump: self.jump,
            ..UserCommand::default()
        }
    }
}
