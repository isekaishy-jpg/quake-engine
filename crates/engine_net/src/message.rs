//! What client and server say to each other.

use engine_core::entity::{Effect, EntityState, ModelInstance, Projectile};
use engine_core::math::Vec3;
use engine_core::player::{Liquid, PlayerState, UserCommand};
use engine_core::sound::{SoundStart, StaticSound};

/// The longest console text a client may send.
const MAX_CONSOLE_BYTES: usize = 256;

/// The tag of a move message.
const TAG_MOVE: u8 = 1;

/// The tag of a console message.
const TAG_CONSOLE: u8 = 2;

/// The tag of a snapshot.
const TAG_SNAPSHOT: u8 = 3;

/// The tag of a light style.
const TAG_LIGHT_STYLE: u8 = 4;

/// The tag of printed text.
const TAG_PRINT: u8 = 5;

/// The tag of a view change.
const TAG_SET_VIEW: u8 = 6;

/// The tag of a sound starting.
const TAG_SOUND: u8 = 7;

/// The tag of a sound at a fixed place.
const TAG_STATIC_SOUND: u8 = 8;

/// The tag of a music track.
const TAG_MUSIC: u8 = 9;

/// The tag of a model's name.
const TAG_MODEL_NAME: u8 = 10;

/// The tag of a temporary entity.
const TAG_TEMP: u8 = 11;

/// The tag of particles.
const TAG_PARTICLES: u8 = 12;

/// The tag of text in the middle of the screen.
const TAG_CENTER_PRINT: u8 = 13;

/// The tag of a command for the client.
const TAG_STUFF: u8 = 14;

/// The tag of damage to the player.
const TAG_DAMAGE: u8 = 15;

/// The tag of an end of level screen.
const TAG_INTERMISSION: u8 = 16;

/// The tag of the level's title.
const TAG_LEVEL_TITLE: u8 = 17;

/// The tag of a snapshot as what changed.
pub(crate) const TAG_SNAPSHOT_DELTA: u8 = 18;

/// The tag of a level to load.
const TAG_LEVEL: u8 = 19;

/// The tag of the scores.
const TAG_SCORES: u8 = 20;

/// The tag of a client having loaded the level.
const TAG_READY: u8 = 21;

/// The tag of who a client says they are.
const TAG_INFO: u8 = 22;

/// The tag of a launch.
const TAG_LAUNCH: u8 = 24;

/// The tag of a shot, sent the moment it's fired.
const TAG_FIRE: u8 = 23;

/// A box to show for a moment.
const TAG_OUTLINE: u8 = 25;

/// The most commands in a move message.
const MAX_MOVE_COMMANDS: usize = 8;

/// The longest name.
const MAX_NAME_BYTES: usize = 32;

/// The most players the scores can list.
const MAX_SCORES: usize = 64;

/// The most stats a snapshot may carry.
const MAX_STATS: usize = 64;

/// The most entities a snapshot may list.
const MAX_ENTITIES: usize = 8192;

/// The longest sound name, from `MAX_QPATH`.
const MAX_SOUND_NAME_BYTES: usize = 64;

/// The most brush models a snapshot may list.
const MAX_INSTANCES: usize = 4096;

/// The longest text a server may send.
const MAX_TEXT_BYTES: usize = 2048;

/// The value sent for standing on no model.
const NO_MODEL: u32 = u32::MAX;

/// The server's view of the game at one tick.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// The tick this describes.
    pub tick: u32,
    /// The last command the server has run.
    pub acknowledged: u32,
    /// The player.
    pub player: PlayerState,
    /// Where brush models are.
    pub instances: Vec<ModelInstance>,
    /// Entities with other models.
    pub entities: Vec<EntityState>,
    /// Solid boxes, as their corners.
    pub boxes: Vec<(Vec3, Vec3)>,
    /// The number of the player's weapon model, or 0 for none.
    pub weapon_model: u16,
    /// The frame it shows.
    pub weapon_frame: u16,
    /// The player's entity, which isn't among `entities`.
    pub player_entity: u64,
    /// The player's effect bits.
    pub player_effects: u8,
    /// How far the view is knocked, in degrees of pitch, yaw and roll.
    pub punch: Vec3,
    /// Game time in seconds.
    pub time: f64,
    /// Numbers for the game's status display, which only the game understands.
    pub stats: Vec<i32>,
}

/// Who a client says they are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerInfo {
    /// Their name.
    pub name: String,
    /// Their shirt colour in the high four bits, and pants in the low.
    pub colors: u8,
}

/// A shot, sent the moment the button goes down (proposal 0050).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FireCommand {
    /// The view's yaw in degrees.
    pub yaw: f32,
    /// The view's pitch in degrees.
    pub pitch: f32,
    /// The game time of what the player was looking at.
    pub view_time: f64,
    /// How far through a tick it was fired, out of 256.
    pub offset: u8,
    /// A number the shot's randomness comes from, so both ends agree on it.
    pub seed: u16,
}

/// A line of the scores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerScore {
    /// Their entity.
    pub entity: u64,
    /// Their name.
    pub name: String,
    /// Their frags.
    pub frags: i32,
    /// Their shirt colour in the high four bits, and pants in the low.
    pub colors: u8,
}

/// A message to the server.
#[derive(Clone, Debug, PartialEq)]
pub enum ClientMessage {
    /// Input, oldest first, sent more than once in case of loss, and the last
    /// snapshot received.
    Move(Vec<UserCommand>, u32),
    /// A shot, ahead of the command that would carry it.
    Fire(FireCommand),
    /// The map named by the server is loaded.
    Ready,
    /// Who this client is.
    Info(PlayerInfo),
    /// A console command for the server to run.
    Console(String),
}

/// A message to a client.
#[derive(Clone, Debug, PartialEq)]
pub enum ServerMessage {
    /// The state of the game.
    Snapshot(Snapshot),
    /// A light style's pattern changed.
    LightStyle {
        /// The style's number.
        index: u8,
        /// Its pattern.
        pattern: String,
    },
    /// Text for the player.
    Print(String),
    /// A sound starts.
    Sound(SoundStart),
    /// A sound loops at a fixed place.
    StaticSound(StaticSound),
    /// A music track plays.
    Music(u8),
    /// Something short-lived happens.
    Effect(Effect),
    /// A model's number is given its name.
    ModelName {
        /// The number snapshots use.
        number: u16,
        /// The file's name.
        name: String,
    },
    /// Turns the player's view.
    SetView {
        /// The yaw in degrees.
        yaw: f32,
        /// The pitch in degrees.
        pitch: f32,
    },
    /// Text for the middle of the screen.
    CenterPrint(String),
    /// A command for the client's console.
    Stuff(String),
    /// The player was hurt.
    Damage {
        /// How much their armour took.
        armor: u8,
        /// How much they took.
        blood: u8,
        /// Where it came from.
        from: Vec3,
    },
    /// An end of level screen starts, by the game's number, or stops for 0.
    Intermission(u8),
    /// The level's title.
    LevelTitle(String),
    /// A snapshot as what changed, to be applied to the one it names.
    SnapshotDelta(Vec<u8>),
    /// Load this map, and say when it's ready.
    Level(String),
    /// Everyone's names and frags.
    Scores(Vec<PlayerScore>),
}

/// Reads fields from the front of a message.
pub(crate) struct Reader<'a> {
    /// What's left.
    pub(crate) bytes: &'a [u8],
}

impl Reader<'_> {
    /// Takes `N` bytes.
    pub(crate) fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (field, rest) = self.bytes.split_first_chunk::<N>()?;
        self.bytes = rest;
        Some(*field)
    }

    /// A byte.
    pub(crate) fn byte(&mut self) -> Option<u8> {
        self.take::<1>().map(|[byte]| byte)
    }

    /// A flag, which must be 0 or 1.
    pub(crate) fn flag(&mut self) -> Option<bool> {
        match self.byte()? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }

    /// A number.
    pub(crate) fn u32(&mut self) -> Option<u32> {
        self.take().map(u32::from_le_bytes)
    }

    /// A float, which must be finite.
    pub(crate) fn f32(&mut self) -> Option<f32> {
        self.take()
            .map(f32::from_le_bytes)
            .filter(|value| value.is_finite())
    }

    /// A double, which must be finite.
    pub(crate) fn f64(&mut self) -> Option<f64> {
        self.take()
            .map(f64::from_le_bytes)
            .filter(|value| value.is_finite())
    }

    /// The rest of the message, as a sound's name.
    fn sound_name(&mut self) -> Option<String> {
        let name = std::mem::take(&mut self.bytes);
        if name.is_empty() || name.len() > MAX_SOUND_NAME_BYTES {
            return None;
        }
        std::str::from_utf8(name).ok().map(str::to_owned)
    }

    /// A vector.
    pub(crate) fn vec3(&mut self) -> Option<Vec3> {
        Some(Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }

    /// Whether everything was read.
    pub(crate) fn finished(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// Appends a float.
fn put_f32(out: &mut Vec<u8>, value: f32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Appends a vector.
pub(crate) fn put_vec3(out: &mut Vec<u8>, value: Vec3) {
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

/// Appends a command.
fn encode_command(command: &UserCommand, out: &mut Vec<u8>) {
    out.extend_from_slice(&command.sequence.to_le_bytes());
    for value in [
        command.seconds,
        command.yaw,
        command.pitch,
        command.forward,
        command.side,
        command.up,
    ] {
        put_f32(out, value);
    }
    out.extend_from_slice(&[
        u8::from(command.jump),
        u8::from(command.attack),
        command.impulse,
    ]);
    out.extend_from_slice(&command.view_time.to_le_bytes());
}

/// Reads a command.
fn read_command(reader: &mut Reader<'_>) -> Option<UserCommand> {
    Some(UserCommand {
        sequence: reader.u32()?,
        seconds: reader.f32()?,
        yaw: reader.f32()?,
        pitch: reader.f32()?,
        forward: reader.f32()?,
        side: reader.f32()?,
        up: reader.f32()?,
        jump: reader.flag()?,
        attack: reader.flag()?,
        impulse: reader.byte()?,
        view_time: reader.f64()?,
    })
}

impl ClientMessage {
    /// The message as bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            Self::Move(commands, seen) => {
                out.push(TAG_MOVE);
                out.extend_from_slice(&seen.to_le_bytes());
                out.push(u8::try_from(commands.len()).unwrap_or(0));
                for command in commands {
                    encode_command(command, &mut out);
                }
            }
            Self::Fire(fire) => {
                out.extend_from_slice(&[TAG_FIRE, fire.offset]);
                out.extend_from_slice(&fire.seed.to_le_bytes());
                put_f32(&mut out, fire.yaw);
                put_f32(&mut out, fire.pitch);
                out.extend_from_slice(&fire.view_time.to_le_bytes());
            }
            Self::Ready => out.push(TAG_READY),
            Self::Info(info) => {
                out.extend_from_slice(&[TAG_INFO, info.colors]);
                out.extend_from_slice(info.name.as_bytes());
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
                let seen = reader.u32()?;
                let count = usize::from(reader.byte()?);
                if count > MAX_MOVE_COMMANDS {
                    return None;
                }
                let mut commands = Vec::with_capacity(count);
                for _ in 0..count {
                    commands.push(read_command(&mut reader)?);
                }
                reader.finished().then_some(Self::Move(commands, seen))
            }
            TAG_FIRE => {
                let mut reader = Reader { bytes: rest };
                let offset = reader.byte()?;
                let seed = u16::from_le_bytes(reader.take()?);
                let fire = FireCommand {
                    yaw: reader.f32()?,
                    pitch: reader.f32()?,
                    view_time: reader.f64()?,
                    offset,
                    seed,
                };
                reader.finished().then_some(Self::Fire(fire))
            }
            TAG_READY => rest.is_empty().then_some(Self::Ready),
            TAG_INFO => {
                let (&colors, name) = rest.split_first()?;
                if name.len() > MAX_NAME_BYTES {
                    return None;
                }
                let name = std::str::from_utf8(name).ok()?.to_owned();
                Some(Self::Info(PlayerInfo { name, colors }))
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
        let mut out = Vec::new();
        match self {
            Self::Snapshot(snapshot) => encode_snapshot(snapshot, &mut out),
            Self::LightStyle { index, pattern } => {
                out.extend_from_slice(&[TAG_LIGHT_STYLE, *index]);
                out.extend_from_slice(pattern.as_bytes());
            }
            Self::Print(text) => {
                out.push(TAG_PRINT);
                out.extend_from_slice(text.as_bytes());
            }
            Self::SetView { yaw, pitch } => {
                out.push(TAG_SET_VIEW);
                put_f32(&mut out, *yaw);
                put_f32(&mut out, *pitch);
            }
            Self::Sound(sound) => {
                out.push(TAG_SOUND);
                out.extend_from_slice(&sound.entity.to_le_bytes());
                out.extend_from_slice(&[sound.channel, u8::from(sound.own)]);
                put_vec3(&mut out, sound.origin);
                put_f32(&mut out, sound.volume);
                put_f32(&mut out, sound.attenuation);
                out.extend_from_slice(sound.name.as_bytes());
            }
            Self::StaticSound(sound) => {
                out.push(TAG_STATIC_SOUND);
                put_vec3(&mut out, sound.origin);
                put_f32(&mut out, sound.volume);
                put_f32(&mut out, sound.attenuation);
                out.extend_from_slice(sound.name.as_bytes());
            }
            Self::Music(track) => out.extend_from_slice(&[TAG_MUSIC, *track]),
            Self::Effect(Effect::Launch(launch)) => {
                out.extend_from_slice(&[TAG_LAUNCH, u8::from(launch.bounces)]);
                out.extend_from_slice(&launch.entity.to_le_bytes());
                out.extend_from_slice(&launch.owner.to_le_bytes());
                put_vec3(&mut out, launch.origin);
                put_vec3(&mut out, launch.velocity);
                out.extend_from_slice(&launch.time.to_le_bytes());
            }
            Self::Effect(Effect::Outline { low, high }) => {
                out.push(TAG_OUTLINE);
                put_vec3(&mut out, *low);
                put_vec3(&mut out, *high);
            }
            Self::Effect(Effect::Temp {
                kind,
                origin,
                end,
                entity,
            }) => {
                out.extend_from_slice(&[TAG_TEMP, *kind]);
                put_vec3(&mut out, *origin);
                put_vec3(&mut out, *end);
                out.extend_from_slice(&entity.to_le_bytes());
            }
            Self::Effect(Effect::Particles {
                origin,
                direction,
                color,
                count,
            }) => {
                out.extend_from_slice(&[TAG_PARTICLES, *color, *count]);
                put_vec3(&mut out, *origin);
                put_vec3(&mut out, *direction);
            }
            Self::ModelName { number, name } => {
                out.push(TAG_MODEL_NAME);
                out.extend_from_slice(&number.to_le_bytes());
                out.extend_from_slice(name.as_bytes());
            }
            Self::CenterPrint(text) => {
                out.push(TAG_CENTER_PRINT);
                out.extend_from_slice(text.as_bytes());
            }
            Self::Stuff(text) => {
                out.push(TAG_STUFF);
                out.extend_from_slice(text.as_bytes());
            }
            Self::Damage { armor, blood, from } => {
                out.extend_from_slice(&[TAG_DAMAGE, *armor, *blood]);
                put_vec3(&mut out, *from);
            }
            Self::Intermission(kind) => out.extend_from_slice(&[TAG_INTERMISSION, *kind]),
            Self::LevelTitle(text) => {
                out.push(TAG_LEVEL_TITLE);
                out.extend_from_slice(text.as_bytes());
            }
            Self::SnapshotDelta(bytes) => {
                out.push(TAG_SNAPSHOT_DELTA);
                out.extend_from_slice(bytes);
            }
            Self::Level(name) => {
                out.push(TAG_LEVEL);
                out.extend_from_slice(name.as_bytes());
            }
            Self::Scores(scores) => {
                out.push(TAG_SCORES);
                out.push(u8::try_from(scores.len()).unwrap_or(0));
                for score in scores {
                    out.extend_from_slice(&score.entity.to_le_bytes());
                    out.extend_from_slice(&score.frags.to_le_bytes());
                    out.push(score.colors);
                    out.push(u8::try_from(score.name.len()).unwrap_or(0));
                    out.extend_from_slice(score.name.as_bytes());
                }
            }
        }
        out
    }

    /// Reads a message, or `None` for anything malformed.
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let (&tag, rest) = bytes.split_first()?;
        match tag {
            TAG_SNAPSHOT => decode_snapshot(rest).map(Self::Snapshot),
            TAG_LIGHT_STYLE => {
                let (&index, pattern) = rest.split_first()?;
                let pattern = text_from(pattern)?;
                Some(Self::LightStyle { index, pattern })
            }
            TAG_PRINT => text_from(rest).map(Self::Print),
            TAG_SET_VIEW => {
                let mut reader = Reader { bytes: rest };
                let (yaw, pitch) = (reader.f32()?, reader.f32()?);
                reader.finished().then_some(Self::SetView { yaw, pitch })
            }
            TAG_SOUND => decode_sound(rest).map(Self::Sound),
            TAG_STATIC_SOUND => decode_static_sound(rest).map(Self::StaticSound),
            TAG_TEMP => {
                let mut reader = Reader { bytes: rest };
                let effect = Effect::Temp {
                    kind: reader.byte()?,
                    origin: reader.vec3()?,
                    end: reader.vec3()?,
                    entity: u64::from_le_bytes(reader.take()?),
                };
                reader.finished().then_some(Self::Effect(effect))
            }
            TAG_LAUNCH => {
                let mut reader = Reader { bytes: rest };
                let bounces = reader.flag()?;
                let launch = Projectile {
                    entity: u64::from_le_bytes(reader.take()?),
                    owner: u64::from_le_bytes(reader.take()?),
                    origin: reader.vec3()?,
                    velocity: reader.vec3()?,
                    time: reader.f64()?,
                    bounces,
                };
                reader
                    .finished()
                    .then_some(Self::Effect(Effect::Launch(launch)))
            }
            TAG_OUTLINE => {
                let mut reader = Reader { bytes: rest };
                let effect = Effect::Outline {
                    low: reader.vec3()?,
                    high: reader.vec3()?,
                };
                reader.finished().then_some(Self::Effect(effect))
            }
            TAG_PARTICLES => {
                let mut reader = Reader { bytes: rest };
                let (color, count) = (reader.byte()?, reader.byte()?);
                let effect = Effect::Particles {
                    origin: reader.vec3()?,
                    direction: reader.vec3()?,
                    color,
                    count,
                };
                reader.finished().then_some(Self::Effect(effect))
            }
            TAG_MODEL_NAME => {
                let mut reader = Reader { bytes: rest };
                let number = u16::from_le_bytes(reader.take()?);
                let name = reader.sound_name()?;
                Some(Self::ModelName { number, name })
            }
            TAG_MUSIC => match rest {
                [track] => Some(Self::Music(*track)),
                _ => None,
            },
            TAG_CENTER_PRINT => text_from(rest).map(Self::CenterPrint),
            TAG_STUFF => text_from(rest).map(Self::Stuff),
            TAG_DAMAGE => {
                let mut reader = Reader { bytes: rest };
                let (armor, blood) = (reader.byte()?, reader.byte()?);
                let from = reader.vec3()?;
                reader
                    .finished()
                    .then_some(Self::Damage { armor, blood, from })
            }
            TAG_INTERMISSION => match rest {
                [kind] => Some(Self::Intermission(*kind)),
                _ => None,
            },
            TAG_LEVEL_TITLE => text_from(rest).map(Self::LevelTitle),
            TAG_SNAPSHOT_DELTA => Some(Self::SnapshotDelta(rest.to_vec())),
            TAG_LEVEL => text_from(rest).map(Self::Level),
            TAG_SCORES => decode_scores(rest).map(Self::Scores),
            _ => None,
        }
    }
}

/// Whether a sound's volume and attenuation are ones the server can send.
fn sound_values_fit(volume: f32, attenuation: f32) -> bool {
    (0.0..=1.0).contains(&volume) && (0.0..=4.0).contains(&attenuation)
}

/// Reads a sound starting.
fn decode_sound(bytes: &[u8]) -> Option<SoundStart> {
    let mut reader = Reader { bytes };
    let entity = u64::from_le_bytes(reader.take()?);
    let channel = reader.byte().filter(|channel| *channel <= 7)?;
    let own = reader.flag()?;
    let origin = reader.vec3()?;
    let (volume, attenuation) = (reader.f32()?, reader.f32()?);
    let name = reader.sound_name()?;
    sound_values_fit(volume, attenuation).then_some(SoundStart {
        entity,
        channel,
        origin,
        volume,
        attenuation,
        own,
        name,
    })
}

/// Reads a sound at a fixed place.
fn decode_static_sound(bytes: &[u8]) -> Option<StaticSound> {
    let mut reader = Reader { bytes };
    let origin = reader.vec3()?;
    let (volume, attenuation) = (reader.f32()?, reader.f32()?);
    let name = reader.sound_name()?;
    sound_values_fit(volume, attenuation).then_some(StaticSound {
        origin,
        volume,
        attenuation,
        name,
    })
}

/// Text from a message, within the size limit.
fn text_from(bytes: &[u8]) -> Option<String> {
    if bytes.len() > MAX_TEXT_BYTES {
        return None;
    }
    std::str::from_utf8(bytes).ok().map(str::to_owned)
}

/// Reads the scores.
fn decode_scores(bytes: &[u8]) -> Option<Vec<PlayerScore>> {
    let mut reader = Reader { bytes };
    let count = usize::from(reader.byte()?);
    if count > MAX_SCORES {
        return None;
    }
    let mut scores = Vec::with_capacity(count);
    for _ in 0..count {
        let entity = u64::from_le_bytes(reader.take()?);
        let frags = i32::from_le_bytes(reader.take()?);
        let colors = reader.byte()?;
        let length = usize::from(reader.byte()?);
        if length > MAX_NAME_BYTES || reader.bytes.len() < length {
            return None;
        }
        let (name, rest) = reader.bytes.split_at(length);
        reader.bytes = rest;
        let name = std::str::from_utf8(name).ok()?.to_owned();
        scores.push(PlayerScore {
            entity,
            name,
            frags,
            colors,
        });
    }
    reader.finished().then_some(scores)
}

/// Appends a player.
pub(crate) fn encode_player(player: &PlayerState, out: &mut Vec<u8>) {
    put_vec3(out, player.origin);
    put_vec3(out, player.velocity);
    put_f32(out, player.water_jump_time);
    out.extend_from_slice(&[
        u8::from(player.on_ground),
        player.water_level,
        liquid_number(player.liquid),
        u8::from(player.jump_held),
        u8::from(player.noclip),
        u8::from(player.dead),
        u8::from(player.frozen),
    ]);
    out.extend_from_slice(&player.ground_model.unwrap_or(NO_MODEL).to_le_bytes());
}

/// Appends a snapshot.
pub(crate) fn encode_snapshot(snapshot: &Snapshot, out: &mut Vec<u8>) {
    out.push(TAG_SNAPSHOT);
    out.extend_from_slice(&snapshot.tick.to_le_bytes());
    out.extend_from_slice(&snapshot.acknowledged.to_le_bytes());
    encode_player(&snapshot.player, out);
    let count = u32::try_from(snapshot.instances.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&count.to_le_bytes());
    for instance in &snapshot.instances {
        out.extend_from_slice(&instance.model.to_le_bytes());
        put_vec3(out, instance.origin);
        out.extend_from_slice(&[u8::from(instance.visible), u8::from(instance.solid)]);
        out.extend_from_slice(&instance.frame.to_le_bytes());
    }
    out.extend_from_slice(&snapshot.weapon_model.to_le_bytes());
    out.extend_from_slice(&snapshot.weapon_frame.to_le_bytes());
    out.extend_from_slice(&snapshot.player_entity.to_le_bytes());
    out.push(snapshot.player_effects);
    put_vec3(out, snapshot.punch);
    out.extend_from_slice(&snapshot.time.to_le_bytes());
    let count = u32::try_from(snapshot.stats.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&count.to_le_bytes());
    for stat in &snapshot.stats {
        out.extend_from_slice(&stat.to_le_bytes());
    }
    let count = u32::try_from(snapshot.entities.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&count.to_le_bytes());
    for entity in &snapshot.entities {
        out.extend_from_slice(&entity.id.to_le_bytes());
        out.extend_from_slice(&entity.model.to_le_bytes());
        out.extend_from_slice(&entity.frame.to_le_bytes());
        out.extend_from_slice(&[entity.skin, entity.effects]);
        put_vec3(out, entity.origin);
        put_vec3(out, entity.angles);
    }
    let count = u32::try_from(snapshot.boxes.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&count.to_le_bytes());
    for (low, high) in &snapshot.boxes {
        put_vec3(out, *low);
        put_vec3(out, *high);
    }
}

/// Reads a player.
pub(crate) fn read_player(reader: &mut Reader<'_>) -> Option<PlayerState> {
    let origin = reader.vec3()?;
    let velocity = reader.vec3()?;
    let water_jump_time = reader.f32()?;
    let on_ground = reader.flag()?;
    let water_level = reader.byte().filter(|level| *level <= 3)?;
    let liquid = liquid_from(reader.byte()?)?;
    let jump_held = reader.flag()?;
    let noclip = reader.flag()?;
    let dead = reader.flag()?;
    let frozen = reader.flag()?;
    let ground = reader.u32()?;
    Some(PlayerState {
        origin,
        velocity,
        on_ground,
        water_level,
        liquid,
        water_jump_time,
        jump_held,
        noclip,
        dead,
        frozen,
        ground_model: (ground != NO_MODEL).then_some(ground),
    })
}

/// Reads a snapshot.
fn decode_snapshot(bytes: &[u8]) -> Option<Snapshot> {
    let mut reader = Reader { bytes };
    let tick = reader.u32()?;
    let acknowledged = reader.u32()?;
    let player = read_player(&mut reader)?;
    let count = usize::try_from(reader.u32()?).ok()?;
    if count > MAX_INSTANCES {
        return None;
    }
    let mut instances = Vec::with_capacity(count);
    for _ in 0..count {
        instances.push(ModelInstance {
            model: reader.u32()?,
            origin: reader.vec3()?,
            visible: reader.flag()?,
            solid: reader.flag()?,
            frame: u16::from_le_bytes(reader.take()?),
        });
    }
    let weapon_model = u16::from_le_bytes(reader.take()?);
    let weapon_frame = u16::from_le_bytes(reader.take()?);
    let player_entity = u64::from_le_bytes(reader.take()?);
    let player_effects = reader.byte()?;
    let punch = reader.vec3()?;
    let time = reader.f64()?;
    let count = usize::try_from(reader.u32()?).ok()?;
    if count > MAX_STATS {
        return None;
    }
    let mut stats = Vec::with_capacity(count);
    for _ in 0..count {
        stats.push(i32::from_le_bytes(reader.take()?));
    }
    let count = usize::try_from(reader.u32()?).ok()?;
    if count > MAX_ENTITIES {
        return None;
    }
    let mut entities = Vec::with_capacity(count);
    for _ in 0..count {
        entities.push(EntityState {
            id: u64::from_le_bytes(reader.take()?),
            model: u16::from_le_bytes(reader.take()?),
            frame: u16::from_le_bytes(reader.take()?),
            skin: reader.byte()?,
            effects: reader.byte()?,
            origin: reader.vec3()?,
            angles: reader.vec3()?,
        });
    }
    let count = usize::try_from(reader.u32()?).ok()?;
    if count > MAX_ENTITIES {
        return None;
    }
    let mut boxes = Vec::with_capacity(count);
    for _ in 0..count {
        boxes.push((reader.vec3()?, reader.vec3()?));
    }
    reader.finished().then_some(Snapshot {
        tick,
        acknowledged,
        player,
        instances,
        entities,
        boxes,
        weapon_model,
        weapon_frame,
        player_entity,
        player_effects,
        punch,
        time,
        stats,
    })
}
