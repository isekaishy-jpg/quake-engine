//! Collision and movement on real maps. Run with `QUAKE_DIR` set and
//! `--ignored`, because they need game data.

#![expect(clippy::print_stdout, reason = "the measurements are the point")]

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use engine_assets::FileSystem;
use engine_audio::Audio;
use engine_client::Client;
use engine_core::math::Vec3;
use engine_core::player::{PlayerState, UserCommand};
use engine_map::{AliasCorner, AliasModel, ClipChild, Contents, Map, Tree};
use engine_movement::{MoveVars, player_move};
use engine_net::{ClientMessage, Loopback, ServerMessage, Transport, loopback};
use engine_physics::CollisionWorld;
use engine_server::{Level, Server};
use fixture_quake::{
    Palette, SCRIPTS, Wad, import_map, load_picture, mount_game, mount_music, parse_item_bsp,
    parse_mdl, parse_spr, parse_wav, spawn_point,
};

/// A level on normal skill, as the console's default sets.
fn normal_level() -> Level {
    Level {
        settings: vec![("skill".to_owned(), 1.0), ("registered".to_owned(), 1.0)],
        ..Level::default()
    }
}

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
        origin,
        SCRIPTS,
        1.0 / 72.0,
        normal_level(),
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

/// The name of every level, leaving out the item models among them.
fn map_names(filesystem: &FileSystem) -> Vec<String> {
    filesystem
        .list("maps/")
        .iter()
        .filter_map(|path| path.strip_prefix("maps/")?.strip_suffix(".bsp"))
        .filter(|name| !name.starts_with("b_"))
        .map(str::to_owned)
        .collect()
}

/// A server on `name`, with its client's end.
fn start(base: &Path, name: &str) -> (Server, Loopback, Arc<Map>) {
    let filesystem = game(base, "id1");
    let map = load(&filesystem, name, &palette(base));
    let world = Rc::new(CollisionWorld::new(Arc::clone(&map)));
    let (client, server_end) = loopback();
    let origin = spawn_point(&map).origin;
    let server = Server::new(
        Box::new(server_end),
        world,
        origin,
        SCRIPTS,
        1.0 / 72.0,
        Level {
            name: name.to_owned(),
            ..normal_level()
        },
    )
    .expect("the scripts load");
    (server, client, map)
}

/// A place under water on `map`, with a player's feet, middle and eyes all in it.
fn under_water(map: &Map) -> Vec3 {
    let wet = |point: Vec3| {
        map.tree
            .leaf(map.tree.leaf_at(point))
            .is_some_and(|leaf| leaf.contents == Contents::Water)
    };
    for face in &map.faces {
        let texture = usize::try_from(face.texture).ok();
        let is_water = texture
            .and_then(|texture| map.textures.get(texture))
            .is_some_and(|texture| texture.name.starts_with("*water"));
        if !is_water {
            continue;
        }
        let first = usize::try_from(face.first_point).expect("small");
        let count = usize::try_from(face.point_count).expect("small");
        let points = &map.points[first..first + count];
        #[expect(clippy::cast_precision_loss, reason = "a few points")]
        let share = 1.0 / points.len() as f32;
        let middle = points.iter().fold(Vec3::ZERO, |sum, point| sum + *point) * share;
        let place = middle - Vec3::new(0.0, 0.0, 40.0);
        let reach = [-23.0, 0.0, 22.0].map(|height| place + Vec3::new(0.0, 0.0, height));
        if reach.iter().all(|point| wet(*point)) {
            return place;
        }
    }
    panic!("no water deep enough");
}

/// How many bubbles are about after a player at `place` is hurt, then killed.
fn bubbles_at(server: &mut Server, client: &mut Loopback, place: Vec3) -> (u32, u32) {
    let move_on = |server: &mut Server, client: &mut Loopback, ticks: u32, from: u32| {
        for tick in 0..ticks {
            let command = UserCommand {
                sequence: from + tick,
                seconds: 1.0 / 72.0,
                ..UserCommand::default()
            };
            client.send(ClientMessage::Move(vec![command], 0).encode());
            server.tick();
        }
    };
    let count = |server: &mut Server, client: &mut Loopback| {
        while client.receive().is_some() {}
        server
            .exec(r#"engine.print("bubbles " .. #find_all("classname", "bubble"))"#)
            .expect("it runs");
        server.tick();
        let mut bubbles = 0;
        while let Some(bytes) = client.receive() {
            if let Some(ServerMessage::Print(text)) = ServerMessage::decode(&bytes)
                && let Some(number) = text.strip_prefix("bubbles ")
            {
                bubbles = number.trim().parse().expect("a number");
            }
        }
        bubbles
    };
    move_on(server, client, 5, 1);
    server
        .exec(&format!(
            r#"
            local player = find_all("classname", "player")[1]
            setorigin(player, vector.create({}, {}, {}))
            "#,
            place.x, place.y, place.z
        ))
        .expect("it runs");
    move_on(server, client, 5, 10);
    server
        .exec(
            r#"
            local player = find_all("classname", "player")[1]
            T_Damage(player, world, world, 20)
            "#,
        )
        .expect("it runs");
    move_on(server, client, 20, 20);
    let hurt = count(server, client);
    server
        .exec(
            r#"
            local player = find_all("classname", "player")[1]
            T_Damage(player, world, world, 90)
            "#,
        )
        .expect("it runs");
    move_on(server, client, 72, 50);
    (hurt, count(server, client))
}

#[test]
#[ignore = "needs game data"]
fn bubbles_rise_from_someone_hurt_under_water_and_nowhere_else() {
    let base = base();
    let (mut server, mut client, map) = start(&base, "e1m1");
    let place = under_water(&map);
    let (hurt, dead) = bubbles_at(&mut server, &mut client, place);
    println!("under water: {hurt} when hurt, {dead} after dying");
    assert!(hurt >= 1, "drowning pain lets one go");
    assert!(
        dead > hurt,
        "and dying lets more go, until the body leaves the water"
    );
    let (mut server, mut client, map) = start(&base, "e1m1");
    let dry = spawn_point(&map).origin;
    assert_eq!(bubbles_at(&mut server, &mut client, dry), (0, 0), "on land");
}

/// How high a jump goes on `name`, and the gravity the client was told of.
fn jump_on(base: &Path, name: &str) -> (f32, Option<f32>) {
    let (mut server, mut client, _map) = start(base, name);
    let (mut lowest, mut highest, mut told) = (f32::INFINITY, f32::NEG_INFINITY, None);
    for tick in 0..720_u32 {
        let command = UserCommand {
            sequence: tick + 1,
            seconds: 1.0 / 72.0,
            jump: tick == 100,
            ..UserCommand::default()
        };
        client.send(ClientMessage::Move(vec![command], 0).encode());
        server.tick();
        while let Some(bytes) = client.receive() {
            match ServerMessage::decode(&bytes) {
                Some(ServerMessage::Gravity(gravity)) => told = Some(gravity),
                Some(ServerMessage::Snapshot(snapshot)) if tick >= 90 => {
                    lowest = lowest.min(snapshot.player.origin.z);
                    highest = highest.max(snapshot.player.origin.z);
                }
                _ => {}
            }
        }
    }
    (highest - lowest, told)
}

#[test]
#[ignore = "needs game data"]
fn e1m8_has_its_low_gravity_and_nowhere_else_does() {
    let base = base();
    let (height, told) = jump_on(&base, "e1m8");
    println!("e1m8: a jump goes {height:.0} units");
    assert_eq!(told, Some(100.0));
    assert!(height > 150.0, "{height}");
    let (height, told) = jump_on(&base, "e1m1");
    println!("e1m1: a jump goes {height:.0} units");
    assert_eq!(told, Some(800.0));
    assert!((35.0..60.0).contains(&height), "{height}");
}

/// Lua that gets ready to fire rockets.
const ROCKETS_AND_SKY: &str = r#"
    blasts = 0
    local real = T_RadiusDamage
    T_RadiusDamage = function(...)
        blasts += 1
        return real(...)
    end
    player = find_all("classname", "player")[1]
    function fire(from, direction)
        local rocket = spawn()
        rocket.owner = player
        rocket.movetype = MOVETYPE_FLYMISSILE
        rocket.solid = SOLID_BBOX
        rocket.velocity = direction * 1000
        rocket.touch = T_MissileTouch
        setmodel(rocket, "progs/missile.mdl")
        setsize(rocket, vector.zero, vector.zero)
        setorigin(rocket, from)
        return rocket
    end
"#;

#[test]
#[ignore = "needs game data"]
fn a_rocket_that_reaches_the_sky_just_goes() {
    let (mut server, _client, map) = start(&base(), "e1m5");
    let world = CollisionWorld::new(Arc::clone(&map));
    let collision = world.with(&[]);
    let model = &map.models[0];
    let (low, high) = (model.mins, model.maxs);
    let steps = |from: f32, to: f32| {
        (0..)
            .map(move |n| from + n as f32 * 16.0)
            .take_while(move |at| *at < to)
    };
    let under_sky = steps(low.x, high.x)
        .flat_map(|x| steps(low.y, high.y).map(move |y| (x, y)))
        .flat_map(|(x, y)| steps(low.z, high.z).map(move |z| Vec3::new(x, y, z)))
        .find(|place| {
            let up = *place + Vec3::new(0.0, 0.0, 256.0);
            let stop = collision.move_box(*place, Vec3::ZERO, Vec3::ZERO, up);
            world.map_contents(*place) == Contents::Empty
                && stop.fraction < 1.0
                && world.map_contents(stop.end) == Contents::Sky
        })
        .expect("this level has sky over open air");
    // High up, away from whoever's standing about.
    let up = under_sky + Vec3::new(0.0, 0.0, 256.0);
    let stop = collision.move_box(under_sky, Vec3::ZERO, Vec3::ZERO, up);
    let under_sky = stop.end - Vec3::new(0.0, 0.0, 40.0);
    assert_eq!(world.map_contents(under_sky), Contents::Empty);
    server.exec(ROCKETS_AND_SKY).expect("it's ready");
    let from = format!(
        "vector.create({}, {}, {})",
        under_sky.x, under_sky.y, under_sky.z
    );
    server
        .exec(&format!("rocket = fire({from}, vector.create(0, 0, 1))"))
        .expect("it fires");
    for _ in 0..144 {
        server.tick();
    }
    server
        .exec(r#"assert(rawget(rocket, "_removed") and blasts == 0, "gone without a blast")"#)
        .expect("it holds");
    server
        .exec(&format!("rocket = fire({from}, vector.create(0, 0, -1))"))
        .expect("it fires");
    for _ in 0..144 {
        server.tick();
    }
    server
        .exec(r#"assert(blasts == 1, "the control: the ground blows it up")"#)
        .expect("it holds");
}

#[test]
#[ignore = "needs game data"]
fn a_saved_game_keeps_its_gravity() {
    let base = base();
    let (server, _client, map) = start(&base, "e1m8");
    let saved = server.save().expect("it saves");
    let world = Rc::new(CollisionWorld::new(Arc::clone(&map)));
    let (_client, server_end) = loopback();
    let resumed = Server::resume(Box::new(server_end), world, SCRIPTS, 1.0 / 72.0, saved)
        .expect("it resumes");
    assert_eq!(resumed.gravity(), 100.0);
}

#[test]
#[ignore = "needs game data"]
fn every_trigger_with_health_can_be_shot() {
    let base = base();
    let filesystem = game(&base, "id1");
    let mut total = 0;
    for name in map_names(&filesystem) {
        let (mut server, mut client, _map) = start(&base, &name);
        for _ in 0..5 {
            server.tick();
        }
        server
            .exec(
                r#"
                local shootable = 0
                local player = find_all("classname", "player")[1]
                for _, class in { "trigger_multiple", "trigger_once" } do
                    for _, trigger in find_all("classname", class) do
                        if (trigger.max_health or 0) ~= 0 then
                            assert(trigger.takedamage == DAMAGE_YES, class)
                            assert(trigger.solid == SOLID_BBOX, class)
                            assert(trigger.touch == nil or trigger.touch == SUB_Null, class)
                            T_Damage(trigger, player, player, 1000)
                            assert(trigger.takedamage == DAMAGE_NO, "it fired")
                            shootable += 1
                        end
                    end
                end
                engine.print("shootable " .. shootable)
                "#,
            )
            .expect("every one fires");
        server.tick();
        while let Some(bytes) = client.receive() {
            if let Some(ServerMessage::Print(text)) = ServerMessage::decode(&bytes)
                && let Some(count) = text.strip_prefix("shootable ")
            {
                total += count.trim().parse::<u32>().expect("a number");
            }
        }
    }
    println!("{total} shootable triggers fired");
    assert_eq!(total, 29, "as many as the maps hold");
}

#[test]
#[ignore = "needs game data"]
fn the_last_level_starts_with_shub_alive_and_her_teleporter_ready() {
    let base = base();
    let (mut server, mut client, _map) = start(&base, "end");
    for _ in 0..30 {
        server.tick();
    }
    while let Some(bytes) = client.receive() {
        if let Some(ServerMessage::Print(text)) = ServerMessage::decode(&bytes) {
            assert!(!text.contains("script error"), "{text}");
        }
    }
    server
        .exec(
            r#"
            local shub = find_all("classname", "monster_oldone")[1]
            assert(shub ~= nil and shub.health > 39000, "she's alive")
            assert(math.abs(shub.origin.y - 2032) < 1, "and where the map put her")
            assert(intermission_running ~= 1, "and the game isn't over")
            train = find_all("classname", "misc_teleporttrain")[1]
            assert(train ~= nil, "it's on the level")
            assert(train.model == "progs/teleport.mdl", tostring(train.model))
            assert(train.target ~= "t32", "it found its first corner")
            train_start = train.origin
            train.use(train)
            "#,
        )
        .expect("the level is as it should be");
    for _ in 0..144 {
        server.tick();
    }
    server
        .exec(r#"assert(train.origin ~= train_start, "and it moves once it's used")"#)
        .expect("it moves");
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
        origin,
        SCRIPTS,
        1.0 / 72.0,
        normal_level(),
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

#[test]
#[ignore = "needs game data"]
fn every_model_in_the_game_parses() {
    let base = base();
    let palette = palette(&base);
    let filesystem = game(&base, "id1");
    let mut models = 0;
    let mut spinners = 0;
    for path in filesystem.list("progs/") {
        if !path.ends_with(".mdl") {
            continue;
        }
        let bytes = filesystem.read(&path).expect("it reads");
        let model = parse_mdl(&bytes, &palette).unwrap_or_else(|| panic!("{path} parses"));
        assert!(!model.corners.is_empty(), "{path} has triangles");
        assert!(!model.frames.is_empty(), "{path} has frames");
        for frame in &model.frames {
            let end = frame.first_pose + frame.pose_count;
            assert!(end as usize <= model.poses.len(), "{path}'s frames fit");
        }
        models += 1;
        spinners += usize::from(model.spins);
    }
    println!("{models} models, {spinners} spin");
    assert_eq!(models, 79);
}

/// Lua that runs `body` on every monster, with `player` and `each` set.
fn on_every_monster(body: &str) -> String {
    format!(
        r#"local player = find_all("classname", "player")[1]
        local classes = {{ "monster_army", "monster_demon1", "monster_dog", "monster_enforcer",
            "monster_fish", "monster_hell_knight", "monster_knight", "monster_ogre",
            "monster_shalrath", "monster_shambler", "monster_tarbaby", "monster_wizard",
            "monster_zombie" }}
        local number = 0
        for _, class in classes do
            for _, each in find_all("classname", class) do
                number += 1
                if each.health > 0 then
                    {body}
                end
            end
        end"#
    )
}

/// Wakes, hurts and kills every monster on `map`, and returns what went wrong
/// and how many monsters there were.
fn fight_everything(map: &Arc<Map>) -> (Vec<String>, u32) {
    let world = Rc::new(CollisionWorld::new(Arc::clone(map)));
    let origin = spawn_point(map).origin;
    let (mut client, server_end) = loopback();
    let mut server = Server::new(
        Box::new(server_end),
        world,
        origin,
        SCRIPTS,
        1.0 / 72.0,
        normal_level(),
    )
    .expect("the scripts load");
    let mut errors = Vec::new();
    let mut run = |server: &mut Server, seconds: u32| {
        for _ in 0..seconds * 72 {
            server.tick();
            while let Some(bytes) = client.receive() {
                if let Some(ServerMessage::Print(text)) = ServerMessage::decode(&bytes)
                    && text.contains("script error")
                {
                    errors.push(text);
                }
            }
        }
    };
    run(&mut server, 1);
    server
        .exec(
            r#"local player = find_all("classname", "player")[1]
            player.flags = bit32.bor(player.flags, FL_GODMODE)"#,
        )
        .expect("godmode");
    server
        .exec(&on_every_monster(
            "each.enemy = player; each.goalentity = player; FoundTarget(each)",
        ))
        .expect("waking");
    run(&mut server, 8);
    for _ in 0..3 {
        server
            .exec(&on_every_monster("T_Damage(each, player, player, 5)"))
            .expect("hurting");
        run(&mut server, 2);
    }
    server
        .exec(&on_every_monster(
            "T_Damage(each, player, player, each.health + if number % 2 == 0 then 0 else 200)",
        ))
        .expect("killing");
    run(&mut server, 4);
    server
        .exec(&on_every_monster(
            r#"if each.takedamage ~= 0 then error("still alive: " .. class) end"#,
        ))
        .expect("none are left");
    let count = map
        .entities
        .iter()
        .filter(|entity| {
            entity
                .get("classname")
                .is_some_and(|class| class.starts_with("monster_"))
        })
        .count();
    (errors, u32::try_from(count).expect("a few monsters"))
}

#[test]
#[ignore = "needs game data"]
fn every_monster_wakes_fights_and_dies_without_errors() {
    let base = base();
    let palette = palette(&base);
    let filesystem = game(&base, "id1");
    let mut monsters = 0;
    let mut failures = 0;
    for path in filesystem.list("maps/") {
        let Some(name) = path
            .strip_prefix("maps/")
            .and_then(|p| p.strip_suffix(".bsp"))
            .filter(|name| !name.starts_with("b_"))
        else {
            continue;
        };
        let map = load(&filesystem, name, &palette);
        let (errors, count) = fight_everything(&map);
        monsters += count;
        if !errors.is_empty() {
            failures += 1;
            println!("{name}: {} errors, first {}", errors.len(), errors[0]);
        }
    }
    println!("{monsters} monsters, {failures} maps with errors");
    assert_eq!(failures, 0);
    assert!(monsters > 1000);
}

/// Lua that fires a line at every shootable brush from all sides, as a shot
/// does, and hurts what it finds. It prints what nothing could reach.
const SHOOT_EVERYTHING: &str = r#"
    local player = find_all("classname", "player")[1]
    local directions = {
        vector.create(1, 0, 0), vector.create(-1, 0, 0), vector.create(0, 1, 0),
        vector.create(0, -1, 0), vector.create(0, 0, 1), vector.create(0, 0, -1),
    }
    shot_things = {}
    for _, class in { "func_button", "func_door", "door", "func_door_secret" } do
        for _, thing in find_all("classname", class) do
            if thing.takedamage ~= 0 and thing.movetype == MOVETYPE_PUSH then
                local middle = (thing.absmin + thing.absmax) * 0.5
                local hit = false
                for _, direction in directions do
                    for _, distance in { 24, 48, 96, 192 } do
                        local start = middle + direction * distance
                        if pointcontents(start) ~= CONTENT_SOLID then
                            traceline(start, middle, false, player)
                            -- Some switches wait behind a door.
                            hit = hit or trace_ent == thing or trace_ent.classname == "door"
                        end
                    end
                end
                if hit then
                    table.insert(shot_things, { thing, thing.origin })
                    -- Secret doors have too much health to kill, and answer to pain.
                    T_Damage(thing, player, player, if thing.th_die then thing.health else 4)
                else
                    engine.print("unreachable " .. class .. " " .. tostring(thing.model))
                end
            end
        end
    end
    engine.print("shot " .. #shot_things)
"#;

/// Lua that prints what didn't answer being shot.
const SHOT_RESULTS: &str = r#"
    for _, pair in shot_things do
        local thing = pair[1]
        local moved = vector.magnitude(thing.origin - pair[2]) >= 1
        -- A door can be a switch with no room to move, which only changes state.
        if not moved and thing.state == STATE_BOTTOM then
            engine.print("no answer from " .. thing.classname .. " " .. tostring(thing.model))
        end
    end
"#;

/// What shooting everything shootable on `map` printed.
fn shoot_everything(map: &Arc<Map>) -> Vec<String> {
    let world = Rc::new(CollisionWorld::new(Arc::clone(map)));
    let (mut client, server_end) = loopback();
    let mut server = Server::new(
        Box::new(server_end),
        world,
        spawn_point(map).origin,
        SCRIPTS,
        1.0 / 72.0,
        normal_level(),
    )
    .expect("the scripts load");
    let mut prints = Vec::new();
    let mut run = |server: &mut Server, ticks: u32| {
        for _ in 0..ticks {
            server.tick();
            while let Some(bytes) = client.receive() {
                if let Some(ServerMessage::Print(text)) = ServerMessage::decode(&bytes) {
                    prints.push(text);
                }
            }
        }
    };
    run(&mut server, 72);
    server.exec(SHOOT_EVERYTHING).expect("shooting");
    run(&mut server, 12);
    server.exec(SHOT_RESULTS).expect("results");
    run(&mut server, 1);
    prints
}

#[test]
#[ignore = "needs game data"]
fn every_shootable_thing_can_be_hit_and_answers() {
    let base = base();
    let palette = palette(&base);
    let filesystem = game(&base, "id1");
    let mut shot = 0;
    let mut wrong = Vec::new();
    for path in filesystem.list("maps/") {
        let Some(name) = path
            .strip_prefix("maps/")
            .and_then(|p| p.strip_suffix(".bsp"))
            .filter(|name| !name.starts_with("b_"))
        else {
            continue;
        };
        for text in shoot_everything(&load(&filesystem, name, &palette)) {
            if let Some(count) = text.strip_prefix("shot ") {
                shot += count.trim().parse::<u32>().expect("a count");
            } else if text.contains("script error")
                || text.contains("unreachable")
                || text.contains("no answer")
            {
                wrong.push(format!("{name}: {text}"));
            }
        }
    }
    println!("{shot} shot, wrong: {wrong:#?}");
    assert!(shot > 50, "{shot}");
    assert!(wrong.is_empty());
}

/// How many of a model's triangles wind clockwise from outside, and how many
/// have normals pointing out, of how many there are.
fn outward_triangles(model: &AliasModel) -> (usize, usize, usize) {
    let pose = &model.poses[0];
    let point = |corner: &AliasCorner| {
        let [x, y, z] = pose[corner.vertex as usize].position;
        Vec3::new(
            f32::from(x) * model.scale.x,
            f32::from(y) * model.scale.y,
            f32::from(z) * model.scale.z,
        )
    };
    let all: Vec<Vec3> = model.corners.iter().map(point).collect();
    #[expect(clippy::cast_precision_loss, reason = "a few hundred corners")]
    let share = 1.0 / all.len() as f32;
    let middle = all.iter().fold(Vec3::ZERO, |sum, at| sum + *at * share);
    let (mut wound, mut facing, mut total) = (0, 0, 0);
    for corners in model.corners.as_chunks::<3>().0 {
        let [a, b, c] = [point(&corners[0]), point(&corners[1]), point(&corners[2])];
        let out = (a + b + c) * (1.0 / 3.0) - middle;
        // Clockwise from outside means this cross product points inwards.
        wound += usize::from((b - a).cross(c - a).dot(out) < 0.0);
        let [x, y, z] = pose[corners[0].vertex as usize].normal.map(f32::from);
        facing += usize::from(Vec3::new(x, y, z).dot(out) > 0.0);
        total += 1;
    }
    (wound, facing, total)
}

#[test]
#[ignore = "needs game data"]
fn item_boxes_wind_and_face_like_models_do() {
    let base = base();
    let palette = palette(&base);
    let filesystem = game(&base, "id1");
    for name in [
        "maps/b_shell0.bsp",
        "maps/b_nail0.bsp",
        "maps/b_rock0.bsp",
        "maps/b_batt0.bsp",
        "maps/b_bh25.bsp",
        "maps/b_explob.bsp",
    ] {
        let bytes = filesystem.read(name).expect("it reads");
        let parts = parse_item_bsp(&bytes, &palette).expect("it parses");
        // Each part is one texture, so look at the whole box together.
        let mut whole = parts[0].clone();
        for part in &parts[1..] {
            let first = u32::try_from(whole.poses[0].len()).expect("small");
            whole.poses[0].extend_from_slice(&part.poses[0]);
            whole
                .corners
                .extend(part.corners.iter().map(|corner| AliasCorner {
                    vertex: corner.vertex + first,
                    uv: corner.uv,
                }));
        }
        let (wound, facing, total) = outward_triangles(&whole);
        println!("{name}: {wound} wound and {facing} facing out of {total}");
        assert_eq!(wound, total, "{name} winds clockwise from outside");
        assert_eq!(facing, total, "{name}'s normals point out");
    }
}

/// Lua that arms the player and sets every monster on them.
const START_A_FIGHT: &str = r#"
    local player = find_all("classname", "player")[1]
    player.items = bit32.bor(player.items, IT_SUPER_SHOTGUN, IT_NAILGUN, IT_SUPER_NAILGUN,
        IT_GRENADE_LAUNCHER, IT_ROCKET_LAUNCHER, IT_LIGHTNING)
    player.ammo_shells, player.ammo_nails, player.ammo_rockets, player.ammo_cells = 99, 99, 99, 99
    player.health = 5000
    for _, entity in saved_entities() do
        if bit32.band(entity.flags, FL_MONSTER) ~= 0 and (entity.health or 0) > 0 then
            entity.enemy = player
            if entity.th_run then
                entity.th_run(entity)
            end
        end
    end
"#;

/// One side of a game, with the numbers it's sent.
struct Side {
    /// The game.
    server: Server,
    /// Its other end.
    client: Box<dyn Transport>,
    /// The next command's number.
    sequence: u32,
    /// The latest snapshot.
    latest: Option<engine_net::Snapshot>,
    /// Any script errors.
    errors: Vec<String>,
}

impl Side {
    /// Runs `command` for a tick.
    fn tick(&mut self, command: UserCommand) {
        self.sequence += 1;
        let command = UserCommand {
            sequence: self.sequence,
            ..command
        };
        self.client
            .send(ClientMessage::Move(vec![command], 0).encode());
        self.server.tick();
        while let Some(bytes) = self.client.receive() {
            match ServerMessage::decode(&bytes).expect("valid") {
                ServerMessage::Snapshot(snapshot) => self.latest = Some(snapshot),
                ServerMessage::Print(text) if text.contains("script error") => {
                    self.errors.push(text);
                }
                _ => {}
            }
        }
    }
}

/// What a player does on tick `tick`: run about, change weapons and fire.
fn busy_player(tick: u32, random: &mut Random) -> UserCommand {
    UserCommand {
        seconds: 1.0 / 72.0,
        yaw: random.range(0.0, 360.0),
        forward: 400.0,
        attack: true,
        jump: tick.is_multiple_of(50),
        impulse: if tick.is_multiple_of(60) {
            u8::try_from(tick / 60 % 7 + 2).expect("small")
        } else {
            0
        },
        ..UserCommand::default()
    }
}

/// Plays `map` a while, saves it, and plays the game and the save on side by
/// side. Returns what differs at the end, and any errors.
fn save_and_compare(map: &Arc<Map>, name: &str) -> Result<(), String> {
    let world = Rc::new(CollisionWorld::new(Arc::clone(map)));
    let (client, server_end) = loopback();
    let mut level = normal_level();
    name.clone_into(&mut level.name);
    let server = Server::new(
        Box::new(server_end),
        Rc::clone(&world),
        spawn_point(map).origin,
        SCRIPTS,
        1.0 / 72.0,
        level,
    )
    .expect("the scripts load");
    let mut first = Side {
        server,
        client: Box::new(client),
        sequence: 0,
        latest: None,
        errors: Vec::new(),
    };
    let mut random = Random(7);
    for tick in 0..72 {
        first.tick(busy_player(tick, &mut random));
    }
    first.server.exec(START_A_FIGHT).expect("a fight");
    for tick in 72..360 {
        first.tick(busy_player(tick, &mut random));
    }
    let saved = first.server.save().map_err(|error| error.to_string())?;
    let text = serde_json::to_string(&saved).map_err(|error| error.to_string())?;
    let saved = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let (client, server_end) = loopback();
    let server = Server::resume(Box::new(server_end), world, SCRIPTS, 1.0 / 72.0, saved)
        .map_err(|error| error.to_string())?;
    let mut second = Side {
        server,
        client: Box::new(client),
        sequence: 0,
        latest: None,
        errors: Vec::new(),
    };
    for tick in 360..720 {
        let command = busy_player(tick, &mut random);
        first.tick(command);
        second.tick(command);
        let (one, two) = (
            first.latest.as_ref().expect("a snapshot"),
            second.latest.as_ref().expect("a snapshot"),
        );
        let difference = if one.player != two.player {
            Some(format!("{:?} and {:?}", one.player, two.player))
        } else if one.stats != two.stats {
            Some(format!("{:?} and {:?}", one.stats, two.stats))
        } else if one.entities != two.entities {
            let pair = one.entities.iter().zip(&two.entities).find(|(a, b)| a != b);
            Some(format!("entities {:?}", pair))
        } else if one.instances != two.instances {
            Some("brush models".to_owned())
        } else {
            None
        };
        if let Some(difference) = difference {
            return Err(format!("tick {tick}: {difference}"));
        }
    }
    let errors = [first.errors, second.errors].concat();
    errors.first().map_or(Ok(()), |error| Err(error.clone()))
}

#[test]
#[ignore = "needs game data"]
fn a_game_goes_on_the_same_after_a_save_on_every_map() {
    let base = base();
    let palette = palette(&base);
    let filesystem = game(&base, "id1");
    let mut wrong = Vec::new();
    let mut maps = 0;
    for path in filesystem.list("maps/") {
        let Some(name) = path
            .strip_prefix("maps/")
            .and_then(|p| p.strip_suffix(".bsp"))
            .filter(|name| !name.starts_with("b_"))
        else {
            continue;
        };
        maps += 1;
        if let Err(error) = save_and_compare(&load(&filesystem, name, &palette), name) {
            wrong.push(format!("{name}: {error}"));
        }
    }
    println!("{maps} maps, wrong: {wrong:#?}");
    assert!(maps > 30);
    assert!(wrong.is_empty());
}

/// The level's name for each map, and what it leads to.
fn exits(map: &Map) -> Vec<String> {
    map.entities
        .iter()
        .filter(|entity| entity.get("classname") == Some("trigger_changelevel"))
        .filter_map(|entity| entity.get("map").map(str::to_owned))
        .collect()
}

#[test]
#[ignore = "needs game data"]
fn every_episode_can_be_walked_from_start_to_the_end_map() {
    let base = base();
    let palette = palette(&base);
    let filesystem = game(&base, "id1");
    let mut seen = std::collections::BTreeSet::new();
    let mut waiting = vec!["start".to_owned()];
    while let Some(name) = waiting.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        let map = load(&filesystem, &name, &palette);
        waiting.extend(exits(&map));
    }
    println!("reachable: {seen:?}");
    for wanted in [
        "e1m7", "e2m6", "e3m6", "e4m7", "end", "e1m8", "e2m7", "e3m7", "e4m8",
    ] {
        assert!(seen.contains(wanted), "{wanted}");
    }
}

#[test]
#[ignore = "needs game data"]
fn coming_back_with_a_rune_lands_in_the_other_start() {
    let base = base();
    let palette = palette(&base);
    let filesystem = game(&base, "id1");
    let map = load(&filesystem, "start", &palette);
    let start = |flags: f64| {
        let (client, server_end) = loopback();
        let mut level = normal_level();
        level.flags = flags;
        "start".clone_into(&mut level.name);
        let server = Server::new(
            Box::new(server_end),
            Rc::new(CollisionWorld::new(Arc::clone(&map))),
            spawn_point(&map).origin,
            SCRIPTS,
            1.0 / 72.0,
            level,
        )
        .expect("the scripts load");
        let mut side = Side {
            server,
            client: Box::new(client),
            sequence: 0,
            latest: None,
            errors: Vec::new(),
        };
        side.tick(UserCommand::default());
        side.latest.expect("a snapshot").player.origin
    };
    let (first, back) = (start(0.0), start(1.0));
    println!("{first:?} and {back:?}");
    assert!((first - back).length() > 100.0);
}

/// Loses every nth unreliable message, as a poor connection does.
struct Lossy {
    /// The real end.
    inner: Loopback,
    /// How many have gone.
    sent: u32,
    /// One in how many is lost.
    drop_every: u32,
    /// Bytes sent, for measuring.
    bytes: Rc<Cell<usize>>,
}

impl Transport for Lossy {
    fn send(&mut self, bytes: Vec<u8>) {
        self.bytes.set(self.bytes.get() + bytes.len());
        self.inner.send(bytes);
    }

    fn send_unreliable(&mut self, bytes: Vec<u8>) {
        self.bytes.set(self.bytes.get() + bytes.len());
        self.sent += 1;
        if !self.sent.is_multiple_of(self.drop_every) {
            self.inner.send(bytes);
        }
    }

    fn receive(&mut self) -> Option<Vec<u8>> {
        self.inner.receive()
    }
}

/// Plays `name` through a real client at one message in `drop_every` lost, with
/// a fight going. Returns the client's error from the server's truth, in units,
/// and bytes a second from the server.
fn play_online(
    filesystem: &FileSystem,
    palette: &Palette,
    name: &str,
    drop_every: u32,
) -> (f32, usize) {
    let map = load(filesystem, name, palette);
    let world = Rc::new(CollisionWorld::new(Arc::clone(&map)));
    let (client_end, server_end) = loopback();
    let bytes = Rc::new(Cell::new(0));
    let lossy_server = Lossy {
        inner: server_end,
        sent: 0,
        drop_every,
        bytes: Rc::clone(&bytes),
    };
    let lossy_client = Lossy {
        inner: client_end,
        sent: 0,
        drop_every,
        bytes: Rc::new(Cell::new(0)),
    };
    let origin = spawn_point(&map).origin;
    let mut level = normal_level();
    name.clone_into(&mut level.name);
    let mut server = Server::new(
        Box::new(lossy_server),
        Rc::clone(&world),
        origin,
        SCRIPTS,
        1.0 / 72.0,
        level,
    )
    .expect("the scripts load");
    let mut client = Client::new(Box::new(lossy_client), world, origin);
    let mut random = Random(3);
    server.exec(START_A_FIGHT).expect("a fight");
    let mut worst: f32 = 0.0;
    let seconds = 10;
    for tick in 0..(72 * seconds) {
        client.send_command(busy_player(tick, &mut random), 1.0 / 72.0);
        server.tick();
        client.receive();
        // Lost snapshots only make the client a tick or two stale.
        if tick > 72 {
            let truth = server.player_origin();
            worst = worst.max((client.predicted().origin - truth).length());
        }
    }
    (
        worst,
        bytes.get() / usize::try_from(seconds).expect("small"),
    )
}

#[test]
#[ignore = "needs game data"]
fn a_real_client_keeps_up_at_two_and_five_percent_loss() {
    let base = base();
    let palette = palette(&base);
    let filesystem = game(&base, "id1");
    for name in ["e1m1", "dm3", "e4m8"] {
        for (loss, drop_every) in [("none", u32::MAX), ("2%", 50), ("5%", 20)] {
            let (worst, rate) = play_online(&filesystem, &palette, name, drop_every);
            println!("{name} at {loss} loss: {worst:.1} units out, {rate} bytes a second");
            assert!(worst < 64.0, "{name} at {loss}: {worst}");
            assert!(rate < 200_000, "{name} at {loss}: {rate}");
        }
    }
}

#[test]
#[ignore = "needs game data"]
fn every_picture_the_screens_use_is_in_the_game() {
    let base = base();
    let palette = palette(&base);
    let filesystem = game(&base, "id1");
    let wad = Wad::parse(&filesystem.read("gfx.wad").expect("gfx.wad")).expect("it parses");
    let names = [
        "conchars",
        "sbar",
        "ibar",
        "scorebar",
        "disc",
        "num_0",
        "num_9",
        "anum_0",
        "num_minus",
        "anum_minus",
        "num_colon",
        "num_slash",
        "inv_shotgun",
        "inv2_lightng",
        "inva5_rlaunch",
        "sb_shells",
        "sb_nails",
        "sb_rocket",
        "sb_cells",
        "sb_armor1",
        "sb_armor3",
        "sb_key1",
        "sb_quad",
        "sb_sigil4",
        "face1",
        "face_p5",
        "face_invis",
        "face_invul2",
        "face_inv2",
        "face_quad",
        "gfx/complete.lmp",
        "gfx/inter.lmp",
        "gfx/finale.lmp",
        "gfx/conback.lmp",
        "gfx/pause.lmp",
        "gfx/loading.lmp",
        "gfx/qplaque.lmp",
        "gfx/ttl_main.lmp",
        "gfx/mainmenu.lmp",
        "gfx/menudot1.lmp",
        "gfx/sp_menu.lmp",
        "gfx/ttl_sgl.lmp",
        "gfx/p_option.lmp",
        "gfx/p_load.lmp",
        "gfx/p_save.lmp",
        "gfx/ttl_cstm.lmp",
        "gfx/vidmodes.lmp",
        "gfx/help0.lmp",
        "gfx/help5.lmp",
        "gfx/box_tl.lmp",
        "gfx/box_mm2.lmp",
    ];
    for name in names {
        let image = load_picture(name, &filesystem, &wad, &palette);
        let image = image.unwrap_or_else(|| panic!("{name} is missing"));
        assert_eq!(
            image.rgba.len(),
            usize::try_from(image.width * image.height * 4).expect("small"),
            "{name}"
        );
    }
}

#[test]
#[ignore = "needs game data"]
fn every_sprite_in_the_game_parses() {
    let base = base();
    let palette = palette(&base);
    let filesystem = game(&base, "id1");
    let mut sprites = 0;
    for path in filesystem.list("progs/") {
        if !path.ends_with(".spr") {
            continue;
        }
        let bytes = filesystem.read(&path).expect("it reads");
        let sprite = parse_spr(&bytes, &palette).unwrap_or_else(|| panic!("{path} parses"));
        for frame in &sprite.frames {
            let end = frame.first_image + frame.image_count;
            assert!(end as usize <= sprite.images.len(), "{path}'s frames fit");
        }
        sprites += 1;
    }
    assert_eq!(sprites, 3);
}

#[test]
#[ignore = "needs game data"]
fn buttons_have_textures_for_when_they_are_used() {
    let base = base();
    let palette = palette(&base);
    let filesystem = game(&base, "id1");
    let map = load(&filesystem, "e1m1", &palette);
    let linked: Vec<&str> = map
        .textures
        .iter()
        .filter(|texture| texture.alternate.is_some())
        .map(|texture| texture.name.as_str())
        .collect();
    println!("{linked:?}");
    assert!(
        linked
            .iter()
            .any(|name| name.eq_ignore_ascii_case("+0basebtn"))
    );
    assert!(
        linked
            .iter()
            .any(|name| name.eq_ignore_ascii_case("+abasebtn"))
    );
}

#[test]
#[ignore = "needs game data"]
fn every_class_on_every_map_has_a_function() {
    let base = base();
    let palette = palette(&base);
    let filesystem = game(&base, "id1");
    let mut classes = std::collections::BTreeSet::new();
    let mut names: Vec<String> = filesystem
        .list("maps/")
        .into_iter()
        .filter(|path| path.ends_with(".bsp") && !path.contains("/b_"))
        .collect();
    names.sort();
    for path in names {
        let name = path.trim_start_matches("maps/").trim_end_matches(".bsp");
        for entity in &load(&filesystem, name, &palette).entities {
            if let Some(class) = entity.get("classname") {
                classes.insert(class.to_owned());
            }
        }
    }
    let map = load(&filesystem, "start", &palette);
    let world = Rc::new(CollisionWorld::new(Arc::clone(&map)));
    let (mut client, server_end) = loopback();
    let mut server = Server::new(
        Box::new(server_end),
        world,
        Vec3::ZERO,
        SCRIPTS,
        1.0 / 72.0,
        normal_level(),
    )
    .expect("the scripts load");
    let source: String = classes
        .iter()
        .map(|name| {
            format!(
                "if {name} == nil then engine.print(\"{name}\") end
"
            )
        })
        .collect();
    server.exec(&source).expect("it runs");
    server.tick();
    let mut found = Vec::new();
    while let Some(bytes) = client.receive() {
        if let Some(ServerMessage::Print(text)) = ServerMessage::decode(&bytes) {
            found.push(text);
        }
    }
    found.retain(|text| !text.ends_with("entered the game"));
    println!("{} classes, none for {found:?}", classes.len());
    assert!(found.is_empty(), "{found:?}");
}
