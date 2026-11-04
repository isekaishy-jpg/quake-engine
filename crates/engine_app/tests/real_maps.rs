//! Collision and movement on real maps. Run with `QUAKE_DIR` set and
//! `--ignored`, because they need game data.

#![expect(clippy::print_stdout, reason = "the measurements are the point")]

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use engine_assets::FileSystem;
use engine_audio::Audio;
use engine_core::math::Vec3;
use engine_core::player::{PlayerState, UserCommand};
use engine_map::{ClipChild, Map, Tree};
use engine_movement::{MoveVars, player_move};
use engine_net::{ClientMessage, ServerMessage, Transport, loopback};
use engine_physics::CollisionWorld;
use engine_server::Server;
use fixture_quake::{
    Palette, SCRIPTS, import_map, mount_game, mount_music, parse_wav, spawn_point,
};

/// A small random number generator, so a failure repeats.
struct Random(u64);

impl Random {
    /// The next number from 0 up to 1.
    fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let bits = u32::try_from(self.0 >> 40).expect("24 bits");
        f32::from(u16::try_from(bits >> 8).expect("16 bits")) / 65536.0
    }

    /// A number between `low` and `high`.
    fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.next()
    }
}

/// A search path over a game directory.
fn game(base: &Path, name: &str) -> FileSystem {
    let mut filesystem = FileSystem::new();
    mount_game(&mut filesystem, &base.join(name)).expect("mount");
    filesystem
}

/// The palette from the original game.
fn palette(base: &Path) -> Palette {
    let bytes = game(base, "id1").read("gfx/palette.lmp").expect("palette");
    Palette::parse(&bytes).expect("palette")
}

/// A map from a search path.
fn load(filesystem: &FileSystem, name: &str, palette: &Palette) -> Arc<Map> {
    let bsp = filesystem.read(&format!("maps/{name}.bsp")).expect("read");
    let lit = filesystem.read(&format!("maps/{name}.lit")).ok();
    Arc::new(import_map(&bsp, lit.as_deref(), palette).expect("import"))
}

/// The base directory.
fn base() -> PathBuf {
    PathBuf::from(std::env::var("QUAKE_DIR").expect("QUAKE_DIR"))
}

/// A port of `SV_RecursiveHullCheck` from `world.c`, without its back-off,
/// giving the fraction where a point first enters solid.
fn reference_trace(
    tree: &Tree,
    child: ClipChild,
    from: (f32, Vec3),
    to: (f32, Vec3),
) -> Option<f32> {
    match child {
        ClipChild::Empty => None,
        ClipChild::Solid => Some(from.0),
        ClipChild::Node(index) => {
            let node = tree.clip_nodes[index as usize];
            let plane = tree.planes[node.plane as usize];
            let start = plane.normal.dot(from.1) - plane.distance;
            let end = plane.normal.dot(to.1) - plane.distance;
            if start >= 0.0 && end >= 0.0 {
                return reference_trace(tree, node.children[0], from, to);
            }
            if start < 0.0 && end < 0.0 {
                return reference_trace(tree, node.children[1], from, to);
            }
            let fraction = start / (start - end);
            let middle = (
                from.0 + (to.0 - from.0) * fraction,
                from.1 + (to.1 - from.1) * fraction,
            );
            let near = usize::from(start < 0.0);
            reference_trace(tree, node.children[near], from, middle)
                .or_else(|| reference_trace(tree, node.children[1 - near], middle, to))
        }
    }
}

/// Compares `count` random traces with the reference, returning how many
/// disagreed on hitting and how many ended over 0.1 units from it.
fn compare_traces(map: &Map, world: &CollisionWorld, count: u32) -> (u32, u32) {
    let model = &map.models[0];
    let root = model.hulls[0];
    let mut random = Random(1);
    let (mut disagree, mut far, mut tested) = (0, 0, 0);
    while tested < count {
        let point = |random: &mut Random| {
            Vec3::new(
                random.range(model.mins.x, model.maxs.x),
                random.range(model.mins.y, model.maxs.y),
                random.range(model.mins.z, model.maxs.z),
            )
        };
        let start = point(&mut random);
        let end = start + (point(&mut random) - start).normalized() * random.range(16.0, 512.0);
        if world.in_solid(start) {
            continue;
        }
        tested += 1;
        let expected = reference_trace(&map.tree, root, (0.0, start), (1.0, end));
        let actual = world.trace(start, end);
        match expected {
            None if actual.fraction < 1.0 => disagree += 1,
            Some(_) if actual.fraction >= 1.0 && actual.normal == Vec3::ZERO => disagree += 1,
            Some(fraction) => {
                // Measured from the surface, because the margin is, and a ray
                // at a shallow angle covers a long way in that margin.
                let along = (end - start).dot(actual.normal).abs();
                if (fraction - actual.fraction).abs() * along > 0.1 {
                    far += 1;
                }
            }
            None => {}
        }
    }
    (disagree, far)
}

/// Runs random input, and returns how many ticks ended in solid, and how far
/// the player travelled.
fn wander(map: &Map, world: &CollisionWorld, ticks: u32) -> (u32, f32) {
    let vars = MoveVars::default();
    let mut random = Random(7);
    let mut state = PlayerState {
        origin: spawn_point(map).origin,
        ..PlayerState::default()
    };
    let mut command = UserCommand {
        seconds: 1.0 / 72.0,
        ..UserCommand::default()
    };
    let mut in_solid = 0;
    let mut travelled = 0.0;
    for tick in 0..ticks {
        let before = state.origin;
        if tick % 36 == 0 {
            command.yaw = random.range(0.0, 360.0);
            command.pitch = random.range(-70.0, 80.0);
            command.forward = random.range(-400.0, 400.0);
            command.side = random.range(-400.0, 400.0);
            command.jump = random.next() < 0.3;
        }
        player_move(
            &mut state,
            &command,
            &vars,
            world.with(&[]),
            &mut Vec::new(),
        );
        travelled += (state.origin - before).length();
        if world.in_solid(state.origin) {
            in_solid += 1;
        }
    }
    (in_solid, travelled)
}

#[test]
#[ignore = "needs game data"]
fn traces_match_quakes_own() {
    let base = base();
    let palette = palette(&base);
    let filesystem = game(&base, "id1");
    for name in ["start", "e1m1", "e1m7", "e4m3", "dm3"] {
        let map = load(&filesystem, name, &palette);
        let world = CollisionWorld::new(Arc::clone(&map));
        let (solids, refused) = world.solid_counts();
        let (disagree, far) = compare_traces(&map, &world, 20_000);
        println!("{name}: {solids} solids, {refused} refused, {disagree} disagree, {far} far");
        assert_eq!(refused, 0, "{name}");
        assert!(disagree < 20, "{name}: {disagree}");
        assert!(far < 40, "{name}: {far}");
    }
}

#[test]
#[ignore = "needs game data"]
fn a_wandering_player_never_ends_up_in_solid() {
    let base = base();
    let palette = palette(&base);
    let filesystem = game(&base, "id1");
    for name in ["start", "e1m1", "e1m3", "e2m1", "e4m3", "dm3", "dm6"] {
        let map = load(&filesystem, name, &palette);
        let world = CollisionWorld::new(Arc::clone(&map));
        let (in_solid, travelled) = wander(&map, &world, 72 * 300);
        println!("{name}: {in_solid} ticks in solid, travelled {travelled:.0}");
        assert_eq!(in_solid, 0, "{name}");
        assert!(travelled > 5000.0, "{name} barely moved: {travelled}");
    }
}

#[test]
#[ignore = "needs game data"]
fn bsp2_maps_build_solids_and_can_be_walked() {
    let base = base().join("rerelease");
    let palette = palette(&base);
    let filesystem = game(&base, "mg1");
    for name in ["hub", "mge1m1"] {
        if !filesystem.exists(&format!("maps/{name}.bsp")) {
            continue;
        }
        let map = load(&filesystem, name, &palette);
        let world = CollisionWorld::new(Arc::clone(&map));
        let (solids, refused) = world.solid_counts();
        let (in_solid, travelled) = wander(&map, &world, 72 * 60);
        println!("{name}: {solids} solids, {refused} refused, {in_solid} in solid, {travelled:.0}");
        assert!(solids > 0, "{name}");
        assert_eq!(in_solid, 0, "{name}");
    }
}

/// Runs a map's scripts for `seconds`, and returns any script errors, and how
/// many brush models moved.
fn run_scripts(map: &Arc<Map>, seconds: u32) -> (Vec<String>, usize) {
    let world = Rc::new(CollisionWorld::new(Arc::clone(map)));
    let (mut client, server_end) = loopback();
    let origin = spawn_point(map).origin;
    let mut server = Server::new(
        Box::new(server_end),
        world,
        map,
        origin,
        SCRIPTS,
        1.0 / 72.0,
    )
    .expect("the scripts load");
    let mut errors = Vec::new();
    let mut first = None;
    let mut moved = std::collections::BTreeSet::new();
    for _ in 0..seconds * 72 {
        server.tick();
        while let Some(bytes) = client.receive() {
            match ServerMessage::decode(&bytes).expect("valid") {
                ServerMessage::Print(text) if text.contains("script error") => errors.push(text),
                ServerMessage::Snapshot(snapshot) => {
                    let start = first.get_or_insert_with(|| snapshot.instances.clone());
                    for (from, to) in start.iter().zip(&snapshot.instances) {
                        if from.origin != to.origin {
                            moved.insert(to.model);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    (errors, moved.len())
}

#[test]
#[ignore = "needs game data"]
fn every_maps_scripts_run_without_errors() {
    let base = base();
    let palette = palette(&base);
    let mut failures = 0;
    let mut maps = 0;
    for (root, game_name) in [
        (base.clone(), "id1"),
        (base.clone(), "hipnotic"),
        (base.clone(), "rogue"),
        (base.join("rerelease"), "mg1"),
    ] {
        let filesystem = game(&root, game_name);
        for path in filesystem.list("maps/") {
            let Some(name) = path
                .strip_prefix("maps/")
                .and_then(|p| p.strip_suffix(".bsp"))
            else {
                continue;
            };
            let map = load(&filesystem, name, &palette);
            let (errors, moved) = run_scripts(&map, 3);
            maps += 1;
            if !errors.is_empty() {
                failures += 1;
                println!(
                    "{game_name}/{name}: {} errors, first {}",
                    errors.len(),
                    errors[0]
                );
            } else if moved > 0 {
                println!("{game_name}/{name}: fine, {moved} models moved");
            }
        }
    }
    println!("{maps} maps, {failures} with errors");
    assert_eq!(failures, 0);
}

/// The models of teleports aimed at an `info_null`, which removes itself.
/// `teleport_touch` stops the game on these in the original too, so maps can't
/// rely on reaching them.
fn dead_teleports(map: &Map) -> Vec<String> {
    let nulls: Vec<&str> = map
        .entities
        .iter()
        .filter(|entity| entity.get("classname") == Some("info_null"))
        .filter_map(|entity| entity.get("targetname"))
        .collect();
    map.entities
        .iter()
        .filter(|entity| entity.get("classname") == Some("trigger_teleport"))
        .filter(|entity| entity.get("target").is_some_and(|t| nulls.contains(&t)))
        .filter_map(|entity| entity.get("model").map(str::to_owned))
        .collect()
}

/// Runs every touch and target in `map`, and returns the script errors.
fn fire_everything(map: &Arc<Map>, world: &Rc<CollisionWorld>) -> Vec<String> {
    fire_everything_listening(map, world).0
}

/// Runs every touch and target in `map`, and returns the script errors and
/// the names of the sounds asked for.
fn fire_everything_listening(
    map: &Arc<Map>,
    world: &Rc<CollisionWorld>,
) -> (Vec<String>, Vec<String>) {
    let (mut client, server_end) = loopback();
    let origin = spawn_point(map).origin;
    let mut server = Server::new(
        Box::new(server_end),
        Rc::clone(world),
        map,
        origin,
        SCRIPTS,
        1.0 / 72.0,
    )
    .expect("the scripts load");
    let mut errors = Vec::new();
    let mut heard = Vec::new();
    let mut exec_errors = Vec::new();
    let mut run = |server: &mut Server, client: &mut dyn Transport, ticks: u32| {
        for _ in 0..ticks {
            server.tick();
            let _level = server.take_next_level();
            while let Some(bytes) = client.receive() {
                match ServerMessage::decode(&bytes) {
                    Some(ServerMessage::Print(text)) if text.contains("script error") => {
                        errors.push(text);
                    }
                    Some(ServerMessage::Sound(sound)) => heard.push(sound.name),
                    Some(ServerMessage::StaticSound(sound)) => heard.push(sound.name),
                    _ => {}
                }
            }
        }
    };
    run(&mut server, &mut client, 20);
    let unreachable = dead_teleports(map);
    for (number, model) in map.models.iter().enumerate().skip(1) {
        if unreachable.contains(&format!("*{number}")) {
            continue;
        }
        let middle = (model.mins + model.maxs) * 0.5;
        let text = format!("setpos {} {} {}", middle.x, middle.y, middle.z);
        client.send(ClientMessage::Console(text).encode());
        run(&mut server, &mut client, 3);
    }
    let mut names: Vec<&str> = map
        .entities
        .iter()
        .filter_map(|entity| entity.get("targetname"))
        .collect();
    names.sort_unstable();
    names.dedup();
    for name in names {
        let source = format!(
            r#"
            activator = find_all("classname", "player")[1]
            for _, entity in find_all("targetname", "{name}") do
                if entity.use and not is_removed(entity) then
                    entity.use(entity, activator)
                end
            end
            "#
        );
        if let Err(error) = server.exec(&source) {
            exec_errors.push(format!("{name}: {error}"));
        }
        run(&mut server, &mut client, 3);
    }
    // Long enough for delays, trains and doors to play out.
    run(&mut server, &mut client, 72 * 5);
    errors.append(&mut exec_errors);
    (errors, heard)
}

#[test]
#[ignore = "needs game data"]
fn every_touch_and_target_runs_without_errors() {
    let base = base();
    let palette = palette(&base);
    let mut failures = 0;
    let mut maps = 0;
    for (root, game_name) in [
        (base.clone(), "id1"),
        (base.clone(), "hipnotic"),
        (base.clone(), "rogue"),
        (base.join("rerelease"), "mg1"),
    ] {
        let filesystem = game(&root, game_name);
        for path in filesystem.list("maps/") {
            let Some(name) = path
                .strip_prefix("maps/")
                .and_then(|p| p.strip_suffix(".bsp"))
            else {
                continue;
            };
            let map = load(&filesystem, name, &palette);
            let world = Rc::new(CollisionWorld::new(Arc::clone(&map)));
            let errors = fire_everything(&map, &world);
            maps += 1;
            if !errors.is_empty() {
                failures += 1;
                let first = errors[0].lines().next().unwrap_or("");
                println!("{game_name}/{name}: {} errors, first {first}", errors.len());
            }
        }
    }
    println!("{maps} maps, {failures} with errors");
    assert_eq!(failures, 0);
}

#[test]
#[ignore = "needs game data"]
fn every_sound_in_the_game_parses() {
    let base = base();
    let filesystem = game(&base, "id1");
    let (mut count, mut loops) = (0, 0);
    for path in filesystem.list("sound/") {
        if !path.ends_with(".wav") {
            continue;
        }
        let bytes = filesystem.read(&path).expect("it reads");
        let clip = parse_wav(&bytes).unwrap_or_else(|| panic!("{path} parses"));
        assert!(!clip.samples.is_empty(), "{path} has samples");
        count += 1;
        loops += usize::from(clip.loop_start.is_some());
    }
    println!("{count} sounds, {loops} loops");
    assert_eq!((count, loops), (225, 23));
}

#[test]
#[ignore = "needs game data"]
fn every_sound_scripts_ask_for_exists() {
    let base = base();
    let palette = palette(&base);
    let filesystem = game(&base, "id1");
    let mut missing = std::collections::BTreeSet::new();
    let mut heard = std::collections::BTreeSet::new();
    for path in filesystem.list("maps/") {
        let Some(name) = path
            .strip_prefix("maps/")
            .and_then(|p| p.strip_suffix(".bsp"))
        else {
            continue;
        };
        let map = load(&filesystem, name, &palette);
        let world = Rc::new(CollisionWorld::new(Arc::clone(&map)));
        let (_errors, names) = fire_everything_listening(&map, &world);
        for sound in names {
            if !filesystem.exists(&format!("sound/{sound}")) {
                missing.insert(sound.clone());
            }
            heard.insert(sound);
        }
    }
    println!("{} different sounds, missing {missing:?}", heard.len());
    assert!(heard.len() > 20);
    assert!(missing.is_empty());
}

#[test]
#[ignore = "needs game data"]
fn every_music_track_decodes() {
    let base = base();
    let mut filesystem = FileSystem::new();
    mount_music(&mut filesystem, &base, "id1");
    let mut audio = Audio::new();
    assert!(audio.has_device(), "this needs a sound device");
    let tracks = filesystem.list("music/");
    println!("{} tracks", tracks.len());
    assert_eq!(tracks.len(), 10);
    for path in tracks {
        let bytes = filesystem.read(&path).expect("it reads");
        audio
            .play_music(bytes)
            .unwrap_or_else(|error| panic!("{path}: {error}"));
    }
    audio.stop_music();
}
