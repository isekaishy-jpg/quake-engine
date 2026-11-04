//! Saving and restoring what scripts hold (proposal 0044).
//!
//! Functions are saved by their global names, so anything a script keeps in a
//! field or global has to be one. A closure can't be saved, and saying which
//! one it was is how that gets noticed.

use std::collections::{HashMap, HashSet};
use std::ffi::c_void;

use mlua::{Function, Table, Value, Vector};
use serde::{Deserialize, Serialize};

use crate::game::{Game, ScriptError};

/// How deep a plain table may go.
const MAX_DEPTH: u32 = 8;

/// The number a script's stand-in for the world goes by.
const NO_WORLD_ID: f64 = -1.0;

/// Globals that belong to the host.
const HOST_GLOBALS: [&str; 2] = ["engine", "_G"];

/// A value a script holds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SavedValue {
    /// True or false.
    Flag(bool),
    /// A number.
    Number(f64),
    /// Text.
    Text(String),
    /// A vector.
    Vector([f32; 3]),
    /// An entity, by its number.
    Entity(f64),
    /// A function, by its global name.
    Function(String),
    /// A plain table.
    Table(Vec<(SavedValue, SavedValue)>),
}

/// An entity's fields.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedEntity {
    /// Its number.
    pub id: f64,
    /// Its fields.
    pub fields: Vec<(String, SavedValue)>,
}

/// Everything scripts hold.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SavedScripts {
    /// Globals that aren't functions.
    pub globals: Vec<(String, SavedValue)>,
    /// Every entity that something refers to, removed ones included.
    pub entities: Vec<SavedEntity>,
    /// The state of `random`.
    pub random_state: f64,
}

/// A walk over what scripts hold.
struct Saver {
    /// Every global function's name.
    names: HashMap<*const c_void, String>,
    /// Entities to save.
    waiting: Vec<(f64, Table)>,
    /// Entities seen.
    seen: HashSet<u64>,
}

/// The number of `table`, if it's an entity.
fn entity_id(table: &Table) -> Option<f64> {
    table.metatable()?;
    match table.raw_get::<Value>("_id").ok()? {
        Value::Number(number) => Some(number),
        #[expect(clippy::cast_precision_loss, reason = "ids fit in 2^53")]
        Value::Integer(number) => Some(number as f64),
        _ => None,
    }
}

impl Saver {
    /// Notes that `entity` has to be saved, and returns its number.
    fn note_entity(&mut self, id: f64, entity: &Table) -> SavedValue {
        if self.seen.insert(id.to_bits()) && id != NO_WORLD_ID {
            self.waiting.push((id, entity.clone()));
        }
        SavedValue::Entity(id)
    }

    /// `value` as it's saved. `None` is nothing to keep. `place` says where it
    /// is, for an error.
    fn value(
        &mut self,
        value: &Value,
        place: &str,
        depth: u32,
    ) -> Result<Option<SavedValue>, ScriptError> {
        Ok(Some(match value {
            Value::Boolean(flag) => SavedValue::Flag(*flag),
            Value::Number(number) => SavedValue::Number(*number),
            #[expect(clippy::cast_precision_loss, reason = "script numbers are doubles")]
            Value::Integer(number) => SavedValue::Number(*number as f64),
            Value::String(text) => SavedValue::Text(text.to_string_lossy()),
            Value::Vector(vector) => SavedValue::Vector([vector.x(), vector.y(), vector.z()]),
            Value::Function(_) => {
                let name = self.names.get(&value.to_pointer()).ok_or_else(|| {
                    ScriptError::new(format!("{place} holds a function with no global name"))
                })?;
                SavedValue::Function(name.clone())
            }
            Value::Table(table) => {
                if let Some(id) = entity_id(table) {
                    self.note_entity(id, table)
                } else {
                    self.plain_table(table, place, depth)?
                }
            }
            _ => return Ok(None),
        }))
    }

    /// A table that isn't an entity.
    fn plain_table(
        &mut self,
        table: &Table,
        place: &str,
        depth: u32,
    ) -> Result<SavedValue, ScriptError> {
        if depth >= MAX_DEPTH {
            return Err(ScriptError::new(format!("{place} is too deep to save")));
        }
        let mut pairs = Vec::new();
        for pair in table.pairs::<Value, Value>() {
            let (key, value) = pair?;
            let inner = format!("{place}.{key:?}");
            let key = self.value(&key, &inner, depth + 1)?;
            let value = self.value(&value, &inner, depth + 1)?;
            if let (Some(key), Some(value)) = (key, value) {
                pairs.push((key, value));
            }
        }
        Ok(SavedValue::Table(pairs))
    }

    /// An entity's fields.
    fn entity(&mut self, id: f64, table: &Table) -> Result<SavedEntity, ScriptError> {
        let class: Option<String> = table.raw_get("classname").ok();
        let class = class.unwrap_or_default();
        let mut fields = Vec::new();
        for pair in table.pairs::<String, Value>() {
            let (name, value) = pair?;
            let place = format!("{class}.{name}");
            if let Some(value) = self.value(&value, &place, 0)? {
                fields.push((name, value));
            }
        }
        fields.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(SavedEntity { id, fields })
    }
}

impl Game {
    /// What scripts hold now.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] if something can't be saved.
    pub fn save(&self) -> Result<SavedScripts, ScriptError> {
        let lua = self.lua();
        let globals = lua.globals();
        let mut saver = Saver {
            names: HashMap::new(),
            waiting: Vec::new(),
            seen: HashSet::new(),
        };
        let mut values = Vec::new();
        for pair in globals.pairs::<String, Value>() {
            let (name, value) = pair?;
            if HOST_GLOBALS.contains(&name.as_str()) {
                continue;
            }
            if matches!(value, Value::Function(_)) {
                saver.names.insert(value.to_pointer(), name);
            } else {
                values.push((name, value));
            }
        }
        let mut saved_globals = Vec::new();
        for (name, value) in values {
            if let Some(value) = saver.value(&value, &name, 0)? {
                saved_globals.push((name, value));
            }
        }
        saved_globals.sort_by(|a, b| a.0.cmp(&b.0));
        let live: Function = globals.get("saved_entities")?;
        let live: Table = live.call(())?;
        for pair in live.pairs::<Value, Table>() {
            let (_id, entity) = pair?;
            if let Some(id) = entity_id(&entity) {
                let _number = saver.note_entity(id, &entity);
            }
        }
        let mut entities = Vec::new();
        while let Some((id, table)) = saver.waiting.pop() {
            entities.push(saver.entity(id, &table)?);
        }
        entities.sort_by(|a, b| a.id.total_cmp(&b.id));
        let random: Function = globals.get("random_seed")?;
        Ok(SavedScripts {
            globals: saved_globals,
            entities,
            random_state: random.call(())?,
        })
    }

    /// A saved value as a script's.
    fn restored_value(
        &self,
        value: &SavedValue,
        tables: &HashMap<u64, Table>,
    ) -> Result<Value, ScriptError> {
        let lua = self.lua();
        Ok(match value {
            SavedValue::Flag(flag) => Value::Boolean(*flag),
            SavedValue::Number(number) => Value::Number(*number),
            SavedValue::Text(text) => Value::String(lua.create_string(text)?),
            SavedValue::Vector([x, y, z]) => Value::Vector(Vector::new(*x, *y, *z)),
            SavedValue::Entity(id) => {
                if *id == NO_WORLD_ID {
                    let stand_in: Function = lua.globals().get("no_world_entity")?;
                    Value::Table(stand_in.call(())?)
                } else {
                    let table = tables.get(&id.to_bits());
                    Value::Table(
                        table
                            .ok_or_else(|| ScriptError::new(format!("no entity {id}")))?
                            .clone(),
                    )
                }
            }
            SavedValue::Function(name) => {
                let function: Option<Function> = lua.globals().get(name.as_str())?;
                Value::Function(
                    function.ok_or_else(|| ScriptError::new(format!("no function {name}")))?,
                )
            }
            SavedValue::Table(pairs) => {
                let table = lua.create_table()?;
                for (key, value) in pairs {
                    table.raw_set(
                        self.restored_value(key, tables)?,
                        self.restored_value(value, tables)?,
                    )?;
                }
                Value::Table(table)
            }
        })
    }

    /// Puts back what [`Game::save`] gave.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] if something in it doesn't fit these scripts.
    pub fn restore(&self, saved: &SavedScripts) -> Result<(), ScriptError> {
        let lua = self.lua();
        let globals = lua.globals();
        let make: Function = globals.get("restored_entity")?;
        let mut tables = HashMap::with_capacity(saved.entities.len());
        for entity in &saved.entities {
            let removed = entity
                .fields
                .iter()
                .any(|(name, value)| name == "_removed" && *value == SavedValue::Flag(true));
            let table: Table = make.call((entity.id, removed))?;
            tables.insert(entity.id.to_bits(), table);
        }
        for entity in &saved.entities {
            if let Some(table) = tables.get(&entity.id.to_bits()) {
                for (name, value) in &entity.fields {
                    table.raw_set(name.as_str(), self.restored_value(value, &tables)?)?;
                }
            }
        }
        // Whatever isn't in the save was empty when it was made.
        let mut stale = Vec::new();
        for pair in globals.pairs::<String, Value>() {
            let (name, value) = pair?;
            if !matches!(value, Value::Function(_)) && !HOST_GLOBALS.contains(&name.as_str()) {
                stale.push(name);
            }
        }
        for name in stale {
            globals.raw_set(name, Value::Nil)?;
        }
        for (name, value) in &saved.globals {
            globals.raw_set(name.as_str(), self.restored_value(value, &tables)?)?;
        }
        let random: Function = globals.get("random_seed")?;
        random.call::<f64>(saved.random_state)?;
        Ok(())
    }
}
