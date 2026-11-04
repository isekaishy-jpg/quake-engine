//! Water and sky by distance, on a map of one leaf.

use std::sync::Arc;

use engine_client::AmbientSounds;
use engine_core::math::Vec3;
use engine_map::{Face, Leaf, Map, Texture, TextureKind, Tree};

/// A map with one leaf holding a small square of each named texture, in order
/// along x, 1000 units apart, with these stored levels.
fn map(names: &[&str], ambient: [u8; 4]) -> Arc<Map> {
    let mut map = Map::default();
    for (number, name) in names.iter().enumerate() {
        let index = u32::try_from(number).expect("few");
        let x = 1000.0 * index as f32;
        map.textures.push(Texture {
            name: (*name).to_owned(),
            kind: TextureKind::Solid,
            mips: Vec::new(),
            animation: None,
            alternate: None,
        });
        map.points.extend([
            Vec3::new(x, 0.0, 0.0),
            Vec3::new(x + 10.0, 0.0, 0.0),
            Vec3::new(x + 10.0, 10.0, 0.0),
        ]);
        map.faces.push(Face {
            first_point: index * 3,
            point_count: 3,
            texture: index,
            axes: Default::default(),
            light: None,
        });
    }
    map.tree = Tree {
        leaves: vec![Leaf {
            first_face: 0,
            face_count: u32::try_from(names.len()).expect("few"),
            ambient,
            ..Leaf::default()
        }],
        leaf_faces: (0..u32::try_from(names.len()).expect("few")).collect(),
        ..Tree::default()
    };
    Arc::new(map)
}

/// The levels above the first face, `height` away.
fn levels_at(sounds: &mut AmbientSounds, height: f32) -> [u8; 2] {
    sounds.levels(Vec3::new(5.0, 5.0, height))
}

#[test]
fn water_is_full_nearby_and_fades_to_nothing_with_distance() {
    let mut sounds = AmbientSounds::new(map(&["*water0"], [255, 255, 0, 0]));
    assert_eq!(levels_at(&mut sounds, 50.0), [255, 0]);
    assert_eq!(levels_at(&mut sounds, 100.0), [255, 0]);
    assert_eq!(levels_at(&mut sounds, 300.0), [128, 0]);
    assert_eq!(levels_at(&mut sounds, 500.0), [0, 0]);
    assert_eq!(levels_at(&mut sounds, 900.0), [0, 0]);
}

#[test]
fn the_fade_has_no_steps_in_it() {
    let mut sounds = AmbientSounds::new(map(&["sky4"], [255, 255, 0, 0]));
    let mut last = 255_i32;
    for height in (100..500).step_by(5) {
        let level = i32::from(levels_at(&mut sounds, height as f32)[1]);
        assert!(level <= last && last - level <= 4, "at {height}");
        last = level;
    }
}

#[test]
fn sky_is_apart_from_water() {
    let mut sounds = AmbientSounds::new(map(&["sky4"], [255, 255, 0, 0]));
    assert_eq!(levels_at(&mut sounds, 50.0), [0, 255]);
}

#[test]
fn the_nearest_surface_counts() {
    let mut sounds = AmbientSounds::new(map(&["*water0", "*water1"], [255, 0, 0, 0]));
    assert_eq!(sounds.levels(Vec3::new(960.0, 5.0, 0.0))[0], 255);
    assert_eq!(sounds.levels(Vec3::new(5.0, 5.0, 0.0))[0], 255);
    assert_eq!(
        sounds.levels(Vec3::new(505.0, 5.0, 0.0))[0],
        3,
        "495 from each"
    );
}

#[test]
fn slime_sounds_like_water_and_lava_is_silent() {
    let mut slime = AmbientSounds::new(map(&["*slime0"], [255, 255, 0, 0]));
    assert_eq!(levels_at(&mut slime, 50.0), [255, 0]);
    let mut lava = AmbientSounds::new(map(&["*lava1"], [255, 255, 0, 0]));
    assert_eq!(levels_at(&mut lava, 50.0), [0, 0]);
    let mut wall = AmbientSounds::new(map(&["wall5"], [255, 255, 0, 0]));
    assert_eq!(levels_at(&mut wall, 50.0), [0, 0]);
}

#[test]
fn names_match_in_any_case() {
    let mut sounds = AmbientSounds::new(map(&["SKY1"], [255, 255, 0, 0]));
    assert_eq!(levels_at(&mut sounds, 50.0), [0, 255]);
}

#[test]
fn a_level_the_map_stores_is_a_limit() {
    let mut quiet = AmbientSounds::new(map(&["*water0"], [40, 255, 0, 0]));
    assert_eq!(levels_at(&mut quiet, 50.0), [40, 0]);
    let mut silent = AmbientSounds::new(map(&["*water0"], [0, 255, 0, 0]));
    assert_eq!(levels_at(&mut silent, 50.0), [0, 0]);
}

#[test]
fn a_map_with_nothing_to_hear_is_silent() {
    let mut sounds = AmbientSounds::new(Arc::new(Map::default()));
    assert_eq!(sounds.levels(Vec3::ZERO), [0, 0]);
}
