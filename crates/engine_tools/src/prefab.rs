//! Prefabs: small Luau programs that build geometry to order (proposal 0073).
//!
//! A prefab file returns a table with a `build(params)` function. It gets back
//! plain brushes and entities, so once placed a prefab is ordinary map data,
//! and the format never hears of it.

use std::path::Path;

use engine_format::{Block, Value};
use engine_script::{ScriptValue, run_builder};
use serde_json::Value as Json;

use crate::json::move_block;

/// What a prefab made.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PrefabOutput {
    /// World brushes and entities, ready to go in a map.
    pub blocks: Vec<Block>,
}

/// The names of the prefabs in `content`.
#[must_use]
pub fn prefab_names(content: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(content.join("prefabs"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            name.strip_suffix(".luau").map(str::to_owned)
        })
        .collect();
    names.sort();
    names
}

/// A document value from a script's.
fn value_from_script(value: &ScriptValue) -> Option<Value> {
    match value {
        ScriptValue::Flag(flag) => Some(Value::Flag(*flag)),
        ScriptValue::Number(number) => Some(Value::Number(*number)),
        ScriptValue::Text(text) => Some(Value::Text(text.clone())),
        ScriptValue::List(items) => items
            .iter()
            .map(value_from_script)
            .collect::<Option<Vec<_>>>()
            .map(Value::List),
        ScriptValue::Nil | ScriptValue::Fields(_) => None,
    }
}

/// A block from `{ block = "brush", cuboid = {...}, ... }`.
fn block_from_script(value: &ScriptValue, offset: [f64; 3]) -> Result<Block, String> {
    let ScriptValue::Fields(fields) = value else {
        return Err("a prefab must return blocks".to_owned());
    };
    let Some(ScriptValue::Text(kind)) = value.get("block") else {
        return Err("a prefab's block needs a `block` kind".to_owned());
    };
    let mut block = Block::new(kind);
    for (key, field) in fields {
        match (key.as_str(), field) {
            ("block", _) => {}
            ("name", ScriptValue::Text(name)) => block.name = Some(name.clone()),
            ("children", ScriptValue::List(children)) => {
                for child in children {
                    block.children.push(block_from_script(child, offset)?);
                }
            }
            _ => {
                let value =
                    value_from_script(field).ok_or_else(|| format!("bad value for {key}"))?;
                block.set(key, value);
            }
        }
    }
    move_block(&mut block, offset);
    Ok(block)
}

/// A script value from JSON.
fn script_from_json(json: &Json) -> ScriptValue {
    match json {
        Json::Null => ScriptValue::Nil,
        Json::Bool(flag) => ScriptValue::Flag(*flag),
        Json::Number(number) => ScriptValue::Number(number.as_f64().unwrap_or(0.0)),
        Json::String(text) => ScriptValue::Text(text.clone()),
        Json::Array(items) => ScriptValue::List(items.iter().map(script_from_json).collect()),
        Json::Object(object) => ScriptValue::Fields(
            object
                .iter()
                .map(|(key, value)| (key.clone(), script_from_json(value)))
                .collect(),
        ),
    }
}

/// Runs the prefab `name` with `params` as JSON, placing it at `at`.
///
/// # Errors
///
/// Returns a message if it's missing or fails.
pub fn build_prefab(
    content: &Path,
    name: &str,
    params: &Json,
    at: [f64; 3],
) -> Result<PrefabOutput, String> {
    if name.contains(['/', '\\', '.']) {
        return Err(format!("bad prefab name {name}"));
    }
    let path = content.join("prefabs").join(format!("{name}.luau"));
    let source =
        std::fs::read_to_string(&path).map_err(|error| format!("prefab {name}: {error}"))?;
    let made = run_builder(name, &source, &script_from_json(params))?;
    let ScriptValue::List(items) = made else {
        return Err(format!("prefab {name} must return a list of blocks"));
    };
    let mut output = PrefabOutput::default();
    for item in &items {
        output.blocks.push(block_from_script(item, at)?);
    }
    Ok(output)
}
