//! Enemies once they move, in the real levels.

#![expect(clippy::print_stdout, reason = "what each enemy did is the point")]

use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use engine_compile::{compile_text, placeholder_texture};
use engine_map::Map;
use engine_net::{Loopback, ServerMessage, Transport, loopback};
use engine_physics::CollisionWorld;
use engine_server::{Level, Server};
use fixture_quake::{SCRIPTS, spawn_point};

/// A level's shape, from its text. Enemies don't need its light.
fn level(name: &str) -> Arc<Map> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../content/maps")
        .join(format!("{name}.emap"));
    let text = std::fs::read_to_string(path).expect("the map");
    let mut source = |material: &str| placeholder_texture(material);
    Arc::new(compile_text(&text, &mut source).expect("compiles"))
}

/// A server on `map`, with its client's end.
fn start(map: &Arc<Map>) -> (Server, Loopback) {
    let world = Rc::new(CollisionWorld::new(Arc::clone(map)));
    let (client, server_end) = loopback();
    let server = Server::new(
        Box::new(server_end),
        world,
        spawn_point(map).origin,
        SCRIPTS,
        1.0 / 72.0,
        Level {
            settings: vec![("skill".to_owned(), 1.0), ("registered".to_owned(), 1.0)],
            ..Level::default()
        },
    )
    .expect("the scripts load");
    (server, client)
}

/// Runs for `seconds`, and returns what was printed.
fn run(server: &mut Server, client: &mut Loopback, seconds: u32) -> Vec<String> {
    let mut printed = Vec::new();
    for _ in 0..seconds * 72 {
        server.tick();
        while let Some(bytes) = client.receive() {
            if let Some(ServerMessage::Print(text)) = ServerMessage::decode(&bytes) {
                printed.push(text);
            }
        }
    }
    printed
}

/// Lua that runs `body` for every enemy, with `player` and `each` set.
fn on_every_enemy(body: &str) -> String {
    format!(
        r#"local player = find_all("classname", "player")[1]
        for _, class in {{ "monster_army", "monster_dog", "monster_knight", "monster_ogre",
            "monster_shambler", "monster_wizard", "monster_zombie" }} do
            for _, each in find_all("classname", class) do
                {body}
            end
        end"#
    )
}

/// Sets the player down near each enemy in turn and has it fight, and returns a line
/// for each that does something wrong.
fn enemies_in_play(name: &str) -> Vec<String> {
    let map = level(name);
    let (mut server, mut client) = start(&map);
    let _settled = run(&mut server, &mut client, 1);
    server
        .exec(
            r#"local player = find_all("classname", "player")[1]
            player.flags = bit32.bor(player.flags, FL_GODMODE)
            enemy_list = {}"#,
        )
        .expect("godmode");
    server
        .exec(&on_every_enemy("table.insert(enemy_list, each)"))
        .expect("listing");
    let mut wrong = Vec::new();
    let count = map
        .entities
        .iter()
        .filter(|entity| {
            entity
                .get("classname")
                .is_some_and(|class| class.starts_with("monster_"))
        })
        .count();
    for number in 1..=count {
        server
            .exec(&format!(
                r#"local player = find_all("classname", "player")[1]
                local each = enemy_list[{number}]
                was = each.origin
                was_frame = each.frame
                -- Somewhere it can see, as a real fight would be.
                for turn = 0, 15 do
                    local angle = turn * math.pi / 8
                    local spot = each.origin
                        + vector.create(math.cos(angle) * 180, math.sin(angle) * 180, 0)
                    traceline(each.origin, spot, true, each)
                    if trace_fraction == 1 and pointcontents(spot) ~= CONTENT_SOLID then
                        setorigin(player, spot)
                        break
                    end
                end
                each.enemy = player
                each.goalentity = player
                FoundTarget(each)"#
            ))
            .expect("waking");
        let printed = run(&mut server, &mut client, 6);
        wrong.extend(printed.into_iter().filter(|line| line.contains("error")));
        server
            .exec(&format!(
                r#"local each = enemy_list[{number}]
                local gone = vector.magnitude(each.origin - was)
                local where_it_is = pointcontents(each.origin)
                engine.print("ENEMY {number} " .. each.classname .. " moved " .. math.floor(gone)
                    .. " at " .. tostring(each.origin) .. " in " .. where_it_is
                    .. " health " .. each.health
                    .. (if each.frame == was_frame then " still" else " busy"))
                each.health = 0
                each.solid = SOLID_NOT
                setorigin(each, vector.create(0, 0, -100000))
                each.think = nil
                each.nextthink = 0"#
            ))
            .expect("asking");
        let told = run(&mut server, &mut client, 1);
        for line in told.iter().filter(|line| line.contains("ENEMY")) {
            println!("{name}: {line}");
            let moved: i32 = line
                .split(" moved ")
                .nth(1)
                .and_then(|rest| rest.split(' ').next())
                .and_then(|word| word.parse().ok())
                .unwrap_or(0);
            // Zombies throw from where they stand, so standing isn't stuck.
            if (moved < 16 && line.contains(" still")) || line.contains(" in -2 ") {
                wrong.push(line.clone());
            }
        }
    }
    wrong
}

/// A server on `map` with `name`'s own script, with its client's end.
fn start_with_script(map: &Arc<Map>, name: &str) -> (Server, Loopback) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../content/maps")
        .join(format!("{name}.luau"));
    let script = std::fs::read_to_string(path).expect("the script");
    let world = Rc::new(CollisionWorld::new(Arc::clone(map)));
    let (client, server_end) = loopback();
    let server = Server::new(
        Box::new(server_end),
        world,
        spawn_point(map).origin,
        SCRIPTS,
        1.0 / 72.0,
        Level {
            settings: vec![("skill".to_owned(), 1.0), ("registered".to_owned(), 1.0)],
            scripts: vec![(name.to_owned(), script)],
            ..Level::default()
        },
    )
    .expect("the scripts load");
    (server, client)
}

/// The city's guards face the way they're walking, and move their legs.
#[test]
#[ignore = "takes a minute"]
fn a_guard_walks_the_way_it_faces() {
    let map = level("vesper_deep");
    let (mut server, mut client) = start_with_script(&map, "vesper_deep");
    let _settled = run(&mut server, &mut client, 2);
    let mut frames = std::collections::BTreeSet::new();
    for _ in 0..40 {
        server
            .exec(
                r#"local guard = find_all("classname", "patrol_walker")[1]
                guard.sleeps = false
                engine.print("AT " .. guard.origin.x .. " " .. guard.origin.y .. " "
                    .. guard.angles.y .. " " .. guard.frame)"#,
            )
            .expect("asking");
        let told = run(&mut server, &mut client, 1);
        for line in told.iter().filter(|line| line.starts_with("AT ")) {
            let parts: Vec<f32> = line[3..]
                .split(' ')
                .filter_map(|word| word.parse().ok())
                .collect();
            println!("{parts:?}");
            frames.insert(parts[3].to_bits());
            assert!((90.0..114.0).contains(&parts[3]), "{parts:?}");
        }
    }
    assert!(frames.len() > 3, "its legs move");
}

/// A name alone doesn't make something wait to be fired.
#[test]
#[ignore = "takes a minute"]
fn lifts_with_names_start_at_the_bottom_and_the_gate_waits_for_its_button() {
    let map = level("ember_keep");
    let (mut server, mut client) = start(&map);
    let _settled = run(&mut server, &mut client, 1);
    server
        .exec(
            r#"local lifts = find_all("classname", "plat")
            for _, lift in lifts do
                engine.print("LIFT " .. tostring(lift.name) .. " " .. tostring(lift.state == STATE_BOTTOM))
            end
            for _, door in find_all("classname", "door") do
                if door.targetname == "gate_up" then
                    engine.print("GATE " .. tostring(door.trigger_field == nil))
                end
            end"#,
        )
        .expect("asking");
    let told = run(&mut server, &mut client, 1);
    let lifts: Vec<_> = told
        .iter()
        .filter(|line| line.starts_with("LIFT"))
        .collect();
    assert!(lifts.len() >= 2, "{told:?}");
    assert!(lifts.iter().all(|line| line.ends_with("true")), "{lifts:?}");
    let gate = told.iter().find(|line| line.starts_with("GATE"));
    assert_eq!(gate.map(String::as_str), Some("GATE true"), "{told:?}");
}

#[test]
#[ignore = "takes a minute"]
fn enemies_in_the_ember_keep_chase_and_fight() {
    let wrong = enemies_in_play("ember_keep");
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
#[ignore = "takes a minute"]
fn enemies_in_vesper_deep_chase_and_fight() {
    let wrong = enemies_in_play("vesper_deep");
    assert!(wrong.is_empty(), "{wrong:#?}");
}
