//! Reading effects (proposal 0114).

use engine_core::math::Vec3;
use engine_format::{Block, Document, Value};
use engine_map::{
    ColorCurve, Curve, EffectDef, EmitterShape, ParticleBlend, ParticleCollide, ParticleEmitter,
    ParticleForce, ParticleKind,
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

/// The columns and rows of a sheet of frames.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held to a sensible count"
)]
fn flipbook_of(block: &Block) -> [u32; 2] {
    match *numbers(block, "flipbook").as_slice() {
        [across, down, ..] => [across.clamp(1.0, 64.0) as u32, down.clamp(1.0, 64.0) as u32],
        _ => [1, 1],
    }
}

/// The force in `block`.
fn read_force(block: &Block) -> Result<ParticleForce, CompileError> {
    let at = vector(block, "at", Vec3::ZERO);
    let strength = number(block, "strength", 100.0);
    match block.get("kind").and_then(Value::text).unwrap_or("") {
        "attract" => Ok(ParticleForce::Attract {
            at,
            strength,
            reach: number(block, "reach", 0.0),
        }),
        "vortex" => Ok(ParticleForce::Vortex {
            at,
            axis: vector(block, "axis", Vec3::new(0.0, 0.0, 1.0)),
            strength,
            inward: number(block, "inward", 0.0),
        }),
        "wind" => Ok(ParticleForce::Wind {
            blowing: vector(block, "blowing", Vec3::new(100.0, 0.0, 0.0)),
            gusts: number(block, "gusts", 0.0),
        }),
        other => Err(CompileError::Bad(format!("no force is called {other:?}"))),
    }
}

/// The emitter in `block`.
fn read_emitter(block: &Block) -> Result<ParticleEmitter, CompileError> {
    let text = |key: &str| block.get(key).and_then(Value::text);
    let kind = match text("kind").unwrap_or("sprite") {
        "sprite" => ParticleKind::Sprite,
        "square" => ParticleKind::Square,
        "mesh" => ParticleKind::Mesh,
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
        "ring" => EmitterShape::Ring(number(block, "radius", 16.0)),
        "disc" => EmitterShape::Disc(number(block, "radius", 16.0)),
        "cone" => EmitterShape::Cone(number(block, "radius", 16.0), number(block, "height", 32.0)),
        "line" => EmitterShape::Line(vector(block, "half_size", Vec3::new(16.0, 0.0, 0.0))),
        "mesh" => EmitterShape::Surface,
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
    let mesh = text("mesh").map(str::to_owned);
    let surface_of = text("surface").map(str::to_owned);
    let named = block.name.as_deref().unwrap_or("emitter");
    if kind == ParticleKind::Mesh && mesh.is_none() {
        return Err(CompileError::Bad(format!(
            "{named} has to say what mesh it is"
        )));
    }
    if shape == EmitterShape::Surface && mesh.is_none() && surface_of.is_none() {
        return Err(CompileError::Bad(format!(
            "{named} has to say what mesh it starts on"
        )));
    }
    if text("gpu") == Some("sim") {
        let name = block.name.as_deref().unwrap_or("emitter");
        if kind != ParticleKind::Sprite {
            let problem = format!("{name} runs in compute, which only has sprites");
            return Err(CompileError::Bad(problem));
        }
        if text("on_death").is_some() || text("on_hit").is_some() {
            let problem = format!("{name} runs in compute, where nothing hears of what ends");
            return Err(CompileError::Bad(problem));
        }
    }
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
        flat_drag: number(block, "flat_drag", 0.0),
        turbulence: number(block, "turbulence", 0.0),
        size: curve(block, "size", 4.0),
        color: color_curve(block, "color"),
        reach: number(block, "reach", 200.0),
        gpu: block.get("gpu").and_then(Value::flag) == Some(true),
        sim: text("gpu") == Some("sim"),
        motion: text("motion").map(str::to_owned),
        field: text("field").map(str::to_owned),
        on_death: text("on_death").map(str::to_owned),
        mesh,
        surface_of,
        image: text("image").map(str::to_owned),
        flipbook: flipbook_of(block),
        flip_rate: number(block, "flip_rate", 0.0),
        shader: text("shader").map(str::to_owned),
        rotation: range(block, "rotation", 0.0),
        spin: range(block, "spin", 0.0),
        stretch: number(block, "stretch", 0.0),
        soft: number(block, "soft", 0.0),
        lit: block.get("lit").and_then(Value::flag) == Some(true),
        collide: match text("collide").unwrap_or("none") {
            "none" => ParticleCollide::None,
            "bounce" => ParticleCollide::Bounce,
            "stick" => ParticleCollide::Stick,
            "die" => ParticleCollide::Die,
            other => return Err(CompileError::Bad(format!("no collide is called {other}"))),
        },
        bounce: number(block, "bounce", 0.4),
        on_hit: text("on_hit").map(str::to_owned),
        forces: block
            .children_of("force")
            .map(read_force)
            .collect::<Result<Vec<_>, _>>()?,
        outward: block.get("outward").and_then(Value::flag) == Some(true),
        far: number(block, "far", 0.0),
        ..ParticleEmitter::default()
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
    let mut timeline = Vec::new();
    for block in document.blocks_of("timeline") {
        let at = number(block, "at", 0.0);
        let start = block.get("start").and_then(Value::text);
        let start = start.ok_or_else(|| {
            CompileError::Bad(format!("{name}: a timeline has to say what it starts"))
        })?;
        timeline.push((at, start.to_owned()));
    }
    timeline.sort_by(|a, b| a.0.total_cmp(&b.0));
    let started = emitters
        .iter()
        .flat_map(|emitter| [&emitter.on_death, &emitter.on_hit])
        .flatten()
        .chain(timeline.iter().map(|(_, start)| start));
    for next in started {
        if emitters.iter().all(|other| other.name != *next) {
            return Err(CompileError::Bad(format!(
                "{name}: no emitter is called {next}"
            )));
        }
    }
    Ok(EffectDef {
        name: name.to_owned(),
        emitters,
        timeline,
    })
}
