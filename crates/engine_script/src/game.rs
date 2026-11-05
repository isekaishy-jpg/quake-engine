//! Running scripts against a simulation.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::time::{Duration, Instant};

use engine_core::entity::{Effect, Projectile};
use engine_core::math::Vec3;
use engine_core::player::Liquid;
use engine_core::sound::{SoundStart, StaticSound};
use engine_map::{Contents, Entity as MapEntity};
use engine_sim::{
    Callback, EntityId, FLAG_FLY, FLAG_ITEM, FLAG_MONSTER, FLAG_ON_GROUND, FLAG_PARTIAL_GROUND,
    FLAG_SWIM, MoveKind, MoveType, Simulation, Solid, Target,
};
use mlua::chunk::Compiler;
use mlua::{Function, Lua, Table, Value, Vector, VmState};

/// What a game tells the engine about itself.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameRules {
    /// Which of the numbers from `ClientData` is someone's frags.
    pub frags_stat: Option<usize>,
    /// The commands guests can use.
    pub guest_commands: Vec<String>,
}

/// The prelude every game runs on.
const PRELUDE: &str = include_str!("prelude.luau");

/// The longest one call into a script may run.
const CALL_LIMIT: Duration = Duration::from_secs(1);

/// How scripts write what blocks what, by the names they know it as.
const SOLID_NUMBERS: [(Solid, f64, &str); 5] = [
    (Solid::Not, 0.0, "SOLID_NOT"),
    (Solid::Trigger, 1.0, "SOLID_TRIGGER"),
    (Solid::Bbox, 2.0, "SOLID_BBOX"),
    (Solid::SlideBox, 3.0, "SOLID_SLIDEBOX"),
    (Solid::Bsp, 4.0, "SOLID_BSP"),
];

/// How scripts write how things move, by the names they know it as.
const MOVE_TYPE_NUMBERS: [(MoveType, f64, &str); 9] = [
    (MoveType::None, 0.0, "MOVETYPE_NONE"),
    (MoveType::Walk, 3.0, "MOVETYPE_WALK"),
    (MoveType::Step, 4.0, "MOVETYPE_STEP"),
    (MoveType::Fly, 5.0, "MOVETYPE_FLY"),
    (MoveType::Toss, 6.0, "MOVETYPE_TOSS"),
    (MoveType::Push, 7.0, "MOVETYPE_PUSH"),
    (MoveType::Noclip, 8.0, "MOVETYPE_NOCLIP"),
    (MoveType::FlyMissile, 9.0, "MOVETYPE_FLYMISSILE"),
    (MoveType::Bounce, 10.0, "MOVETYPE_BOUNCE"),
];

/// How scripts write what fills a point, by the names they know it as.
const CONTENTS_NUMBERS: [(Contents, f64, &str); 6] = [
    (Contents::Empty, -1.0, "CONTENT_EMPTY"),
    (Contents::Solid, -2.0, "CONTENT_SOLID"),
    (Contents::Water, -3.0, "CONTENT_WATER"),
    (Contents::Slime, -4.0, "CONTENT_SLIME"),
    (Contents::Lava, -5.0, "CONTENT_LAVA"),
    (Contents::Sky, -6.0, "CONTENT_SKY"),
];

/// The flags the engine acts on, by the names scripts know them as.
const FLAG_NUMBERS: [(u32, &str); 6] = [
    (FLAG_FLY, "FL_FLY"),
    (FLAG_SWIM, "FL_SWIM"),
    (FLAG_MONSTER, "FL_MONSTER"),
    (FLAG_ITEM, "FL_ITEM"),
    (FLAG_ON_GROUND, "FL_ONGROUND"),
    (FLAG_PARTIAL_GROUND, "FL_PARTIALGROUND"),
];

/// Gives scripts the numbers they share with the engine.
fn share_numbers(lua: &Lua) -> mlua::Result<()> {
    let globals = lua.globals();
    for (_, number, name) in SOLID_NUMBERS {
        globals.set(name, number)?;
    }
    for (_, number, name) in MOVE_TYPE_NUMBERS {
        globals.set(name, number)?;
    }
    for (_, number, name) in CONTENTS_NUMBERS {
        globals.set(name, number)?;
    }
    for (flag, name) in FLAG_NUMBERS {
        globals.set(name, f64::from(flag))?;
    }
    Ok(())
}

/// How scripts write `contents`.
fn contents_number(contents: Contents) -> f64 {
    CONTENTS_NUMBERS
        .iter()
        .find(|(known, ..)| *known == contents)
        .map_or(-1.0, |(_, number, _)| *number)
}

/// What `engine.launch` takes: an entity, its owner, its origin and velocity,
/// the time, whether it bounces, whether it blows up on what it hits, how
/// strong it is, and its fuse.
type LaunchArguments = (f64, f64, Vector, Vector, f64, bool, bool, f32, f32);

/// A script that couldn't load.
#[derive(Debug)]
pub struct ScriptError(String);

impl ScriptError {
    /// An error saying `message`.
    pub(crate) fn new(message: String) -> Self {
        Self(message)
    }
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ScriptError {}

impl From<mlua::Error> for ScriptError {
    fn from(error: mlua::Error) -> Self {
        Self(error.to_string())
    }
}

/// What scripts asked for, for the server to pass on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outbox {
    /// Text for one player, or everyone.
    pub prints: Vec<(Option<EntityId>, String)>,
    /// A level to change to.
    pub level: Option<String>,
    /// Light styles that changed.
    pub light_styles: Vec<(u8, String)>,
    /// A yaw and pitch to turn one player, or everyone, to.
    pub views: Vec<(Option<EntityId>, f32, f32)>,
    /// Sounds starting.
    pub sounds: Vec<SoundStart>,
    /// Sounds looping at fixed places.
    pub static_sounds: Vec<StaticSound>,
    /// A music track to play.
    pub music: Option<u8>,
    /// Short-lived effects.
    pub effects: Vec<Effect>,
    /// What each player carries to the next level.
    pub parms: Vec<(EntityId, Vec<f64>)>,
    /// Whether the level should start again.
    pub restart: bool,
    /// The game's flags to carry to the next level.
    pub level_flags: Option<f64>,
    /// Text for the middle of one player's screen, or everyone's.
    pub center_prints: Vec<(Option<EntityId>, String)>,
    /// Commands for one client's console, or everyone's.
    pub stuffed: Vec<(Option<EntityId>, String)>,
    /// Commands for the host's console.
    pub local_commands: Vec<String>,
    /// Damage to a player, as who, what armour took, what they took, and where
    /// it came from.
    pub damage: Vec<(EntityId, u8, u8, Vec3)>,
    /// An end of level screen, by the game's number.
    pub intermission: Option<u8>,
    /// The level's title.
    pub level_title: Option<String>,
}

/// Turns a script's vector into an engine one.
fn to_vec3(vector: Vector) -> Vec3 {
    Vec3::new(vector.x(), vector.y(), vector.z())
}

/// Turns an engine vector into a script's.
fn from_vec3(vector: Vec3) -> Vector {
    Vector::new(vector.x, vector.y, vector.z)
}

/// Turns a script's id number into an id.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "ids are whole numbers well inside what a script's numbers hold"
)]
fn id_from(number: f64) -> EntityId {
    EntityId::from_bits(number as u64)
}

/// Turns an id into a script's number.
#[expect(
    clippy::cast_precision_loss,
    reason = "ids stay well inside what a script's numbers hold"
)]
fn id_to(id: EntityId) -> f64 {
    id.to_bits() as f64
}

/// A script's error for a bad field name.
fn no_field(name: &str) -> mlua::Error {
    mlua::Error::runtime(format!("no engine field called {name}"))
}

/// Refuses a sound's values, as `SV_StartSound` does.
fn check_sound(volume: f32, attenuation: f32) -> mlua::Result<()> {
    if !(0.0..=1.0).contains(&volume) {
        return Err(mlua::Error::runtime(format!("sound volume {volume}")));
    }
    if !(0.0..=4.0).contains(&attenuation) {
        return Err(mlua::Error::runtime(format!(
            "sound attenuation {attenuation}"
        )));
    }
    Ok(())
}

/// Reads a field only the player has, which is nil on anything else.
fn read_player_field(sim: &Simulation, id: EntityId, field: &str) -> Value {
    let Some(player) = sim.player_of(id) else {
        return Value::Nil;
    };
    match field {
        "waterlevel" => Value::Number(f64::from(player.water_level)),
        "watertype" => Value::Number(contents_number(match player.liquid {
            Liquid::None => Contents::Empty,
            Liquid::Water => Contents::Water,
            Liquid::Slime => Contents::Slime,
            Liquid::Lava => Contents::Lava,
        })),
        "onground" => Value::Boolean(player.on_ground),
        "button2" => Value::Boolean(sim.jump_down(id)),
        "button0" => Value::Boolean(sim.attack_down(id)),
        "impulse" => Value::Number(f64::from(sim.impulse(id))),
        "dead" => Value::Boolean(player.dead),
        "frozen" => Value::Boolean(player.frozen),
        "v_angle" => {
            let (yaw, pitch) = sim.view(id);
            Value::Vector(from_vec3(Vec3::new(pitch, yaw, 0.0)))
        }
        "waterjump" => Value::Boolean(player.water_jump_time > 0.0),
        "noclip" => Value::Boolean(player.noclip),
        _ => Value::Nil,
    }
}

/// Reads an engine field.
fn read_field(lua: &Lua, sim: &Simulation, id: EntityId, field: &str) -> mlua::Result<Value> {
    let Some(entity) = sim.entity(id) else {
        return Ok(Value::Nil);
    };
    let vector = |value: Vec3| Value::Vector(from_vec3(value));
    let origin = sim.origin(id).unwrap_or(entity.origin);
    Ok(match field {
        "origin" => vector(origin),
        "velocity" => vector(sim.velocity(id).unwrap_or(entity.velocity)),
        "mins" => vector(entity.mins),
        "maxs" => vector(entity.maxs),
        "size" => vector(entity.maxs - entity.mins),
        "absmin" => vector(origin + entity.mins),
        "absmax" => vector(origin + entity.maxs),
        "solid" => Value::Number(
            SOLID_NUMBERS
                .iter()
                .find(|(solid, ..)| *solid == entity.solid)
                .map_or(0.0, |(_, number, _)| *number),
        ),
        "movetype" => Value::Number(
            MOVE_TYPE_NUMBERS
                .iter()
                .find(|(kind, ..)| *kind == entity.move_type)
                .map_or(0.0, |(_, number, _)| *number),
        ),
        "nextthink" => Value::Number(entity.next_think),
        "ltime" => Value::Number(entity.local_time),
        "visible" => Value::Boolean(entity.visible),
        "angles" => vector(entity.angles),
        "frame" => Value::Number(f64::from(entity.frame)),
        "skin" => Value::Number(f64::from(entity.skin)),
        "effects" => Value::Number(f64::from(entity.effects)),
        "punchangle" => vector(sim.punch(id)),
        "ideal_yaw" => Value::Number(f64::from(entity.ideal_yaw)),
        "yaw_speed" => Value::Number(f64::from(entity.yaw_speed)),
        "weaponframe" => Value::Number(f64::from(sim.weapon_frame(id))),
        "weaponmodel" => Value::String(lua.create_string(sim.weapon_model_name(id))?),
        "view" => {
            let (yaw, pitch) = sim.view(id);
            vector(Vec3::new(pitch, yaw, 0.0))
        }
        "v_angle" if !sim.is_player(id) => vector(entity.view_angle),
        "waterlevel" | "watertype" | "onground" | "button0" | "button2" | "impulse" | "dead"
        | "v_angle" | "waterjump" | "noclip" | "frozen" => read_player_field(sim, id, field),
        "flags" => {
            let on_ground = sim.player_of(id).is_some_and(|player| player.on_ground);
            let ground = if on_ground { FLAG_ON_GROUND } else { 0 };
            Value::Number(f64::from(entity.flags | ground))
        }
        "avelocity" => vector(entity.angular_velocity),
        _ => return Err(no_field(field)),
    })
}

/// A small whole number from a script's value, or an error.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "frames and skins are small, and a cast saturates anything that isn't"
)]
fn small_number(value: &Value) -> mlua::Result<u16> {
    Ok(as_number(value)? as u16)
}

/// A small real number from a script's value, or an error.
#[expect(
    clippy::cast_possible_truncation,
    reason = "yaws need no more than f32"
)]
fn as_f32(value: &Value) -> mlua::Result<f32> {
    Ok(as_number(value)? as f32)
}

/// A number from a script's value, or an error.
fn as_number(value: &Value) -> mlua::Result<f64> {
    match value {
        Value::Number(number) => Ok(*number),
        #[expect(
            clippy::cast_precision_loss,
            reason = "engine fields hold small numbers"
        )]
        Value::Integer(number) => Ok(*number as f64),
        _ => Err(mlua::Error::runtime("expected a number")),
    }
}

/// A vector from a script's value, or an error.
fn as_vec3(value: &Value) -> mlua::Result<Vec3> {
    match value {
        Value::Vector(vector) => Ok(to_vec3(*vector)),
        _ => Err(mlua::Error::runtime("expected a vector")),
    }
}

/// Writes an engine field.
fn write_field(sim: &mut Simulation, id: EntityId, field: &str, value: &Value) -> mlua::Result<()> {
    match field {
        "origin" => sim.set_origin(id, as_vec3(value)?),
        "velocity" => sim.set_velocity(id, as_vec3(value)?),
        "punchangle" => sim.set_punch(id, as_vec3(value)?),
        "weaponmodel" => {
            let name = if let Value::String(name) = value {
                name.to_string_lossy()
            } else {
                String::new()
            };
            sim.set_weapon_model(id, &name);
        }
        "weaponframe" => sim.set_weapon_frame(id, small_number(value)?),
        "impulse" => sim.set_impulse(id, u8::try_from(small_number(value)?).unwrap_or(0)),
        "dead" => sim.set_dead(id, matches!(value, Value::Boolean(true))),
        "frozen" => sim.set_frozen(id, matches!(value, Value::Boolean(true))),
        "weaponhidden" => sim.set_weapon_hidden(id, matches!(value, Value::Boolean(true))),
        _ => {
            let number = as_number(value);
            let Some(entity) = sim.entity_mut(id) else {
                return Ok(());
            };
            match field {
                "mins" => entity.mins = as_vec3(value)?,
                "maxs" => entity.maxs = as_vec3(value)?,
                "nextthink" => entity.next_think = number?,
                "ltime" => entity.local_time = number?,
                "v_angle" => entity.view_angle = as_vec3(value)?,
                "ideal_yaw" => entity.ideal_yaw = as_f32(value)?,
                "yaw_speed" => entity.yaw_speed = as_f32(value)?,
                "visible" => entity.visible = matches!(value, Value::Boolean(true)),
                "angles" => entity.angles = as_vec3(value)?,
                "avelocity" => entity.angular_velocity = as_vec3(value)?,
                "flags" => entity.flags = u32::from(small_number(value)?),
                "frame" => entity.frame = small_number(value)?,
                "skin" => entity.skin = u8::try_from(small_number(value)?).unwrap_or(0),
                "effects" => entity.effects = u8::try_from(small_number(value)?).unwrap_or(0),
                "solid" => {
                    let number = number?;
                    entity.solid = SOLID_NUMBERS
                        .iter()
                        .find(|(_, value, _)| (*value - number).abs() < f64::EPSILON)
                        .map_or(Solid::Not, |(solid, ..)| *solid);
                }
                "movetype" => {
                    let number = number?;
                    entity.move_type = MOVE_TYPE_NUMBERS
                        .iter()
                        .find(|(_, value, _)| (*value - number).abs() < f64::EPSILON)
                        .map_or(MoveType::None, |(kind, ..)| *kind);
                }
                _ => return Err(no_field(field)),
            }
        }
    }
    Ok(())
}

/// A running game's scripts.
pub struct Game {
    /// The Luau state.
    lua: Lua,
    /// What scripts asked for.
    outbox: Rc<RefCell<Outbox>>,
    /// When the current call started.
    call_started: Rc<Cell<Instant>>,
    /// The prelude's `dispatch`.
    dispatch: Function,
    /// Each player's table.
    players: RefCell<HashMap<EntityId, Table>>,
    /// The game's state.
    simulation: Rc<RefCell<Simulation>>,
}

impl Game {
    /// Loads `sources`, as names and text, over the prelude.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] if a script won't compile or fails as it loads.
    pub fn new(
        simulation: &Rc<RefCell<Simulation>>,
        sources: &[(&str, &str)],
    ) -> Result<Self, ScriptError> {
        let lua = Lua::new();
        // `sandbox` leaves these in place. `require` reads files from disk, and
        // scripts arrive only through `sources`.
        for global in ["require", "loadstring"] {
            lua.globals().raw_set(global, Value::Nil)?;
        }
        lua.sandbox(true)?;
        share_numbers(&lua)?;
        // Above level 0, Luau caches any global a chunk doesn't assign itself, as
        // if it were a constant. Games share globals between scripts, so one
        // script would never see what another writes.
        lua.set_compiler(Compiler::new().set_optimization_level(0));
        let call_started = Rc::new(Cell::new(Instant::now()));
        let started = Rc::clone(&call_started);
        lua.set_interrupt(move |_lua| {
            if started.get().elapsed() > CALL_LIMIT {
                Err(mlua::Error::runtime("a script ran for too long"))
            } else {
                Ok(VmState::Continue)
            }
        });
        let outbox = Rc::new(RefCell::new(Outbox::default()));
        Self::register(&lua, simulation, &outbox)?;
        lua.load(PRELUDE).set_name("prelude").exec()?;
        for (name, source) in sources {
            lua.load(*source).set_name(*name).exec()?;
        }
        let dispatch: Function = lua.globals().get("dispatch")?;
        let adopt: Function = lua.globals().get("adopt_player")?;
        let player = simulation.borrow().player_id();
        let table: Table = adopt.call(id_to(player))?;
        Ok(Self {
            lua,
            outbox,
            call_started,
            dispatch,
            players: RefCell::new(HashMap::from([(player, table)])),
            simulation: Rc::clone(simulation),
        })
    }

    /// The Luau state.
    pub(crate) fn lua(&self) -> &Lua {
        &self.lua
    }

    /// Adds the `engine` table.
    fn register(
        lua: &Lua,
        simulation: &Rc<RefCell<Simulation>>,
        outbox: &Rc<RefCell<Outbox>>,
    ) -> mlua::Result<()> {
        let engine = lua.create_table()?;
        let sim = Rc::clone(simulation);
        engine.set(
            "get",
            lua.create_function(move |lua, (id, field): (f64, String)| {
                read_field(lua, &sim.borrow(), id_from(id), &field)
            })?,
        )?;
        let sim = Rc::clone(simulation);
        engine.set(
            "set",
            lua.create_function(move |_, (id, field, value): (f64, String, Value)| {
                write_field(&mut sim.borrow_mut(), id_from(id), &field, &value)
            })?,
        )?;
        let sim = Rc::clone(simulation);
        engine.set(
            "spawn",
            lua.create_function(move |_, ()| Ok(id_to(sim.borrow_mut().spawn())))?,
        )?;
        let sim = Rc::clone(simulation);
        engine.set(
            "remove",
            lua.create_function(move |_, id: f64| {
                sim.borrow_mut().remove(id_from(id));
                Ok(())
            })?,
        )?;
        let sim = Rc::clone(simulation);
        engine.set(
            "ids",
            lua.create_function(move |_, ()| {
                Ok(sim
                    .borrow()
                    .ids()
                    .into_iter()
                    .map(id_to)
                    .collect::<Vec<f64>>())
            })?,
        )?;
        let sim = Rc::clone(simulation);
        engine.set(
            "force_retouch",
            lua.create_function(move |_, ()| {
                sim.borrow_mut().force_retouch();
                Ok(())
            })?,
        )?;
        let sim = Rc::clone(simulation);
        engine.set(
            "set_model",
            lua.create_function(move |_, (id, model): (f64, String)| {
                if let Some(number) = model.strip_prefix('*').and_then(|n| n.parse().ok()) {
                    sim.borrow_mut().set_model(id_from(id), number);
                } else if !model.is_empty() {
                    sim.borrow_mut().set_shape(id_from(id), &model);
                }
                Ok(())
            })?,
        )?;
        let sim = Rc::clone(simulation);
        engine.set(
            "precache_model",
            lua.create_function(move |_, name: String| {
                sim.borrow_mut().precache_shape(&name);
                Ok(())
            })?,
        )?;
        let sim = Rc::clone(simulation);
        engine.set(
            "time",
            lua.create_function(move |_, ()| Ok(sim.borrow().time()))?,
        )?;
        let out = Rc::clone(outbox);
        engine.set(
            "print",
            lua.create_function(move |_, (text, target): (String, Option<f64>)| {
                out.borrow_mut().prints.push((target.map(id_from), text));
                Ok(())
            })?,
        )?;
        let out = Rc::clone(outbox);
        engine.set(
            "changelevel",
            lua.create_function(move |_, (name, flags): (String, Option<f64>)| {
                let mut out = out.borrow_mut();
                out.level = Some(name);
                out.level_flags = flags;
                Ok(())
            })?,
        )?;
        let out = Rc::clone(outbox);
        engine.set(
            "lightstyle",
            lua.create_function(move |_, (index, pattern): (u8, String)| {
                out.borrow_mut().light_styles.push((index, pattern));
                Ok(())
            })?,
        )?;
        let out = Rc::clone(outbox);
        engine.set(
            "set_view",
            lua.create_function(move |_, (yaw, pitch, target): (f32, f32, Option<f64>)| {
                out.borrow_mut()
                    .views
                    .push((target.map(id_from), yaw, pitch));
                Ok(())
            })?,
        )?;
        Self::register_sound(lua, simulation, outbox, &engine)?;
        Self::register_world(lua, simulation, outbox, &engine)?;
        Self::register_effects(lua, outbox, &engine)?;
        lua.globals().set("engine", engine)
    }

    /// Adds what looks at the world to the `engine` table.
    fn register_world(
        lua: &Lua,
        simulation: &Rc<RefCell<Simulation>>,
        outbox: &Rc<RefCell<Outbox>>,
        engine: &Table,
    ) -> mlua::Result<()> {
        let sim = Rc::clone(simulation);
        engine.set(
            "traceline",
            lua.create_function(
                move |_, (start, end, nomonsters, pass): (Vector, Vector, bool, Option<f64>)| {
                    let kind = if nomonsters {
                        MoveKind::NoMonsters
                    } else {
                        MoveKind::Normal
                    };
                    let trace = sim.borrow().trace_line(
                        to_vec3(start),
                        to_vec3(end),
                        kind,
                        pass.map(id_from),
                    );
                    Ok((
                        trace.fraction,
                        from_vec3(trace.end),
                        from_vec3(trace.normal),
                        trace.entity.map(id_to),
                        trace.in_open,
                        trace.in_water,
                    ))
                },
            )?,
        )?;
        let sim = Rc::clone(simulation);
        engine.set(
            "pointcontents",
            lua.create_function(move |_, point: Vector| {
                Ok(contents_number(sim.borrow().point_contents(to_vec3(point))))
            })?,
        )?;
        Self::register_walking(lua, simulation, engine)?;
        let sim = Rc::clone(simulation);
        engine.set(
            "set_gravity",
            lua.create_function(move |_, gravity: f32| {
                sim.borrow_mut().set_gravity(gravity);
                Ok(())
            })?,
        )?;
        let sim = Rc::clone(simulation);
        engine.set(
            "no_backing",
            lua.create_function(move |_, (id, seconds): (f64, f32)| {
                sim.borrow_mut().set_no_backing(id_from(id), seconds);
                Ok(())
            })?,
        )?;
        let sim = Rc::clone(simulation);
        let rewind_out = Rc::clone(outbox);
        engine.set(
            "rewind",
            lua.create_function(move |_, id: f64| {
                let shooter = id_from(id);
                sim.borrow_mut().rewind(shooter);
                let shown = sim.borrow().shown_rewind(shooter);
                let outlines = shown
                    .into_iter()
                    .map(|(low, high)| Effect::Outline { low, high });
                rewind_out.borrow_mut().effects.extend(outlines);
                Ok(())
            })?,
        )?;
        let sim = Rc::clone(simulation);
        engine.set(
            "end_rewind",
            lua.create_function(move |_, ()| {
                sim.borrow_mut().end_rewind();
                Ok(())
            })?,
        )?;
        let sim = Rc::clone(simulation);
        engine.set(
            "droptofloor",
            lua.create_function(move |_, id: f64| Ok(sim.borrow_mut().drop_to_floor(id_from(id))))?,
        )?;
        let sim = Rc::clone(simulation);
        engine.set(
            "set_owner",
            lua.create_function(move |_, (id, owner): (f64, Option<f64>)| {
                if let Some(entity) = sim.borrow_mut().entity_mut(id_from(id)) {
                    entity.owner = owner.map(id_from);
                }
                Ok(())
            })?,
        )
    }

    /// Adds how monsters get about to the `engine` table.
    fn register_walking(
        lua: &Lua,
        simulation: &Rc<RefCell<Simulation>>,
        engine: &Table,
    ) -> mlua::Result<()> {
        let sim = Rc::clone(simulation);
        engine.set(
            "walkmove",
            lua.create_function(
                move |_, (id, yaw, distance, enemy): (f64, f32, f32, Option<f64>)| {
                    Ok(sim
                        .borrow_mut()
                        .walk_move(id_from(id), yaw, distance, enemy.map(id_from)))
                },
            )?,
        )?;
        let sim = Rc::clone(simulation);
        engine.set(
            "movetogoal",
            lua.create_function(
                move |_, (id, distance, goal, enemy): (f64, f32, Option<f64>, Option<f64>)| {
                    let target = Target {
                        goal: goal.map(id_from),
                        enemy: enemy.map(id_from),
                    };
                    sim.borrow_mut().move_to_goal(id_from(id), distance, target);
                    Ok(())
                },
            )?,
        )?;
        let sim = Rc::clone(simulation);
        engine.set(
            "find_path",
            lua.create_function(move |lua, (from, to): (Vector, Vector)| {
                let simulation = sim.borrow();
                let nav = simulation.map().nav.as_ref();
                let path = nav.and_then(|nav| nav.find_path(to_vec3(from), to_vec3(to)));
                match path {
                    Some(points) => {
                        let list = lua.create_table()?;
                        for (index, point) in points.into_iter().enumerate() {
                            list.set(index + 1, from_vec3(point))?;
                        }
                        Ok(Some(list))
                    }
                    None => Ok(None),
                }
            })?,
        )?;
        let sim = Rc::clone(simulation);
        engine.set(
            "spline_point",
            lua.create_function(move |_, (name, share): (String, f32)| {
                let simulation = sim.borrow();
                let splines = &simulation.map().splines;
                let found = splines.iter().find(|spline| spline.name == name);
                Ok(found
                    .and_then(|spline| spline.point_at(share))
                    .map(from_vec3))
            })?,
        )?;
        let sim = Rc::clone(simulation);
        engine.set(
            "changeyaw",
            lua.create_function(move |_, id: f64| {
                sim.borrow_mut().change_yaw(id_from(id));
                Ok(())
            })?,
        )?;
        let sim = Rc::clone(simulation);
        engine.set(
            "checkbottom",
            lua.create_function(move |_, id: f64| Ok(sim.borrow().check_bottom(id_from(id))))?,
        )?;
        let sim = Rc::clone(simulation);
        engine.set(
            "player_in_view",
            lua.create_function(move |_, (id, view_offset): (f64, Vector)| {
                Ok(sim
                    .borrow_mut()
                    .player_in_view(id_from(id), to_vec3(view_offset))
                    .map(id_to))
            })?,
        )
    }

    /// Adds effects and level changes to the `engine` table.
    fn register_effects(
        lua: &Lua,
        outbox: &Rc<RefCell<Outbox>>,
        engine: &Table,
    ) -> mlua::Result<()> {
        let out = Rc::clone(outbox);
        engine.set(
            "temp_entity",
            lua.create_function(
                move |_, (kind, origin, end, entity): (u8, Vector, Vector, Option<f64>)| {
                    out.borrow_mut().effects.push(Effect::Temp {
                        kind,
                        origin: to_vec3(origin),
                        end: to_vec3(end),
                        entity: entity.map_or(0, |id| id_from(id).to_bits()),
                    });
                    Ok(())
                },
            )?,
        )?;
        let out = Rc::clone(outbox);
        engine.set(
            "particle",
            lua.create_function(
                move |_, (origin, direction, color, count): (Vector, Vector, u8, f64)| {
                    #[expect(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        reason = "a cast saturates a count too large for a byte"
                    )]
                    let count = count as u8;
                    out.borrow_mut().effects.push(Effect::Particles {
                        origin: to_vec3(origin),
                        direction: to_vec3(direction),
                        color,
                        count,
                    });
                    Ok(())
                },
            )?,
        )?;
        let out = Rc::clone(outbox);
        engine.set(
            "launch",
            lua.create_function(move |_, arguments: LaunchArguments| {
                let (id, owner, origin, velocity, time, bounces, blasts, strength, fuse) =
                    arguments;
                out.borrow_mut().effects.push(Effect::Launch(Projectile {
                    entity: id_from(id).to_bits(),
                    owner: id_from(owner).to_bits(),
                    origin: to_vec3(origin),
                    velocity: to_vec3(velocity),
                    time,
                    bounces,
                    blasts,
                    strength,
                    fuse,
                }));
                Ok(())
            })?,
        )?;
        let out = Rc::clone(outbox);
        engine.set(
            "set_parms",
            lua.create_function(move |_, (parms, target): (Vec<f64>, f64)| {
                out.borrow_mut().parms.push((id_from(target), parms));
                Ok(())
            })?,
        )?;
        let out = Rc::clone(outbox);
        engine.set(
            "restart",
            lua.create_function(move |_, ()| {
                out.borrow_mut().restart = true;
                Ok(())
            })?,
        )?;
        Self::register_screen(lua, outbox, engine)
    }

    /// Adds what's shown to the player to the `engine` table.
    fn register_screen(
        lua: &Lua,
        outbox: &Rc<RefCell<Outbox>>,
        engine: &Table,
    ) -> mlua::Result<()> {
        let out = Rc::clone(outbox);
        engine.set(
            "centerprint",
            lua.create_function(move |_, (text, target): (String, Option<f64>)| {
                out.borrow_mut()
                    .center_prints
                    .push((target.map(id_from), text));
                Ok(())
            })?,
        )?;
        let out = Rc::clone(outbox);
        engine.set(
            "stuffcmd",
            lua.create_function(move |_, (text, target): (String, Option<f64>)| {
                out.borrow_mut().stuffed.push((target.map(id_from), text));
                Ok(())
            })?,
        )?;
        let out = Rc::clone(outbox);
        engine.set(
            "localcmd",
            lua.create_function(move |_, text: String| {
                out.borrow_mut().local_commands.push(text);
                Ok(())
            })?,
        )?;
        let out = Rc::clone(outbox);
        engine.set(
            "damage",
            lua.create_function(
                move |_, (target, armor, blood, from): (f64, f64, f64, Vector)| {
                    #[expect(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        reason = "a cast saturates damage too large for a byte"
                    )]
                    let (armor, blood) = (armor as u8, blood as u8);
                    out.borrow_mut()
                        .damage
                        .push((id_from(target), armor, blood, to_vec3(from)));
                    Ok(())
                },
            )?,
        )?;
        let out = Rc::clone(outbox);
        engine.set(
            "intermission",
            lua.create_function(move |_, kind: u8| {
                out.borrow_mut().intermission = Some(kind);
                Ok(())
            })?,
        )?;
        let out = Rc::clone(outbox);
        engine.set(
            "level_title",
            lua.create_function(move |_, text: String| {
                out.borrow_mut().level_title = Some(text);
                Ok(())
            })?,
        )
    }

    /// Adds the sound half of the `engine` table.
    fn register_sound(
        lua: &Lua,
        simulation: &Rc<RefCell<Simulation>>,
        outbox: &Rc<RefCell<Outbox>>,
        engine: &Table,
    ) -> mlua::Result<()> {
        let sim = Rc::clone(simulation);
        let out = Rc::clone(outbox);
        engine.set(
            "sound",
            lua.create_function(
                move |_, (id, channel, name, volume, attenuation): (f64, u8, String, f32, f32)| {
                    check_sound(volume, attenuation)?;
                    if channel > 7 {
                        return Err(mlua::Error::runtime(format!("sound channel {channel}")));
                    }
                    let sim = sim.borrow();
                    let id = id_from(id);
                    let Some(entity) = sim.entity(id) else {
                        return Ok(());
                    };
                    let middle = (entity.mins + entity.maxs) * 0.5;
                    out.borrow_mut().sounds.push(SoundStart {
                        entity: id.to_bits(),
                        channel,
                        origin: sim.origin(id).unwrap_or(entity.origin) + middle,
                        volume,
                        attenuation,
                        own: false,
                        name,
                    });
                    Ok(())
                },
            )?,
        )?;
        let out = Rc::clone(outbox);
        engine.set(
            "ambientsound",
            lua.create_function(
                move |_, (origin, name, volume, attenuation): (Vector, String, f32, f32)| {
                    check_sound(volume, attenuation)?;
                    out.borrow_mut().static_sounds.push(StaticSound {
                        origin: to_vec3(origin),
                        volume,
                        attenuation,
                        name,
                    });
                    Ok(())
                },
            )?,
        )?;
        let out = Rc::clone(outbox);
        engine.set(
            "music",
            lua.create_function(move |_, track: u8| {
                out.borrow_mut().music = Some(track);
                Ok(())
            })?,
        )
    }

    /// Reports a script's failure, which never stops the game.
    fn report(&self, result: mlua::Result<()>) {
        if let Err(error) = result {
            self.outbox
                .borrow_mut()
                .prints
                .push((None, format!("script error: {error}")));
        }
    }

    /// Tells the game which level this is, with the host's `settings` and the
    /// flags carried from the last level, before anything spawns.
    pub fn start_level(&self, name: &str, settings: &[(String, f64)], flags: f64) {
        self.call_started.set(Instant::now());
        let function: mlua::Result<Option<Function>> = self.lua.globals().get("start_level");
        let settings: HashMap<&str, f64> = settings
            .iter()
            .map(|(name, value)| (name.as_str(), *value))
            .collect();
        match function {
            Ok(Some(function)) => self.report(function.call::<()>((name, settings, flags))),
            Ok(None) => {}
            Err(error) => self.report(Err(error)),
        }
    }

    /// Takes a rule the host changed in play.
    pub fn set_setting(&self, name: &str, value: f64) {
        self.call_started.set(Instant::now());
        let function: mlua::Result<Option<Function>> = self.lua.globals().get("set_level_setting");
        match function {
            Ok(Some(function)) => self.report(function.call::<()>((name, value))),
            Ok(None) => {}
            Err(error) => self.report(Err(error)),
        }
    }

    /// Runs the game's `StartFrame`, if it has one.
    pub fn start_frame(&self) {
        self.call_started.set(Instant::now());
        let function: mlua::Result<Option<Function>> = self.lua.globals().get("StartFrame");
        match function {
            Ok(Some(function)) => self.report(function.call::<()>(())),
            Ok(None) => {}
            Err(error) => self.report(Err(error)),
        }
    }

    /// Spawns a map's entities.
    pub fn spawn_map(&self, entities: &[MapEntity]) {
        let spawner: mlua::Result<Function> = self.lua.globals().get("spawn_from_map");
        let spawner = match spawner {
            Ok(spawner) => spawner,
            Err(error) => return self.report(Err(error)),
        };
        for entity in entities {
            self.call_started.set(Instant::now());
            let fields: Vec<Vec<String>> = entity
                .fields
                .iter()
                .map(|(key, value)| vec![key.clone(), value.clone()])
                .collect();
            let class = entity.get("classname").unwrap_or("");
            let function: mlua::Result<Option<Function>> = self.lua.globals().get(class);
            match function {
                Ok(function) => self.report(spawner.call::<()>((fields, function))),
                Err(error) => self.report(Err(error)),
            }
        }
    }

    /// Runs what the simulation says is due.
    pub fn run(&self, callback: Callback) {
        self.call_started.set(Instant::now());
        let mut event_time = None;
        let (kind, id, other) = match callback {
            Callback::Think(id, time) => {
                event_time = Some(time).filter(|time| *time > 0.0);
                ("think", id, None)
            }
            Callback::Blocked { id, other } => ("blocked", id, Some(other)),
            Callback::Touch { id, other } => ("touch", id, other),
            Callback::Splash(id) => ("splash", id, None),
            Callback::Land(id) => ("land", id, None),
            Callback::Wake(id, seconds) => {
                let globals = self.lua.globals();
                let engine: mlua::Result<Table> = globals.get("engine");
                if let Ok(engine) = engine {
                    self.report(engine.set("slept", seconds));
                }
                ("wake", id, None)
            }
        };
        self.simulation.borrow_mut().set_event_time(event_time);
        self.report(
            self.dispatch
                .call::<()>((kind, id_to(id), other.map(id_to))),
        );
        self.simulation.borrow_mut().set_event_time(None);
    }

    /// The table of player `id`.
    fn player(&self, id: EntityId) -> Option<Table> {
        self.players.borrow().get(&id).cloned()
    }

    /// Runs `name` on a player, if the game has one.
    fn run_on_player(&self, name: &str, id: EntityId) {
        self.call_started.set(Instant::now());
        let function: mlua::Result<Option<Function>> = self.lua.globals().get(name);
        match (function, self.player(id)) {
            (Ok(Some(function)), Some(player)) => self.report(function.call::<()>(player)),
            (Err(error), _) => self.report(Err(error)),
            _ => {}
        }
    }

    /// Gives the game a table for player `id`.
    pub fn add_player(&self, id: EntityId) {
        let adopt: mlua::Result<Function> = self.lua.globals().get("adopt_player");
        let table = adopt.and_then(|adopt| adopt.call::<Table>(id_to(id)));
        match table {
            Ok(table) => {
                self.players.borrow_mut().insert(id, table);
            }
            Err(error) => self.report(Err(error)),
        }
    }

    /// Runs the game's `ClientConnect`, to ready player `id` for level `name`
    /// with what they carried from the last one, if anything.
    pub fn connect_player(&self, id: EntityId, name: &str, parms: Option<&[f64]>, netname: &str) {
        self.call_started.set(Instant::now());
        let function: mlua::Result<Option<Function>> = self.lua.globals().get("ClientConnect");
        match (function, self.player(id)) {
            (Ok(Some(function)), Some(player)) => {
                let parms = parms.map(<[f64]>::to_vec);
                self.report(function.call::<()>((player, name, parms, netname)));
            }
            (Err(error), _) => self.report(Err(error)),
            _ => {}
        }
    }

    /// Tells the game who player `id` is now, as `Host_Name_f` and
    /// `Host_Color_f` do, through its `ClientInfo`.
    pub fn player_info(&self, id: EntityId, netname: &str, colors: u8) {
        self.call_started.set(Instant::now());
        let function: mlua::Result<Option<Function>> = self.lua.globals().get("ClientInfo");
        match (function, self.player(id)) {
            (Ok(Some(function)), Some(player)) => {
                self.report(function.call::<()>((player, netname, colors)));
            }
            (Err(error), _) => self.report(Err(error)),
            _ => {}
        }
    }

    /// What the game's `GameRules` table says.
    #[must_use]
    pub fn rules(&self) -> GameRules {
        let table: mlua::Result<Option<Table>> = self.lua.globals().get("GameRules");
        let Ok(Some(table)) = table else {
            return GameRules::default();
        };
        GameRules {
            frags_stat: table.get("frags_stat").ok(),
            guest_commands: table.get("guest_commands").unwrap_or_default(),
        }
    }

    /// Whether a team message from someone wearing `colors` reaches someone
    /// wearing `heard_by`. A game with no `SameTeam` has no teams.
    pub fn same_team(&self, colors: u8, heard_by: u8) -> bool {
        self.call_started.set(Instant::now());
        let function: mlua::Result<Option<Function>> = self.lua.globals().get("SameTeam");
        let Ok(Some(function)) = function else {
            return true;
        };
        match function.call::<bool>((colors, heard_by)) {
            Ok(same) => same,
            Err(error) => {
                self.report(Err(error));
                true
            }
        }
    }

    /// Runs the game's `ClientDisconnect`, and forgets player `id`.
    pub fn disconnect_player(&self, id: EntityId) {
        self.run_on_player("ClientDisconnect", id);
        let forget: mlua::Result<Function> = self.lua.globals().get("forget_player");
        self.report(forget.and_then(|forget| forget.call::<()>(id_to(id))));
        let _table = self.players.borrow_mut().remove(&id);
    }

    /// Runs the game's `ClientFire` for a shot at game time `time`, with
    /// `seed` for its randomness.
    pub fn client_fire(&self, id: EntityId, time: f64, seed: u16) {
        self.call_started.set(Instant::now());
        let function: mlua::Result<Option<Function>> = self.lua.globals().get("ClientFire");
        self.simulation.borrow_mut().set_event_time(Some(time));
        match (function, self.player(id)) {
            (Ok(Some(function)), Some(player)) => {
                self.report(function.call::<()>((player, seed)));
            }
            (Err(error), _) => self.report(Err(error)),
            _ => {}
        }
        self.simulation.borrow_mut().set_event_time(None);
    }

    /// Runs the game's `PlayerPreThink`.
    pub fn player_pre_think(&self, id: EntityId) {
        self.run_on_player("PlayerPreThink", id);
    }

    /// Runs the game's `PlayerPostThink`.
    pub fn player_post_think(&self, id: EntityId) {
        self.run_on_player("PlayerPostThink", id);
    }

    /// Passes a console command from a player to the game's `ClientCommand`.
    pub fn client_command(&self, id: EntityId, command: &str, argument: &str) {
        self.call_started.set(Instant::now());
        let function: mlua::Result<Option<Function>> = self.lua.globals().get("ClientCommand");
        match (function, self.player(id)) {
            (Ok(Some(function)), Some(player)) => {
                self.report(function.call::<()>((player, command, argument)));
            }
            (Err(error), _) => self.report(Err(error)),
            _ => {}
        }
    }

    /// The numbers the game's `ClientData` gives for a player's status
    /// display, as `SV_WriteClientdataToMessage`. It sends what's due to the
    /// player, too.
    #[must_use]
    pub fn client_data(&self, id: EntityId) -> Vec<i32> {
        self.call_started.set(Instant::now());
        let function: mlua::Result<Option<Function>> = self.lua.globals().get("ClientData");
        let (Ok(Some(function)), Some(player)) = (function, self.player(id)) else {
            return Vec::new();
        };
        match function.call::<Vec<f64>>(player) {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "stats are whole, and item bits fill all 32"
            )]
            Ok(stats) => stats.into_iter().map(|stat| stat as i64 as i32).collect(),
            Err(error) => {
                self.report(Err(error));
                Vec::new()
            }
        }
    }

    /// Takes what scripts have asked for.
    #[must_use]
    pub fn take_outbox(&self) -> Outbox {
        std::mem::take(&mut self.outbox.borrow_mut())
    }

    /// Runs `source` in the game, for tests and tools.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] if it fails.
    pub fn exec(&self, source: &str) -> Result<(), ScriptError> {
        self.call_started.set(Instant::now());
        Ok(self.lua.load(source).exec()?)
    }
}
