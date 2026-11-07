//! Being in water of any shape.

use std::sync::Arc;

use engine_core::math::Vec3;
use engine_core::player::Liquid;
use engine_map::{Contents, Leaf, Map, PoolLook, TerrainData, TerrainSheet, Tree, WaterSheet};
use engine_physics::{CollisionWorld, PointContents};

/// Slime 100 deep from the origin to 64 each way, with its top at 50.
fn slime() -> WaterSheet {
    WaterSheet {
        origin: [0.0, 0.0],
        cell: 32.0,
        size: [3, 3],
        heights: vec![50.0; 9],
        depth: 100.0,
        contents: Contents::Slime,
        look: PoolLook::of(Contents::Slime).expect("a liquid"),
    }
}

/// A world with `map`'s things in it, in open air.
fn world(map: Map) -> CollisionWorld {
    let air = Leaf {
        contents: Contents::Empty,
        visibility: None,
        first_face: 0,
        face_count: 0,
        ambient: [0; 4],
    };
    let tree = Tree {
        leaves: vec![air],
        ..Tree::default()
    };
    CollisionWorld::new(Arc::new(Map { tree, ..map }))
}

#[test]
fn a_point_under_a_sheet_is_in_it() {
    let world = world(Map {
        water_sheets: vec![slime()],
        ..Map::default()
    });
    let inside = Vec3::new(32.0, 32.0, 20.0);
    assert_eq!(world.contents(inside), PointContents::Liquid(Liquid::Slime));
    assert_eq!(world.map_contents(inside), Contents::Slime);
    let over = Vec3::new(32.0, 32.0, 60.0);
    assert_ne!(world.contents(over), PointContents::Liquid(Liquid::Slime));
    let off = Vec3::new(500.0, 32.0, 20.0);
    assert_ne!(world.contents(off), PointContents::Liquid(Liquid::Slime));
}

/// Land goes through water, and what's in land is in land.
#[test]
fn land_comes_before_water() {
    let world = world(Map {
        water_sheets: vec![slime()],
        terrain: vec![TerrainSheet {
            data: TerrainData::flat([1, 1], 32.0),
            ..TerrainSheet::default()
        }],
        ..Map::default()
    });
    let buried = Vec3::new(32.0, 32.0, -10.0);
    assert_eq!(world.contents(buried), PointContents::Solid);
    let above = Vec3::new(32.0, 32.0, 10.0);
    assert_eq!(world.contents(above), PointContents::Liquid(Liquid::Slime));
}
