//! Commands, prediction and the render tick, with a server played by the test.

mod common;

use std::rc::Rc;

use common::room;
use engine_client::{Buttons, Client, LightStyles, ModelGlide, StepSmoother, ViewClock};
use engine_core::entity::{EntityState, ModelInstance};
use engine_core::math::Vec3;
use engine_core::player::{PlayerState, UserCommand};
use engine_core::sound::{SoundStart, StaticSound};
use engine_movement::{MoveVars, player_move};
use engine_net::{ClientMessage, Loopback, ServerMessage, Snapshot, Transport, loopback};
use engine_physics::CollisionWorld;

/// One tick.
const TICK: f32 = 1.0 / 72.0;

/// Where the player starts.
const START: Vec3 = Vec3::new(0.0, 0.0, -255.0);

/// A client, its server's end, and the world.
fn setup() -> (Client, Loopback, Rc<CollisionWorld>) {
    let world = Rc::new(CollisionWorld::new(room()));
    let (client_end, server_end) = loopback();
    let client = Client::new(Box::new(client_end), Rc::clone(&world), START);
    (client, server_end, world)
}

/// Running forward.
fn forward() -> UserCommand {
    let mut buttons = Buttons::default();
    assert!(buttons.set("forward", true));
    buttons.command(0.0, 0.0)
}

/// Every command waiting at the server.
fn received(server: &mut Loopback) -> Vec<UserCommand> {
    std::iter::from_fn(|| server.receive())
        .filter_map(|bytes| match ClientMessage::decode(&bytes) {
            Some(ClientMessage::Move(commands, _seen)) => commands.last().copied(),
            _ => None,
        })
        .collect()
}

/// Sends a snapshot of `player`, with `acknowledged` run.
fn answer(server: &mut Loopback, acknowledged: u32, player: PlayerState) {
    let snapshot = Snapshot {
        tick: acknowledged,
        acknowledged,
        player,
        instances: Vec::new(),
        entities: Vec::new(),
        boxes: Vec::new(),
        weapon_model: 0,
        weapon_frame: 0,
        player_entity: 0,
        player_effects: 0,
        punch: Vec3::ZERO,
        time: 0.0,
        stats: Vec::new(),
    };
    server.send(ServerMessage::Snapshot(snapshot).encode());
}

#[test]
fn commands_are_numbered_in_order_and_carry_their_time() {
    let (mut client, mut server, _world) = setup();
    client.send_command(forward(), TICK);
    client.send_command(forward(), TICK);
    let commands = received(&mut server);
    assert_eq!(
        commands.iter().map(|c| c.sequence).collect::<Vec<_>>(),
        [1, 2]
    );
    assert!((commands[0].seconds - TICK).abs() < f32::EPSILON);
}

#[test]
fn the_client_predicts_without_waiting_for_the_server() {
    let (mut client, _server, _world) = setup();
    for _ in 0..10 {
        client.send_command(forward(), TICK);
    }
    assert!(client.predicted().origin.x > 1.0);
    assert_eq!(client.pending_count(), 10);
}

/// `cl_pred.c`: start from the server's state and replay what it hasn't run.
#[test]
fn a_late_snapshot_replays_to_the_same_place() {
    let (mut client, mut server, world) = setup();
    for _ in 0..20 {
        client.send_command(forward(), TICK);
    }
    let predicted = *client.predicted();
    let commands = received(&mut server);
    let mut truth = PlayerState {
        origin: START,
        ..PlayerState::default()
    };
    for command in &commands[..5] {
        player_move(
            &mut truth,
            command,
            &MoveVars::default(),
            world.with(&[]),
            &mut Vec::new(),
        );
    }
    answer(&mut server, 5, truth);
    client.receive();
    assert_eq!(client.pending_count(), 15);
    assert_eq!(*client.predicted(), predicted);
}

#[test]
fn the_server_wins_when_it_disagrees() {
    let (mut client, mut server, _world) = setup();
    client.send_command(forward(), TICK);
    let moved = PlayerState {
        origin: Vec3::new(100.0, 100.0, -255.0),
        ..PlayerState::default()
    };
    answer(&mut server, 1, moved);
    client.receive();
    assert_eq!(client.predicted().origin, moved.origin);
    assert_eq!(client.pending_count(), 0);
}

#[test]
fn bad_bytes_from_the_server_are_ignored() {
    let (mut client, mut server, _world) = setup();
    server.send(vec![1, 2, 3]);
    client.receive();
    assert_eq!(client.predicted().origin, START);
}

#[test]
fn the_render_tick_moves_a_copy_and_leaves_the_state_alone() {
    let (mut client, _server, _world) = setup();
    for _ in 0..20 {
        client.send_command(forward(), TICK);
    }
    let before = *client.predicted();
    let drawn = client.render_state(&forward(), TICK * 0.5);
    assert!(drawn.origin.x > before.origin.x);
    assert_eq!(*client.predicted(), before);
    assert_eq!(client.render_state(&forward(), 0.0), before);
}

#[test]
fn going_up_a_stair_is_smoothed_and_catches_up() {
    let mut step = StepSmoother::default();
    assert!(step.update(0.0, true, 0.01).abs() < f32::EPSILON);
    let lag = step.update(16.0, true, 0.01);
    assert!((lag + 12.0).abs() < 0.001, "at most 12 behind: {lag}");
    let later = step.update(16.0, true, 0.05);
    assert!(
        (later + 8.0).abs() < 0.001,
        "catches up at 80 a second: {later}"
    );
    assert!(step.update(16.0, true, 1.0).abs() < f32::EPSILON);
}

#[test]
fn going_down_or_in_the_air_is_not_smoothed() {
    let mut step = StepSmoother::default();
    assert!(step.update(100.0, false, 0.01).abs() < f32::EPSILON);
    assert!(step.update(50.0, true, 0.01).abs() < f32::EPSILON);
}

#[test]
fn with_always_run_off_speed_runs() {
    let mut buttons = Buttons::default();
    assert!(buttons.set("forward", true));
    assert!((buttons.command(0.0, 0.0).forward - 200.0).abs() < f32::EPSILON);
    assert!(buttons.set("speed", true));
    assert!((buttons.command(0.0, 0.0).forward - 400.0).abs() < f32::EPSILON);
}

#[test]
fn turning_keys_turn_and_strafe_makes_them_sidestep() {
    let mut buttons = Buttons::default();
    assert!(buttons.set("left", true));
    assert!((buttons.turn(1.0).0 - 140.0).abs() < f32::EPSILON);
    assert!(buttons.command(0.0, 0.0).side.abs() < f32::EPSILON);
    assert!(buttons.set("strafe", true));
    assert!(buttons.turn(1.0).0.abs() < f32::EPSILON);
    assert!(buttons.command(0.0, 0.0).side < 0.0);
}

#[test]
fn letting_go_of_everything_keeps_always_run() {
    let mut buttons = Buttons {
        always_run: true,
        forward: true,
        ..Buttons::default()
    };
    buttons.release();
    assert!(buttons.always_run);
    assert!(!buttons.forward);
}

#[test]
fn held_buttons_become_run_speed_and_speed_walks() {
    let mut buttons = Buttons {
        always_run: true,
        ..Buttons::default()
    };
    assert!(buttons.set("forward", true));
    assert!(buttons.set("moveleft", true));
    let command = buttons.command(10.0, 20.0);
    assert!((command.forward - 400.0).abs() < f32::EPSILON);
    assert!((command.side + 400.0).abs() < f32::EPSILON);
    assert!((command.yaw - 10.0).abs() < f32::EPSILON);
    assert!(buttons.set("speed", true));
    assert!((buttons.command(0.0, 0.0).forward - 200.0).abs() < f32::EPSILON);
}

#[test]
fn light_styles_step_ten_times_a_second_and_default_to_normal() {
    let mut styles = LightStyles::new();
    styles.set(1, "mz");
    let values = styles.values(0.0);
    assert!((values[0] - 1.0).abs() < f32::EPSILON, "unset is normal");
    assert!((values[1] - 1.0).abs() < f32::EPSILON);
    assert!(
        (styles.values(0.1)[1] - 25.0 / 12.0).abs() < 1e-5,
        "the second letter"
    );
    assert!(
        (styles.values(0.2)[1] - 1.0).abs() < f32::EPSILON,
        "it loops"
    );
}

#[test]
fn out_of_range_styles_are_ignored() {
    let mut styles = LightStyles::new();
    styles.set(200, "a");
    assert_eq!(styles, LightStyles::new());
}

/// A brush model at height `z`.
fn instance(z: f32) -> ModelInstance {
    ModelInstance {
        model: 1,
        origin: Vec3::new(0.0, 0.0, z),
        visible: true,
        frame: 0,
        solid: true,
    }
}

/// Sends a snapshot with `instances`.
fn snapshot_with(server: &mut Loopback, tick: u32, instances: Vec<ModelInstance>) {
    let snapshot = Snapshot {
        tick,
        acknowledged: 0,
        player: PlayerState {
            origin: START,
            ..PlayerState::default()
        },
        instances,
        entities: Vec::new(),
        boxes: Vec::new(),
        weapon_model: 0,
        weapon_frame: 0,
        player_entity: 0,
        player_effects: 0,
        punch: Vec3::ZERO,
        time: 0.0,
        stats: Vec::new(),
    };
    server.send(ServerMessage::Snapshot(snapshot).encode());
}

#[test]
fn brush_models_draw_between_the_last_two_snapshots() {
    let (mut client, mut server, _world) = setup();
    snapshot_with(&mut server, 1, vec![instance(0.0)]);
    snapshot_with(&mut server, 2, vec![instance(10.0)]);
    client.receive();
    let drawn = client.draw_instances(0.5);
    assert!((drawn[0].origin.z - 5.0).abs() < f32::EPSILON);
    assert!((client.draw_instances(1.0)[0].origin.z - 10.0).abs() < f32::EPSILON);
}

#[test]
fn a_changed_list_of_models_is_not_interpolated() {
    let (mut client, mut server, _world) = setup();
    snapshot_with(&mut server, 1, vec![instance(0.0)]);
    snapshot_with(&mut server, 2, vec![instance(10.0), instance(20.0)]);
    client.receive();
    assert_eq!(client.draw_instances(0.5).len(), 2);
}

#[test]
fn messages_from_the_server_are_kept_until_asked_for() {
    let (mut client, mut server, _world) = setup();
    server.send(ServerMessage::Print("hello".to_owned()).encode());
    server.send(
        ServerMessage::SetView {
            yaw: 90.0,
            pitch: 0.0,
        }
        .encode(),
    );
    server.send(
        ServerMessage::LightStyle {
            index: 3,
            pattern: "a".to_owned(),
        }
        .encode(),
    );
    client.receive();
    assert_eq!(client.take_prints(), ["hello"]);
    assert!(client.take_prints().is_empty());
    assert_eq!(client.take_view(), Some((90.0, 0.0)));
    assert_eq!(client.take_view(), None);
    assert!(client.light_styles(0.0)[3].abs() < f32::EPSILON);
}

#[test]
fn unknown_buttons_are_reported() {
    assert!(!Buttons::default().set("nonsense", true));
}

#[test]
fn a_model_that_replaces_another_is_not_drawn_between_them() {
    let (mut client, mut server, _world) = setup();
    let other = ModelInstance {
        model: instance(10.0).model + 1,
        ..instance(10.0)
    };
    snapshot_with(&mut server, 1, vec![instance(0.0)]);
    snapshot_with(&mut server, 2, vec![other]);
    client.receive();
    assert!((client.draw_instances(0.5)[0].origin.z - 10.0).abs() < f32::EPSILON);
}

#[test]
fn sounds_wait_to_be_taken() {
    let (mut client, mut server, _world) = setup();
    let sound = SoundStart {
        entity: 3,
        channel: 2,
        origin: Vec3::ZERO,
        volume: 1.0,
        attenuation: 1.0,
        own: false,
        name: "a.wav".to_owned(),
    };
    let ambient = StaticSound {
        origin: Vec3::ZERO,
        volume: 1.0,
        attenuation: 3.0,
        name: "b.wav".to_owned(),
    };
    server.send(ServerMessage::Sound(sound.clone()).encode());
    server.send(ServerMessage::StaticSound(ambient.clone()).encode());
    server.send(ServerMessage::Music(4).encode());
    client.receive();
    assert_eq!(client.take_sounds(), [sound]);
    assert_eq!(client.take_static_sounds(), [ambient]);
    assert_eq!(client.take_music(), Some(4));
    assert!(client.take_sounds().is_empty());
    assert_eq!(client.take_music(), None);
}

/// Sends a snapshot with `entities`.
fn snapshot_of(server: &mut Loopback, tick: u32, entities: Vec<EntityState>) {
    let snapshot = Snapshot {
        tick,
        acknowledged: 0,
        player: PlayerState::default(),
        instances: Vec::new(),
        entities,
        boxes: Vec::new(),
        weapon_model: 2,
        weapon_frame: 5,
        player_entity: 0,
        player_effects: 0,
        punch: Vec3::ZERO,
        time: 0.0,
        stats: Vec::new(),
    };
    server.send(ServerMessage::Snapshot(snapshot).encode());
}

/// An entity at `x` showing `frame`.
fn thing(id: u64, x: f32, frame: u16) -> EntityState {
    EntityState {
        id,
        model: 1,
        frame,
        origin: Vec3::new(x, 0.0, 0.0),
        ..EntityState::default()
    }
}

#[test]
fn entities_draw_between_the_last_two_snapshots_by_id() {
    let (mut client, mut server, _world) = setup();
    snapshot_of(&mut server, 1, vec![thing(1, 0.0, 0), thing(2, 100.0, 0)]);
    snapshot_of(&mut server, 2, vec![thing(2, 120.0, 0), thing(1, 10.0, 0)]);
    client.receive();
    let drawn = client.draw_entities(0.5);
    assert!((drawn[0].state.origin.x - 110.0).abs() < 1e-4);
    assert!((drawn[1].state.origin.x - 5.0).abs() < 1e-4);
    assert_eq!(client.weapon(), (2, 5));
}

#[test]
fn a_new_entity_appears_where_it_is() {
    let (mut client, mut server, _world) = setup();
    snapshot_of(&mut server, 1, vec![]);
    snapshot_of(&mut server, 2, vec![thing(1, 50.0, 0)]);
    client.receive();
    assert!((client.draw_entities(0.5)[0].state.origin.x - 50.0).abs() < 1e-4);
}

#[test]
fn a_teleport_is_not_drawn_between_its_ends() {
    let (mut client, mut server, _world) = setup();
    snapshot_of(&mut server, 1, vec![thing(1, 0.0, 0)]);
    snapshot_of(&mut server, 2, vec![thing(1, 500.0, 0)]);
    client.receive();
    assert!((client.draw_entities(0.5)[0].state.origin.x - 500.0).abs() < 1e-4);
}

#[test]
fn angles_blend_the_short_way_round() {
    let (mut client, mut server, _world) = setup();
    let turned = |yaw| EntityState {
        angles: Vec3::new(0.0, yaw, 0.0),
        ..thing(1, 0.0, 0)
    };
    snapshot_of(&mut server, 1, vec![turned(350.0)]);
    snapshot_of(&mut server, 2, vec![turned(10.0)]);
    client.receive();
    let yaw = client.draw_entities(0.5)[0].state.angles.y;
    assert!((yaw.rem_euclid(360.0)).abs() < 1e-3, "{yaw}");
}

#[test]
fn a_new_frame_blends_in_over_a_tenth_of_a_second() {
    let (mut client, mut server, _world) = setup();
    let mut glide = ModelGlide::default();
    let mut shown = |client: &mut Client, seconds: f32| {
        let mut drawn = client.draw_entities(0.0);
        glide.smooth(&mut drawn, seconds);
        drawn[0]
    };
    snapshot_of(&mut server, 1, vec![thing(1, 0.0, 3)]);
    client.receive();
    assert!(
        (shown(&mut client, 0.0).blend - 1.0).abs() < 1e-6,
        "nothing to blend"
    );
    snapshot_of(&mut server, 2, vec![thing(1, 0.0, 4)]);
    client.receive();
    let drawn = shown(&mut client, 0.0);
    assert_eq!((drawn.from_frame, drawn.state.frame), (3, 4));
    assert!(drawn.blend.abs() < 1e-6);
    let later = shown(&mut client, 0.01);
    assert!((later.blend - 0.1).abs() < 1e-4);
    assert!((shown(&mut client, 0.09).blend - 1.0).abs() < 1e-6);
}

#[test]
fn model_names_wait_to_be_taken() {
    let (mut client, mut server, _world) = setup();
    let name = "progs/flame.mdl".to_owned();
    server.send(ServerMessage::ModelName { number: 3, name }.encode());
    client.receive();
    assert_eq!(
        client.take_model_names(),
        [(3, "progs/flame.mdl".to_owned())]
    );
    assert!(client.take_model_names().is_empty());
}

#[test]
fn the_view_clock_starts_two_ticks_behind() {
    let mut clock = ViewClock::default();
    let time = clock.advance(0.0, 10.0);
    assert!((time - (10.0 - 2.0 / 72.0)).abs() < 1e-9);
}

#[test]
fn the_view_clock_moves_evenly_through_uneven_snapshots() {
    let mut clock = ViewClock::default();
    let mut newest = 10.0;
    let mut last = clock.advance(0.0, newest);
    let frame = 1.0 / 144.0;
    for frame_number in 0..1440 {
        // Snapshots come in bursts, two ticks at once every other tick.
        if frame_number % 4 == 0 {
            newest += 2.0 / 72.0;
        }
        let time = clock.advance(frame, newest);
        let step = time - last;
        assert!(step >= 0.0, "time never runs back");
        assert!(step < frame * 1.2, "and never jumps: {step}");
        last = time;
    }
    assert!(newest - last < 0.1, "it keeps up");
}

#[test]
fn the_view_clock_jumps_after_a_long_gap() {
    let mut clock = ViewClock::default();
    let _first = clock.advance(0.0, 10.0);
    let time = clock.advance(0.01, 20.0);
    assert!(time > 19.9);
}

#[test]
fn the_view_clock_never_passes_the_newest_snapshot() {
    let mut clock = ViewClock::default();
    let _first = clock.advance(0.0, 10.0);
    for _ in 0..100 {
        assert!(clock.advance(0.05, 10.0) <= 10.0);
    }
}
