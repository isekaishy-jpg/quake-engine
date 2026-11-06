//! Ways that aren't a walk: drops, jumps, water and lifts (proposal 0122).

use engine_compile::{
    BakeInput, CompileError, apply_bake, bake_light, compile, placeholder_texture, read_map,
    write_bake,
};
use engine_core::math::Vec3;
use engine_format::from_text;
use engine_map::{Map, MipLevel, NavWay};
use std::sync::Arc;

/// A map from `text`, baked.
fn baked(text: &str) -> Map {
    let document = from_text(text).expect("parses");
    let working = read_map(&document).expect("reads");
    let mut source = |name: &str| placeholder_texture(name);
    let mut map = compile(&working, &mut source).expect("compiles");
    let mut images =
        |_: &str| -> Result<Arc<Vec<MipLevel>>, CompileError> { Ok(Arc::new(Vec::new())) };
    let mut input = BakeInput {
        working: &working,
        images: &mut images,
    };
    bake_light(&mut map, &mut input).expect("bakes");
    map
}

/// A closed room with `extra` in it, from -1024 to 1024, its floor at 0 and with nothing
/// to light it, as paths don't need that.
fn room(extra: &str) -> Map {
    baked(&format!(
        "format 1 map
entity world {{ type = worldspawn light_spacing = 64 }}
brush {{ material = floor cuboid = -1056 -320 -320 1056 320 -288 }}
brush {{ material = roof cuboid = -1056 -320 512 1056 320 544 }}
brush {{ material = wall cuboid = -1056 -320 -288 -1024 320 512 }}
brush {{ material = wall cuboid = 1024 -320 -288 1056 320 512 }}
brush {{ material = wall cuboid = -1024 -320 -288 1024 -288 512 }}
brush {{ material = wall cuboid = -1024 288 -288 1024 320 512 }}
{extra}"
    ))
}

/// How the way from `from` to `to` goes.
fn ways(map: &Map, from: [f32; 3], to: [f32; 3]) -> Option<Vec<NavWay>> {
    let [fx, fy, fz] = from;
    let [tx, ty, tz] = to;
    let nav = map.nav.as_ref().expect("baked");
    let route = nav.find_route(Vec3::new(fx, fy, fz), Vec3::new(tx, ty, tz))?;
    Some(route.iter().map(|step| step.way).collect())
}

/// A shelf 96 high on the left, with the floor below it on the right.
const SHELF: &str = "brush { material = shelf cuboid = -1024 -288 -288 0 288 96 }\n";

#[test]
fn a_way_drops_off_an_edge_and_not_back_up() {
    let map = room(&format!(
        "{SHELF}brush {{ material = floor cuboid = 0 -288 -288 1024 288 0 }}"
    ));
    let down = ways(&map, [-300.0, 0.0, 120.0], [300.0, 0.0, 24.0]).expect("a way down");
    assert!(down.contains(&NavWay::Drop), "{down:?}");
    assert_eq!(
        ways(&map, [300.0, 0.0, 24.0], [-300.0, 0.0, 120.0]),
        None,
        "it's too high to get back"
    );
}

#[test]
fn a_way_jumps_a_gap_in_both_directions() {
    let map = room(
        "brush { material = floor cuboid = -1024 -288 -288 -64 288 0 }
         brush { material = floor cuboid = 64 -288 -288 1024 288 0 }",
    );
    let over = ways(&map, [-300.0, 0.0, 24.0], [300.0, 0.0, 24.0]).expect("a way over");
    assert!(over.contains(&NavWay::Jump), "{over:?}");
    let back = ways(&map, [300.0, 0.0, 24.0], [-300.0, 0.0, 24.0]).expect("a way back");
    assert!(back.contains(&NavWay::Jump), "{back:?}");
}

#[test]
fn a_gap_too_wide_is_not_jumped() {
    let map = room(
        "brush { material = floor cuboid = -1024 -288 -288 -200 288 0 }
         brush { material = floor cuboid = 200 -288 -288 1024 288 0 }",
    );
    let way = ways(&map, [-500.0, 0.0, 24.0], [500.0, 0.0, 24.0]);
    assert!(way.is_none(), "{way:?}");
}

/// A pool across the room, 128 deep, its top 16 below the floor.
const POOL: &str = "brush { material = floor cuboid = -1024 -288 -288 -128 288 0 }
    brush { material = floor cuboid = 128 -288 -288 1024 288 0 }
    brush { material = floor cuboid = -128 -288 -288 128 288 -144 }
    brush { material = water contents = water cuboid = -128 -288 -144 128 288 -16 }\n";

#[test]
fn a_way_swims_over_deep_water() {
    let map = room(POOL);
    let over = ways(&map, [-300.0, 0.0, 24.0], [300.0, 0.0, 24.0]).expect("a way over");
    assert!(over.contains(&NavWay::Swim), "{over:?}");
}

/// Nearest means near. Right at an edge a column can have no place at the height asked
/// for, and the roof of the level over it isn't anywhere to start from.
#[test]
fn the_nearest_place_is_not_up_on_the_roof() {
    let map = room(POOL);
    let nav = map.nav.as_ref().expect("baked");
    let place = nav.nearest(Vec3::new(-112.0, 0.0, 24.0)).expect("a place");
    let point = nav.places[place as usize].point;
    assert!(point.z < 100.0, "{point:?}");
}

#[test]
fn nobody_walks_along_the_bottom_of_deep_water() {
    let map = room(POOL);
    let nav = map.nav.as_ref().expect("baked");
    let on_the_bed = nav
        .places
        .iter()
        .filter(|place| place.point.x.abs() < 96.0 && place.point.z < -60.0);
    assert_eq!(on_the_bed.count(), 0);
}

#[test]
fn a_dry_way_round_beats_a_swim_that_saves_little() {
    // A bridge across the pool, a little to one side.
    let map = room(&format!(
        "{POOL}brush {{ material = bridge cuboid = -128 100 -16 128 200 0 }}"
    ));
    let over = ways(&map, [-300.0, 0.0, 24.0], [300.0, 0.0, 24.0]).expect("a way over");
    assert!(!over.contains(&NavWay::Swim), "{over:?}");
}

/// A shelf with a lift up to it, in a shaft at the shelf's edge. The lift is placed at
/// its top, as they are, and rests 88 lower, on the floor.
const LIFT: &str = "entity { type = func_plat height = 88
        brush { material = lift cuboid = -64 -64 88 64 64 96 }
    }\n";

#[test]
fn a_lift_is_a_way_up() {
    let map = room(&format!(
        "brush {{ material = floor cuboid = -1024 -288 -288 1024 288 0 }}
         brush {{ material = shelf cuboid = -1024 -288 0 -64 288 96 }}
         {LIFT}"
    ));
    let nav = map.nav.as_ref().expect("baked");
    let lifts = nav
        .leaps
        .all()
        .iter()
        .filter(|leap| leap.way == NavWay::Lift)
        .count();
    assert!(lifts > 0, "{:?}", nav.leaps.all().len());
    let up = ways(&map, [300.0, 0.0, 24.0], [-300.0, 0.0, 120.0]).expect("a way up");
    assert!(up.contains(&NavWay::Lift), "{up:?}");
}

#[test]
fn leaps_survive_a_bake_file() {
    let text = "format 1 map
entity world { type = worldspawn light_spacing = 64 }
brush { material = floor cuboid = -512 -256 -64 512 256 -32 }
brush { material = shelf cuboid = -512 -256 -32 0 256 96 }
brush { material = wall cuboid = -544 -256 -64 -512 256 300 }
brush { material = wall cuboid = 512 -256 -64 544 256 300 }
brush { material = wall cuboid = -544 -288 -64 544 -256 300 }
brush { material = wall cuboid = -544 256 -64 544 288 300 }
brush { material = roof cuboid = -544 -288 300 544 288 332 }
";
    let map = baked(text);
    let count = map.nav.as_ref().expect("baked").leaps.len();
    assert!(count > 0);
    let bytes = write_bake(&map, 7);
    let document = from_text(text).expect("parses");
    let working = read_map(&document).expect("reads");
    let mut source = |name: &str| placeholder_texture(name);
    let mut fresh = compile(&working, &mut source).expect("compiles");
    assert!(apply_bake(&mut fresh, &bytes, 7).expect("reads"));
    let read = fresh.nav.as_ref().expect("baked");
    assert_eq!(read.leaps, map.nav.as_ref().expect("baked").leaps);
}
