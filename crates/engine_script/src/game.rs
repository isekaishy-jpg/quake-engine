//! Running scripts against a simulation.

use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;
use std::time::{Duration, Instant};

use engine_core::math::Vec3;
use engine_core::player::Liquid;
use engine_core::sound::{SoundStart, StaticSound};
use engine_map::Entity as MapEntity;
use engine_sim::{Callback, EntityId, MoveType, Simulation, Solid};
use mlua::{Function, Lua, Table, Value, Vector, VmState};

/// The prelude every game runs on.
const PRELUDE: &str = include_str!("prelude.luau");

/// The longest one call into a script may run.
const CALL_LIMIT: Duration = Duration::from_secs(1);

/// `SOLID_NOT`, `SOLID_TRIGGER` and `SOLID_BSP`, as scripts write them.
const SOLID_NUMBERS: [(Solid, f64); 3] =
    [(Solid::Not, 0.0), (Solid::Trigger, 1.0), (Solid::Bsp, 4.0)];

/// `MOVETYPE_NONE` and `MOVETYPE_PUSH`, as scripts write them.
const MOVE_TYPE_NUMBERS: [(MoveType, f64); 2] = [(MoveType::None, 0.0), (MoveType::Push, 7.0)];

/// A script that couldn't load.
#[derive(Debug)]
pub struct ScriptError(String);

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
    /// Text for the player.
    pub prints: Vec<String>,
    /// A level to change to.
    pub level: Option<String>,
    /// Light styles that changed.
    pub light_styles: Vec<(u8, String)>,
    /// A yaw and pitch to turn the player to.
    pub view: Option<(f32, f32)>,
    /// Sounds starting.
    pub sounds: Vec<SoundStart>,
    /// Sounds looping at fixed places.
    pub static_sounds: Vec<StaticSound>,
    /// A music track to play.
    pub music: Option<u8>,
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
    if id != sim.player_id() {
        return Value::Nil;
    }
    let player = sim.player();
    match field {
        "waterlevel" => Value::Number(f64::from(player.water_level)),
        "watertype" => Value::Number(match player.liquid {
            Liquid::None => -1.0,
            Liquid::Water => -3.0,
            Liquid::Slime => -4.0,
            Liquid::Lava => -5.0,
        }),
        "onground" => Value::Boolean(player.on_ground),
        "button2" => Value::Boolean(sim.jump_down()),
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
    let _lua = lua;
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
                .find(|(solid, _)| *solid == entity.solid)
                .map_or(0.0, |(_, number)| *number),
        ),
        "movetype" => Value::Number(
            MOVE_TYPE_NUMBERS
                .iter()
                .find(|(kind, _)| *kind == entity.move_type)
                .map_or(0.0, |(_, number)| *number),
        ),
        "nextthink" => Value::Number(entity.next_think),
        "ltime" => Value::Number(entity.local_time),
        "visible" => Value::Boolean(entity.visible),
        "view" => {
            let (yaw, pitch) = sim.view();
            vector(Vec3::new(pitch, yaw, 0.0))
        }
        "waterlevel" | "watertype" | "onground" | "button2" | "waterjump" | "noclip" => {
            read_player_field(sim, id, field)
        }
        _ => return Err(no_field(field)),
    })
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
                "visible" => entity.visible = matches!(value, Value::Boolean(true)),
                "solid" => {
                    let number = number?;
                    entity.solid = SOLID_NUMBERS
                        .iter()
                        .find(|(_, value)| (*value - number).abs() < f64::EPSILON)
                        .map_or(Solid::Not, |(solid, _)| *solid);
                }
                "movetype" => {
                    let number = number?;
                    entity.move_type = MOVE_TYPE_NUMBERS
                        .iter()
                        .find(|(_, value)| (*value - number).abs() < f64::EPSILON)
                        .map_or(MoveType::None, |(kind, _)| *kind);
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
    /// The player's table.
    player: Table,
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
        let player: Table = adopt.call(id_to(player))?;
        Ok(Self {
            lua,
            outbox,
            call_started,
            dispatch,
            player,
        })
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
            "set_model",
            lua.create_function(move |_, (id, model): (f64, String)| {
                if let Some(number) = model.strip_prefix('*').and_then(|n| n.parse().ok()) {
                    sim.borrow_mut().set_model(id_from(id), number);
                }
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
            lua.create_function(move |_, text: String| {
                out.borrow_mut().prints.push(text);
                Ok(())
            })?,
        )?;
        let out = Rc::clone(outbox);
        engine.set(
            "changelevel",
            lua.create_function(move |_, name: String| {
                out.borrow_mut().level = Some(name);
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
            lua.create_function(move |_, (yaw, pitch): (f32, f32)| {
                out.borrow_mut().view = Some((yaw, pitch));
                Ok(())
            })?,
        )?;
        Self::register_sound(lua, simulation, outbox, &engine)?;
        lua.globals().set("engine", engine)
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
                        own: id == sim.player_id(),
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
                .push(format!("script error: {error}"));
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
        let (kind, id, other) = match callback {
            Callback::Think(id) => ("think", id, None),
            Callback::Blocked { id, other } => ("blocked", id, Some(other)),
            Callback::Touch { id, other } => ("touch", id, Some(other)),
        };
        self.report(
            self.dispatch
                .call::<()>((kind, id_to(id), other.map(id_to))),
        );
    }

    /// Runs `name` on the player, if the game has one.
    fn run_on_player(&self, name: &str) {
        self.call_started.set(Instant::now());
        let function: mlua::Result<Option<Function>> = self.lua.globals().get(name);
        match function {
            Ok(Some(function)) => self.report(function.call::<()>(self.player.clone())),
            Ok(None) => {}
            Err(error) => self.report(Err(error)),
        }
    }

    /// Runs the game's `PlayerPreThink`.
    pub fn player_pre_think(&self) {
        self.run_on_player("PlayerPreThink");
    }

    /// Runs the game's `PlayerPostThink`.
    pub fn player_post_think(&self) {
        self.run_on_player("PlayerPostThink");
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
