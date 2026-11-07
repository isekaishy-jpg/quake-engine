//! Reading effects.

use engine_compile::read_effect;
use engine_format::from_text;
use engine_map::{EmitterShape, ParticleBlend, ParticleCollide, ParticleForce, ParticleKind};

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

#[test]
fn an_emitter_says_how_it_looks() {
    let effect = read(
        "format 1 effect
emitter puff {
    image = textures/puff.png
    flipbook = 4 2
    flip_rate = 12
    shader = shimmer
    rotation = 0 360
    spin = -30 30
    stretch = 0.02
    soft = 24
    lit = true
}
",
    )
    .expect("it reads");
    let emitter = &effect.emitters[0];
    assert_eq!(emitter.image.as_deref(), Some("textures/puff.png"));
    assert_eq!(emitter.flipbook, [4, 2]);
    assert!((emitter.flip_rate - 12.0).abs() < f32::EPSILON);
    assert_eq!(emitter.shader.as_deref(), Some("shimmer"));
    assert_eq!(emitter.rotation, [0.0, 360.0]);
    assert_eq!(emitter.spin, [-30.0, 30.0]);
    assert!((emitter.stretch - 0.02).abs() < f32::EPSILON);
    assert!((emitter.soft - 24.0).abs() < f32::EPSILON);
    assert!(emitter.lit);
}

/// A level's effects' pictures and shaders are read once each, and each emitter's told where.
#[test]
fn a_level_holds_what_its_effects_show() {
    let root = std::env::temp_dir().join(format!("engine-effects-{}", std::process::id()));
    let _gone = std::fs::remove_dir_all(&root);
    for folder in ["maps", "effects", "textures", "shaders"] {
        std::fs::create_dir_all(root.join(folder)).expect("folder");
    }
    let picture = |side: u32| {
        let pixels = [200, 100, 50, 255].repeat((side * side) as usize);
        engine_compile::encode_png(side, side, &pixels).expect("encodes")
    };
    std::fs::write(root.join("textures/small.png"), picture(16)).expect("write");
    std::fs::write(root.join("textures/large.png"), picture(64)).expect("write");
    std::fs::write(
        root.join("shaders/tint.wgsl"),
        "fn shader_particle(look: ParticleLook) -> vec4<f32> { return look.color; }
",
    )
    .expect("write");
    std::fs::write(
        root.join("effects/one.eeffect"),
        "format 1 effect
emitter a { image = textures/small.png shader = tint }
emitter b { image = textures/large.png }
emitter c { image = textures/small.png shader = tint }
emitter d { }
",
    )
    .expect("write");
    let level = "format 1 map
entity world { type = worldspawn }
brush { material = rock cuboid = -64 -64 -96 64 64 -64 }
";
    std::fs::write(root.join("maps/level.emap"), level).expect("write");
    let mut filesystem = engine_assets::FileSystem::new();
    filesystem.mount(Box::new(engine_assets::DirectoryArchive::new(&root)));
    let document = engine_compile::document_of(level.as_bytes()).expect("reads");
    let played = engine_compile::play_map(&filesystem, "level", &document).expect("loads");
    let map = played.map;
    assert_eq!(map.effect_pictures.len(), 2, "each once");
    for picture in &map.effect_pictures {
        assert_eq!(picture[0].width, 64, "all of one size");
    }
    let emitters = &map.effects[0].emitters;
    let numbers: Vec<u32> = emitters.iter().map(|emitter| emitter.picture).collect();
    assert_eq!(numbers, [1, 2, 1, 0]);
    let shaders: Vec<u32> = emitters
        .iter()
        .map(|emitter| emitter.shader_number)
        .collect();
    assert_eq!(shaders, [1, 0, 1, 0]);
    assert_eq!(map.effect_shaders.len(), 1);
    assert!(
        played
            .pictures
            .iter()
            .any(|path| path.ends_with("large.png")),
        "a pack keeps it"
    );
    let _gone = std::fs::remove_dir_all(&root);
}

#[test]
fn an_emitter_says_what_moves_it_and_what_it_does() {
    let effect = read(
        "format 1 effect
timeline { at = 1.5 start = bang }
timeline { at = 0.5 start = bang }
emitter drop {
    shape = ring
    radius = 40
    outward = true
    collide = bounce
    bounce = 0.7
    on_hit = bang
    far = 2000
    force { kind = attract at = 0 0 50 strength = 200 reach = 300 }
    force { kind = vortex axis = 0 1 0 strength = 90 inward = 10 }
    force { kind = wind blowing = 10 20 0 gusts = 0.5 }
}
emitter bang {
    shape = cone
    radius = 10
    height = 30
    burst = 4
}
",
    )
    .expect("it reads");
    let emitter = &effect.emitters[0];
    assert_eq!(emitter.shape, EmitterShape::Ring(40.0));
    assert!(emitter.outward);
    assert_eq!(emitter.collide, ParticleCollide::Bounce);
    assert!((emitter.bounce - 0.7).abs() < f32::EPSILON);
    assert_eq!(emitter.on_hit.as_deref(), Some("bang"));
    assert!((emitter.far - 2000.0).abs() < f32::EPSILON);
    assert_eq!(emitter.forces.len(), 3);
    assert!(matches!(emitter.forces[0], ParticleForce::Attract { reach, .. } if reach == 300.0));
    assert!(matches!(emitter.forces[1], ParticleForce::Vortex { inward, .. } if inward == 10.0));
    assert!(matches!(emitter.forces[2], ParticleForce::Wind { gusts, .. } if gusts == 0.5));
    assert_eq!(effect.emitters[1].shape, EmitterShape::Cone(10.0, 30.0));
    let times: Vec<f32> = effect.timeline.iter().map(|(at, _)| *at).collect();
    assert_eq!(times, [0.5, 1.5], "in order");
}

#[test]
fn what_starts_what_has_to_be_there() {
    for text in [
        "format 1 effect\nemitter a { on_hit = gone }\n",
        "format 1 effect\nemitter a { on_death = gone }\n",
        "format 1 effect\nemitter a { }\ntimeline { at = 1 start = gone }\n",
        "format 1 effect\nemitter a { }\ntimeline { at = 1 }\n",
        "format 1 effect\nemitter a { collide = wobble }\n",
        "format 1 effect\nemitter a { force { kind = wobble } }\n",
    ] {
        assert!(read(text).is_err(), "{text}");
    }
}

/// `gpu = sim` runs in compute, and says what it moves by and reads.
#[test]
fn an_emitter_can_run_in_compute() {
    let effect = read(
        "format 1 effect
emitter rain { gpu = sim motion = sway field = wind }
emitter dust { gpu = true }
emitter ash { }
",
    )
    .expect("reads");
    let [rain, dust, ash] = &effect.emitters[..] else {
        panic!("three of them");
    };
    assert!(rain.sim && !rain.gpu);
    assert_eq!(rain.motion.as_deref(), Some("sway"));
    assert_eq!(rain.field.as_deref(), Some("wind"));
    assert!(dust.gpu && !dust.sim);
    assert!(!ash.gpu && !ash.sim);
}

/// What compute can't do is an error where it's written, not something that doesn't happen.
#[test]
fn compute_turns_down_what_it_cannot_do() {
    for (line, told) in [
        ("gpu = sim kind = ribbon", "sprites"),
        ("gpu = sim on_death = next", "ends"),
        ("gpu = sim on_hit = next", "ends"),
    ] {
        let text = format!("format 1 effect\nemitter rain {{ {line} }}\nemitter next {{ }}\n");
        let problem = read(&text).expect_err("turned down").to_string();
        assert!(
            problem.contains(told) && problem.contains("rain"),
            "{problem}"
        );
    }
}

/// A level holds each motion once, and numbers emitters' fields as it has them.
#[test]
fn a_level_numbers_what_compute_reads() {
    let root = std::env::temp_dir().join(format!("engine-sims-{}", std::process::id()));
    let _gone = std::fs::remove_dir_all(&root);
    for folder in ["maps", "effects", "shaders"] {
        std::fs::create_dir_all(root.join(folder)).expect("folder");
    }
    std::fs::write(
        root.join("shaders/sway.wgsl"),
        "fn shader_motion(particle: SimParticle, seconds: f32) -> SimParticle { return particle; }
",
    )
    .expect("write");
    std::fs::write(
        root.join("shaders/flow.wgsl"),
        "fn shader_field(at: vec2<i32>, size: vec2<i32>) -> vec4<f32> { return vec4<f32>(0.0); }
",
    )
    .expect("write");
    std::fs::write(
        root.join("effects/one.eeffect"),
        "format 1 effect
emitter a { gpu = sim motion = sway field = second }
emitter b { gpu = sim motion = sway field = first }
emitter c { gpu = sim field = unknown }
emitter d { gpu = sim }
",
    )
    .expect("write");
    let level = "format 1 map
field { name = first shader = flow }
field { name = second shader = flow }
entity world { type = worldspawn }
brush { material = rock cuboid = -64 -64 -96 64 64 -64 }
";
    std::fs::write(root.join("maps/level.emap"), level).expect("write");
    let mut filesystem = engine_assets::FileSystem::new();
    filesystem.mount(Box::new(engine_assets::DirectoryArchive::new(&root)));
    let document = engine_compile::document_of(level.as_bytes()).expect("reads");
    let played = engine_compile::play_map(&filesystem, "level", &document).expect("loads");
    let map = played.map;
    assert_eq!(map.effect_motions.len(), 1);
    assert!(map.effect_motions[0].code.contains("shader_motion"));
    let found = map
        .effects
        .iter()
        .find(|effect| effect.name == "one")
        .expect("there");
    let numbers: Vec<_> = found
        .emitters
        .iter()
        .map(|emitter| (emitter.motion_number, emitter.field_number))
        .collect();
    assert_eq!(numbers, [(1, 2), (1, 1), (0, 0), (0, 0)]);
    let _gone = std::fs::remove_dir_all(&root);
}

/// What's dated is what effects come from: their files, and what those name.
#[test]
fn what_effects_come_from_can_be_watched() {
    let root = std::env::temp_dir().join(format!("engine-watch-{}", std::process::id()));
    let _gone = std::fs::remove_dir_all(&root);
    for folder in ["effects", "shaders", "textures"] {
        std::fs::create_dir_all(root.join(folder)).expect("folder");
    }
    let pixels = [200, 100, 50, 255].repeat(16);
    let picture = engine_compile::encode_png(4, 4, &pixels).expect("encodes");
    std::fs::write(root.join("textures/dot.png"), &picture).expect("write");
    std::fs::write(root.join("textures/other.png"), &picture).expect("write");
    std::fs::write(
        root.join("shaders/tint.wgsl"),
        "fn shader_particle(look: ParticleLook) -> vec4<f32> { return look.color; }
",
    )
    .expect("write");
    let path = root.join("effects/one.eeffect");
    let text = "format 1 effect\nemitter a { image = textures/dot.png shader = tint }\n";
    std::fs::write(&path, text).expect("write");
    let mut filesystem = engine_assets::FileSystem::new();
    filesystem.mount(Box::new(engine_assets::DirectoryArchive::new(&root)));
    let files = engine_compile::effect_files(&filesystem);
    let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        [
            "effects/one.eeffect",
            "shaders/tint.wgsl",
            "textures/dot.png"
        ]
    );
    let read = engine_compile::read_effects(&filesystem, &[]).expect("reads");
    assert_eq!(
        (read.effects.len(), read.pictures.len(), read.shaders.len()),
        (1, 1, 1)
    );
    // Clocks on files can be coarse, so the date's moved by hand.
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
    let file = std::fs::File::options()
        .write(true)
        .open(root.join("textures/dot.png"))
        .expect("opens");
    file.set_modified(later).expect("dates");
    assert_ne!(engine_compile::effect_files(&filesystem), files);
    std::fs::write(&path, "this isn't an effect").expect("write");
    let told = engine_compile::read_effects(&filesystem, &[]).expect_err("wrong");
    assert!(told.to_string().contains("effects/one.eeffect"), "{told}");
    let _gone = std::fs::remove_dir_all(&root);
}

/// What has no dates isn't watched.
#[test]
fn what_ships_has_nothing_to_watch() {
    let filesystem = engine_assets::FileSystem::new();
    assert!(engine_compile::effect_files(&filesystem).is_empty());
}

/// One that's being changed takes the place of the one in the files, or stands beside them.
#[test]
fn an_effect_being_changed_goes_with_the_rest() {
    let root = std::env::temp_dir().join(format!("engine-changing-{}", std::process::id()));
    let _gone = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("effects")).expect("folder");
    let old = "format 1 effect\nemitter a { rate = 1 }\n";
    std::fs::write(root.join("effects/one.eeffect"), old).expect("write");
    std::fs::write(root.join("effects/two.eeffect"), old).expect("write");
    let mut filesystem = engine_assets::FileSystem::new();
    filesystem.mount(Box::new(engine_assets::DirectoryArchive::new(&root)));
    let new = "format 1 effect\nemitter a { rate = 9 }\n";
    let edited = |name: &str| read_effect(name, &from_text(new).expect("parses")).expect("reads");
    let read = engine_compile::read_effects_with(&filesystem, &[], edited("one")).expect("reads");
    assert_eq!(read.effects.len(), 2);
    let one = read.effects.iter().find(|effect| effect.name == "one");
    assert_eq!(one.expect("there").emitters[0].rate, 9.0);
    let read = engine_compile::read_effects_with(&filesystem, &[], edited("new")).expect("reads");
    assert_eq!(read.effects.len(), 3);
    let _gone = std::fs::remove_dir_all(&root);
}
