//! What client and server say to each other.

use engine_core::math::Vec3;
use engine_core::player::{Liquid, PlayerState, UserCommand};

/// The longest console text a client may send.
const MAX_CONSOLE_BYTES: usize = 256;

/// The tag of a move message.
const TAG_MOVE: u8 = 1;

/// The tag of a console message.
const TAG_CONSOLE: u8 = 2;

/// The tag of a snapshot.
const TAG_SNAPSHOT: u8 = 3;

/// The server's view of the game at one tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapshot {
    /// The tick this describes.
    pub tick: u32,
    /// The last command the server has run.
    pub acknowledged: u32,
    /// The player.
    pub player: PlayerState,
}

/// A message to the server.
#[derive(Clone, Debug, PartialEq)]
pub enum ClientMessage {
    /// Input.
    Move(UserCommand),
    /// A console command for the server to run.
    Console(String),
}

/// A message to a client.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ServerMessage {
    /// The state of the game.
    Snapshot(Snapshot),
}

/// Reads fields from the front of a message.
struct Reader<'a> {
    /// What's left.
    bytes: &'a [u8],
}

impl Reader<'_> {
    /// Takes `N` bytes.
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (field, rest) = self.bytes.split_first_chunk::<N>()?;
        self.bytes = rest;
        Some(*field)
    }

    /// A byte.
    fn byte(&mut self) -> Option<u8> {
        self.take::<1>().map(|[byte]| byte)
    }

    /// A flag, which must be 0 or 1.
    fn flag(&mut self) -> Option<bool> {
        match self.byte()? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }

    /// A number.
    fn u32(&mut self) -> Option<u32> {
        self.take().map(u32::from_le_bytes)
    }

    /// A float, which must be finite.
    fn f32(&mut self) -> Option<f32> {
        self.take()
            .map(f32::from_le_bytes)
            .filter(|value| value.is_finite())
    }

    /// A vector.
    fn vec3(&mut self) -> Option<Vec3> {
        Some(Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }

    /// Whether everything was read.
    fn finished(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// Appends a float.
fn put_f32(out: &mut Vec<u8>, value: f32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Appends a vector.
fn put_vec3(out: &mut Vec<u8>, value: Vec3) {
    put_f32(out, value.x);
    put_f32(out, value.y);
    put_f32(out, value.z);
}

/// The number sent for a liquid.
fn liquid_number(liquid: Liquid) -> u8 {
    match liquid {
        Liquid::None => 0,
        Liquid::Water => 1,
        Liquid::Slime => 2,
        Liquid::Lava => 3,
    }
}

/// The liquid for a number.
fn liquid_from(number: u8) -> Option<Liquid> {
    match number {
        0 => Some(Liquid::None),
        1 => Some(Liquid::Water),
        2 => Some(Liquid::Slime),
        3 => Some(Liquid::Lava),
        _ => None,
    }
}

impl ClientMessage {
    /// The message as bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            Self::Move(command) => {
                out.push(TAG_MOVE);
                out.extend_from_slice(&command.sequence.to_le_bytes());
                for value in [
                    command.seconds,
                    command.yaw,
                    command.pitch,
                    command.forward,
                    command.side,
                    command.up,
                ] {
                    put_f32(&mut out, value);
                }
                out.push(u8::from(command.jump));
            }
            Self::Console(text) => {
                out.push(TAG_CONSOLE);
                out.extend_from_slice(text.as_bytes());
            }
        }
        out
    }

    /// Reads a message, or `None` for anything malformed.
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let (&tag, rest) = bytes.split_first()?;
        match tag {
            TAG_MOVE => {
                let mut reader = Reader { bytes: rest };
                let command = UserCommand {
                    sequence: reader.u32()?,
                    seconds: reader.f32()?,
                    yaw: reader.f32()?,
                    pitch: reader.f32()?,
                    forward: reader.f32()?,
                    side: reader.f32()?,
                    up: reader.f32()?,
                    jump: reader.flag()?,
                };
                reader.finished().then_some(Self::Move(command))
            }
            TAG_CONSOLE if rest.len() <= MAX_CONSOLE_BYTES => std::str::from_utf8(rest)
                .ok()
                .map(|text| Self::Console(text.to_owned())),
            _ => None,
        }
    }
}

impl ServerMessage {
    /// The message as bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let Self::Snapshot(snapshot) = self;
        let player = &snapshot.player;
        let mut out = vec![TAG_SNAPSHOT];
        out.extend_from_slice(&snapshot.tick.to_le_bytes());
        out.extend_from_slice(&snapshot.acknowledged.to_le_bytes());
        put_vec3(&mut out, player.origin);
        put_vec3(&mut out, player.velocity);
        put_f32(&mut out, player.water_jump_time);
        out.extend_from_slice(&[
            u8::from(player.on_ground),
            player.water_level,
            liquid_number(player.liquid),
            u8::from(player.jump_held),
            u8::from(player.noclip),
        ]);
        out
    }

    /// Reads a message, or `None` for anything malformed.
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let (&tag, rest) = bytes.split_first()?;
        if tag != TAG_SNAPSHOT {
            return None;
        }
        let mut reader = Reader { bytes: rest };
        let tick = reader.u32()?;
        let acknowledged = reader.u32()?;
        let origin = reader.vec3()?;
        let velocity = reader.vec3()?;
        let water_jump_time = reader.f32()?;
        let player = PlayerState {
            origin,
            velocity,
            water_jump_time,
            on_ground: reader.flag()?,
            water_level: reader.byte().filter(|level| *level <= 3)?,
            liquid: liquid_from(reader.byte()?)?,
            jump_held: reader.flag()?,
            noclip: reader.flag()?,
        };
        reader.finished().then_some(Self::Snapshot(Snapshot {
            tick,
            acknowledged,
            player,
        }))
    }
}
