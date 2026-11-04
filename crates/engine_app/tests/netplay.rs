//! Bots playing real maps over simulated networks, measured (proposal 0049).
//! Run with `QUAKE_DIR` set and `--ignored`, because they need game data.

#![expect(clippy::print_stdout, reason = "the measurements are the point")]

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use engine_assets::FileSystem;
use engine_client::{Bot, BotSkill, Client, Path};
use engine_core::math::Vec3;
use engine_core::player::UserCommand;
use engine_map::Map;
use engine_net::{Conditioned, Conditions, ServerMessage, Transport, loopback};
use engine_physics::CollisionWorld;
use engine_server::{Level, Seat, SeatInfo, Server};
use fixture_quake::{Palette, ROCKET_BLAST, SCRIPTS, import_map, mount_game, spawn_point};

/// A tick.
const TICK: f64 = 1.0 / 72.0;

/// The original's game files.
fn base() -> PathBuf {
    PathBuf::from(std::env::var_os("QUAKE_DIR").expect("QUAKE_DIR is set"))
}

/// Loads a map from `id1`.
fn load(name: &str) -> Arc<Map> {
    let mut filesystem = FileSystem::new();
    mount_game(&mut filesystem, &base().join("id1")).expect("mount");
    let palette = Palette::parse(&filesystem.read("gfx/palette.lmp").expect("palette"));
    let palette = palette.expect("it parses");
    let bsp = filesystem.read(&format!("maps/{name}.bsp")).expect("a map");
    Arc::new(import_map(&bsp, None, &palette).expect("it imports"))
}

/// A network with `ping` round trip, in milliseconds, of which `jitter` is
/// random and `loss` lost, each way.
fn network(ping: u64, jitter: u64, loss: f64) -> Conditions {
    Conditions {
        send_delay: Duration::from_millis(ping / 2),
        receive_delay: Duration::ZERO,
        jitter: Duration::from_millis(jitter),
        loss,
    }
}

/// A game on `name` with a host who stands still, and rules from `settings`.
fn game(
    map: &Arc<Map>,
    name: &str,
    settings: &[(&str, f64)],
) -> (Server, Client, Rc<CollisionWorld>) {
    let world = Rc::new(CollisionWorld::new(Arc::clone(map)));
    let (host_end, server_end) = loopback();
    let origin = spawn_point(map).origin;
    let server = Server::new(
        Box::new(server_end),
        Rc::clone(&world),
        map,
        origin,
        SCRIPTS,
        TICK,
        Level {
            name: name.to_owned(),
            settings: settings
                .iter()
                .map(|(name, value)| ((*name).to_owned(), *value))
                .collect(),
            host: SeatInfo::default(),
            ..Level::default()
        },
    )
    .expect("the scripts load");
    let host = Client::new(Box::new(host_end), Rc::clone(&world), origin);
    (server, host, world)
}

/// Joins a bot through `conditions`, as far as having a player.
fn add_bot(
    server: &mut Server,
    world: &Rc<CollisionWorld>,
    conditions: Conditions,
    skill: BotSkill,
    number: u32,
) -> Bot {
    let (bot_end, server_end) = loopback();
    let seed = u64::from(number) * 7919 + 1;
    let bot_end = Conditioned::new(Box::new(bot_end), conditions, seed);
    let server_end = Conditioned::new(Box::new(server_end), conditions, seed + 1);
    let info = SeatInfo {
        name: format!("bot{number}"),
        colors: u8::try_from(number % 14).expect("small") * 17,
    };
    server.add_seat(Seat::new(Box::new(server_end), info));
    let mut client = Client::new(Box::new(bot_end), Rc::clone(world), Vec3::ZERO);
    client.send_ready();
    Bot::new(client, skill, number + 1)
}

/// Runs `seconds` of game with everyone playing.
fn run(
    server: &mut Server,
    host: &mut Client,
    bots: &mut [Bot],
    world: &Rc<CollisionWorld>,
    seconds: f64,
) {
    #[expect(clippy::cast_possible_truncation, reason = "a tick fits f32")]
    let tick = TICK as f32;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a test's length"
    )]
    let ticks = (seconds * 72.0) as u32;
    for _ in 0..ticks {
        host.send_command(UserCommand::default(), tick);
        for bot in bots.iter_mut() {
            bot.tick(tick, world);
        }
        server.tick();
        host.receive();
        let _sounds = host.take_sounds();
        let _effects = host.take_effects();
        let _prints = host.take_prints();
        let _events = host.take_events();
        let _names = host.take_model_names();
    }
}

/// Lua that counts what each shot hits, in place of the shotgun's spread, so a
/// shot goes exactly where it's aimed, as the record's model assumes.
const COUNT_SHOTS: &str = r#"
    shots_fired = 0
    shots_hit = 0
    speed_total = 0
    function W_FireShotgun(self)
        self.currentammo = 25
        self.ammo_shells = 25
        if rewinding then
            engine.rewind(rawget(self, "_id"))
        end
        local forward = makevectors(self.v_angle)
        local start = self.origin + vector.create(0, 0, 16)
        traceline(start, start + forward * 2048, false, self)
        -- Only a target at full tilt counts: one that's just turned round is
        -- near where it was, and that would hit with or without rewind.
        local speed = vector.magnitude(find_all("netname", "bot2")[1].velocity)
        if speed >= 315 then
            shots_fired += 1
            speed_total += speed
            if trace_ent.classname == "player" then
                shots_hit += 1
            end
        end
        engine.end_rewind()
    end
    function T_Damage() end
"#;

/// Fires at a target bot running to and fro, and returns the share that hit,
/// for a shooter on `ping`.
fn share_of_shots_that_hit(
    map: &Arc<Map>,
    ping: u64,
    jitter: u64,
    loss: f64,
    rewinding: bool,
) -> (f64, f64, f64) {
    shots_in(map, ping, jitter, loss, rewinding, &[])
}

/// The same, with `rules` too.
fn shots_in(
    map: &Arc<Map>,
    ping: u64,
    jitter: u64,
    loss: f64,
    rewinding: bool,
    rules: &[(&str, f64)],
) -> (f64, f64, f64) {
    let mut settings = vec![("deathmatch", 1.0)];
    settings.extend_from_slice(rules);
    let (mut server, mut host, world) = game(map, "dm3", &settings);
    server.exec(COUNT_SHOTS).expect("the counter loads");
    server
        .exec(&format!("rewinding = {rewinding}"))
        .expect("it runs");
    let shooter = BotSkill {
        aim_error: 0.0,
        turn_speed: 100_000.0,
        moves: false,
        shoots: true,
        patrol_seconds: 0.0,
    };
    // Not a multiple of the shotgun's half second, or every shot would catch
    // the target at the same point of its run.
    let target = BotSkill {
        moves: true,
        shoots: false,
        patrol_seconds: 0.61,
        ..BotSkill::default()
    };
    let mut bots = vec![
        add_bot(&mut server, &world, network(ping, jitter, loss), shooter, 1),
        add_bot(&mut server, &world, network(30, 0, 0.0), target, 2),
    ];
    run(&mut server, &mut host, &mut bots, &world, 1.0);
    // Stand them where they can see each other, across the biggest open room.
    server
        .exec(
            r#"
            local host = find_all("netname", "player")[1]
            setorigin(host, vector.create(0, 0, 4000))
            host.frozen = true
            setorigin(find_all("netname", "bot1")[1], vector.create(-880, -232, -16))
            setorigin(find_all("netname", "bot2")[1], vector.create(-640, -232, -16))
            "#,
        )
        .expect("it runs");
    run(&mut server, &mut host, &mut bots, &world, 60.0);
    server
        .exec(
            r#"
            local speed = speed_total / math.max(shots_fired, 1)
            engine.print(shots_hit .. "/" .. shots_fired .. "/" .. speed)
            "#,
        )
        .expect("it runs");
    server.tick();
    host.receive();
    let prints = host.take_prints();
    let line = prints
        .iter()
        .find(|text| text.contains('/'))
        .expect("a count");
    let mut numbers = line.split('/').map(|part| part.parse().unwrap_or(0.0));
    (
        numbers.next().unwrap_or(0.0),
        numbers.next().unwrap_or(0.0),
        numbers.next().unwrap_or(0.0),
    )
}

#[test]
#[ignore = "needs game data"]
fn shots_land_where_they_were_aimed_at_every_ordinary_ping() {
    let map = load("dm3");
    println!("ping jitter loss: with rewind, without, at target speed");
    for (ping, jitter, loss) in [
        (0, 0, 0.0),
        (20, 0, 0.0),
        (40, 0, 0.0),
        (40, 5, 0.02),
        (60, 0, 0.0),
        (60, 10, 0.02),
        (60, 10, 0.05),
        (75, 5, 0.02),
    ] {
        let (hit, fired, speed) = share_of_shots_that_hit(&map, ping, jitter, loss, true);
        let (control, control_fired, _speed) =
            share_of_shots_that_hit(&map, ping, jitter, loss, false);
        let share = |hit: f64, fired: f64| hit / fired.max(1.0) * 100.0;
        println!(
            "{ping} {jitter} {loss}: {:.0}%, {:.0}%, {speed:.0} u/s",
            share(hit, fired),
            share(control, control_fired)
        );
        assert!(fired > 40.0, "they have to have fired");
        assert!(speed > 250.0, "the target has to be running");
        assert!(share(hit, fired) >= 95.0, "rewind lands what was aimed");
        // A hit on a running target has about 50 ms of give in it, and firing
        // on the click spends less of that, so 60 ms sits right on the edge.
        if ping >= 75 {
            assert!(
                share(control, control_fired) < 20.0,
                "and it's rewind doing it"
            );
        }
    }
}

#[test]
#[ignore = "needs game data"]
fn a_full_game_of_bots_runs_for_minutes_with_no_errors() {
    let map = load("dm3");
    let (mut server, mut host, world) = game(&map, "dm3", &[("deathmatch", 1.0)]);
    let pings = [15, 30, 45, 60, 80, 120, 200];
    let mut bots: Vec<Bot> = (0_u32..)
        .zip(pings)
        .map(|(number, ping)| {
            add_bot(
                &mut server,
                &world,
                network(ping, 8, 0.02),
                BotSkill::default(),
                number,
            )
        })
        .collect();
    run(&mut server, &mut host, &mut bots, &world, 180.0);
    let scores = host.scores();
    println!("{scores:#?}");
    assert_eq!(scores.len(), 8, "everyone's still here");
    let frags: i32 = scores.iter().map(|score| score.frags.abs()).sum();
    assert!(frags > 0, "somebody killed somebody");
}

#[test]
#[ignore = "needs game data"]
fn prediction_stays_close_to_the_server_at_every_ping() {
    let map = load("e1m1");
    for (ping, jitter, loss) in [
        (30, 5, 0.02),
        (60, 10, 0.02),
        (60, 10, 0.05),
        (100, 10, 0.02),
    ] {
        let (mut server, mut host, world) = game(&map, "e1m1", &[("coop", 1.0)]);
        let mut bots = vec![add_bot(
            &mut server,
            &world,
            network(ping, jitter, loss),
            BotSkill::default(),
            1,
        )];
        run(&mut server, &mut host, &mut bots, &world, 30.0);
        let snapshot_age = host.time() - bots[0].client().time();
        println!(
            "{ping} ms: the bot's snapshots are {:.0} ms old",
            snapshot_age * 1000.0
        );
        assert!(snapshot_age < 0.25);
    }
}

/// Messages that aren't snapshots mustn't stop a client that's lost a few.
#[test]
fn a_conditioned_link_keeps_reliable_messages_in_order() {
    let (one, two) = loopback();
    let conditions = network(100, 40, 0.5);
    let mut sender = Conditioned::new(Box::new(one), conditions, 5);
    let mut receiver = Conditioned::new(Box::new(two), Conditions::default(), 6);
    for number in 0..50_u8 {
        sender.send(ServerMessage::Music(number).encode());
    }
    let step = Duration::from_millis(14);
    let mut got = Vec::new();
    for _ in 0..50 {
        sender.update(step);
        sender.flush();
        receiver.update(step);
        got.extend(std::iter::from_fn(|| receiver.receive()));
    }
    let expected: Vec<Vec<u8>> = (0..50_u8)
        .map(|number| ServerMessage::Music(number).encode())
        .collect();
    assert_eq!(got, expected);
}

#[test]
fn a_conditioned_link_loses_about_what_it_says() {
    let (one, two) = loopback();
    let conditions = network(0, 0, 0.05);
    let mut sender = Conditioned::new(Box::new(one), conditions, 9);
    let mut receiver = Conditioned::new(Box::new(two), Conditions::default(), 10);
    for _ in 0..10_000 {
        sender.send_unreliable(vec![1]);
    }
    let step = Duration::from_millis(14);
    sender.update(step);
    sender.flush();
    receiver.update(step);
    let arrived = std::iter::from_fn(|| receiver.receive()).count();
    assert!((9300..9700).contains(&arrived), "{arrived}");
}

#[test]
fn a_conditioned_link_holds_messages_for_its_delay() {
    let (one, two) = loopback();
    let mut sender = Conditioned::new(Box::new(one), network(100, 0, 0.0), 1);
    let mut receiver = Conditioned::new(Box::new(two), Conditions::default(), 2);
    sender.send(vec![1]);
    let step = Duration::from_millis(10);
    let mut waited = 0;
    loop {
        sender.update(step);
        sender.flush();
        receiver.update(step);
        if receiver.receive().is_some() {
            break;
        }
        waited += 10;
        assert!(waited < 500);
    }
    assert!((40..=60).contains(&waited), "{waited}");
}

#[test]
#[ignore = "needs game data"]
fn the_server_measures_ping_and_the_zone_follows() {
    let map = load("dm3");
    let (mut server, mut host, world) = game(
        &map,
        "dm3",
        &[("deathmatch", 1.0), ("sv_fair_interval", 5.0)],
    );
    let pings = [10, 40, 60, 75, 130];
    let mut bots: Vec<Bot> = (0_u32..)
        .zip(pings)
        .map(|(number, ping)| {
            add_bot(
                &mut server,
                &world,
                network(ping, 4, 0.02),
                BotSkill::default(),
                number,
            )
        })
        .collect();
    run(&mut server, &mut host, &mut bots, &world, 60.0);
    let (zone, _rules) = server.fairness();
    println!("{zone:?}");
    for seat in server.report() {
        println!("{seat:?}");
    }
    let report = server.report();
    let seat = |index: usize| &report[index + 1];
    for (index, ping) in (0..).zip(pings) {
        #[expect(clippy::cast_precision_loss, reason = "a small ping")]
        let expected = ping as f64;
        let measured = seat(index).ping;
        assert!((measured - expected).abs() < 16.0, "{ping}: {measured}");
    }
    assert!(seat(4).over_ceiling, "130 ms gets no help");
    assert_eq!(seat(4).compensated, 0.0);
    assert!(!seat(2).over_ceiling);
    assert!(report[0].added > 5.0, "the host is brought up to the floor");
    assert!(seat(0).added < 15.0);
}

/// Fires at a target with a shooter on `ping` among a lobby around 40 ms.
fn hit_share_in_a_lobby(map: &Arc<Map>, ping: u64) -> f64 {
    let (hit, fired, _speed) = shots_in(map, ping, 0, 0.0, true, &[("sv_fair_interval", 1.0)]);
    hit / fired.max(1.0) * 100.0
}

#[test]
#[ignore = "needs game data"]
fn over_the_ceiling_means_leading_your_shots() {
    let map = load("dm3");
    let within = hit_share_in_a_lobby(&map, 60);
    let beyond = hit_share_in_a_lobby(&map, 130);
    println!("within {within:.0}%, beyond {beyond:.0}%");
    assert!(within > 95.0);
    assert!(beyond < 10.0);
}

#[test]
#[ignore = "needs game data"]
fn a_shot_past_a_door_is_judged_where_the_door_was() {
    let map = load("dm3");
    let (mut server, mut host, world) = game(&map, "dm3", &[("deathmatch", 1.0)]);
    let mut bots = vec![add_bot(
        &mut server,
        &world,
        network(60, 0, 0.0),
        BotSkill {
            moves: false,
            shoots: false,
            ..BotSkill::default()
        },
        1,
    )];
    run(&mut server, &mut host, &mut bots, &world, 1.0);
    // Any mover will do: note where it is, move it, and look back.
    server
        .exec(
            r#"
            mover = nil
            for _, entity in saved_entities() do
                if entity.movetype == MOVETYPE_PUSH and entity.solid == SOLID_BSP then
                    mover = entity
                end
            end
            assert(mover)
            was = mover.origin
            "#,
        )
        .expect("a mover");
    run(&mut server, &mut host, &mut bots, &world, 0.5);
    server
        .exec("mover.origin = mover.origin + vector.create(0, 0, 200) moved = mover.origin")
        .expect("it moves");
    run(&mut server, &mut host, &mut bots, &world, 0.05);
    server
        .exec(
            r#"
            local bot = find_all("netname", "bot1")[1]
            engine.rewind(rawget(bot, "_id"))
            local back = mover.origin
            engine.end_rewind()
            assert(back == was, "it's back where the shooter saw it")
            assert(mover.origin == moved, "and only for the shot")
            "#,
        )
        .expect("it holds");
}

/// Fires a projectile from where `from` stands along `direction`, and follows
/// the server's copy, returning the worst gap between it and the client's own
/// worked out path, in units, and how long it flew.
fn path_error(name: &str, weapon: &str, from: Vec3, yaw_pitch: (f32, f32)) -> (f32, f64) {
    let map = load(name);
    let (mut server, mut host, world) = game(&map, name, &[("deathmatch", 1.0)]);
    #[expect(clippy::cast_possible_truncation, reason = "a tick fits f32")]
    let tick = TICK as f32;
    for _ in 0..72 {
        host.send_command(UserCommand::default(), tick);
        server.tick();
        host.receive();
    }
    server
        .exec(&format!(
            r#"
            local player = find_all("classname", "player")[1]
            setorigin(player, vector.create({}, {}, {}))
            player.ammo_rockets = 50
            player.items = bit32.bor(player.items, IT_ROCKET_LAUNCHER, IT_GRENADE_LAUNCHER)
            player.weapon = {weapon}
            W_SetCurrentAmmo(player)
            "#,
            from.x, from.y, from.z
        ))
        .expect("it runs");
    let mut worst = 0.0_f32;
    let mut flown = 0.0;
    let mut path: Option<(u64, Path)> = None;
    for tick_number in 0..300 {
        let command = UserCommand {
            yaw: yaw_pitch.0,
            pitch: yaw_pitch.1,
            attack: tick_number == 5,
            ..UserCommand::default()
        };
        host.send_command(command, tick);
        server.tick();
        host.receive();
        if path.is_none()
            && let Some((id, launch)) = host.last_launch()
        {
            path = Some((id, Path::new(launch, &world, &[])));
        }
        let Some((id, path)) = &path else {
            continue;
        };
        let time = host.time();
        let drawn = host.draw_entities(1.0);
        // Once it's gone off, it's an explosion, not a projectile.
        let flying = path.launch().bounces && time < path.launch().time + 2.4
            || path.impact().is_none_or(|(when, _)| time < when);
        if let Some(entity) = drawn.iter().find(|entity| entity.state.id == *id)
            && flying
        {
            worst = worst.max((path.at(time) - entity.state.origin).length());
            flown = time - path.launch().time;
        }
    }
    (worst, flown)
}

#[test]
#[ignore = "needs game data"]
fn a_clients_path_matches_the_servers_projectile() {
    let from = Vec3::new(-880.0, -232.0, -16.0);
    let rocket = path_error("dm3", "IT_ROCKET_LAUNCHER", from, (0.0, 0.0));
    let grenade = path_error("dm3", "IT_GRENADE_LAUNCHER", from, (0.0, 0.0));
    println!("rocket {rocket:?}, grenade {grenade:?}");
    assert!(rocket.1 > 0.05, "it flew");
    assert!(grenade.1 > 0.5, "it flew");
    assert!(rocket.0 < 2.0, "a rocket stays on its path");
    assert!(grenade.0 < 16.0, "a grenade stays close");
}

/// A rocket jump at `ping`: fires at the floor, and returns the worst gap
/// between where the client thinks it is and where the server says, over the
/// jump, with and without predicting the knock.
fn rocket_jump_error(ping: u64, predicting: bool) -> f32 {
    let map = load("dm3");
    let (mut server, mut host, world) = game(&map, "dm3", &[("deathmatch", 1.0)]);
    let mut bots = vec![add_bot(
        &mut server,
        &world,
        network(ping, 0, 0.0),
        BotSkill {
            moves: false,
            shoots: false,
            ..BotSkill::default()
        },
        1,
    )];
    bots[0]
        .client_mut()
        .set_blast(predicting.then_some(ROCKET_BLAST));
    run(&mut server, &mut host, &mut bots, &world, 1.0);
    server
        .exec(
            r#"
            local host = find_all("netname", "player")[1]
            setorigin(host, vector.create(0, 0, 4000))
            host.frozen = true
            local bot = find_all("netname", "bot1")[1]
            setorigin(bot, vector.create(-880, -232, -16))
            bot.ammo_rockets = 50
            bot.health = 500
            bot.items = bit32.bor(bot.items, IT_ROCKET_LAUNCHER)
            bot.weapon = IT_ROCKET_LAUNCHER
            W_SetCurrentAmmo(bot)
            "#,
        )
        .expect("it runs");
    run(&mut server, &mut host, &mut bots, &world, 1.0);
    #[expect(clippy::cast_possible_truncation, reason = "a tick fits f32")]
    let tick = TICK as f32;
    // Straight down, then compare what was predicted for each command with
    // what the server says that command really did.
    let mut worst = 0.0_f32;
    let mut predictions = std::collections::HashMap::new();
    for tick_number in 0..144 {
        let command = UserCommand {
            pitch: 89.0,
            attack: tick_number < 2,
            ..UserCommand::default()
        };
        let client = bots[0].client_mut();
        client.update_network(Duration::from_secs_f32(tick));
        client.receive();
        if tick_number == 0 {
            // As a click does, before the server has heard of it.
            let start = client.predicted().origin + Vec3::new(0.14, 0.0, 8.0);
            let down = Vec3::new(0.017, 0.0, -1.0);
            client.guess_launch("progs/missile.mdl", start, down * 1000.0, false);
        }
        let sequence = client.next_sequence();
        client.send_command(command, tick);
        client.flush_network();
        predictions.insert(sequence, client.predicted().origin);
        if let Some((answered, truth)) = client.last_answer()
            && let Some(predicted) = predictions.get(&answered)
        {
            worst = worst.max((*predicted - truth.origin).length());
        }
        host.send_command(UserCommand::default(), tick);
        server.tick();
        host.receive();
    }
    worst
}

#[test]
#[ignore = "needs game data"]
fn predicting_a_rocket_jump_keeps_the_client_with_the_server() {
    for ping in [40, 80] {
        let with = rocket_jump_error(ping, true);
        let without = rocket_jump_error(ping, false);
        println!("{ping} ms: {with:.1} units with prediction, {without:.1} without");
        assert!(with * 2.0 < without, "it has to make a real difference");
    }
}
