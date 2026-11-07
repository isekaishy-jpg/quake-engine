//! Things held to a place on another's model (proposal 0140).

use std::rc::Rc;
use std::sync::Arc;

use engine_core::math::Vec3;
use engine_map::{AttachPoint, ClipChild, Map, Model};
use engine_physics::CollisionWorld;
use engine_sim::{EntityId, MoveType, Simulation, Solid};
use engine_test_maps::room;

/// One tick.
const TICK: f64 = 1.0 / 72.0;

/// The place on the lift, where it is before the lift moves.
const SPOUT: Vec3 = Vec3::new(10.0, 0.0, 40.0);

/// A lift going up at 72 units a second, with a place on its model, and something to hold.
fn lift_and_thing() -> (Simulation, Rc<CollisionWorld>, EntityId, EntityId) {
    let mut map = Map::clone(&room());
    map.models.push(Model {
        mins: Vec3::new(-32.0, -32.0, -8.0),
        maxs: Vec3::new(32.0, 32.0, 8.0),
        hulls: [ClipChild::Empty, ClipChild::Empty],
        ..Model::default()
    });
    let number = u32::try_from(map.models.len() - 1).expect("few");
    let point = AttachPoint {
        name: "spout".to_owned(),
        origin: SPOUT,
        angles: Vec3::new(0.0, 90.0, 0.0),
    };
    map.model_points.push((number, point));
    let map = Arc::new(map);
    let world = Rc::new(CollisionWorld::new(Arc::clone(&map)));
    let mut simulation = Simulation::new(map, Vec3::new(200.0, 200.0, 0.0));
    let lift = simulation.spawn();
    simulation.set_model(lift, number);
    let pusher = simulation.entity_mut(lift).expect("it's there");
    pusher.solid = Solid::Bsp;
    pusher.move_type = MoveType::Push;
    pusher.velocity = Vec3::new(0.0, 0.0, 72.0);
    pusher.next_think = 100.0;
    let thing = simulation.spawn();
    (simulation, world, lift, thing)
}

/// One tick of `lift` moving, and the end of it.
fn tick(simulation: &mut Simulation, world: &Rc<CollisionWorld>, lift: EntityId) {
    let _callbacks = simulation.advance(lift, TICK, world);
    simulation.end_tick(TICK);
}

/// Where `id` is.
fn origin_of(simulation: &Simulation, id: EntityId) -> Vec3 {
    simulation.entity(id).expect("it's there").origin
}

#[test]
fn a_place_is_where_its_model_is_now() {
    let (mut simulation, world, lift, _) = lift_and_thing();
    for _ in 0..36 {
        tick(&mut simulation, &world, lift);
    }
    let moved = origin_of(&simulation, lift);
    assert!(moved.z > 30.0, "it went up: {moved:?}");
    let (origin, angles) = simulation.place_on(lift, "spout").expect("there");
    assert!((origin - (SPOUT + moved)).length() < 0.001, "{origin:?}");
    assert!((angles.y - 90.0).abs() < 0.001);
}

#[test]
fn a_place_that_is_not_there_is_nothing() {
    let (mut simulation, _, lift, thing) = lift_and_thing();
    assert!(simulation.place_on(lift, "nothing").is_none());
    assert!(
        simulation.place_on(thing, "spout").is_none(),
        "it has no model"
    );
    assert!(!simulation.attach(thing, lift, "nothing"));
    assert!(!simulation.attach(lift, thing, "spout"));
}

/// Read in the same tick as the move: a thing a tick behind would judder on anything moving.
#[test]
fn what_is_held_does_not_lag_what_holds_it() {
    let (mut simulation, world, lift, thing) = lift_and_thing();
    assert!(simulation.attach(thing, lift, "spout"));
    for _ in 0..50 {
        tick(&mut simulation, &world, lift);
        let wanted = SPOUT + origin_of(&simulation, lift);
        let held = origin_of(&simulation, thing);
        assert!(
            (held - wanted).length() < 0.001,
            "{held:?} against {wanted:?}"
        );
    }
    let angles = simulation.entity(thing).expect("it's there").angles;
    assert!(
        (angles.y - 90.0).abs() < 0.001,
        "it's turned as the place is"
    );
}

#[test]
fn what_is_let_go_stays_where_it_was() {
    let (mut simulation, world, lift, thing) = lift_and_thing();
    assert!(simulation.attach(thing, lift, "spout"));
    for _ in 0..10 {
        tick(&mut simulation, &world, lift);
    }
    simulation.detach(thing);
    let left = origin_of(&simulation, thing);
    for _ in 0..10 {
        tick(&mut simulation, &world, lift);
    }
    assert_eq!(origin_of(&simulation, thing), left);
}

/// What's held outlives what held it, and stays put, where it would otherwise fly to the
/// origin or keep a dead number that a new entity could take.
#[test]
fn what_is_held_lets_go_when_what_holds_it_goes() {
    let (mut simulation, world, lift, thing) = lift_and_thing();
    assert!(simulation.attach(thing, lift, "spout"));
    for _ in 0..10 {
        tick(&mut simulation, &world, lift);
    }
    let left = origin_of(&simulation, thing);
    simulation.remove(lift);
    simulation.end_tick(TICK);
    assert_eq!(origin_of(&simulation, thing), left);
    let held = simulation.entity(thing).expect("it's there").attached;
    assert_eq!(held, None);
}
