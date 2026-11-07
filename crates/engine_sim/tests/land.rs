//! What's thrown on land: it goes through the same collision as a player, heights and all.

use std::sync::Arc;

use engine_core::math::Vec3;
use engine_map::{Map, TerrainData, TerrainSheet};
use engine_physics::CollisionWorld;
use engine_sim::{EntityId, MoveType, Simulation, Solid};

/// One tick.
const TICK: f64 = 1.0 / 72.0;

/// A chunk of land 100 up, with a hole near its middle.
fn land() -> Arc<Map> {
    let mut data = TerrainData::flat([1, 1], 32.0);
    data.heights.fill(100.0);
    for y in 30..34 {
        for x in 30..34 {
            data.set_hole(x, y, true);
        }
    }
    let sheet = TerrainSheet {
        data,
        ..TerrainSheet::default()
    };
    Arc::new(Map {
        terrain: vec![sheet],
        ..Map::default()
    })
}

/// Something small thrown from `from` at `velocity`, in a world of land.
fn thrown(
    map: Arc<Map>,
    kind: MoveType,
    from: Vec3,
    velocity: Vec3,
) -> (Simulation, CollisionWorld, EntityId) {
    let world = CollisionWorld::new(Arc::clone(&map));
    let mut simulation = Simulation::new(map, Vec3::new(10.0, 10.0, 500.0));
    let id = simulation.spawn();
    let entity = simulation.entity_mut(id).expect("it's there");
    entity.solid = Solid::Bbox;
    entity.move_type = kind;
    entity.origin = from;
    entity.velocity = velocity;
    entity.next_think = 1000.0;
    (simulation, world, id)
}

/// Where it is after `seconds`.
fn after(simulation: &mut Simulation, world: &CollisionWorld, id: EntityId, seconds: f64) -> Vec3 {
    let mut left = seconds;
    while left > 0.0 {
        let _callbacks = simulation.advance(id, TICK, world);
        left -= TICK;
    }
    simulation.entity(id).expect("it's there").origin
}

#[test]
fn what_falls_lands_on_the_land() {
    let (mut simulation, world, id) = thrown(
        land(),
        MoveType::Toss,
        Vec3::new(300.0, 300.0, 400.0),
        Vec3::ZERO,
    );
    let rest = after(&mut simulation, &world, id, 3.0);
    assert!((rest.z - 100.0).abs() < 2.0, "{rest:?}");
}

#[test]
fn what_bounces_comes_up_off_the_land_and_comes_to_rest_on_it() {
    let (mut simulation, world, id) = thrown(
        land(),
        MoveType::Bounce,
        Vec3::new(300.0, 300.0, 400.0),
        Vec3::new(120.0, 0.0, 0.0),
    );
    let mut lowest = f32::MAX;
    let mut rose = false;
    let mut last = 400.0;
    for _ in 0..432 {
        let _callbacks = simulation.advance(id, TICK, &world);
        let z = simulation.entity(id).expect("it's there").origin.z;
        lowest = lowest.min(z);
        rose |= z > last + 0.1;
        last = z;
    }
    assert!(lowest > 98.0, "never through it: {lowest}");
    assert!(rose, "it came up again");
    assert!((last - 100.0).abs() < 2.0, "and stopped on it: {last}");
}

#[test]
fn what_falls_goes_through_a_hole() {
    let (mut simulation, world, id) = thrown(
        land(),
        MoveType::Toss,
        Vec3::new(1024.0, 1024.0, 400.0),
        Vec3::ZERO,
    );
    let at = after(&mut simulation, &world, id, 2.0);
    assert!(at.z < 50.0, "{at:?}");
}
