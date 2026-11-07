//! The ported game logic, on a small map, through the real server.

use std::rc::Rc;
use std::sync::Arc;

use engine_client::{Client, StepSmoother};
mod common;

use common::map;
use engine_core::entity::{Effect, ModelInstance};
use engine_core::math::Vec3;
use engine_core::player::{PlayerState, UserCommand};
use engine_core::sound::{SoundStart, StaticSound};
use engine_map::Map;
use engine_net::{ClientMessage, Loopback, ServerMessage, Snapshot, Transport, loopback};
use engine_physics::CollisionWorld;
use engine_server::{Level, Server};
use fixture_quake::{SCRIPTS, shot_launch, weapon_rule};

/// A level on normal skill, as the console's default sets.
fn normal_level() -> Level {
    Level {
        settings: vec![("skill".to_owned(), 1.0), ("registered".to_owned(), 1.0)],
        ..Level::default()
    }
}

/// One tick.
const TICK: f64 = 1.0 / 72.0;

/// Where a player's centre rests on the floor.
const FLOOR: f32 = -256.0;

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
    /// Sounds received.
    sounds: Vec<SoundStart>,
    /// Sounds at fixed places received.
    static_sounds: Vec<StaticSound>,
    /// The last music track received.
    music: Option<u8>,
    /// Effects received.
    effects: Vec<Effect>,
    /// Commands for the client's console received.
    stuffed: Vec<String>,
    /// The last end of level screen asked for.
    intermission: u8,
    /// Damage reported to the player.
    damage: Vec<(u8, u8, Vec3)>,
}

impl Game {
    /// Starts `map` with the player at `origin`.
    fn new(map: &Arc<Map>, origin: Vec3) -> Self {
        let world = Rc::new(CollisionWorld::new(Arc::clone(map)));
        let (client, server_end) = loopback();
        let server = Server::new(
            Box::new(server_end),
            world,
            origin,
            SCRIPTS,
            TICK,
            normal_level(),
        )
        .expect("the scripts load");
        Self {
            server,
            client,
            sequence: 0,
            latest: None,
            prints: Vec::new(),
            views: Vec::new(),
            sounds: Vec::new(),
            static_sounds: Vec::new(),
            music: None,
            effects: Vec::new(),
            stuffed: Vec::new(),
            intermission: 0,
            damage: Vec::new(),
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
            self.client
                .send(ClientMessage::Move(vec![command], 0).encode());
            self.server.tick();
            while let Some(bytes) = self.client.receive() {
                match ServerMessage::decode(&bytes).expect("valid") {
                    ServerMessage::Snapshot(mut snapshot) => {
                        // Others see the player, but tests are about the rest.
                        let own = snapshot.player_entity;
                        snapshot.entities.retain(|entity| entity.id != own);
                        self.latest = Some(snapshot);
                    }
                    ServerMessage::Print(text) => {
                        assert!(!text.contains("script error"), "{text}");
                        if !text.ends_with("entered the game") {
                            self.prints.push(text);
                        }
                    }
                    ServerMessage::SetView { yaw, pitch } => self.views.push((yaw, pitch)),
                    ServerMessage::Sound(sound) => self.sounds.push(sound),
                    ServerMessage::StaticSound(sound) => self.static_sounds.push(sound),
                    ServerMessage::Music(track) => self.music = Some(track),
                    ServerMessage::Effect(effect) => self.effects.push(effect),
                    ServerMessage::CenterPrint(text) => self.prints.push(text),
                    ServerMessage::Stuff(text) => self.stuffed.push(text),
                    ServerMessage::Intermission(kind) => self.intermission = kind,
                    ServerMessage::Damage { armor, blood, from } => {
                        self.damage.push((armor, blood, from));
                    }
                    ServerMessage::LightStyle { .. }
                    | ServerMessage::Gravity(_)
                    | ServerMessage::ModelName { .. }
                    | ServerMessage::LevelTitle(_)
                    | ServerMessage::SnapshotDelta(_)
                    | ServerMessage::Level(_)
                    | ServerMessage::Camera { .. }
                    | ServerMessage::Scores(_) => {}
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
fn a_closed_door_stays_solid_in_a_loaded_game() {
    let (mins, maxs) = door_box();
    let targeted = [DOOR, &[("targetname", "never")]].concat();
    let map = map(&[(mins, maxs)], &[&targeted]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.1);
    let saved = game.server.save().expect("it saves");
    let (client, server_end) = loopback();
    let world = Rc::new(CollisionWorld::new(Arc::clone(&map)));
    game.server =
        Server::resume(Box::new(server_end), world, SCRIPTS, TICK, saved).expect("it loads");
    game.client = client;
    let forward = UserCommand {
        forward: 400.0,
        ..UserCommand::default()
    };
    game.run(72, forward);
    assert!(game.player().origin.x > 50.0, "the control: it moved");
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
        start,
        SCRIPTS,
        TICK,
        normal_level(),
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

/// The names of the sounds received so far.
fn sound_names(game: &Game) -> Vec<&str> {
    game.sounds
        .iter()
        .map(|sound| sound.name.as_str())
        .collect()
}

#[test]
fn a_door_starts_a_move_sound_and_replaces_it_when_it_stops() {
    let (mins, maxs) = door_box();
    let door = [DOOR, &[("sounds", "1")]].concat();
    let map = map(&[(mins, maxs)], &[&door]);
    let mut game = Game::new(&map, at_floor(60.0, 0.0));
    game.wait(0.3);
    assert_eq!(sound_names(&game), ["doors/doormv1.wav"]);
    let start = &game.sounds[0];
    assert_eq!(start.channel, 2);
    assert!(!start.own);
    assert!((start.volume - 1.0).abs() < f32::EPSILON);
    assert!((start.attenuation - 1.0).abs() < f32::EPSILON);
    assert!((start.origin.x - 108.0).abs() < 1.5, "the door's middle");
    game.wait(1.3);
    assert_eq!(
        sound_names(&game),
        ["doors/doormv1.wav", "doors/drclos4.wav"]
    );
    assert_eq!(
        game.sounds[0].entity, game.sounds[1].entity,
        "so it replaces"
    );
    assert_eq!(game.sounds[0].channel, game.sounds[1].channel);
}

#[test]
fn a_plat_hums_while_it_moves() {
    let (mins, maxs) = plat_box();
    let plat: &[(&str, &str)] = &[("classname", "func_plat"), ("model", "*1")];
    let map = map(&[(mins, maxs)], &[plat]);
    let mut game = Game::new(&map, Vec3::new(0.0, 0.0, -248.0 + 25.0));
    game.wait(1.5);
    assert_eq!(
        sound_names(&game),
        ["plats/medplat1.wav", "plats/medplat2.wav"]
    );
}

#[test]
fn a_map_asks_for_its_music_and_its_ambient_sounds() {
    let world: &[(&str, &str)] = &[("classname", "worldspawn"), ("sounds", "6")];
    let torch: &[(&str, &str)] = &[
        ("classname", "light_torch_small_walltorch"),
        ("origin", "1 2 3"),
    ];
    let hum: &[(&str, &str)] = &[("classname", "ambient_comp_hum"), ("origin", "4 5 6")];
    let map = map(&[], &[world, torch, hum]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    assert_eq!(game.music, Some(6));
    assert_eq!(game.static_sounds.len(), 2);
    let fire = &game.static_sounds[0];
    assert_eq!(fire.name, "ambience/fire1.wav");
    assert_eq!(fire.origin, Vec3::new(1.0, 2.0, 3.0));
    assert!((fire.volume - 0.5).abs() < f32::EPSILON);
    assert!((fire.attenuation - 3.0).abs() < f32::EPSILON);
    assert_eq!(game.static_sounds[1].name, "ambience/comp1.wav");
}

#[test]
fn a_teleport_hums_and_plays_a_sound_at_each_end() {
    let (mins, maxs) = (
        Vec3::new(-32.0, -32.0, -256.0),
        Vec3::new(32.0, 32.0, -200.0),
    );
    let trigger: &[(&str, &str)] = &[
        ("classname", "trigger_teleport"),
        ("model", "*1"),
        ("target", "there"),
    ];
    let destination: &[(&str, &str)] = &[
        ("classname", "info_teleport_destination"),
        ("targetname", "there"),
        ("origin", "300 0 -200"),
    ];
    let map = map(&[(mins, maxs)], &[trigger, destination]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.5);
    assert_eq!(game.static_sounds[0].name, "ambience/hum1.wav");
    assert_eq!(game.sounds.len(), 2);
    assert!(game.sounds[0].name.starts_with("misc/r_tele"));
    assert_eq!(game.temps(11), 2, "a splash at each end");
}

#[test]
fn the_player_makes_a_sound_jumping_but_not_for_holding_jump() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.3);
    assert!(game.sounds.is_empty(), "standing is silent");
    let jump = UserCommand {
        jump: true,
        ..UserCommand::default()
    };
    game.run(36, jump);
    assert_eq!(sound_names(&game), ["player/plyrjmp8.wav"]);
    assert!(game.sounds[0].own);
    assert_eq!(game.sounds[0].channel, 4);
    game.run(36, jump);
    assert_eq!(game.sounds.len(), 1);
    game.wait(0.1);
    game.wait(1.0);
    game.run(2, jump);
    assert_eq!(game.sounds.len(), 2, "letting go allows another");
}

#[test]
fn a_long_fall_lands_with_a_sound() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, Vec3::new(0.0, 0.0, FLOOR + 200.0));
    game.wait(1.5);
    assert_eq!(sound_names(&game), ["player/land.wav"]);
    assert_eq!(game.sounds[0].channel, 2);
}

#[test]
fn a_short_fall_lands_quietly() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, Vec3::new(0.0, 0.0, FLOOR + 40.0));
    game.wait(1.0);
    assert!(game.sounds.is_empty());
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
    assert_eq!(game.instance(2).frame, 1, "it shows it's been used");
    game.wait(2.0);
    assert_eq!(game.instance(2).frame, 0, "until it comes back");
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
        ("spawnflags", "1"),
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

/// A player's stored entity stays at the middle of the world, and when a level asks for
/// everything to touch again it mustn't touch what's there. That sent a lift up the moment
/// a player joined, from the far end of a level.
#[test]
fn a_player_far_away_does_not_touch_what_is_at_the_middle_of_the_world() {
    let trigger = (Vec3::new(-32.0, -32.0, -32.0), Vec3::new(32.0, 32.0, 32.0));
    let entities: &[&[(&str, &str)]] = &[&[
        ("classname", "trigger_once"),
        ("model", "*1"),
        ("message", "hello"),
    ]];
    let map = map(&[trigger], entities);
    let mut game = Game::new(&map, at_floor(300.0, 0.0));
    game.wait(0.5);
    game.server.exec("engine.force_retouch()").expect("asks");
    game.wait(0.5);
    assert!(game.prints.is_empty(), "{:?}", game.prints);
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

/// A command holding attack, facing along x.
fn attack() -> UserCommand {
    UserCommand {
        attack: true,
        ..UserCommand::default()
    }
}

/// An impulse.
fn impulse(number: u8) -> UserCommand {
    UserCommand {
        impulse: number,
        ..UserCommand::default()
    }
}

impl Game {
    /// Runs Luau with `player` set, which fails the test if it fails.
    fn check(&self, source: &str) {
        let source = format!(r#"local player = find_all("classname", "player")[1] {source}"#);
        self.server.exec(&source).expect("the check holds");
    }

    /// How many temporary entities of `wanted` have happened.
    fn temps(&self, wanted: u8) -> usize {
        self.effects
            .iter()
            .filter(|effect| matches!(effect, Effect::Temp { kind, .. } if *kind == wanted))
            .count()
    }
}

/// A wall to shoot at x = 100.
fn wall() -> (Vec3, Vec3) {
    (
        Vec3::new(100.0, -64.0, -256.0),
        Vec3::new(116.0, 64.0, -128.0),
    )
}

/// A wall that doesn't move.
const WALL: &[(&str, &str)] = &[("classname", "func_wall"), ("model", "*1")];

#[test]
fn a_new_player_has_a_shotgun_and_shells() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.check(
        r#"
        assert(player.weapon == IT_SHOTGUN)
        assert(player.ammo_shells == 25)
        assert(player.weaponmodel == "progs/v_shot.mdl")
        assert(player.health == 100)
        "#,
    );
    assert_ne!(game.latest.as_ref().expect("a snapshot").weapon_model, 0);
}

#[test]
fn the_shotgun_uses_a_shell_and_hits_the_wall() {
    let (mins, maxs) = wall();
    let map = map(&[(mins, maxs)], &[WALL]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.run(1, attack());
    assert_eq!(sound_names(&game), ["weapons/guncock.wav"]);
    assert_eq!(game.temps(2), 6, "six pellets");
    game.check("assert(player.ammo_shells == 24 and player.weaponframe == 1)");
    game.wait(0.15);
    game.check("assert(player.weaponframe == 2)");
    game.wait(0.6);
    game.check("assert(player.weaponframe == 0)");
}

#[test]
fn holding_attack_fires_at_the_weapons_rate() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.run(72, attack());
    game.check("assert(player.ammo_shells == 23)");
}

#[test]
fn an_impulse_changes_weapon_and_refuses_what_you_lack() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.run(2, impulse(1));
    game.check(r#"assert(player.weapon == IT_AXE and player.weaponmodel == "progs/v_axe.mdl")"#);
    game.run(2, impulse(7));
    game.check("assert(player.weapon == IT_AXE)");
    assert!(game.prints.iter().any(|text| text == "no weapon."));
}

#[test]
fn a_rocket_flies_explodes_and_hurts_who_is_close() {
    let (mins, maxs) = wall();
    let map = map(&[(mins, maxs)], &[WALL]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.run(2, impulse(9));
    game.run(1, attack());
    let flying = game.latest.as_ref().expect("a snapshot").entities.len();
    assert_eq!(flying, 1, "the rocket");
    game.check("assert(player.ammo_rockets == 99)");
    game.wait(0.3);
    assert_eq!(game.temps(3), 1, "one explosion");
    game.check("assert(player.health < 100 and player.health > 0)");
    assert!(game.player().velocity.x < 0.0, "it knocks the player back");
    game.wait(1.0);
    assert!(
        game.latest
            .as_ref()
            .expect("a snapshot")
            .entities
            .is_empty(),
        "the explosion ends"
    );
}

#[test]
fn a_grenade_bounces_and_goes_off_on_its_fuse() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.run(2, impulse(9));
    game.run(2, impulse(6));
    game.run(1, attack());
    game.wait(1.5);
    assert!(sound_names(&game).contains(&"weapons/bounce.wav"));
    assert_eq!(game.temps(3), 0, "not yet");
    game.wait(1.5);
    assert_eq!(game.temps(3), 1);
}

#[test]
fn nails_fire_while_held_and_stop_when_let_go() {
    let (mins, maxs) = wall();
    let map = map(&[(mins, maxs)], &[WALL]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.run(2, impulse(9));
    game.run(2, impulse(4));
    game.run(36, attack());
    game.wait(0.5);
    game.check("assert(player.ammo_nails < 198 and player.ammo_nails > 188)");
    assert!(game.temps(0) >= 2, "spikes hit the wall");
    game.check("assert(player.weaponframe == 0)");
}

/// An item on the floor at the origin.
fn item(class: &'static str) -> Vec<(&'static str, &'static str)> {
    vec![("classname", class), ("origin", "0 0 -255")]
}

#[test]
fn ammo_is_picked_up_once_and_disappears() {
    let shells = item("item_shells");
    let map = map(&[], &[&shells]);
    let mut game = Game::new(&map, at_floor(-200.0, 0.0));
    game.wait(0.5);
    let count = |game: &Game| game.latest.as_ref().expect("a snapshot").entities.len();
    assert_eq!(count(&game), 1, "it's on the floor");
    game.client
        .send(ClientMessage::Console(format!("setpos 16 16 {}", FLOOR + 1.0)).encode());
    game.wait(0.2);
    game.check("assert(player.ammo_shells == 45)");
    assert!(sound_names(&game).contains(&"weapons/lock4.wav"));
    assert_eq!(count(&game), 0, "it's gone");
    game.wait(0.5);
    game.check("assert(player.ammo_shells == 45)");
}

#[test]
fn a_weapon_on_the_floor_is_taken_up_at_once() {
    let weapon = item("weapon_rocketlauncher");
    let map = map(&[], &[&weapon]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.5);
    game.check(
        r#"
        assert(player.weapon == IT_ROCKET_LAUNCHER)
        assert(player.ammo_rockets == 5)
        assert(player.weaponmodel == "progs/v_rock2.mdl")
        "#,
    );
}

#[test]
fn health_heals_up_to_the_limit_and_stays_if_not_needed() {
    let health = item("item_health");
    let map = map(&[], &[&health]);
    let mut game = Game::new(&map, at_floor(-200.0, 0.0));
    game.wait(0.5);
    game.client
        .send(ClientMessage::Console(format!("setpos 16 16 {}", FLOOR + 1.0)).encode());
    game.wait(0.2);
    game.check("assert(player.health == 100)");
    assert_eq!(game.latest.as_ref().expect("a snapshot").entities.len(), 1);
    game.check("player.health = 50");
    game.wait(0.2);
    game.check("assert(player.health == 75)");
    assert!(
        game.latest
            .as_ref()
            .expect("a snapshot")
            .entities
            .is_empty()
    );
}

#[test]
fn armor_soaks_up_damage() {
    let armor = item("item_armor1");
    let map = map(&[], &[&armor]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.5);
    game.check(
        r#"
        assert(player.armorvalue == 100)
        T_Damage(player, world, world, 10)
        assert(player.armorvalue == 97)
        assert(player.health == 93)
        "#,
    );
}

#[test]
fn a_key_opens_its_door() {
    let (mins, maxs) = door_box();
    let door = [DOOR, &[("spawnflags", "16")]].concat();
    let map = map(&[(mins, maxs)], &[&door]);
    let mut game = Game::new(&map, at_floor(60.0, 0.0));
    game.wait(0.3);
    assert!(
        game.instance(1).origin.z.abs() < 0.5,
        "no field on a key door"
    );
    game.check("player.items = bit32.bor(player.items, IT_KEY1)");
    game.run(
        36,
        UserCommand {
            forward: 400.0,
            ..UserCommand::default()
        },
    );
    game.wait(1.0);
    assert!(game.instance(1).origin.z > 50.0, "it opened");
    game.check("assert(bit32.band(player.items, IT_KEY1) == 0)");
}

#[test]
fn a_door_with_health_opens_when_shot() {
    let (mins, maxs) = door_box();
    let door = [DOOR, &[("health", "10")]].concat();
    let map = map(&[(mins, maxs)], &[&door]);
    let mut game = Game::new(&map, at_floor(-100.0, 0.0));
    game.wait(0.3);
    assert!(
        game.instance(1).origin.z.abs() < 0.5,
        "shot doors have no field"
    );
    game.run(1, attack());
    game.wait(0.3);
    assert!(game.instance(1).origin.z > 0.0, "the shot opened it");
}

#[test]
fn a_button_with_health_fires_when_shot() {
    let (door_mins, door_maxs) = door_box();
    let button_mins = Vec3::new(40.0, -16.0, -256.0);
    let button_maxs = Vec3::new(50.0, 16.0, -224.0);
    let door = [DOOR, &[("targetname", "d"), ("wait", "0.2")]].concat();
    let button: &[(&str, &str)] = &[
        ("classname", "func_button"),
        ("model", "*2"),
        ("angle", "-2"),
        ("target", "d"),
        ("health", "1"),
        ("wait", "1"),
    ];
    let map = map(
        &[(door_mins, door_maxs), (button_mins, button_maxs)],
        &[&door, button],
    );
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.3);
    assert!(game.instance(1).origin.z.abs() < 0.5);
    game.run(1, attack());
    game.wait(1.2);
    assert!(game.instance(1).origin.z > 0.0, "the shot pressed it");
}

/// A map with a door, and a trigger with health in front of the player.
fn shootable_trigger(class: &str) -> Arc<Map> {
    let (door_mins, door_maxs) = door_box();
    let trigger_mins = Vec3::new(40.0, -32.0, -256.0);
    let trigger_maxs = Vec3::new(50.0, 32.0, -160.0);
    let door = [DOOR, &[("targetname", "d"), ("wait", "0.2")]].concat();
    let trigger: &[(&str, &str)] = &[
        ("classname", class),
        ("model", "*2"),
        ("target", "d"),
        ("health", "1"),
        ("wait", "1"),
    ];
    map(
        &[(door_mins, door_maxs), (trigger_mins, trigger_maxs)],
        &[&door, trigger],
    )
}

#[test]
fn a_trigger_with_health_fires_when_shot_not_when_touched() {
    let map = shootable_trigger("trigger_multiple");
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.3);
    let forward = UserCommand {
        forward: 400.0,
        ..UserCommand::default()
    };
    game.run(72, forward);
    assert!(
        game.player().origin.x < 40.0,
        "it's solid, like the original"
    );
    assert!(
        game.instance(1).origin.z.abs() < 0.5,
        "walking in does nothing"
    );
    game.run(1, attack());
    game.wait(0.4);
    assert!(game.instance(1).origin.z > 0.0, "the shot fired it");
}

#[test]
fn a_shot_trigger_comes_back_after_its_wait() {
    let map = shootable_trigger("trigger_multiple");
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.3);
    game.check(r#"shot = find(nil, "classname", "trigger_multiple")"#);
    game.check("assert(shot.takedamage == DAMAGE_YES)");
    game.run(1, attack());
    game.wait(0.1);
    game.check("assert(shot.takedamage == DAMAGE_NO)");
    game.wait(1.2);
    game.check("assert(shot.takedamage == DAMAGE_YES and shot.health == 1)");
}

#[test]
fn a_shot_trigger_once_goes_away() {
    let map = shootable_trigger("trigger_once");
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.3);
    game.run(1, attack());
    game.wait(0.5);
    game.check(r#"assert(find(nil, "classname", "trigger_once") == nil)"#);
}

#[test]
fn a_teleport_waiting_to_be_used_leaves_what_stands_in_it_alone() {
    let trigger = (
        Vec3::new(-300.0, -32.0, -256.0),
        Vec3::new(-236.0, 32.0, -200.0),
    );
    let entities: &[&[(&str, &str)]] = &[
        &[
            ("classname", "trigger_teleport"),
            ("model", "*1"),
            ("target", "there"),
            ("targetname", "later"),
        ],
        &[
            ("classname", "info_teleport_destination"),
            ("targetname", "there"),
            ("origin", "200 200 -200"),
        ],
        &[("classname", "monster_ogre"), ("origin", "-268 0 -230")],
    ];
    let map = map(&[trigger], entities);
    let mut game = Game::new(&map, at_floor(300.0, -300.0));
    game.wait(0.2);
    game.check(
        r#"
        ogre = find(nil, "classname", "monster_ogre")
        assert(ogre.origin.x < -200, "it hasn't gone anywhere")
        find(nil, "classname", "trigger_teleport").use(find(nil, "classname", "trigger_teleport"))
        "#,
    );
    game.wait(0.1);
    game.check(r#"assert(ogre.origin.x > 100, "it goes once it's used")"#);
}

/// Lua that makes `thing`, a monster standing still at `x`.
fn still_monster(x: f32) -> String {
    format!(
        r#"
        thing = spawn()
        thing.classname = "monster_test"
        thing.movetype = MOVETYPE_STEP
        thing.solid = SOLID_SLIDEBOX
        thing.flags = FL_ONGROUND
        thing.health = 100
        thing.takedamage = DAMAGE_AIM
        thing.th_pain = SUB_Null
        thing.th_die = SUB_Null
        setsize(thing, vector.create(-16, -16, -24), vector.create(16, 16, 32))
        setorigin(thing, vector.create({x}, 0, {z}))
        "#,
        z = at_floor(0.0, 0.0).z
    )
}

#[test]
fn a_monster_only_touches_a_trigger_by_moving() {
    let hurt = (
        Vec3::new(-200.0, -100.0, -256.0),
        Vec3::new(0.0, 100.0, -100.0),
    );
    let entities: &[&[(&str, &str)]] = &[&[("classname", "trigger_hurt"), ("model", "*1")]];
    let map = map(&[hurt], entities);
    let mut game = Game::new(&map, at_floor(200.0, 200.0));
    game.wait(0.1);
    game.check(&still_monster(-100.0));
    game.wait(2.5);
    game.check(r#"assert(thing.health == 100, "it's been left alone")"#);
    game.check(r#"assert(walkmove(thing, 0, 8), "it can step")"#);
    game.wait(0.05);
    game.check(r#"assert(thing.health < 100, "stepping touched it")"#);
}

#[test]
fn a_noclipping_player_still_touches_triggers() {
    let hurt = (
        Vec3::new(100.0, 100.0, -256.0),
        Vec3::new(250.0, 250.0, -100.0),
    );
    let entities: &[&[(&str, &str)]] = &[&[("classname", "trigger_hurt"), ("model", "*1")]];
    let map = map(&[hurt], entities);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.1);
    game.client
        .send(ClientMessage::Console("noclip".to_owned()).encode());
    game.wait(0.1);
    assert!(game.player().noclip);
    game.check(
        r#"
        local player = find_all("classname", "player")[1]
        assert(player.health == 100, "the control: it's not there yet")
        setorigin(player, vector.create(200, 200, -200))
        "#,
    );
    game.wait(0.1);
    game.check(r#"assert(find_all("classname", "player")[1].health < 100)"#);
}

#[test]
fn someone_arriving_telefrags_a_monster_standing_still() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(200.0, 200.0));
    game.wait(0.1);
    game.check(&still_monster(-100.0));
    game.wait(0.1);
    game.check(
        r#"
        assert(thing.health == 100)
        spawn_tdeath(thing.origin, find_all("classname", "player")[1])
        "#,
    );
    game.wait(0.1);
    game.check(r#"assert(thing.health <= 0, "it was still, and it's dead")"#);
}

#[test]
fn a_single_player_arrival_telefrags_too() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(300.0, 300.0));
    game.wait(0.3);
    game.check(
        r#"
        assert(deathmatch == 0 and coop == 0)
        assert(#find_all("classname", "teledeath") == 0, "the first has gone")
        PutClientInServer(find_all("classname", "player")[1])
        assert(#find_all("classname", "teledeath") == 1, "there's a new one")
        "#,
    );
}

#[test]
fn slow_spikes_are_announced_at_their_real_speed() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(300.0, 300.0));
    game.wait(0.1);
    game.check(
        r#"
        local real = engine.launch
        local told = nil
        engine.launch = function(id, from, origin, velocity, ...)
            told = velocity
            return real(id, from, origin, velocity, ...)
        end
        local player = find_all("classname", "player")[1]
        local fast = launch_spike(player, player.origin, vector.create(1, 0, 0))
        assert(told.x == 1000, "a nail")
        local slow = launch_spike(player, player.origin, vector.create(1, 0, 0), 300)
        assert(told.x == 300 and slow.velocity.x == 300, "a knight's")
        local trap = spawn()
        trap.movedir = vector.create(0, 1, 0)
        trap.origin = player.origin
        spikeshooter_use(trap)
        assert(told.y == 500, "a trap's")
        engine.launch = real
        "#,
    );
}

#[test]
fn grunts_fire_again_on_nightmare() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(200.0, 200.0));
    game.wait(0.1);
    game.check(&still_monster(0.0));
    game.check(
        r#"
        thing.enemy = find_all("classname", "player")[1]
        army_atk7(thing)
        assert(thing.think == army_atk8, "only on nightmare")
        "#,
    );
    game.server.set_setting("skill", 3.0);
    game.wait(TICK);
    game.check(
        r#"
        army_atk7(thing)
        assert(thing.think == army_atk1 and thing.cnt == 1, "again")
        army_atk7(thing)
        assert(thing.think == army_atk8, "but only once")
        "#,
    );
}

#[test]
fn a_player_firing_or_unseen_makes_no_pain_sound() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(300.0, 300.0));
    game.wait(0.1);
    game.sounds.clear();
    game.check(
        r#"
        local player = find_all("classname", "player")[1]
        player.weaponframe = 1
        player_pain(player)
        player.weaponframe = 0
        player.invisible_finished = time() + 10
        player_pain(player)
        player.invisible_finished = 0
        "#,
    );
    game.wait(0.05);
    let pained = |game: &Game| {
        game.sounds
            .iter()
            .any(|sound| sound.name.starts_with("player/pain"))
    };
    assert!(!pained(&game), "{:?}", game.sounds);
    for _ in 0..16 {
        game.check(
            r#"
            local player = find_all("classname", "player")[1]
            player.pain_finished = 0
            player_pain(player)
            "#,
        );
        game.wait(0.05);
    }
    assert!(pained(&game), "the control: it's heard otherwise");
}

#[test]
fn the_invisible_die_as_themselves() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(300.0, 300.0));
    game.wait(0.1);
    game.check(
        r#"
        local real = engine.set_model
        shown = nil
        engine.set_model = function(id, model)
            shown = model
            return real(id, model)
        end
        local player = find_all("classname", "player")[1]
        player.items = bit32.bor(player.items, IT_INVISIBILITY)
        player.invisible_finished = time() + 10
        CheckPowerups(player)
        assert(shown == "progs/eyes.mdl", "the control: just eyes")
        player.health = -1
        PlayerDie(player)
        assert(shown == "progs/player.mdl", "a body")
        engine.set_model = real
        "#,
    );
}

#[test]
fn angles_are_cut_towards_zero_as_c_does() {
    let map = map(&[], &[]);
    let game = Game::new(&map, at_floor(300.0, 300.0));
    game.check(
        r#"
        local function towards(degrees)
            local radians = math.rad(degrees)
            return vector.create(math.cos(radians), math.sin(radians), 0)
        end
        assert(vectoyaw(towards(45.7)) == 45)
        assert(vectoyaw(towards(-45.7)) == 315)
        assert(vectoyaw(towards(-0.5)) == 0)
        assert(vectoangles(towards(-45.7)).y == 315)
        local down = vector.create(math.cos(math.rad(10.5)), 0, -math.sin(math.rad(10.5)))
        assert(vectoangles(down).x == 350)
        "#,
    );
}

#[test]
fn a_facing_trigger_goes_by_a_players_angles() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(300.0, 300.0));
    game.run(
        1,
        UserCommand {
            pitch: 60.0,
            ..UserCommand::default()
        },
    );
    game.check(
        r#"
        local player = find_all("classname", "player")[1]
        local trigger = spawn()
        trigger.movedir = vector.create(0, 0, 1)
        trigger.wait = 0.2
        multi_touch(trigger, player)
        assert(trigger.enemy == player, "looking down tilts a player's body up")
        "#,
    );
    game.run(
        1,
        UserCommand {
            pitch: -60.0,
            ..UserCommand::default()
        },
    );
    game.check(
        r#"
        local player = find_all("classname", "player")[1]
        local other = spawn()
        other.movedir = vector.create(0, 0, 1)
        other.wait = 0.2
        multi_touch(other, player)
        assert(other.enemy == nil, "the control")
        "#,
    );
}

#[test]
fn what_flies_thinks_before_it_moves() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(300.0, 300.0));
    game.wait(0.1);
    game.check(
        r#"
        flying = spawn()
        flying.movetype = MOVETYPE_FLY
        flying.solid = SOLID_NOT
        setsize(flying, vector.zero, vector.zero)
        setorigin(flying, vector.create(0, 0, -100))
        flying.velocity = vector.create(100, 0, 0)
        flying.nextthink = time()
        flying.think = function(self)
            thought_at = self.origin
        end
        "#,
    );
    game.wait(0.1);
    game.check(
        r#"
        assert(thought_at.x == 0, "it hadn't gone anywhere")
        assert(flying.origin.x > 0, "the control: it went then")
        "#,
    );
}

#[test]
fn a_grenades_spread_is_worked_out_the_same_on_both_sides() {
    let map = map(&[], &[]);
    let game = Game::new(&map, at_floor(0.0, 0.0));
    let shot = weapon_rule(16, 5)
        .and_then(|rule| rule.shot)
        .expect("grenades fly");
    let axes = [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
    ];
    for seed in [0_u16, 1, 999, 40503, 65535] {
        let (_, velocity) = shot_launch(&shot, Vec3::ZERO, axes, 10.0, seed);
        game.check(&format!(
            r#"
            local shooter = {{ fire_seed = {seed} }}
            assert(math.abs(grenade_spread(shooter, 0) * 10 - {right}) < 0.001)
            assert(math.abs(grenade_spread(shooter, 1) * 10 - {up}) < 0.001)
            "#,
            right = velocity.y,
            up = velocity.z - 200.0
        ));
    }
}

#[test]
fn a_wind_tunnel_takes_a_grenade_too() {
    let tunnel = (
        Vec3::new(-400.0, -32.0, -256.0),
        Vec3::new(-200.0, 32.0, -100.0),
    );
    let entities: &[&[(&str, &str)]] = &[&[
        ("classname", "trigger_push"),
        ("model", "*1"),
        ("angle", "-1"),
        ("speed", "1000"),
    ]];
    let map = map(&[tunnel], entities);
    let mut game = Game::new(&map, at_floor(300.0, 300.0));
    game.wait(0.1);
    game.check(
        r#"
        flying = spawn()
        flying.classname = "grenade"
        flying.movetype = MOVETYPE_BOUNCE
        flying.solid = SOLID_BBOX
        setsize(flying, vector.zero, vector.zero)
        setorigin(flying, vector.create(-300, 0, -200))
        flying.velocity = vector.create(0, 0, -1)
        "#,
    );
    game.wait(0.05);
    game.check(r#"assert(flying.velocity.z > 1000, "it's thrown upwards")"#);
}

#[test]
fn nobody_can_back_up_straight_after_a_teleport() {
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
    assert!(
        game.player().no_backing > 0.0,
        "it starts with the teleport"
    );
    let back = UserCommand {
        yaw: 90.0,
        forward: -400.0,
        ..UserCommand::default()
    };
    let start = game.player().origin.y;
    game.run(36, back);
    let held = game.player().origin.y - start;
    assert!(held > 0.0, "still going forwards: {held}");
    game.run(36, UserCommand::default());
    assert_eq!(game.player().no_backing, 0.0, "and it wears off");
    let start = game.player().origin.y;
    game.run(36, back);
    assert!(
        game.player().origin.y < start,
        "after which backing up works"
    );
}

/// The sounds a player makes taking `damage` from something called `class`.
fn hurt_by(class: &str, damage: u32) -> Vec<String> {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.sounds.clear();
    game.check(&format!(
        r#"
        local from = spawn()
        from.classname = "{class}"
        from.owner = player
        T_Damage(player, from, from, {damage})
        "#
    ));
    game.wait(0.05);
    game.sounds.iter().map(|sound| sound.name.clone()).collect()
}

#[test]
fn a_telefrag_has_a_sound_of_its_own() {
    let telefrag = "player/teledth1.wav".to_owned();
    assert!(hurt_by("teledeath", 10).contains(&telefrag), "as pain");
    assert!(hurt_by("teledeath", 50_000).contains(&telefrag), "as a gib");
    assert!(hurt_by("teledeath2", 50_000).contains(&telefrag));
    assert!(
        !hurt_by("monster_ogre", 10).contains(&telefrag),
        "and only then"
    );
    assert!(!hurt_by("monster_ogre", 50_000).contains(&telefrag));
}

#[test]
fn a_gibbed_player_is_dead_at_once() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.check("T_Damage(player, world, world, 50000)");
    game.check("assert(player.deadflag == DEAD_DEAD)");
}

#[test]
fn a_dying_player_is_thrown_up_a_little() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.3);
    let floor = game.player().origin.z;
    // The first numbers from a new seed are small, and the hop is random.
    game.check("for _ = 1, 8 do random() end");
    game.check("T_Damage(player, world, world, 110)");
    let mut highest = floor;
    for _ in 0..36 {
        game.run(1, UserCommand::default());
        highest = highest.max(game.player().origin.z);
    }
    assert!(highest > floor + 0.5, "{highest} from {floor}");
    assert!(game.player().origin.z - floor < 0.5, "and lands again");
}

/// Whether taking an item brings it back later, in deathmatch `kind`.
fn comes_back(class: &'static str, kind: u8) -> bool {
    let thing = item(class);
    let map = map(&[], &[&thing]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.check(&format!("deathmatch = {kind}"));
    game.check(&format!(r#"thing = find(nil, "classname", "{class}")"#));
    game.wait(0.3);
    game.check("came_back = thing.think == SUB_regen");
    game.check("engine.print(tostring(came_back))");
    game.wait(0.05);
    game.prints.iter().any(|line| line.contains("true"))
}

#[test]
fn powerups_come_back_in_every_deathmatch_and_armor_only_the_first() {
    assert!(comes_back("item_artifact_super_damage", 1));
    assert!(comes_back("item_artifact_super_damage", 2));
    assert!(!comes_back("item_artifact_super_damage", 0));
    assert!(comes_back("item_armor1", 1));
    assert!(!comes_back("item_armor1", 2));
}

/// Whether touching a `trigger_onlyregistered` fires it, by `registered`.
fn registered_fires(registered: u8) -> (bool, Vec<String>) {
    let trigger = (
        Vec3::new(-32.0, -32.0, -256.0),
        Vec3::new(32.0, 32.0, -200.0),
    );
    let entities: &[&[(&str, &str)]] = &[&[
        ("classname", "trigger_onlyregistered"),
        ("model", "*1"),
        ("message", "Registered only"),
    ]];
    let map = map(&[trigger], entities);
    let mut game = Game::new(&map, at_floor(200.0, 200.0));
    game.check(&format!(r#"set_level_setting("registered", {registered})"#));
    game.check(r#"setorigin(player, vector.create(0, 0, -232))"#);
    game.wait(0.1);
    game.check(
        r#"engine.print(tostring(find(nil, "classname", "trigger_onlyregistered") == nil))"#,
    );
    game.wait(0.05);
    let gone = game.prints.iter().any(|line| line.contains("true"));
    (gone, game.prints.clone())
}

#[test]
fn only_a_registered_game_gets_past_a_registered_trigger() {
    let (fired, _prints) = registered_fires(1);
    assert!(fired);
    let (fired, prints) = registered_fires(0);
    assert!(!fired, "the shareware game stops here");
    assert!(
        prints.iter().any(|line| line == "Registered only"),
        "{prints:?}"
    );
}

#[test]
fn rules_changed_in_play_take_effect_on_the_next_tick() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.check("assert(teamplay == 0 and skill == 1)");
    game.server.set_setting("teamplay", 1.0);
    game.server.set_setting("skill", 3.0);
    game.check("assert(teamplay == 0)");
    game.wait(0.05);
    game.check("assert(teamplay == 1 and skill == 3)");
    game.server.set_setting("skill", 9.0);
    game.wait(0.05);
    game.check("assert(skill == 3)");
}

#[test]
fn a_dead_player_stays_down_and_a_button_restarts_the_level() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.check("T_Damage(player, world, world, 110)");
    game.wait(0.05);
    assert!(game.player().dead);
    game.check("assert(player.deadflag == DEAD_DYING)");
    let forward = UserCommand {
        forward: 400.0,
        ..UserCommand::default()
    };
    let before = game.player().origin;
    game.run(36, forward);
    assert!(
        (game.player().origin - before).length() < 0.5,
        "they can't walk"
    );
    game.wait(1.2);
    assert_eq!(game.server.take_next_level(), None, "it waits for a button");
    game.run(2, attack());
    assert!(game.server.take_next_level().is_some());
}

#[test]
fn a_level_change_carries_gear_but_not_keys() {
    let trigger: &[(&str, &str)] = &[
        ("classname", "trigger_changelevel"),
        ("model", "*1"),
        ("map", "next"),
        ("spawnflags", "1"),
    ];
    let (mins, maxs) = (
        Vec3::new(-32.0, -32.0, -256.0),
        Vec3::new(32.0, 32.0, -200.0),
    );
    let map_one = map(&[(mins, maxs)], &[trigger]);
    let mut game = Game::new(&map_one, at_floor(-300.0, 0.0));
    game.wait(0.05);
    game.check(
        r#"
        player.items = bit32.bor(player.items, IT_NAILGUN, IT_KEY1)
        player.ammo_nails = 40
        player.health = 20
        "#,
    );
    game.client
        .send(ClientMessage::Console(format!("setpos 0 0 {}", FLOOR + 1.0)).encode());
    game.wait(0.1);
    assert_eq!(game.server.take_next_level().as_deref(), Some("next"));
    let parms = game.server.take_parms().expect("gear to carry");
    assert_eq!(parms[1], 50.0, "health is at least 50");
    assert_eq!(parms[4], 40.0, "nails");
    let keyed = 131_072.0;
    assert!(parms[0] < keyed, "keys stay behind: {}", parms[0]);
}

#[test]
fn lava_burns() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.check(
        r#"
        T_Damage(player, world, world, 5)
        assert(player.health == 95)
        "#,
    );
}

#[test]
fn firing_knocks_the_view_and_it_recovers() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.run(1, attack());
    let punch = game.latest.as_ref().expect("a snapshot").punch;
    assert!(punch.x < -1.5, "{punch:?}");
    game.wait(0.3);
    let punch = game.latest.as_ref().expect("a snapshot").punch;
    assert!(punch.x.abs() < f32::EPSILON, "{punch:?}");
}

#[test]
fn a_shot_flashes_on_the_player() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    let latest = game.latest.as_ref().expect("a snapshot");
    assert_eq!(latest.player_effects, 0);
    let mut flashes = 0;
    for _ in 0..6 {
        game.run(1, attack());
        let latest = game.latest.as_ref().expect("a snapshot");
        flashes += usize::from(latest.player_effects & 2 != 0);
    }
    assert_eq!(flashes, 1, "for one snapshot");
}

/// A barrel on the floor at x = 100.
const BARREL: &[(&str, &str)] = &[("classname", "misc_explobox"), ("origin", "84 -16 -200")];

#[test]
fn a_barrel_stands_on_the_floor_and_goes_off_when_hit() {
    let map = map(&[], &[BARREL]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.check(
        r#"local barrel = find_all("classname", "misc_explobox")[1]
        assert(barrel.origin.z < -250, "it drops")"#,
    );
    game.run(2, impulse(9));
    game.run(1, attack());
    game.wait(0.3);
    game.check(
        r#"assert(#find_all("classname", "misc_explobox") == 0, "it's gone")
        assert(#find_all("classname", "explo_box") == 1, "it's exploding")
        assert(player.health < 100)"#,
    );
    assert_eq!(game.temps(3), 1, "the rocket");
    assert!(
        game.effects
            .iter()
            .any(|effect| matches!(effect, Effect::Particles { count: 255, .. })),
        "the barrel's burst"
    );
}

#[test]
fn a_spike_trap_fires_when_used_and_a_shooter_on_its_own() {
    let trap: &[(&str, &str)] = &[
        ("classname", "trap_spikeshooter"),
        ("origin", "-100 0 -200"),
        ("targetname", "t"),
    ];
    let shooter: &[(&str, &str)] = &[
        ("classname", "trap_shooter"),
        ("origin", "0 200 -200"),
        ("wait", "0.25"),
        ("angle", "90"),
    ];
    let map = map(&[], &[trap, shooter]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.6);
    let shots = sound_names(&game)
        .iter()
        .filter(|name| **name == "weapons/spike2.wav")
        .count();
    assert_eq!(
        shots, 3,
        "the shooter, at once, then every quarter of a second"
    );
    game.check(
        r#"local trap = find_all("classname", "trap_spikeshooter")[1]
        local before = #find_all("classname", "spike")
        trap.use(trap)
        assert(#find_all("classname", "spike") == before + 1, "the trap")"#,
    );
}

#[test]
fn bubbles_rise_and_burst() {
    let bubbles: &[(&str, &str)] = &[("classname", "air_bubbles"), ("origin", "100 0 -200")];
    let map = map(&[], &[bubbles]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(2.0);
    game.check(
        r#"local bubble = find_all("classname", "bubble")[1]
        assert(bubble ~= nil and bubble.origin.z > -200, "it rises")"#,
    );
    game.wait(15.0);
    let count = game.latest.as_ref().expect("a snapshot").entities.len();
    assert!(count < 30, "they don't pile up: {count}");
}

#[test]
fn a_fireball_is_thrown_up_and_goes_when_it_lands() {
    let fireball: &[(&str, &str)] = &[
        ("classname", "misc_fireball"),
        ("origin", "0 0 -200"),
        ("speed", "300"),
    ];
    let world: &[(&str, &str)] = &[("classname", "worldspawn")];
    let map = map(&[], &[world, fireball]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    let (mut flying, mut landed) = (0, 0);
    for _ in 0..120 {
        game.wait(0.1);
        let count = game.latest.as_ref().expect("a snapshot").entities.len();
        flying += usize::from(count > 0);
        landed += usize::from(flying > 0 && count == 0);
    }
    assert!(flying > 0, "it throws them");
    assert!(landed > 0, "and they go when they land");
}

/// A soldier facing the player from `x`.
fn soldier(x: &str) -> [(&'static str, String); 3] {
    [
        ("classname", "monster_army".to_owned()),
        ("origin", format!("{x} 0 -200")),
        ("angle", "180".to_owned()),
    ]
}

/// `fields` as the borrowed list a map takes.
fn borrowed<'a>(fields: &'a [(&'static str, String)]) -> Vec<(&'a str, &'a str)> {
    fields
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect()
}

#[test]
fn a_map_angle_turns_a_monster() {
    let fields = soldier("200");
    let map = map(&[], &[&borrowed(&fields)]);
    let mut game = Game::new(&map, at_floor(-200.0, 0.0));
    game.wait(0.1);
    game.check(
        r#"local grunt = find_all("classname", "monster_army")[1]
        assert(grunt.angles.y == 180, tostring(grunt.angles))
        assert(grunt.origin.z < -250, "it dropped to the floor")"#,
    );
}

#[test]
fn a_soldier_sees_the_player_and_shoots() {
    let fields = soldier("200");
    let map = map(&[], &[&borrowed(&fields)]);
    let mut game = Game::new(&map, at_floor(-100.0, 0.0));
    game.wait(0.8);
    assert!(
        sound_names(&game).contains(&"soldier/sight1.wav"),
        "{:?}",
        sound_names(&game)
    );
    game.wait(6.0);
    assert!(sound_names(&game).contains(&"soldier/sattck1.wav"));
    game.check(r#"assert(player.health < 100, "it hit")"#);
}

#[test]
fn a_soldier_with_its_back_turned_does_not_notice_from_afar() {
    let mut fields = soldier("200");
    fields[2].1 = "0".to_owned();
    let map = map(&[], &[&borrowed(&fields)]);
    let mut game = Game::new(&map, at_floor(-240.0, 0.0));
    game.wait(2.0);
    assert!(!sound_names(&game).contains(&"soldier/sight1.wav"));
}

#[test]
fn a_soldier_hears_nothing_through_a_wall() {
    let fields = soldier("200");
    let (mins, maxs) = (
        Vec3::new(100.0, -256.0, -256.0),
        Vec3::new(116.0, 256.0, 256.0),
    );
    let map = map(&[(mins, maxs)], &[WALL, &borrowed(&fields)]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(2.0);
    assert!(!sound_names(&game).contains(&"soldier/sight1.wav"));
}

#[test]
fn a_shot_wakes_a_monster_up_and_it_comes_for_the_player() {
    let mut fields = soldier("200");
    fields[2].1 = "0".to_owned();
    let map = map(&[], &[&borrowed(&fields)]);
    let mut game = Game::new(&map, at_floor(-240.0, 0.0));
    game.wait(0.2);
    game.check(
        r#"local grunt = find_all("classname", "monster_army")[1]
        T_Damage(grunt, player, player, 1)
        assert(grunt.enemy == player)"#,
    );
    game.wait(0.1);
    game.check(
        r#"local grunt = find_all("classname", "monster_army")[1]
        assert(grunt.goalentity == player)"#,
    );
}

#[test]
fn a_dead_soldier_falls_is_no_longer_solid_and_drops_a_backpack() {
    let fields = soldier("100");
    let map = map(&[], &[&borrowed(&fields)]);
    let mut game = Game::new(&map, at_floor(-100.0, 0.0));
    game.wait(0.2);
    game.check(
        r#"local grunt = find_all("classname", "monster_army")[1]
        T_Damage(grunt, player, player, 30)
        assert(killed_monsters == 1)
        assert(grunt.health <= 0)"#,
    );
    let frame = |game: &Game| game.latest.as_ref().expect("a snapshot").entities[0].frame;
    let first = frame(&game);
    game.wait(1.5);
    game.check(
        r#"local grunt = find_all("classname", "monster_army")[1]
        assert(grunt.solid == SOLID_NOT)
        assert(#find_all("touch", BackpackTouch) == 1)"#,
    );
    assert_ne!(frame(&game), first, "it plays its death");
    assert!(sound_names(&game).iter().any(|name| name.contains("death")));
}

#[test]
fn a_monster_blown_apart_leaves_gibs() {
    let fields = soldier("100");
    let map = map(&[], &[&borrowed(&fields)]);
    let mut game = Game::new(&map, at_floor(-100.0, 0.0));
    game.wait(0.2);
    game.check(
        r#"local grunt = find_all("classname", "monster_army")[1]
        T_Damage(grunt, player, player, 500)"#,
    );
    game.wait(0.05);
    let parts = game.latest.as_ref().expect("a snapshot").entities.len();
    assert!(parts >= 4, "a head and gibs and a backpack: {parts}");
}

#[test]
fn a_monster_follows_its_path() {
    let fields = [
        ("classname", "monster_army".to_owned()),
        ("origin", "-200 -200 -200".to_owned()),
        ("target", "first".to_owned()),
    ];
    let first: &[(&str, &str)] = &[
        ("classname", "path_corner"),
        ("targetname", "first"),
        ("target", "second"),
        ("origin", "-100 -200 -232"),
    ];
    let second: &[(&str, &str)] = &[
        ("classname", "path_corner"),
        ("targetname", "second"),
        ("target", "first"),
        ("origin", "-100 100 -232"),
    ];
    let map = map(&[], &[&borrowed(&fields), first, second]);
    let mut game = Game::new(&map, at_floor(200.0, 200.0));
    game.server
        .exec(r#"local p = find_all("classname", "player")[1] p.flags = 128"#)
        .expect("notarget");
    game.wait(7.0);
    game.check(
        r#"local grunt = find_all("classname", "monster_army")[1]
        assert(grunt.origin.x > -130, "it reached the first corner: " .. tostring(grunt.origin))"#,
    );
    game.wait(5.0);
    game.check(
        r#"local grunt = find_all("classname", "monster_army")[1]
        assert(grunt.origin.y > -150, "it turned to the second: " .. tostring(grunt.origin))"#,
    );
}

#[test]
fn a_plat_carries_a_monster_standing_on_it() {
    let (mins, maxs) = plat_box();
    let plat: &[(&str, &str)] = &[("classname", "func_plat"), ("model", "*1")];
    let fields = [
        ("classname", "monster_army".to_owned()),
        ("origin", "0 0 -100".to_owned()),
        ("angle", "0".to_owned()),
    ];
    let map = map(&[(mins, maxs)], &[plat, &borrowed(&fields)]);
    let mut game = Game::new(&map, at_floor(200.0, 200.0));
    game.server
        .exec(r#"local p = find_all("classname", "player")[1] p.flags = 128"#)
        .expect("notarget");
    game.wait(0.3);
    game.check(
        r#"local grunt = find_all("classname", "monster_army")[1]
        local plat = find_all("classname", "plat")[1] or find_all("classname", "func_plat")[1]
        grunt.nextthink = 0
        plat_go_down(plat)"#,
    );
    game.wait(0.5);
    game.check(
        r#"local grunt = find_all("classname", "monster_army")[1]
        local plat = find_all("classname", "plat")[1] or find_all("classname", "func_plat")[1]
        local gap = grunt.origin.z + grunt.mins.z - (plat.origin.z + plat.maxs.z)
        assert(math.abs(gap + 24) < 8, "it goes down with it: " .. gap)"#,
    );
}

#[test]
fn a_leaping_monster_comes_down_and_lands() {
    let fields = soldier("100");
    let map = map(&[], &[&borrowed(&fields)]);
    let mut game = Game::new(&map, at_floor(-200.0, 0.0));
    game.wait(0.1);
    game.check(
        r#"local dog = find_all("classname", "monster_army")[1]
        dog.flags -= bit32.band(dog.flags, FL_ONGROUND)
        dog.velocity = vector.create(0, 0, 300)
        dog.origin = dog.origin + vector.create(0, 0, 1)"#,
    );
    game.wait(0.1);
    game.check(
        r#"local dog = find_all("classname", "monster_army")[1]
        assert(dog.origin.z > -240, "it's in the air")"#,
    );
    game.wait(1.5);
    game.check(
        r#"local dog = find_all("classname", "monster_army")[1]
        assert(bit32.band(dog.flags, FL_ONGROUND) ~= 0, "it landed")"#,
    );
}

#[test]
fn a_rocket_finds_a_monster_near_its_line() {
    let fields = soldier("150");
    let map = map(&[], &[&borrowed(&fields)]);
    let mut game = Game::new(&map, at_floor(-100.0, 0.0));
    game.wait(0.1);
    game.run(2, impulse(9));
    game.check(
        r#"local grunt = find_all("classname", "monster_army")[1]
        grunt.nextthink = 0
        makevectors(vector.create(10, 0, 0))
        local bent = aim(player, 1000)
        assert(bent.z > v_forward.z, "it bends up towards the soldier: " .. tostring(bent))"#,
    );
}

#[test]
fn a_monster_on_a_teleport_arrives_and_frags_what_is_there() {
    let (mins, maxs) = (
        Vec3::new(-32.0, -32.0, -256.0),
        Vec3::new(32.0, 32.0, -200.0),
    );
    let trigger: &[(&str, &str)] = &[
        ("classname", "trigger_teleport"),
        ("model", "*1"),
        ("target", "there"),
    ];
    let destination: &[(&str, &str)] = &[
        ("classname", "info_teleport_destination"),
        ("targetname", "there"),
        ("origin", "200 200 -256"),
    ];
    let fields = [
        ("classname", "monster_army".to_owned()),
        ("origin", "0 0 -230".to_owned()),
    ];
    let victim = [
        ("classname", "monster_ogre".to_owned()),
        ("origin", "200 200 -230".to_owned()),
    ];
    let map = map(
        &[(mins, maxs)],
        &[trigger, destination, &borrowed(&fields), &borrowed(&victim)],
    );
    let mut game = Game::new(&map, at_floor(-200.0, -200.0));
    game.server
        .exec(r#"local p = find_all("classname", "player")[1] p.flags = 128"#)
        .expect("notarget");
    game.wait(1.5);
    game.check(
        r#"local grunt = find_all("classname", "monster_army")[1]
        assert(grunt.origin.x > 100, "it's there: " .. tostring(grunt.origin))"#,
    );
}

/// Whether firing `weapon`, by its impulse, for `seconds` at a switch ten units
/// thick presses it. Explosives go off a little way down the room.
fn shot_switch_is_pressed(weapon: u8, seconds: f64) -> bool {
    let (mins, maxs) = (
        Vec3::new(120.0, -64.0, -280.0),
        Vec3::new(130.0, 64.0, -180.0),
    );
    let button: &[(&str, &str)] = &[
        ("classname", "func_button"),
        ("model", "*1"),
        ("health", "1"),
        ("wait", "-1"),
        ("angle", "0"),
    ];
    let map = map(&[(mins, maxs)], &[button]);
    let mut game = Game::new(&map, at_floor(60.0, 0.0));
    game.wait(0.05);
    game.run(2, impulse(9));
    game.run(2, impulse(weapon));
    game.wait(0.1);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "test durations are short"
    )]
    let ticks = (seconds * 72.0) as u32;
    game.run(ticks, attack());
    game.wait(1.0);
    game.instance(1).frame == 1
}

#[test]
fn every_weapon_can_press_a_shot_switch() {
    for weapon in [1, 2, 3, 4, 5, 7, 8] {
        assert!(shot_switch_is_pressed(weapon, 0.6), "weapon {weapon}");
    }
}

#[test]
fn an_explosion_nearby_presses_a_shot_switch() {
    let (mins, maxs) = (
        Vec3::new(120.0, -64.0, -280.0),
        Vec3::new(130.0, 64.0, -180.0),
    );
    let button: &[(&str, &str)] = &[
        ("classname", "func_button"),
        ("model", "*1"),
        ("health", "1"),
        ("wait", "-1"),
        ("angle", "0"),
    ];
    let map = map(&[(mins, maxs)], &[button]);
    let mut game = Game::new(&map, at_floor(-200.0, 0.0));
    game.wait(0.05);
    game.check(
        r#"local blast = spawn()
        blast.origin = vector.create(60, 0, -230)
        T_RadiusDamage(blast, player, 120, nil)"#,
    );
    game.wait(1.0);
    assert_eq!(game.instance(1).frame, 1);
}

#[test]
fn a_touch_button_does_not_mind_being_shot() {
    let (mins, maxs) = (
        Vec3::new(120.0, -64.0, -280.0),
        Vec3::new(130.0, 64.0, -180.0),
    );
    let button: &[(&str, &str)] = &[
        ("classname", "func_button"),
        ("model", "*1"),
        ("wait", "-1"),
        ("angle", "0"),
    ];
    let map = map(&[(mins, maxs)], &[button]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.run(36, attack());
    game.wait(1.0);
    assert_eq!(game.instance(1).frame, 0);
}

#[test]
fn the_ring_of_shadows_hides_the_gun_until_it_runs_out() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    let model = |game: &Game| game.latest.as_ref().expect("a snapshot").weapon_model;
    assert_ne!(model(&game), 0);
    game.check(
        r#"player.items = bit32.bor(player.items, IT_INVISIBILITY)
        player.invisible_finished = time() + 0.2
        player.invisible_time = 1"#,
    );
    game.wait(0.1);
    assert_eq!(model(&game), 0);
    game.wait(0.3);
    assert_ne!(model(&game), 0);
}

/// A level with an exit around the origin and a camera to end on, with
/// `world` as its world.
fn level_with_an_exit(world: &[(&str, &str)]) -> Arc<Map> {
    let exit = (
        Vec3::new(-32.0, -32.0, -256.0),
        Vec3::new(32.0, 32.0, -200.0),
    );
    let trigger: &[(&str, &str)] = &[
        ("classname", "trigger_changelevel"),
        ("model", "*1"),
        ("map", "next"),
    ];
    let camera: &[(&str, &str)] = &[
        ("classname", "info_intermission"),
        ("origin", "100 50 -100"),
        ("mangle", "10 90 0"),
    ];
    map(&[exit], &[world, trigger, camera])
}

#[test]
fn the_end_of_a_level_holds_the_player_at_the_camera_until_a_button() {
    let map = level_with_an_exit(&[("classname", "worldspawn")]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.3);
    assert_eq!(game.intermission, 1);
    assert_eq!(game.music, Some(3));
    assert_eq!(game.views.last(), Some(&(90.0, 10.0)));
    let held = game.player();
    assert!(held.frozen);
    assert_eq!(held.origin, Vec3::new(100.0, 50.0, -100.0));
    // The camera's at the same place, so their body would be all round it.
    game.check(r#"assert(not find_entity(world, "classname", "player").visible, "still shown")"#);
    let walking = UserCommand {
        forward: 400.0,
        ..UserCommand::default()
    };
    game.run(36, walking);
    assert_eq!(game.player().origin, held.origin, "nothing moves them");
    assert_eq!(game.server.take_next_level(), None, "not straight away");
    game.wait(2.0);
    assert_eq!(game.server.take_next_level(), None, "it waits for a button");
    game.run(2, attack());
    assert_eq!(game.server.take_next_level().as_deref(), Some("next"));
    assert!(game.server.take_parms().is_some());
}

#[test]
fn the_end_of_an_episode_has_its_text_first() {
    let map = level_with_an_exit(&[("classname", "worldspawn")]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.check(r#"world.model = "maps/e3m6.bsp""#);
    game.wait(2.5);
    game.run(2, attack());
    assert_eq!(game.intermission, 2);
    assert_eq!(game.server.take_next_level(), None);
    game.wait(1.1);
    game.run(2, attack());
    assert_eq!(game.server.take_next_level().as_deref(), Some("next"));
}

#[test]
fn all_four_runes_are_worth_a_last_word() {
    let map = level_with_an_exit(&[("classname", "worldspawn")]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.check(r#"world.model = "maps/e4m7.bsp" serverflags = 15"#);
    game.wait(2.5);
    game.run(2, attack());
    game.wait(1.1);
    game.run(2, attack());
    assert_eq!(game.server.take_next_level(), None, "another page");
    game.wait(1.1);
    game.run(2, attack());
    assert_eq!(game.server.take_next_level().as_deref(), Some("next"));
    assert_eq!(game.server.flags(), 15.0);
}

#[test]
fn a_rune_stays_collected_on_the_next_level() {
    let map = map(&[], &[]);
    let level = Level {
        flags: 5.0,
        ..normal_level()
    };
    let (client, server_end) = loopback();
    let world = Rc::new(CollisionWorld::new(Arc::clone(&map)));
    let server = Server::new(
        Box::new(server_end),
        world,
        at_floor(0.0, 0.0),
        SCRIPTS,
        TICK,
        level,
    )
    .expect("the scripts load");
    server.exec("assert(serverflags == 5)").expect("it carries");
    drop(client);
}

#[test]
fn the_skill_chosen_in_the_start_map_goes_to_the_host() {
    let (mins, maxs) = (
        Vec3::new(-32.0, -32.0, -256.0),
        Vec3::new(32.0, 32.0, -200.0),
    );
    let trigger: &[(&str, &str)] = &[
        ("classname", "trigger_setskill"),
        ("model", "*1"),
        ("message", "3"),
    ];
    let map = map(&[(mins, maxs)], &[trigger]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    assert!(
        game.server
            .take_commands()
            .iter()
            .any(|command| command == "skill 3")
    );
}

#[test]
fn stats_follow_the_player() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.check("player.health = 62 player.ammo_shells = 17 W_SetCurrentAmmo(player)");
    game.wait(0.05);
    let stats = game.latest.as_ref().expect("a snapshot").stats.clone();
    assert_eq!(stats.len(), 17);
    assert_eq!(stats[0], 62, "health");
    assert_eq!(stats[3], 17, "the weapon's ammo");
    assert_eq!(stats[6], 17, "shells");
    assert_eq!(stats[10], 1, "the shotgun");
    assert_ne!(stats[15] & 256, 0, "the shells icon");
}

#[test]
fn being_hurt_is_told_once_with_where_it_came_from() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.check(
        r#"
        local from = spawn()
        from.origin = vector.create(100, 0, 0)
        player.armorvalue = 10
        player.armortype = 0.5
        T_Damage(player, from, from, 8)
        "#,
    );
    game.wait(0.05);
    assert_eq!(game.damage, [(4, 4, Vec3::new(100.0, 0.0, 0.0))]);
}

#[test]
fn a_pickup_flashes_the_screen() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.check("bonus_flash(player)");
    game.wait(0.05);
    assert_eq!(game.stuffed, ["bf"]);
}

#[test]
fn obituaries_say_what_happened() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.check(
        r#"
        local ogre = spawn()
        ogre.classname = "monster_ogre"
        ogre.flags = FL_MONSTER
        T_Damage(player, ogre, ogre, 500)
        "#,
    );
    game.wait(0.05);
    assert!(
        game.prints
            .iter()
            .any(|text| text == "player was destroyed by an Ogre"),
        "{:?}",
        game.prints
    );
}

#[test]
fn cheats_arrive_through_the_console() {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.client
        .send(ClientMessage::Console("god ".to_owned()).encode());
    game.wait(0.05);
    game.check("T_Damage(player, world, world, 500) assert(player.health == 100)");
    game.client
        .send(ClientMessage::Console("kill ".to_owned()).encode());
    game.wait(0.05);
    assert!(game.server.take_next_level().is_some(), "it starts again");
}

/// How many shots `weapon`'s impulse gets away in ten seconds of holding fire.
fn shots_in_ten_seconds(weapon: u8, ammo: &str) -> f64 {
    let map = map(&[], &[]);
    let mut game = Game::new(&map, at_floor(0.0, 0.0));
    game.wait(0.05);
    game.run(2, impulse(9));
    game.run(2, impulse(weapon));
    game.wait(0.1);
    game.check(&format!("player.{ammo} = 200 W_SetCurrentAmmo(player)"));
    game.run(720, attack());
    let left = format!("assert(false, player.{ammo})");
    let error = game.server.exec(&format!(
        "local player = find_all(\"classname\", \"player\")[1] {left}"
    ));
    let text = format!("{:?}", error.err());
    let number = text
        .split(|c: char| !c.is_ascii_digit())
        .find(|part| !part.is_empty() && part.len() <= 3 && part != &"1")
        .unwrap_or("0");
    200.0 - number.parse::<f64>().unwrap_or(0.0)
}

#[test]
fn rates_of_fire_dont_round_up_to_whole_ticks() {
    let rockets = shots_in_ten_seconds(7, "ammo_rockets");
    assert!((12.0..=13.0).contains(&rockets), "0.8 s apart: {rockets}");
    let nails = shots_in_ten_seconds(4, "ammo_nails");
    assert!((98.0..=101.0).contains(&nails), "ten a second: {nails}");
}
