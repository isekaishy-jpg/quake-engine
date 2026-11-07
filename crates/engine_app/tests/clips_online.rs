//! A clip seen through a bad network (proposal 0143). A client is told which clip is playing and
//! when it began, and works the rest out, so what matters is how soon it hears.

#![expect(clippy::print_stdout, reason = "the measurements are the point")]

mod common;

use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use common::map;
use engine_client::Client;
use engine_core::entity::EntityState;
use engine_core::math::Vec3;
use engine_core::player::UserCommand;
use engine_map::{Clip, Map, MovingMesh, Skeleton};
use engine_net::{Conditioned, Conditions, loopback};
use engine_physics::CollisionWorld;
use engine_server::{Level, Seat, SeatInfo, Server};
use fixture_quake::SCRIPTS;

/// A tick.
const TICK: f64 = 1.0 / 72.0;

/// What the human asked networks to be tested at: 2% lost, each way, with a round trip of 100 ms.
fn bad_network() -> Conditions {
    Conditions {
        send_delay: Duration::from_millis(50),
        receive_delay: Duration::ZERO,
        jitter: Duration::from_millis(20),
        loss: 0.02,
        burst: 0.3,
    }
}

/// A map with something to play clips on.
fn map_with_clips() -> Arc<Map> {
    let mut built = Map::clone(&map(&[], &[]));
    let clip = |name: &str| Clip {
        name: name.to_owned(),
        length: 1.0,
        channels: Vec::new(),
    };
    built.moving_meshes.push(MovingMesh {
        name: "arm".to_owned(),
        skeleton: Skeleton {
            clips: vec![clip("one"), clip("two")],
            ..Skeleton::default()
        },
        ..MovingMesh::default()
    });
    Arc::new(built)
}

/// A server with a figure on it, and its host.
fn game(built: &Arc<Map>) -> (Server, Client, Rc<CollisionWorld>) {
    let world = Rc::new(CollisionWorld::new(Arc::clone(built)));
    let (host_end, server_end) = loopback();
    let server = Server::new(
        Box::new(server_end),
        Rc::clone(&world),
        Vec3::ZERO,
        SCRIPTS,
        TICK,
        Level::default(),
    )
    .expect("the scripts load");
    server
        .exec(
            r#"
            figure = spawn()
            engine.precache_model("mesh:arm")
            engine.set_model(rawget(figure, "_id"), "mesh:arm")
            "#,
        )
        .expect("it runs");
    let host = Client::new(Box::new(host_end), Rc::clone(&world), Vec3::ZERO);
    (server, host, world)
}

/// Plays clip `name` on the figure.
fn play(server: &Server, name: &str) {
    let line = format!(r#"engine.set_clip(rawget(figure, "_id"), "{name}")"#);
    server.exec(&line).expect("it runs");
}

/// Someone joining over a bad network, with `seed` choosing what's lost.
fn join(server: &mut Server, world: &Rc<CollisionWorld>, seed: u64) -> Client {
    let (their_end, server_end) = loopback();
    let their_end = Conditioned::new(Box::new(their_end), bad_network(), seed);
    let server_end = Conditioned::new(Box::new(server_end), bad_network(), seed + 1);
    let info = SeatInfo {
        name: "far away".to_owned(),
        colors: 0,
    };
    server.add_seat(Seat::new(Box::new(server_end), info));
    let mut client = Client::new(Box::new(their_end), Rc::clone(world), Vec3::ZERO);
    client.send_ready();
    client
}

/// One tick for everyone.
fn tick(server: &mut Server, clients: &mut [&mut Client]) {
    #[expect(clippy::cast_possible_truncation, reason = "a tick fits f32")]
    let seconds = TICK as f32;
    for client in clients.iter_mut() {
        client.update_network(Duration::from_secs_f64(TICK));
        client.receive();
        client.send_command(UserCommand::default(), seconds);
        client.flush_network();
        let _sounds = client.take_sounds();
        let _static_sounds = client.take_static_sounds();
        let _effects = client.take_effects();
        let _prints = client.take_prints();
        let _events = client.take_events();
        let _names = client.take_model_names();
        let _music = client.take_music();
    }
    server.tick();
}

/// What `client` knows of the figure, if it's heard of it.
fn figure_on(client: &Client) -> Option<EntityState> {
    let seen = client.draw_entities(1.0);
    let found = seen.iter().find(|entity| entity.state.clip != 0)?;
    Some(found.state)
}

/// How long each change of clip took to reach someone over a bad network, in seconds, sorted.
fn time_to_hear(seed: u64) -> Vec<f64> {
    let built = map_with_clips();
    let (mut server, mut host, world) = game(&built);
    let mut far = join(&mut server, &world, seed);
    for _ in 0..144 {
        tick(&mut server, &mut [&mut host, &mut far]);
    }
    let mut waits = Vec::new();
    for change in 0..200_u16 {
        let (name, number) = if change % 2 == 0 {
            ("one", 1)
        } else {
            ("two", 2)
        };
        play(&server, name);
        let mut heard = None;
        for ticks in 0..72_u32 {
            tick(&mut server, &mut [&mut host, &mut far]);
            let state = figure_on(&far).filter(|state| state.clip == number);
            if heard.is_none() && state.is_some() {
                heard = Some(f64::from(ticks + 1) * TICK);
            }
        }
        waits.push(heard.expect("it heard within the second"));
    }
    waits.sort_by(f64::total_cmp);
    waits
}

/// A clip gets through a bad network, and soon. What's lost is sent again with the next change, as
/// a snapshot goes against what was last acknowledged.
#[test]
fn a_change_of_clip_gets_through_two_percent_loss() {
    let mut waits = Vec::new();
    for seed in [1, 40, 900] {
        waits.extend(time_to_hear(seed));
    }
    waits.sort_by(f64::total_cmp);
    let at = |share: f64| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "a place in a short list"
        )]
        let place = ((waits.len() - 1) as f64 * share) as usize;
        waits[place]
    };
    println!(
        "{} changes: middle {:.3} s, 99% {:.3} s, worst {:.3} s",
        waits.len(),
        at(0.5),
        at(0.99),
        at(1.0)
    );
    assert!(
        at(0.5) < 0.15,
        "most arrive in about the delay: {}",
        at(0.5)
    );
    assert!(at(1.0) < 0.5, "and none is lost for long: {}", at(1.0));
}

/// Both ends agree when a clip began, so that it shows at the same place in it on each.
#[test]
fn a_clip_begins_at_the_same_moment_everywhere() {
    let built = map_with_clips();
    let (mut server, mut host, world) = game(&built);
    let mut far = join(&mut server, &world, 7);
    for _ in 0..144 {
        tick(&mut server, &mut [&mut host, &mut far]);
    }
    play(&server, "two");
    for _ in 0..72 {
        tick(&mut server, &mut [&mut host, &mut far]);
    }
    let here = figure_on(&host).expect("the host sees it");
    let there = figure_on(&far).expect("so does the far client");
    assert_eq!(here.clip, 2);
    assert_eq!(there.clip, here.clip);
    assert_eq!(there.clip_start.to_bits(), here.clip_start.to_bits());
}

/// Someone who joins part-way through a clip is told what's playing and when it began, so they
/// pick it up where it's got to and not from the start.
#[test]
fn joining_part_way_through_a_clip_finds_it_where_it_has_got_to() {
    let built = map_with_clips();
    let (mut server, mut host, world) = game(&built);
    for _ in 0..144 {
        tick(&mut server, &mut [&mut host]);
    }
    play(&server, "two");
    for _ in 0..216 {
        tick(&mut server, &mut [&mut host]);
    }
    let started = figure_on(&host).expect("the host sees it").clip_start;
    let mut far = join(&mut server, &world, 11);
    for _ in 0..144 {
        tick(&mut server, &mut [&mut host, &mut far]);
    }
    let there = figure_on(&far).expect("it hears of the figure");
    assert_eq!(there.clip, 2);
    assert_eq!(there.clip_start.to_bits(), started.to_bits());
    #[expect(clippy::cast_possible_truncation, reason = "a client's time")]
    let into = far.time() as f32 - there.clip_start;
    assert!(into > 4.0, "it's seconds in: {into}");
}
