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
            let stem = name.strip_suffix(".luau")?;
            (!stem.starts_with('_')).then(|| stem.to_owned())
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

/// `point` turned anticlockwise by `quarters` quarter turns, seen from above.
fn turned(point: [f64; 2], quarters: u8) -> [f64; 2] {
    (0..quarters % 4).fold(point, |[x, y], _| [-y, x])
}

/// Replaces the field `key` of `block`.
fn replace(block: &mut Block, key: &str, new: Value) {
    if let Some((_, value)) = block.fields.iter_mut().find(|(name, _)| name == key) {
        *value = new;
    }
}

/// Turns a three-number field of `block`.
fn turn_point(block: &mut Block, key: &str, quarters: u8) {
    let Some(numbers) = block.get(key).and_then(Value::numbers) else {
        return;
    };
    if let [x, y, z] = numbers.as_slice() {
        let [x, y] = turned([*x, *y], quarters);
        let list = [x, y, *z].map(Value::Number).to_vec();
        replace(block, key, Value::List(list));
    }
}

/// Turns `block` about the origin, before it's moved into place.
fn turn_block(block: &mut Block, quarters: u8) {
    turn_point(block, "origin", quarters);
    turn_point(block, "normal", quarters);
    if let Some(numbers) = block.get("cuboid").and_then(Value::numbers)
        && let [x0, y0, z0, x1, y1, z1] = numbers.as_slice()
    {
        let ([ax, ay], [bx, by]) = (turned([*x0, *y0], quarters), turned([*x1, *y1], quarters));
        let corners = [ax.min(bx), ay.min(by), *z0, ax.max(bx), ay.max(by), *z1];
        replace(
            block,
            "cuboid",
            Value::List(corners.map(Value::Number).to_vec()),
        );
    }
    if let Some(angle) = block.get("angle").and_then(Value::number)
        && angle >= 0.0
    {
        let angle = (angle + f64::from(quarters) * 90.0) % 360.0;
        replace(block, "angle", Value::Number(angle));
    }
}

/// A block from `{ block = "brush", cuboid = {...}, ... }`.
fn block_from_script(value: &ScriptValue, offset: [f64; 3], quarters: u8) -> Result<Block, String> {
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
                    block
                        .children
                        .push(block_from_script(child, offset, quarters)?);
                }
            }
            _ => {
                let value =
                    value_from_script(field).ok_or_else(|| format!("bad value for {key}"))?;
                block.set(key, value);
            }
        }
    }
    turn_block(&mut block, quarters);
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

/// `source` with the shapes prefabs share in `prefabs/_kit.luau` in front of it,
/// as `kit`, if there are any (proposal 0097).
fn with_kit(content: &Path, source: String) -> String {
    match std::fs::read_to_string(content.join("prefabs").join("_kit.luau")) {
        Ok(kit) => format!("local kit = (function()\n{kit}\nend)()\n{source}"),
        Err(_) => source,
    }
}

/// Runs the prefab `name` with `params` as JSON, placing it at `at`. A `turn`
/// among them turns it first, by quarter turns given in degrees.
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
    let source = with_kit(
        content,
        std::fs::read_to_string(&path).map_err(|error| format!("prefab {name}: {error}"))?,
    );
    let quarters = params.get("turn").and_then(Json::as_f64).unwrap_or(0.0) / 90.0;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "wrapped to four quarters"
    )]
    let quarters = quarters.round().rem_euclid(4.0) as u8;
    let made = run_builder(name, &source, &script_from_json(params))?;
    let ScriptValue::List(items) = made else {
        return Err(format!("prefab {name} must return a list of blocks"));
    };
    let mut output = PrefabOutput::default();
    for item in &items {
        output.blocks.push(block_from_script(item, at, quarters)?);
    }
    Ok(output)
}
