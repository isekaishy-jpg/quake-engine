//! Things held to a place on another's model (proposal 0140).

use std::rc::Rc;
use std::sync::Arc;

use engine_core::math::Vec3;
use engine_map::{
    AttachPoint, Channel, ChannelPath, Clip, ClipChild, Interpolation, Joint, Map, Model,
    MovingMesh, NO_MOVE, Skeleton, SkeletonPlace,
};
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

/// A joint at the origin, turning about Z, with a place one unit along it.
fn turning_skeleton() -> Skeleton {
    let (sine, cosine) = 45.0_f32.to_radians().sin_cos();
    let turned = [0.0, 0.0, sine, cosine];
    let mut offset = NO_MOVE;
    offset[0][3] = 10.0;
    Skeleton {
        joints: vec![Joint {
            name: "root".to_owned(),
            inverse_bind: NO_MOVE,
            ..Joint::default()
        }],
        clips: vec![Clip {
            name: "turn".to_owned(),
            length: 2.0,
            channels: vec![Channel {
                joint: 0,
                path: ChannelPath::Rotation,
                interpolation: Interpolation::Step,
                times: vec![0.0, 1.0],
                values: [[0.0, 0.0, 0.0, 1.0], turned].concat(),
            }],
        }],
        places: vec![SkeletonPlace {
            name: "tip".to_owned(),
            joint: Some(0),
            offset,
        }],
        reach: 0.0,
        ..Skeleton::default()
    }
}

/// A simulation with a thing showing a moving mesh, and another to hold to it.
fn figure_and_thing() -> (Simulation, EntityId, EntityId) {
    let mut map = Map::clone(&room());
    map.moving_meshes.push(MovingMesh {
        name: "arm".to_owned(),
        skeleton: turning_skeleton(),
        ..MovingMesh::default()
    });
    let mut simulation = Simulation::new(Arc::new(map), Vec3::new(200.0, 200.0, 0.0));
    let figure = simulation.spawn();
    simulation.set_shape(figure, "mesh:arm");
    simulation.entity_mut(figure).expect("there").origin = Vec3::new(100.0, 0.0, 0.0);
    let thing = simulation.spawn();
    (simulation, figure, thing)
}

#[test]
fn a_clip_is_chosen_by_name_and_says_if_it_is_there() {
    let (mut simulation, figure, thing) = figure_and_thing();
    assert!(simulation.set_clip(figure, "turn"));
    assert_eq!(simulation.entity(figure).expect("there").clip, 1);
    assert!(!simulation.set_clip(figure, "dance"));
    assert!(
        !simulation.set_clip(thing, "turn"),
        "it shows no moving mesh"
    );
    assert!(simulation.set_clip(figure, ""), "no name stops it");
    assert_eq!(simulation.entity(figure).expect("there").clip, 0);
}

/// Asking for what's playing leaves it, or a script asking each think would hold it at the start.
#[test]
fn asking_for_the_clip_that_is_playing_leaves_it_playing() {
    let (mut simulation, figure, _) = figure_and_thing();
    assert!(simulation.set_clip(figure, "turn"));
    let started = simulation.entity(figure).expect("there").clip_start;
    simulation.end_tick(0.5);
    assert!(simulation.set_clip(figure, "turn"));
    assert!((simulation.entity(figure).expect("there").clip_start - started).abs() < 0.0001);
}

/// A place on a joint goes where the pose takes it, which the server has to know for itself.
#[test]
fn a_place_on_a_joint_follows_the_clip() {
    let (mut simulation, figure, _) = figure_and_thing();
    assert!(simulation.set_clip(figure, "turn"));
    let (still, _) = simulation.place_on(figure, "tip").expect("there");
    assert!(
        (still - Vec3::new(110.0, 0.0, 0.0)).length() < 0.001,
        "{still:?}"
    );
    simulation.end_tick(1.5);
    let (turned, angles) = simulation.place_on(figure, "tip").expect("there");
    assert!(
        (turned - Vec3::new(100.0, 10.0, 0.0)).length() < 0.001,
        "{turned:?}"
    );
    assert!((angles.y - 90.0).abs() < 0.01, "{angles:?}");
    assert!(simulation.place_on(figure, "nothing").is_none());
}

/// A place turns with what it's on, as well as with the clip.
#[test]
fn a_place_on_a_joint_goes_with_its_entity() {
    let (mut simulation, figure, _) = figure_and_thing();
    simulation.entity_mut(figure).expect("there").angles = Vec3::new(0.0, 90.0, 0.0);
    let (place, _) = simulation.place_on(figure, "tip").expect("there");
    assert!(
        (place - Vec3::new(100.0, 10.0, 0.0)).length() < 0.001,
        "{place:?}"
    );
}

#[test]
fn what_is_held_to_a_joint_follows_it() {
    let (mut simulation, figure, thing) = figure_and_thing();
    assert!(simulation.set_clip(figure, "turn"));
    assert!(simulation.attach(thing, figure, "tip"));
    simulation.end_tick(TICK);
    assert!((origin_of(&simulation, thing) - Vec3::new(110.0, 0.0, 0.0)).length() < 0.001);
    simulation.end_tick(1.2);
    simulation.end_tick(TICK);
    let held = origin_of(&simulation, thing);
    assert!(
        (held - Vec3::new(100.0, 10.0, 0.0)).length() < 0.001,
        "{held:?}"
    );
}

/// What a client's told is enough to play the same clip from the same moment.
#[test]
fn a_client_is_told_the_clip_and_when_it_began() {
    let (mut simulation, figure, _) = figure_and_thing();
    assert!(simulation.set_clip(figure, "turn"));
    let started = simulation.entity(figure).expect("there").clip_start;
    let told = simulation.entity_states();
    let state = told
        .iter()
        .find(|state| state.id == figure.to_bits())
        .expect("told");
    assert_eq!(state.clip, 1);
    assert!((f64::from(state.clip_start) - started).abs() < 0.001);
}

/// A figure that has morph targets.
fn morphing_figure() -> (Simulation, EntityId) {
    let mut skeleton = turning_skeleton();
    skeleton.morph_names = ["a", "b", "c", "d", "e"].map(str::to_owned).to_vec();
    let mut map = Map::clone(&room());
    map.moving_meshes.push(MovingMesh {
        name: "arm".to_owned(),
        skeleton,
        ..MovingMesh::default()
    });
    let mut simulation = Simulation::new(Arc::new(map), Vec3::new(200.0, 200.0, 0.0));
    let figure = simulation.spawn();
    simulation.set_shape(figure, "mesh:arm");
    (simulation, figure)
}

#[test]
fn a_morph_is_set_by_name_and_told_to_clients() {
    let (mut simulation, figure) = morphing_figure();
    assert!(simulation.set_morph(figure, "b", 1.0));
    assert!(!simulation.set_morph(figure, "nothing", 1.0));
    let told = simulation.entity_states();
    let state = told
        .iter()
        .find(|state| state.id == figure.to_bits())
        .expect("told");
    assert_eq!(state.morphs[0], [1, 255]);
}

/// Setting one again changes it, and doesn't take another place.
#[test]
fn setting_a_morph_again_changes_it() {
    let (mut simulation, figure) = morphing_figure();
    assert!(simulation.set_morph(figure, "a", 1.0));
    assert!(simulation.set_morph(figure, "a", 0.5));
    let morphs = simulation.entity(figure).expect("there").morphs;
    assert_eq!(morphs[0], [0, 128]);
    assert_eq!(morphs[1][1], 0);
}

/// Only so many can be set at once, and it says when there's no room, until one's let go.
#[test]
fn only_so_many_morphs_can_be_set_at_once() {
    let (mut simulation, figure) = morphing_figure();
    for name in ["a", "b", "c", "d"] {
        assert!(simulation.set_morph(figure, name, 1.0), "{name}");
    }
    assert!(!simulation.set_morph(figure, "e", 1.0), "no room");
    assert!(
        simulation.set_morph(figure, "e", 0.0),
        "letting go of nothing is fine"
    );
    assert!(simulation.set_morph(figure, "b", 0.0));
    assert!(simulation.set_morph(figure, "e", 1.0), "room again");
}
