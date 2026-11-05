//! Between JSON and document values.

use engine_format::{Block, Value};
use serde_json::{Map, Value as Json, json};

/// A document value from JSON. Arrays become lists, and objects aren't values.
pub(crate) fn value_from_json(json: &Json) -> Option<Value> {
    match json {
        Json::Number(number) => number.as_f64().map(Value::Number),
        Json::String(text) => Some(Value::Text(text.clone())),
        Json::Bool(flag) => Some(Value::Flag(*flag)),
        Json::Array(items) => items
            .iter()
            .map(value_from_json)
            .collect::<Option<Vec<_>>>()
            .map(Value::List),
        Json::Null | Json::Object(_) => None,
    }
}

/// JSON for a number, as a whole number where it is one.
fn number_to_json(number: f64) -> Json {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "checked to be whole and small"
    )]
    let whole = number as i64;
    #[expect(clippy::cast_precision_loss, reason = "compared only when small")]
    let exact = number.fract() == 0.0 && number.abs() < 9e15 && whole as f64 == number;
    if exact { json!(whole) } else { json!(number) }
}

/// JSON from a document value.
pub(crate) fn value_to_json(value: &Value) -> Json {
    match value {
        Value::Number(number) => number_to_json(*number),
        Value::Text(text) => json!(text),
        Value::Flag(flag) => json!(flag),
        Value::List(values) => Json::Array(values.iter().map(value_to_json).collect()),
        Value::Bytes(bytes) => json!(bytes.len()),
    }
}

/// JSON describing a block.
pub(crate) fn block_to_json(block: &Block) -> Json {
    let mut out = Map::new();
    out.insert("kind".to_owned(), json!(block.kind));
    if let Some(name) = &block.name {
        out.insert("name".to_owned(), json!(name));
    }
    for (key, value) in &block.fields {
        out.insert(key.clone(), value_to_json(value));
    }
    if !block.children.is_empty() {
        out.insert(
            "children".to_owned(),
            Json::Array(block.children.iter().map(block_to_json).collect()),
        );
    }
    Json::Object(out)
}

/// Three numbers from JSON.
pub(crate) fn point_from_json(json: &Json) -> Option<[f64; 3]> {
    let items = json.as_array()?;
    let [x, y, z] = items.as_slice() else {
        return None;
    };
    Some([x.as_f64()?, y.as_f64()?, z.as_f64()?])
}

/// A list value of numbers.
pub(crate) fn numbers(values: &[f64]) -> Value {
    Value::List(values.iter().map(|value| Value::Number(*value)).collect())
}

/// Fields that are places, so that they move with what holds them, and how many
/// numbers go round: a box's two corners are six, and a line's points are three.
const PLACES: [(&str, usize); 6] = [
    ("cuboid", 6),
    ("origin", 3),
    ("corner", 3),
    ("points", 3),
    ("path", 3),
    ("keyframes", 3),
];

/// Moves what's in `block` by `by`, leaving its children to be moved themselves.
pub(crate) fn move_block(block: &mut Block, by: [f64; 3]) {
    if block.kind == "face" {
        let normal = block.get("normal").and_then(Value::numbers);
        let distance = block.get("distance").and_then(Value::number);
        if let (Some(normal), Some(distance)) = (normal, distance)
            && normal.len() == 3
        {
            let length = normal.iter().map(|part| part * part).sum::<f64>().sqrt();
            let along: f64 = normal.iter().zip(by).map(|(a, b)| a * b).sum();
            let moved_distance = distance + along / length.max(1e-9);
            for (key, value) in &mut block.fields {
                if key == "distance" {
                    *value = Value::Number(moved_distance);
                }
            }
        }
        return;
    }
    // A stencil's light is a place, unless it's a sun's direction.
    let sun = block.get("sun").is_some_and(|value| {
        value.flag() == Some(true) || value.number().is_some_and(|n| n != 0.0)
    });
    for (key, value) in &mut block.fields {
        let group = PLACES
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, size)| *size);
        let group = group.or_else(|| (key == "light" && !sun).then_some(3));
        let (Some(group), Some(mut parts)) = (group, value.numbers()) else {
            continue;
        };
        if parts.is_empty() || parts.len() % group != 0 {
            continue;
        }
        for (index, part) in parts.iter_mut().enumerate() {
            *part += by[index % 3];
        }
        *value = numbers(&parts);
    }
}
