//! What's drawn of effects, from what they say.

use engine_core::math::Vec3;
use engine_map::{
    ColorCurve, Curve, EffectDraw, EffectSettings, EmitterShape, GpuEmitter, ParticleBlend,
    ParticleCollide, ParticleEmitter, ParticleForce, ParticleKind, SimEmitter,
};
use engine_render::{GpuParticles, ParticleDraw, ParticleShape, ParticleSim};

/// A draw of `kind`, in plain colours.
fn draw_of(kind: ParticleKind, color: [f32; 4]) -> EffectDraw {
    EffectDraw {
        origin: Vec3::new(1.0, 2.0, 3.0),
        end: Vec3::new(4.0, 5.0, 6.0),
        color,
        size: 7.0,
        kind,
        adds: true,
        rotation: 0.5,
        soft: 3.0,
        picture: 2,
        frame: 1.5,
        sheet: [2, 3],
        life: 0.25,
        shader: 4,
    }
}

#[test]
fn a_draw_keeps_what_it_says() {
    let drawn = ParticleDraw::from(&draw_of(ParticleKind::Sprite, [1.0, 1.0, 1.0, 0.5]));
    assert_eq!(drawn.origin, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(drawn.end, Vec3::new(4.0, 5.0, 6.0));
    assert_eq!(drawn.size, 7.0);
    assert!(drawn.adds);
    assert_eq!(drawn.rotation, 0.5);
    assert_eq!(drawn.soft, 3.0);
    assert_eq!(drawn.picture, 2);
    assert_eq!(drawn.frame, 1.5);
    assert_eq!(drawn.sheet, [2, 3]);
    assert_eq!(drawn.life, 0.25);
    assert_eq!(drawn.shader, 4);
    assert_eq!(drawn.alpha, 128);
}

#[test]
fn each_kind_has_its_shape() {
    for (kind, shape) in [
        (ParticleKind::Sprite, ParticleShape::Soft),
        (ParticleKind::Light, ParticleShape::Soft),
        (ParticleKind::Ribbon, ParticleShape::Line),
        (ParticleKind::Beam, ParticleShape::Line),
        (ParticleKind::Decal, ParticleShape::Flat),
    ] {
        let drawn = ParticleDraw::from(&draw_of(kind, [1.0; 4]));
        assert_eq!(drawn.shape, shape, "{kind:?}");
    }
}

/// A colour past 1 glows by that much, and what's left fits a byte.
#[test]
fn a_bright_colour_is_a_glow_and_a_colour() {
    let drawn = ParticleDraw::from(&draw_of(ParticleKind::Sprite, [4.0, 2.0, 0.0, 1.0]));
    assert_eq!(drawn.glow, 4.0);
    assert_eq!(drawn.color.red, 255);
    assert!((180..190).contains(&drawn.color.green), "{:?}", drawn.color);
    assert_eq!(drawn.color.blue, 0);
    let plain = ParticleDraw::from(&draw_of(ParticleKind::Sprite, [0.5, 0.25, 0.0, 1.0]));
    assert_eq!(plain.glow, 1.0);
    assert!(
        (185..190).contains(&plain.color.red),
        "curved: {:?}",
        plain.color
    );
}

#[test]
fn what_the_gpu_works_out_keeps_what_it_says() {
    let emitter = GpuEmitter {
        origin: Vec3::new(1.0, 2.0, 3.0),
        direction: Vec3::new(0.0, 0.0, 1.0),
        spread: 0.5,
        age: 2.0,
        count: 30,
        life: 3.0,
        speed: 40.0,
        gravity: 5.0,
        turbulence: 6.0,
        size: [7.0, 8.0],
        color: [0.1, 0.2, 0.3, 0.4],
        end_color: [0.5, 0.6, 0.7, 0.8],
        adds: true,
        radius: 9.0,
        picture: 2,
        soft: 11.0,
    };
    let drawn = GpuParticles::from(&emitter);
    assert_eq!(drawn.origin, emitter.origin);
    assert_eq!(drawn.direction, emitter.direction);
    assert_eq!(drawn.spread, 0.5);
    assert_eq!(drawn.age, 2.0);
    assert_eq!(drawn.count, 30);
    assert_eq!(drawn.life, 3.0);
    assert_eq!(drawn.speed, 40.0);
    assert_eq!(drawn.gravity, 5.0);
    assert_eq!(drawn.turbulence, 6.0);
    assert_eq!(drawn.size, [7.0, 8.0]);
    assert_eq!(drawn.color, emitter.color);
    assert_eq!(drawn.end_color, emitter.end_color);
    assert!(drawn.adds);
    assert_eq!(drawn.radius, 9.0);
    assert_eq!(drawn.picture, 2);
    assert_eq!(drawn.soft, 11.0);
}

/// One step of an emitter.
fn step() -> SimEmitter {
    SimEmitter {
        id: 3,
        origin: Vec3::new(1.0, 2.0, 3.0),
        count: 40,
        spawn: 5,
        seconds: 0.25,
        time: 9.0,
        gravity: 12.0,
        seed: 77,
        settings: EffectSettings::default(),
        fade: 1.0,
        emitter: 0,
    }
}

#[test]
fn a_step_of_compute_has_what_the_emitter_says() {
    let emitter = ParticleEmitter {
        speed: [10.0, 20.0],
        life: [1.0, 2.0],
        spread: 60.0,
        direction: Vec3::new(0.0, 0.0, 2.0),
        drag: 0.5,
        turbulence: 3.0,
        picture: 2,
        flipbook: [4, 2],
        flip_rate: 6.0,
        stretch: 0.1,
        rotation: [90.0, 180.0],
        spin: [0.0, 360.0],
        blend: ParticleBlend::Add,
        soft: 8.0,
        bounce: 0.3,
        collide: ParticleCollide::Stick,
        outward: true,
        shader_number: 4,
        motion_number: 5,
        field_number: 6,
        ..ParticleEmitter::default()
    };
    let sim = ParticleSim::of(&step(), &emitter, [1.0; 3]);
    assert_eq!(sim.id, 3);
    assert_eq!(sim.origin, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!((sim.count, sim.spawn, sim.seed), (40, 5, 77));
    assert_eq!((sim.seconds, sim.time, sim.gravity), (0.25, 9.0, 12.0));
    assert_eq!(sim.direction, Vec3::new(0.0, 0.0, 1.0), "of length 1");
    assert!((sim.spread - 0.5).abs() < 0.001, "as a cosine");
    assert_eq!(sim.speed, [10.0, 20.0]);
    assert_eq!(sim.life, [1.0, 2.0]);
    assert_eq!((sim.drag, sim.turbulence), (0.5, 3.0));
    assert_eq!((sim.picture, sim.sheet, sim.flip_rate), (2, [4, 2], 6.0));
    assert_eq!(sim.stretch, 0.1);
    assert!((sim.rotation[0] - std::f32::consts::FRAC_PI_2).abs() < 0.001);
    assert!(
        (sim.spin[1] - std::f32::consts::TAU).abs() < 0.001,
        "in radians"
    );
    assert!(sim.adds);
    assert_eq!((sim.soft, sim.bounce), (8.0, 0.3));
    assert_eq!(sim.collide, 3);
    assert!(sim.outward);
    assert_eq!((sim.shader, sim.motion, sim.field), (4, 5, 6));
}

#[test]
fn each_shape_says_what_compute_wants() {
    let half = Vec3::new(1.0, 2.0, 3.0);
    for (shape, number, radius, extent) in [
        (EmitterShape::Point, 0, 0.0, Vec3::ZERO),
        (EmitterShape::Ball(4.0), 0, 4.0, Vec3::ZERO),
        (EmitterShape::Ring(4.0), 1, 4.0, Vec3::ZERO),
        (EmitterShape::Disc(4.0), 2, 4.0, Vec3::ZERO),
        (
            EmitterShape::Cone(4.0, 5.0),
            3,
            4.0,
            Vec3::new(0.0, 0.0, 5.0),
        ),
        (EmitterShape::Line(half), 4, 0.0, half),
        (EmitterShape::Box(half), 5, 0.0, half),
    ] {
        let emitter = ParticleEmitter {
            shape,
            ..ParticleEmitter::default()
        };
        let sim = ParticleSim::of(&step(), &emitter, [1.0; 3]);
        assert_eq!(
            (sim.shape, sim.radius, sim.extent),
            (number, radius, extent)
        );
    }
}

#[test]
fn each_way_of_hitting_has_its_number() {
    for (collide, number) in [
        (ParticleCollide::None, 0),
        (ParticleCollide::Bounce, 1),
        (ParticleCollide::Die, 2),
        (ParticleCollide::Stick, 3),
    ] {
        let emitter = ParticleEmitter {
            collide,
            ..ParticleEmitter::default()
        };
        assert_eq!(ParticleSim::of(&step(), &emitter, [1.0; 3]).collide, number);
    }
}

/// Colour and size are read along a life, and what an instance says and light go over them.
#[test]
fn a_step_has_colours_and_sizes_along_a_life() {
    let emitter = ParticleEmitter {
        color: ColorCurve(vec![
            (0.0, [1.0, 1.0, 1.0, 1.0]),
            (1.0, [0.0, 0.0, 0.0, 0.5]),
        ]),
        size: Curve(vec![(0.0, 7.0), (1.0, 0.0)]),
        ..ParticleEmitter::default()
    };
    let faded = SimEmitter {
        settings: EffectSettings {
            color: [0.5, 1.0, 1.0],
            size: 2.0,
            strength: 1.0,
        },
        fade: 0.5,
        ..step()
    };
    let sim = ParticleSim::of(&faded, &emitter, [0.5, 0.25, 1.0]);
    assert_eq!(sim.ramp[0], [0.25, 0.25, 1.0, 0.5]);
    assert_eq!(sim.ramp[7], [0.0, 0.0, 0.0, 0.25]);
    assert!((sim.sizes[0] - 14.0).abs() < 0.001);
    assert!((sim.sizes[1] - 12.0).abs() < 0.001, "{:?}", sim.sizes);
    assert_eq!(sim.sizes[7], 0.0);
}

/// Forces come across as compute's numbers, up to as many as it has.
#[test]
fn a_step_has_its_forces() {
    let way = Vec3::new(0.0, 0.0, 1.0);
    let spot = Vec3::new(1.0, 2.0, 3.0);
    let emitter = ParticleEmitter {
        forces: vec![
            ParticleForce::Attract {
                at: spot,
                strength: 10.0,
                reach: 20.0,
            },
            ParticleForce::Vortex {
                at: spot,
                axis: way,
                strength: 30.0,
                inward: 40.0,
            },
            ParticleForce::Wind {
                blowing: spot,
                gusts: 0.5,
            },
        ],
        ..ParticleEmitter::default()
    };
    let sim = ParticleSim::of(&step(), &emitter, [1.0; 3]);
    let [one, two, three, none] = sim.forces;
    assert_eq!(
        (one.kind, one.strength, one.extra, one.at),
        (1.0, 10.0, 20.0, spot)
    );
    assert_eq!(
        (two.kind, two.strength, two.extra, two.axis),
        (2.0, 30.0, 40.0, way)
    );
    assert_eq!((three.kind, three.extra, three.at), (3.0, 0.5, spot));
    assert_eq!(none.kind, 0.0);
}
