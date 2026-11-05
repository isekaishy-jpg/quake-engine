//! Running small build scripts for tools, such as prefabs (proposal 0073).
//!
//! These run in their own sandbox, with no access to the game.

use mlua::{Function, Lua, LuaOptions, StdLib, Table, Value};

/// A value going in or out of a build script.
#[derive(Clone, Debug, PartialEq)]
pub enum ScriptValue {
    /// Nothing.
    Nil,
    /// A flag.
    Flag(bool),
    /// A number.
    Number(f64),
    /// Text.
    Text(String),
    /// An ordered list.
    List(Vec<Self>),
    /// Named fields, sorted by name.
    Fields(Vec<(String, Self)>),
}

impl ScriptValue {
    /// The field called `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Self> {
        let Self::Fields(fields) = self else {
            return None;
        };
        fields
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }
}

/// A script value from Luau.
fn from_lua(value: &Value) -> Result<ScriptValue, String> {
    Ok(match value {
        Value::Nil => ScriptValue::Nil,
        Value::Boolean(flag) => ScriptValue::Flag(*flag),
        Value::Integer(number) => {
            #[expect(clippy::cast_precision_loss, reason = "script values are small")]
            let number = *number as f64;
            ScriptValue::Number(number)
        }
        Value::Number(number) => ScriptValue::Number(*number),
        Value::String(text) => {
            ScriptValue::Text(text.to_str().map_err(|e| e.to_string())?.to_string())
        }
        Value::Table(table) => table_from_lua(table)?,
        other => return Err(format!("a build script returned a {}", other.type_name())),
    })
}

/// A list or named fields from a table.
fn table_from_lua(table: &Table) -> Result<ScriptValue, String> {
    if table.raw_len() > 0 {
        let mut items = Vec::new();
        for item in table.sequence_values::<Value>() {
            items.push(from_lua(&item.map_err(|e| e.to_string())?)?);
        }
        return Ok(ScriptValue::List(items));
    }
    let mut fields = Vec::new();
    for pair in table.pairs::<String, Value>() {
        let (key, value) = pair.map_err(|e| e.to_string())?;
        fields.push((key, from_lua(&value)?));
    }
    fields.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(ScriptValue::Fields(fields))
}

/// A Luau value from a script value.
fn to_lua(lua: &Lua, value: &ScriptValue) -> mlua::Result<Value> {
    Ok(match value {
        ScriptValue::Nil => Value::Nil,
        ScriptValue::Flag(flag) => Value::Boolean(*flag),
        ScriptValue::Number(number) => Value::Number(*number),
        ScriptValue::Text(text) => Value::String(lua.create_string(text)?),
        ScriptValue::List(items) => {
            let table = lua.create_table()?;
            for (index, item) in items.iter().enumerate() {
                table.set(index + 1, to_lua(lua, item)?)?;
            }
            Value::Table(table)
        }
        ScriptValue::Fields(fields) => {
            let table = lua.create_table()?;
            for (key, item) in fields {
                table.set(key.as_str(), to_lua(lua, item)?)?;
            }
            Value::Table(table)
        }
    })
}

/// Runs a script returning a table with `build(params)`, and returns what
/// `build` made.
///
/// # Errors
///
/// Returns a message if the script fails or returns something unusable.
pub fn run_builder(name: &str, source: &str, params: &ScriptValue) -> Result<ScriptValue, String> {
    let failed = |error: mlua::Error| format!("{name}: {error}");
    let lua = Lua::new_with(
        StdLib::MATH | StdLib::TABLE | StdLib::STRING,
        LuaOptions::new(),
    )
    .map_err(failed)?;
    let table: Table = lua.load(source).set_name(name).eval().map_err(failed)?;
    let build: Function = table
        .get("build")
        .map_err(|_| format!("{name} has no build function"))?;
    let made: Value = build
        .call(to_lua(&lua, params).map_err(failed)?)
        .map_err(failed)?;
    from_lua(&made)
}
