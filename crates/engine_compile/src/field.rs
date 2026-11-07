//! Fields as a level names them (proposal 0145).

use std::sync::Arc;

use engine_format::{Block, Value};
use engine_map::{FieldDef, SHADER_PARAMS, ShaderCode};

use crate::error::CompileError;

/// The widest a field can be.
const WIDEST: u32 = 2048;

/// How many steps a second a field takes unless it says.
const USUAL_RATE: f32 = 30.0;

/// What a level says of a field.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FieldSource {
    /// What it's called.
    pub name: String,
    /// The name of its shader.
    pub shader: String,
    /// How wide it is, each way.
    pub size: u32,
    /// How many steps a second it takes.
    pub rate: f32,
    /// What it's given.
    pub params: [f32; SHADER_PARAMS],
    /// Its shader's code, once it's been read.
    pub code: Arc<ShaderCode>,
}

impl FieldSource {
    /// It as the map has it.
    pub(crate) fn def(&self) -> FieldDef {
        FieldDef {
            name: self.name.clone(),
            shader: Arc::clone(&self.code),
            size: self.size,
            rate: self.rate,
            params: self.params,
        }
    }
}

/// Reads a `field` block.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a field's size and rate are small"
)]
pub(crate) fn read_field(block: &Block) -> Result<FieldSource, CompileError> {
    let text = |key: &str| block.get(key).and_then(Value::text);
    let name = text("name").ok_or_else(|| CompileError::Bad("a field needs a name".into()))?;
    let bad = |what: &str| CompileError::Bad(format!("field {name} {what}"));
    let shader = text("shader").ok_or_else(|| bad("needs a shader"))?;
    let size = block.get("size").and_then(Value::number).unwrap_or(256.0) as u32;
    if size == 0 || size > WIDEST {
        return Err(bad(&format!("has to be from 1 to {WIDEST} wide")));
    }
    let rate = block
        .get("rate")
        .and_then(Value::number)
        .map_or(USUAL_RATE, |rate| rate as f32);
    if rate <= 0.0 {
        return Err(bad("needs a rate above 0"));
    }
    let mut params = [0.0; SHADER_PARAMS];
    let given = block.get("shader_params").and_then(Value::numbers);
    for (param, value) in params.iter_mut().zip(given.unwrap_or_default()) {
        *param = value as f32;
    }
    Ok(FieldSource {
        name: name.to_owned(),
        shader: shader.to_owned(),
        size,
        rate,
        params,
        code: Arc::default(),
    })
}
