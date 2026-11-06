//! Reading effects (proposal 0114).

use engine_core::math::Vec3;
use engine_format::{Block, Document, Value};
use engine_map::{
    ColorCurve, Curve, EffectDef, EmitterShape, ParticleBlend, ParticleEmitter, ParticleKind,
};

use crate::error::CompileError;

/// A field's numbers.
#[expect(clippy::cast_possible_truncation, reason = "effects store f32")]
fn numbers(block: &Block, key: &str) -> Vec<f32> {
    let value = block.get(key);
    let one = value.and_then(Value::number).map(|one| vec![one]);
    let many = value.and_then(Value::numbers);
    many.or(one)
        .unwrap_or_default()
        .iter()
        .map(|number| *number as f32)
        .collect()
}

/// A field that's one number.
fn number(block: &Block, key: &str, default: f32) -> f32 {
    numbers(block, key).first().copied().unwrap_or(default)
}

/// A field that's one number or a low and a high one.
fn range(block: &Block, key: &str, default: f32) -> [f32; 2] {
    match *numbers(block, key).as_slice() {
        [low, high, ..] => [low, high],
        [one] => [one, one],
        [] => [default, default],
    }
}

/// A field that's a place or direction.
fn vector(block: &Block, key: &str, default: Vec3) -> Vec3 {
    match *numbers(block, key).as_slice() {
        [x, y, z, ..] => Vec3::new(x, y, z),
        _ => default,
    }
}

/// A curve: one number for a steady value, or pairs of a time and a value.
fn curve(block: &Block, key: &str, default: f32) -> Curve {
    let found = numbers(block, key);
    match found.as_slice() {
        [] => Curve(vec![(0.0, default)]),
        [one] => Curve(vec![(0.0, *one)]),
        many => Curve(
            many.as_chunks::<2>()
                .0
                .iter()
                .map(|[at, value]| (*at, *value))
                .collect(),
        ),
    }
}

/// A colour curve: four numbers for a steady one, or a time and four numbers for
/// each stop.
fn color_curve(block: &Block, key: &str) -> ColorCurve {
    let found = numbers(block, key);
    match found.as_slice() {
        [] => ColorCurve(vec![(0.0, [1.0; 4])]),
        [r, g, b] => ColorCurve(vec![(0.0, [*r, *g, *b, 1.0])]),
        [r, g, b, a] => ColorCurve(vec![(0.0, [*r, *g, *b, *a])]),
        many => ColorCurve(
            many.as_chunks::<5>()
                .0
                .iter()
                .map(|[at, r, g, b, a]| (*at, [*r, *g, *b, *a]))
                .collect(),
        ),
    }
}

/// The emitter in `block`.
fn read_emitter(block: &Block) -> Result<ParticleEmitter, CompileError> {
    let text = |key: &str| block.get(key).and_then(Value::text);
    let kind = match text("kind").unwrap_or("sprite") {
        "sprite" => ParticleKind::Sprite,
        "ribbon" => ParticleKind::Ribbon,
        "beam" => ParticleKind::Beam,
        "light" => ParticleKind::Light,
        "decal" => ParticleKind::Decal,
        other => return Err(CompileError::Bad(format!("no particle is a {other}"))),
    };
    let blend = match text("blend").unwrap_or("over") {
        "over" => ParticleBlend::Over,
        "add" => ParticleBlend::Add,
        other => return Err(CompileError::Bad(format!("no blend is called {other}"))),
    };
    let shape = match text("shape").unwrap_or("point") {
        "point" => EmitterShape::Point,
        "ball" => EmitterShape::Ball(number(block, "radius", 16.0)),
        "box" => EmitterShape::Box(vector(block, "half_size", Vec3::new(16.0, 16.0, 16.0))),
        other => return Err(CompileError::Bad(format!("no shape is called {other}"))),
    };
    let life = range(block, "life", 1.0);
    if life[0] <= 0.0 || life[1] < life[0] {
        return Err(CompileError::Bad(
            "a particle has to live a while".to_owned(),
        ));
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "held to a sensible count"
    )]
    let burst = number(block, "burst", 0.0).clamp(0.0, 65_536.0) as u32;
    Ok(ParticleEmitter {
        name: block.name.clone().unwrap_or_default(),
        kind,
        blend,
        rate: number(block, "rate", 0.0),
        burst,
        duration: number(block, "duration", 0.0),
        delay: number(block, "delay", 0.0),
        shape,
        life,
        speed: range(block, "speed", 0.0),
        direction: vector(block, "direction", Vec3::new(0.0, 0.0, 1.0)),
        spread: number(block, "spread", 180.0),
        gravity: number(block, "gravity", 0.0),
        drag: number(block, "drag", 0.0),
        turbulence: number(block, "turbulence", 0.0),
        size: curve(block, "size", 4.0),
        color: color_curve(block, "color"),
        reach: number(block, "reach", 200.0),
        gpu: block.get("gpu").and_then(Value::flag) == Some(true),
        on_death: text("on_death").map(str::to_owned),
    })
}

/// Reads an effect called `name` from `document`.
///
/// # Errors
///
/// Returns an error if it isn't an effect, or an emitter makes no sense.
pub fn read_effect(name: &str, document: &Document) -> Result<EffectDef, CompileError> {
    if document.kind != "effect" {
        return Err(CompileError::Bad(format!("{name} isn't an effect")));
    }
    let mut emitters = Vec::new();
    for block in document.blocks_of("emitter") {
        emitters.push(
            read_emitter(block).map_err(|error| CompileError::Bad(format!("{name}: {error}")))?,
        );
    }
    for emitter in &emitters {
        if let Some(next) = &emitter.on_death
            && emitters.iter().all(|other| other.name != *next)
        {
            return Err(CompileError::Bad(format!(
                "{name}: no emitter is called {next}"
            )));
        }
    }
    Ok(EffectDef {
        name: name.to_owned(),
        emitters,
    })
}
