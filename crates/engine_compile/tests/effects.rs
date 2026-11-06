//! Reading effects.

use engine_compile::read_effect;
use engine_format::from_text;
use engine_map::{EmitterShape, ParticleBlend, ParticleKind};

/// The effect in `text`.
fn read(text: &str) -> Result<engine_map::EffectDef, engine_compile::CompileError> {
    read_effect("test", &from_text(text).expect("parses"))
}

#[test]
fn an_emitter_has_what_it_says() {
    let effect = read(
        "format 1 effect
emitter sparks {
    kind = ribbon
    blend = add
    rate = 12
    burst = 3
    shape = ball
    radius = 8
    life = 0.5 2
    speed = 10
    spread = 30
    gravity = 0.5
    size = 0 4 1 0
    color = 0 1 0 0 1 1 0 0 1 0
    on_death = sparks
}
",
    )
    .expect("reads");
    let emitter = &effect.emitters[0];
    assert_eq!(emitter.name, "sparks");
    assert_eq!(emitter.kind, ParticleKind::Ribbon);
    assert_eq!(emitter.blend, ParticleBlend::Add);
    assert_eq!(emitter.rate, 12.0);
    assert_eq!(emitter.burst, 3);
    assert_eq!(emitter.shape, EmitterShape::Ball(8.0));
    assert_eq!(emitter.life, [0.5, 2.0]);
    assert_eq!(emitter.speed, [10.0, 10.0]);
    assert_eq!(emitter.size.at(0.5), 2.0);
    assert_eq!(emitter.color.at(0.5), [0.5, 0.0, 0.5, 0.5]);
}

#[test]
fn a_steady_size_and_colour_are_one_value() {
    let effect = read("format 1 effect\nemitter a {\n size = 3\n color = 1 0.5 0.25 1\n}\n");
    let effect = effect.expect("reads");
    assert_eq!(effect.emitters[0].size.at(0.7), 3.0);
    assert_eq!(effect.emitters[0].color.at(0.7), [1.0, 0.5, 0.25, 1.0]);
}

#[test]
fn what_makes_no_sense_is_refused() {
    assert!(read("format 1 effect\nemitter a { kind = fish }\n").is_err());
    assert!(read("format 1 effect\nemitter a { blend = fish }\n").is_err());
    assert!(read("format 1 effect\nemitter a { shape = fish }\n").is_err());
    assert!(read("format 1 effect\nemitter a { life = 0 }\n").is_err());
    let lost = read("format 1 effect\nemitter a { on_death = b }\n");
    assert!(lost.is_err(), "there's no b");
    assert!(read("format 1 material\nsurface { base = a }\n").is_err());
}
