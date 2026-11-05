//! Pushers, from `SV_Physics_Pusher` and `SV_PushMove`.

use std::rc::Rc;
use std::sync::Arc;

use engine_core::math::Vec3;
use engine_map::{ClipChild, Map, Model};
use engine_physics::CollisionWorld;
use engine_sim::{Callback, EntityId, MoveType, Simulation, Solid};
use engine_test_maps::room;

/// One tick.
const TICK: f64 = 1.0 / 72.0;

/// The room, with an empty brush model for something to be.
fn room_with_a_model() -> Arc<Map> {
    let mut map = Map::clone(&room());
    map.models.push(Model {
        mins: Vec3::new(-32.0, -32.0, -8.0),
        maxs: Vec3::new(32.0, 32.0, 8.0),
        hulls: [ClipChild::Empty, ClipChild::Empty],
        ..Model::default()
    });
    Arc::new(map)
}

/// A simulation with a pusher going up at `speed`, for now and then `think`.
fn lift(speed: f32, think: f64) -> (Simulation, Rc<CollisionWorld>, EntityId) {
    let map = room_with_a_model();
    let world = Rc::new(CollisionWorld::new(Arc::clone(&map)));
    let mut simulation = Simulation::new(map, Vec3::new(200.0, 200.0, 0.0));
    let id = simulation.spawn();
    simulation.set_model(id, 1);
    let pusher = simulation.entity_mut(id).expect("it's there");
    pusher.solid = Solid::Bsp;
    pusher.move_type = MoveType::Push;
    pusher.velocity = Vec3::new(0.0, 0.0, speed);
    pusher.next_think = think;
    (simulation, world, id)
}

#[test]
fn a_pusher_moves_by_its_velocity_on_its_own_clock() {
    let (mut simulation, world, id) = lift(72.0, 100.0);
    for _ in 0..72 {
        let _callbacks = simulation.advance(id, TICK, &world);
    }
    let pusher = simulation.entity(id).expect("it's there");
    assert!((pusher.origin.z - 72.0).abs() < 0.01, "{}", pusher.origin.z);
    assert!((pusher.local_time - 1.0).abs() < 0.001);
}

#[test]
fn a_pusher_stops_at_its_think_and_thinks() {
    let (mut simulation, world, id) = lift(72.0, 0.5);
    let mut thoughts = 0;
    for _ in 0..72 {
        let callbacks = simulation.advance(id, TICK, &world);
        thoughts += callbacks
            .iter()
            .filter(|callback| matches!(callback, Callback::Think(..)))
            .count();
        if thoughts > 0 {
            break;
        }
    }
    assert_eq!(thoughts, 1);
    let pusher = simulation.entity(id).expect("it's there");
    assert!((pusher.origin.z - 36.0).abs() < 0.01, "{}", pusher.origin.z);
}

#[test]
fn a_pusher_with_nothing_to_do_stays_put() {
    let (mut simulation, world, id) = lift(0.0, 0.0);
    let _callbacks = simulation.advance(id, TICK, &world);
    assert_eq!(simulation.entity(id).map(|e| e.origin), Some(Vec3::ZERO));
}
