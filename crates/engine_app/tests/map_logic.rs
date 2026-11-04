//! The ported game logic, on a small map, through the real server.

use std::rc::Rc;
use std::sync::Arc;

use engine_client::{Client, StepSmoother};
use engine_core::entity::ModelInstance;
use engine_core::math::Vec3;
use engine_core::player::{PlayerState, UserCommand};
use engine_map::{ClipChild, ClipNode, Entity, Leaf, Map, Model, Node, NodeChild, Plane, Tree};
use engine_net::{ClientMessage, Loopback, ServerMessage, Snapshot, Transport, loopback};
use engine_physics::CollisionWorld;
use engine_server::Server;
use fixture_quake::SCRIPTS;

/// One tick.
const TICK: f64 = 1.0 / 72.0;

/// Where a player's centre rests on the floor.
const FLOOR: f32 = -256.0;

/// Adds a plane and returns its number.
fn add_plane(tree: &mut Tree, normal: Vec3, distance: f32) -> u32 {
    tree.planes.push(Plane { normal, distance });
    u32::try_from(tree.planes.len() - 1).expect("small")
}

/// Adds a hull that's solid inside a box, and returns its root.
fn add_box_hull(tree: &mut Tree, mins: Vec3, maxs: Vec3) -> ClipChild {
    let x = Vec3::new(1.0, 0.0, 0.0);
    let y = Vec3::new(0.0, 1.0, 0.0);
    let z = Vec3::new(0.0, 0.0, 1.0);
    let sides = [
        (x, maxs.x, false),
        (x, mins.x, true),
        (y, maxs.y, false),
        (y, mins.y, true),
        (z, maxs.z, false),
        (z, mins.z, true),
    ];
    let first = u32::try_from(tree.clip_nodes.len()).expect("small");
    for (index, (normal, distance, inside_is_front)) in (0_u32..).zip(sides) {
        let plane = add_plane(tree, normal, distance);
        let inner = if index == 5 {
            ClipChild::Solid
        } else {
            ClipChild::Node(first + index + 1)
        };
        let children = if inside_is_front {
            [inner, ClipChild::Empty]
        } else {
            [ClipChild::Empty, inner]
        };
        tree.clip_nodes.push(ClipNode { plane, children });
    }
    ClipChild::Node(first)
}

/// An open room from -256 to 256, with brush models for `boxes`, and `entities`.
fn map(boxes: &[(Vec3, Vec3)], entities: &[&[(&str, &str)]]) -> Arc<Map> {
    let mut tree = Tree::default();
    let x = Vec3::new(1.0, 0.0, 0.0);
    let y = Vec3::new(0.0, 1.0, 0.0);
    let z = Vec3::new(0.0, 0.0, 1.0);
    let walls = [
        (x, 256.0, true),
        (x, -256.0, false),
        (y, 256.0, true),
        (y, -256.0, false),
        (z, 256.0, true),
        (z, -256.0, false),
    ];
    for (index, (normal, distance, solid_in_front)) in (0_u32..).zip(walls) {
        let plane = add_plane(&mut tree, normal, distance);
        let next = if index == 5 {
            ClipChild::Empty
        } else {
            ClipChild::Node(index + 1)
        };
        let children = if solid_in_front {
            [ClipChild::Solid, next]
        } else {
            [next, ClipChild::Solid]
        };
        tree.clip_nodes.push(ClipNode { plane, children });
    }
    tree.nodes = vec![Node {
        plane: 5,
        children: [NodeChild::Leaf(1), NodeChild::Leaf(0)],
    }];
    tree.leaves = vec![Leaf::default(), Leaf::default()];
    let mut models = vec![Model {
        mins: Vec3::new(-256.0, -256.0, -256.0),
        maxs: Vec3::new(256.0, 256.0, 256.0),
        hulls: [ClipChild::Node(0), ClipChild::Empty],
        ..Model::default()
    }];
    for &(mins, maxs) in boxes {
        let root = add_box_hull(&mut tree, mins, maxs);
        models.push(Model {
            mins,
            maxs,
            hulls: [root, ClipChild::Empty],
            ..Model::default()
        });
    }
    let entities = entities
        .iter()
        .map(|fields| Entity {
            fields: fields
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        })
        .collect();
    Arc::new(Map {
        tree,
        models,
        entities,
        ..Map::default()
    })
}

/// A game with the client's side played by the test.
struct Game {
    /// The server.
    server: Server,
    /// The client's end.
    client: Loopback,
    /// The next command's number.
    sequence: u32,
    /// The latest snapshot.
    latest: Option<Snapshot>,
    /// Text received.
    prints: Vec<String>,
    /// Views received.
    views: Vec<(f32, f32)>,
}

impl Game {
    /// Starts `map` with the player at `origin`.
    fn new(map: &Arc<Map>, origin: Vec3) -> Self {
        let world = Rc::new(CollisionWorld::new(Arc::clone(map)));
        let (client, server_end) = loopback();
        let server = Server::new(Box::new(server_end), world, map, origin, SCRIPTS, TICK)
            .expect("the scripts load");
        Self {
            server,
            client,
            sequence: 0,
            latest: None,
            prints: Vec::new(),
            views: Vec::new(),
        }
    }

    /// Runs `ticks` ticks of `command`.
    fn run(&mut self, ticks: u32, mut command: UserCommand) {
        for _ in 0..ticks {
            self.sequence += 1;
            command.sequence = self.sequence;
            #[expect(clippy::cast_possible_truncation, reason = "a tick fits f32")]
            {
                command.seconds = TICK as f32;
            }
            self.client.send(ClientMessage::Move(command).encode());
            self.server.tick();
            while let Some(bytes) = self.client.receive() {
                match ServerMessage::decode(&bytes).expect("valid") {
                    ServerMessage::Snapshot(snapshot) => self.latest = Some(snapshot),
                    ServerMessage::Print(text) => {
                        assert!(!text.contains("script error"), "{text}");
                        self.prints.push(text);
                    }
                    ServerMessage::SetView { yaw, pitch } => self.views.push((yaw, pitch)),
                    ServerMessage::LightStyle { .. } => {}
                }
            }
        }
    }

    /// Stands still for `seconds`.
    fn wait(&mut self, seconds: f64) {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "test durations are short"
        )]
        let ticks = (seconds / TICK) as u32;
        self.run(ticks, UserCommand::default());
    }

    /// The player.
    fn player(&self) -> PlayerState {
        self.latest.as_ref().expect("a snapshot").player
    }

    /// Model `number`.
    fn instance(&self, number: u32) -> ModelInstance {
        *self
            .latest
            .as_ref()
            .expect("a snapshot")
            .instances
            .iter()
            .find(|instance| instance.model == number)
            .expect("the model")
    }
}

/// A door across the room at x = 100, opening upwards.
fn door_box() -> (Vec3, Vec3) {
    (
        Vec3::new(100.0, -64.0, -256.0),
        Vec3::new(116.0, 64.0, -128.0),
    )
}

/// Where a player starts on the floor.
fn at_floor(x: f32, y: f32) -> Vec3 {
    Vec3::new(x, y, FLOOR + 1.0)
}

/// Fields of a door opening upwards.
const DOOR: &[(&str, &str)] = &[
    ("classname", "func_door"),
    ("model", "*1"),
    ("angle", "-1"),
    ("speed", "100"),
    ("wait", "1"),
];

#[test]
fn a_door_opens_when_the_player_comes_near_and_closes_after_its_wait() {
    let (mins, maxs) = door_box();
    let map = map(&[(mins, maxs)], &[DOOR]);
    let mut game = Game::new(&map, at_floor(60.0, 0.0));
    game.wait(0.3);
    assert!(game.instance(1).origin.z > 0.0, "it started moving");
    game.wait(1.3);
    let open = game.instance(1).origin.z;
    assert!(
        (open - 120.0).abs() < 0.5,
        "128 tall less the lip of 8: {open}"
    );
    game.wait(3.0);
    assert!(
        (game.instance(1).origin.z - open).abs() < 0.5,
        "a player nearby holds it open"
    );
    let away = ClientMessage::Console(format!("setpos -200 0 {}", FLOOR + 1.0));
    game.client.send(away.encode());
    game.wait(3.0);
    assert!(game.instance(1).origin.z < open, "it came back down");
}

/// `SV_LinkEdict` grows the field and the player by 1 each, so a door at
/// x = 100 reaches a player of half width 16 at 100 - 60 - 16 - 2 = 22.
#[test]
fn a_door_field_reaches_as_far_as_quakes() {
    let (mins, maxs) = door_box();
    let map = map(&[(mins, maxs)], &[DOOR]);
    let mut outside = Game::new(&map, at_floor(21.5, 0.0));
    outside.wait(0.3);
    assert!(outside.instance(1).origin.z.abs() < 0.5, "out of reach");
    let mut inside = Game::new(&map, at_floor(22.5, 0.0));
    inside.wait(0.3);
    assert!(inside.instance(1).origin.z > 0.0, "in reach");
}

/// `ED_LoadFromFile`: at normal skill, `NOT_MEDIUM` entities don't spawn.
#[test]
fn entities_for_other_skill_levels_do_not_spawn() {
    let (mins, maxs) = door_box();
    let flagged = [DOOR, &[("spawnflags", "512")]].concat();
    let map_one = map(&[(mins, maxs)], &[&flagged]);
    let mut game = Game::new(&map_one, at_floor(-200.0, 0.0));
    game.wait(0.1);
    assert!(
        game.latest
            .as_ref()
            .expect("a snapshot")
            .instances
            .is_empty()
    );
    let other_skill = [DOOR, &[("spawnflags", "256")]].concat();
    let map_two = map(&[(mins, maxs)], &[&other_skill]);
    let mut game = Game::new(&map_two, at_floor(-200.0, 0.0));
    game.wait(0.1);
    assert!(game.instance(1).solid, "this one is for this skill");
}

#[test]
fn a_door_stays_shut_with_nobody_near() {
    let (mins, maxs) = door_box();
    let map = map(&[(mins, maxs)], &[DOOR]);
    let mut game = Game::new(&map, at_floor(-200.0, 0.0));
    game.wait(1.0);
    assert!(game.instance(1).origin.z.abs() < f32::EPSILON);
}

#[test]
fn a_closed_door_is_solid() {
    let (mins, maxs) = door_box();
    let targeted = [DOOR, &[("targetname", "never")]].concat();
    let map = map(&[(mins, maxs)], &[&targeted]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    let forward = UserCommand {
        forward: 400.0,
        ..UserCommand::default()
    };
    game.run(72, forward);
    assert!(game.player().origin.x < 101.0, "{}", game.player().origin.x);
}

#[test]
fn a_door_coming_down_on_the_player_goes_back_up() {
    let (mins, maxs) = door_box();
    let map = map(&[(mins, maxs)], &[DOOR]);
    let mut game = Game::new(&map, at_floor(108.0, 0.0));
    game.wait(2.2);
    let open = game.instance(1).origin.z;
    assert!(open > 100.0, "it opened: {open}");
    game.wait(3.0);
    assert!(game.instance(1).origin.z > 30.0, "the player holds it up");
}

/// A plat at the bottom of its travel, 100 tall.
fn plat_box() -> (Vec3, Vec3) {
    (
        Vec3::new(-64.0, -64.0, -256.0),
        Vec3::new(64.0, 64.0, -156.0),
    )
}

#[test]
fn a_plat_rises_when_stood_on_and_carries_the_player() {
    let (mins, maxs) = plat_box();
    let plat: &[(&str, &str)] = &[
        ("classname", "func_plat"),
        ("model", "*1"),
        ("speed", "100"),
    ];
    let map = map(&[(mins, maxs)], &[plat]);
    let top_of_plat = -156.0 - 92.0;
    let mut game = Game::new(&map, Vec3::new(0.0, 0.0, top_of_plat + 1.0));
    game.wait(0.05);
    let low = game.instance(1).origin.z;
    assert!(
        (low + 92.0).abs() < 5.0,
        "it starts low and is rising: {low}"
    );
    game.wait(1.5);
    let high = game.instance(1).origin.z;
    assert!(high > low + 80.0, "it rose: {high}");
    let feet = game.player().origin.z;
    assert!(feet > top_of_plat + 80.0, "the player rode it: {feet}");
    assert!(game.player().on_ground);
}

/// The most the gap between the view and a plat varies between frames while
/// it rises and while it falls, with and without following the plat.
fn view_wobble(follow: bool) -> (f32, f32) {
    // Tall, so the rise lasts long enough for the view to settle.
    let (mins, maxs) = (Vec3::new(-64.0, -64.0, -256.0), Vec3::new(64.0, 64.0, 44.0));
    let plat: &[(&str, &str)] = &[("classname", "func_plat"), ("model", "*1")];
    let map = map(&[(mins, maxs)], &[plat]);
    let world = Rc::new(CollisionWorld::new(Arc::clone(&map)));
    let start = Vec3::new(0.0, 0.0, -248.0 + 25.0);
    let (client_end, server_end) = loopback();
    let mut server = Server::new(
        Box::new(server_end),
        Rc::clone(&world),
        &map,
        start,
        SCRIPTS,
        TICK,
    )
    .expect("the scripts load");
    let mut client = Client::new(Box::new(client_end), world, start);
    let mut step = StepSmoother::default();
    let (mut rising, mut falling) = (Vec::new(), Vec::new());
    let frames = 3_u8;
    #[expect(clippy::cast_possible_truncation, reason = "a tick fits f32")]
    let tick = TICK as f32;
    for count in 0..290 {
        // Standing on a plat keeps it up, so this one has to be sent down.
        if count == 180 {
            let down = r#"plat_go_down(find_all("classname", "plat")[1])"#;
            server.exec(down).expect("it runs");
        }
        client.send_command(UserCommand::default(), tick);
        server.tick();
        client.receive();
        for frame in 1..=frames {
            let fraction = f32::from(frame) / f32::from(frames);
            let state = client.render_state(&UserCommand::default(), tick * fraction);
            let lag = if follow {
                client.ground_lag(&state, fraction)
            } else {
                Vec3::ZERO
            };
            let body = state.origin + lag;
            let eye = body.z + step.update(body.z, state.on_ground, tick / f32::from(frames));
            let at = |fraction| client.draw_instances(fraction)[0].origin.z;
            let (drawn, speed) = (at(fraction), at(1.0) - at(0.0));
            if state.on_ground && speed > 0.0 {
                rising.push(eye - drawn);
            } else if state.on_ground && speed < 0.0 {
                falling.push(eye - drawn);
            }
        }
    }
    assert!(
        rising.len() > 300 && falling.len() > 200,
        "it went both ways"
    );
    // The view settles behind a rising floor first, as in `V_CalcRefdef`.
    let wobble = |gaps: &[f32]| {
        gaps.windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .fold(0.0, f32::max)
    };
    // At the top its last short move lets the view start catching up.
    let end = rising.len() - 3;
    (wobble(&rising[200..end]), wobble(&falling[1..]))
}

#[test]
fn the_view_stays_steady_on_a_moving_plat() {
    let before = view_wobble(false);
    let after = view_wobble(true);
    assert!(before.0 > 0.1 && before.1 > 0.1, "it shows up: {before:?}");
    assert!(after.0 < 0.02, "rising: {}", after.0);
    assert!(after.1 < 0.02, "falling: {}", after.1);
}

#[test]
fn a_button_opens_the_door_it_targets() {
    let (door_mins, door_maxs) = door_box();
    let button_mins = Vec3::new(-200.0, -16.0, -256.0);
    let button_maxs = Vec3::new(-190.0, 16.0, -224.0);
    let door = [DOOR, &[("targetname", "d")]].concat();
    let button: &[(&str, &str)] = &[
        ("classname", "func_button"),
        ("model", "*2"),
        ("angle", "180"),
        ("target", "d"),
    ];
    let map = map(
        &[(door_mins, door_maxs), (button_mins, button_maxs)],
        &[&door, button],
    );
    let mut game = Game::new(&map, at_floor(-150.0, 0.0));
    game.wait(0.5);
    assert!(
        game.instance(1).origin.z.abs() < f32::EPSILON,
        "shut before"
    );
    let back = UserCommand {
        forward: -400.0,
        ..UserCommand::default()
    };
    game.run(36, back);
    game.wait(0.8);
    assert!(
        game.instance(1).origin.z > 5.0,
        "pressing it opened the door"
    );
}

#[test]
fn a_trigger_teleports_the_player_and_turns_them() {
    let trigger = (
        Vec3::new(-32.0, -32.0, -256.0),
        Vec3::new(32.0, 32.0, -200.0),
    );
    let entities: &[&[(&str, &str)]] = &[
        &[
            ("classname", "trigger_teleport"),
            ("model", "*1"),
            ("target", "there"),
        ],
        &[
            ("classname", "info_teleport_destination"),
            ("targetname", "there"),
            ("origin", "200 200 -200"),
            ("angle", "90"),
        ],
    ];
    let map = map(&[trigger], entities);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    assert!(game.player().origin.x > 150.0, "{:?}", game.player().origin);
    assert_eq!(game.views, [(90.0, 0.0)]);
}

#[test]
fn a_changelevel_trigger_asks_for_its_map() {
    let trigger = (
        Vec3::new(-32.0, -32.0, -256.0),
        Vec3::new(32.0, 32.0, -200.0),
    );
    let entities: &[&[(&str, &str)]] = &[&[
        ("classname", "trigger_changelevel"),
        ("model", "*1"),
        ("map", "e1m2"),
    ]];
    let map = map(&[trigger], entities);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    assert_eq!(game.server.take_next_level().as_deref(), Some("e1m2"));
    game.wait(0.05);
    assert_eq!(game.server.take_next_level(), None, "only once");
}

#[test]
fn a_trigger_once_fires_its_message_and_is_gone() {
    let trigger = (
        Vec3::new(-32.0, -32.0, -256.0),
        Vec3::new(32.0, 32.0, -200.0),
    );
    let entities: &[&[(&str, &str)]] = &[&[
        ("classname", "trigger_once"),
        ("model", "*1"),
        ("message", "hello"),
    ]];
    let map = map(&[trigger], entities);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(1.0);
    assert_eq!(game.prints, ["hello"]);
}

#[test]
fn triggers_are_not_drawn_or_solid() {
    let trigger = (
        Vec3::new(-32.0, -32.0, -256.0),
        Vec3::new(32.0, 32.0, -200.0),
    );
    let entities: &[&[(&str, &str)]] = &[&[("classname", "trigger_multiple"), ("model", "*1")]];
    let map = map(&[trigger], entities);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.1);
    assert!(
        game.latest
            .as_ref()
            .expect("a snapshot")
            .instances
            .is_empty()
    );
}

#[test]
fn a_wall_is_solid_and_an_illusion_is_not() {
    let wall = (
        Vec3::new(50.0, -64.0, -256.0),
        Vec3::new(66.0, 64.0, -100.0),
    );
    let entities: &[&[(&str, &str)]] = &[&[("classname", "func_illusionary"), ("model", "*1")]];
    let forward = UserCommand {
        forward: 400.0,
        ..UserCommand::default()
    };
    let map_one = map(&[wall], entities);
    let mut game = Game::new(&map_one, at_floor(0.0, 0.0));
    game.run(72, forward);
    assert!(game.player().origin.x > 100.0, "through the illusion");
    assert!(game.instance(1).visible);
    let solid: &[&[(&str, &str)]] = &[&[("classname", "func_wall"), ("model", "*1")]];
    let map_two = map(&[wall], solid);
    let mut game = Game::new(&map_two, at_floor(0.0, 0.0));
    game.run(72, forward);
    assert!(game.player().origin.x < 51.0, "stopped by the wall");
}
