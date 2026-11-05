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

/// JSON from a document value.
pub(crate) fn value_to_json(value: &Value) -> Json {
    match value {
        Value::Number(number) => json!(number),
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
