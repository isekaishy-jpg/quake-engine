//! The status bar and menus as a script (proposal 0118).
//!
//! The script runs on the client, in a sandbox of its own, with no reach into the
//! game. It's told what's going on, and says what to draw and do, which the engine
//! then carries out, so nothing it does can get in the way of a frame.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use mlua::{Function, Lua, Table, Value};

use crate::game::ScriptError;

/// What a script asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum UiCommand {
    /// Changes camera, by what it's called in text, over some seconds.
    Camera {
        /// The camera.
        camera: String,
        /// How long it takes to blend to it.
        blend: f32,
    },
    /// A picture at its own size, by name.
    Picture {
        /// Its name.
        name: String,
        /// Its top left corner.
        at: [f32; 2],
    },
    /// A picture stretched over a rectangle, as left, top, right and bottom.
    Stretched {
        /// Its name.
        name: String,
        /// Where it goes.
        place: [f32; 4],
        /// How solid it is.
        alpha: f32,
    },
    /// A rectangle of colour.
    Fill {
        /// Where it goes.
        place: [f32; 4],
        /// Its colour.
        color: [f32; 4],
    },
    /// Text.
    Text {
        /// `regular`, `bold` or `display`.
        style: String,
        /// Where it starts.
        at: [f32; 2],
        /// How tall it is.
        size: f32,
        /// What it says.
        text: String,
        /// Its colour.
        color: [f32; 4],
        /// How much of its width goes left of `at`: 0, 0.5 or 1.
        align: f32,
    },
    /// A frame from the kit, which keeps its corners.
    Frame {
        /// Its name.
        name: String,
        /// Where it goes.
        place: [f32; 4],
        /// What it's multiplied by.
        color: [f32; 4],
    },
    /// An icon or control from the kit.
    Piece {
        /// Its name.
        name: String,
        /// Where it goes.
        place: [f32; 4],
        /// What it's multiplied by.
        color: [f32; 4],
    },
    /// A moving picture, `age` seconds in.
    Moving {
        /// Its name.
        name: String,
        /// Where it goes.
        place: [f32; 4],
        /// How long it's been going.
        age: f32,
    },
    /// A line for the console.
    Run(String),
    /// A sound with no place.
    Sound(String),
    /// Closes the menu.
    Close,
}

/// What a script is told.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiState {
    /// The screen's width in units.
    pub width: f32,
    /// The screen's height in units.
    pub height: f32,
    /// Seconds since the engine started.
    pub time: f32,
    /// Whether a game is going.
    pub in_game: bool,
    /// Numbers about the game, by name.
    pub stats: BTreeMap<String, f64>,
    /// The settings it asked for, by name.
    pub settings: BTreeMap<String, f64>,
}

/// The status bar and menus.
pub struct UiScript {
    /// The script's state.
    lua: Lua,
    /// What it's asked for so far.
    out: Rc<RefCell<Vec<UiCommand>>>,
}

/// Four numbers from a table, with `fallback` for what's missing.
fn four(table: Option<Table>, fallback: [f32; 4]) -> [f32; 4] {
    let Some(table) = table else {
        return fallback;
    };
    let at = |index: i32, default: f32| table.get::<f32>(index).unwrap_or(default);
    [
        at(1, fallback[0]),
        at(2, fallback[1]),
        at(3, fallback[2]),
        at(4, fallback[3]),
    ]
}

impl UiScript {
    /// Runs `source`, which sets up the functions the engine calls.
    ///
    /// # Errors
    ///
    /// Returns an error if the script fails.
    pub fn new(name: &str, source: &str) -> Result<Self, ScriptError> {
        let lua = Lua::new();
        let out = Rc::new(RefCell::new(Vec::new()));
        register(&lua, &out)?;
        lua.sandbox(true)?;
        lua.load(source).set_name(name).exec()?;
        Ok(Self { lua, out })
    }

    /// The settings it wants to be told about.
    #[must_use]
    pub fn wanted_settings(&self) -> Vec<String> {
        let list: Option<Table> = self.lua.globals().get("settings").ok();
        list.map_or_else(Vec::new, |list| {
            list.sequence_values::<String>().flatten().collect()
        })
    }

    /// `state` as a table.
    fn state_table(&self, state: &UiState) -> Result<Table, ScriptError> {
        let table = self.lua.create_table()?;
        table.set("width", state.width)?;
        table.set("height", state.height)?;
        table.set("time", state.time)?;
        table.set("in_game", state.in_game)?;
        let numbers = |map: &BTreeMap<String, f64>| -> Result<Table, ScriptError> {
            let table = self.lua.create_table()?;
            for (name, value) in map {
                table.set(name.as_str(), *value)?;
            }
            Ok(table)
        };
        table.set("stats", numbers(&state.stats)?)?;
        table.set("settings", numbers(&state.settings)?)?;
        Ok(table)
    }

    /// Calls the function called `name`, if the script has one, and returns what
    /// it asked for and what it gave back. A script that fails gives nothing.
    fn call(
        &self,
        name: &str,
        state: &UiState,
        argument: Option<&str>,
    ) -> Result<(Vec<UiCommand>, bool), ScriptError> {
        let Ok(function) = self.lua.globals().get::<Function>(name) else {
            return Ok((Vec::new(), false));
        };
        let table = self.state_table(state)?;
        let answer: Value = match argument {
            Some(text) => function.call((table, text))?,
            None => function.call(table)?,
        };
        let commands = std::mem::take(&mut *self.out.borrow_mut());
        Ok((commands, answer.as_boolean().unwrap_or(false)))
    }

    /// Whether it draws the status bar.
    #[must_use]
    pub fn draws_status(&self) -> bool {
        self.lua.globals().get::<Function>("draw_status").is_ok()
    }

    /// What to draw for the status bar.
    ///
    /// # Errors
    ///
    /// Returns an error if the script fails.
    pub fn draw_status(&self, state: &UiState) -> Result<Vec<UiCommand>, ScriptError> {
        Ok(self.call("draw_status", state, None)?.0)
    }

    /// What to draw for the menu.
    ///
    /// # Errors
    ///
    /// Returns an error if the script fails.
    pub fn draw_menu(&self, state: &UiState) -> Result<Vec<UiCommand>, ScriptError> {
        Ok(self.call("draw_menu", state, None)?.0)
    }

    /// Tells it the menu's opening.
    ///
    /// # Errors
    ///
    /// Returns an error if the script fails.
    pub fn open_menu(&self, state: &UiState) -> Result<Vec<UiCommand>, ScriptError> {
        Ok(self.call("open_menu", state, None)?.0)
    }

    /// Says a level has begun, for what a game wants from the start, such as its camera.
    ///
    /// # Errors
    ///
    /// Returns an error if the script fails.
    pub fn level_started(&self, state: &UiState) -> Result<Vec<UiCommand>, ScriptError> {
        Ok(self.call("level_started", state, None)?.0)
    }

    /// Hands `key` to the menu, and returns what it asked for.
    ///
    /// # Errors
    ///
    /// Returns an error if the script fails.
    pub fn menu_key(&self, state: &UiState, key: &str) -> Result<Vec<UiCommand>, ScriptError> {
        Ok(self.call("menu_key", state, Some(key))?.0)
    }
}

/// Adds the `ui` table.
fn register(lua: &Lua, out: &Rc<RefCell<Vec<UiCommand>>>) -> Result<(), ScriptError> {
    let ui = lua.create_table()?;
    let push = |command: UiCommand, out: &Rc<RefCell<Vec<UiCommand>>>| {
        out.borrow_mut().push(command);
    };
    let sink = Rc::clone(out);
    ui.set(
        "picture",
        lua.create_function(move |_, (name, x, y): (String, f32, f32)| {
            push(UiCommand::Picture { name, at: [x, y] }, &sink);
            Ok(())
        })?,
    )?;
    let sink = Rc::clone(out);
    ui.set(
        "stretched",
        lua.create_function(
            move |_, (name, place, alpha): (String, Table, Option<f32>)| {
                let place = four(Some(place), [0.0; 4]);
                let alpha = alpha.unwrap_or(1.0);
                push(UiCommand::Stretched { name, place, alpha }, &sink);
                Ok(())
            },
        )?,
    )?;
    let sink = Rc::clone(out);
    ui.set(
        "fill",
        lua.create_function(move |_, (place, color): (Table, Option<Table>)| {
            let place = four(Some(place), [0.0; 4]);
            let color = four(color, [0.0, 0.0, 0.0, 1.0]);
            push(UiCommand::Fill { place, color }, &sink);
            Ok(())
        })?,
    )?;
    let sink = Rc::clone(out);
    ui.set(
        "text",
        lua.create_function(move |_, (text, options): (String, Table)| {
            let style = options
                .get::<String>("style")
                .unwrap_or_else(|_| "regular".to_owned());
            let x = options.get::<f32>("x").unwrap_or(0.0);
            let y = options.get::<f32>("y").unwrap_or(0.0);
            let size = options.get::<f32>("size").unwrap_or(12.0);
            let color = four(options.get::<Table>("color").ok(), [1.0; 4]);
            let align = options.get::<f32>("align").unwrap_or(0.0);
            push(
                UiCommand::Text {
                    style,
                    at: [x, y],
                    size,
                    text,
                    color,
                    align,
                },
                &sink,
            );
            Ok(())
        })?,
    )?;
    let sink = Rc::clone(out);
    ui.set(
        "frame",
        lua.create_function(
            move |_, (name, place, color): (String, Table, Option<Table>)| {
                let place = four(Some(place), [0.0; 4]);
                let color = four(color, [1.0; 4]);
                push(UiCommand::Frame { name, place, color }, &sink);
                Ok(())
            },
        )?,
    )?;
    let sink = Rc::clone(out);
    ui.set(
        "piece",
        lua.create_function(
            move |_, (name, place, color): (String, Table, Option<Table>)| {
                let place = four(Some(place), [0.0; 4]);
                let color = four(color, [1.0; 4]);
                push(UiCommand::Piece { name, place, color }, &sink);
                Ok(())
            },
        )?,
    )?;
    let sink = Rc::clone(out);
    ui.set(
        "moving",
        lua.create_function(move |_, (name, place, age): (String, Table, f32)| {
            let place = four(Some(place), [0.0; 4]);
            push(UiCommand::Moving { name, place, age }, &sink);
            Ok(())
        })?,
    )?;
    let sink = Rc::clone(out);
    ui.set(
        "run",
        lua.create_function(move |_, line: String| {
            push(UiCommand::Run(line), &sink);
            Ok(())
        })?,
    )?;
    let sink = Rc::clone(out);
    ui.set(
        "sound",
        lua.create_function(move |_, name: String| {
            push(UiCommand::Sound(name), &sink);
            Ok(())
        })?,
    )?;
    let sink = Rc::clone(out);
    ui.set(
        "camera",
        lua.create_function(move |_, (camera, blend): (String, Option<f32>)| {
            let blend = blend.unwrap_or(0.0);
            push(UiCommand::Camera { camera, blend }, &sink);
            Ok(())
        })?,
    )?;
    let sink = Rc::clone(out);
    ui.set(
        "close",
        lua.create_function(move |_, ()| {
            push(UiCommand::Close, &sink);
            Ok(())
        })?,
    )?;
    lua.globals().set("ui", ui)?;
    Ok(())
}
