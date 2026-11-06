//! The platform's operations, run headless.

use std::fs;
use std::path::{Path, PathBuf};

use engine_tools::{Session, build_prefab, prefab_names, run};
use serde_json::{Value, json};

/// A fresh content folder with a prefab.
fn content(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("engine-tools-{name}-{}", std::process::id()));
    let _gone = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("prefabs")).expect("directory");
    let prefab = r#"
return {
    build = function(params)
        local out = {}
        local steps = params.steps or 3
        for index = 0, steps - 1 do
            local height = (index + 1) * 16
            table.insert(out, {
                block = "brush",
                material = "stone",
                cuboid = { index * 32, 0, 0, index * 32 + 32, 64, height },
            })
        end
        return out
    end,
}
"#;
    fs::write(root.join("prefabs").join("stairs.luau"), prefab).expect("write");
    root
}

/// Runs `name`, expecting it to work.
fn ok(session: &mut Session, name: &str, args: Value) -> Value {
    run(session, name, &args).unwrap_or_else(|error| panic!("{name}: {error}"))
}

/// The number of things in the map.
fn count(session: &mut Session) -> usize {
    let described = ok(session, "describe", json!({}));
    described["things"].as_array().expect("things").len()
}

/// A session with a floor and a start.
fn started(root: &Path) -> Session {
    let mut session = Session::new(root);
    ok(
        &mut session,
        "add_brush",
        json!({ "cuboid": [-256, -256, -32, 256, 256, 0], "material": "floor" }),
    );
    ok(
        &mut session,
        "add_entity",
        json!({ "type": "info_player_start", "origin": [0, 0, 24] }),
    );
    session
}

#[test]
fn undo_and_redo_restore_a_map_exactly() {
    let root = content("undo");
    let mut session = started(&root);
    let before = session.text();
    let added = ok(
        &mut session,
        "add_brush",
        json!({ "cuboid": [0, 0, 0, 32, 32, 32] }),
    );
    let after = session.text();
    assert_ne!(before, after);
    assert_eq!(ok(&mut session, "undo", json!({}))["changed"], true);
    assert_eq!(session.text(), before);
    assert_eq!(ok(&mut session, "redo", json!({}))["changed"], true);
    assert_eq!(session.text(), after);
    assert!(added["id"].is_u64());
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn ids_survive_saving_and_are_never_reused() {
    let root = content("ids");
    let mut session = started(&root);
    let path = root.join("map.emap");
    session.save(Some(&path)).expect("saves");
    let mut reopened = Session::open(&path, &root).expect("opens");
    assert_eq!(reopened.text(), session.text());
    let next = ok(
        &mut reopened,
        "add_brush",
        json!({ "cuboid": [0, 0, 0, 1, 1, 1] }),
    );
    assert_eq!(next["id"], 3, "the two saved things keep 1 and 2");
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_prefab_builds_to_order_and_compiles() {
    let root = content("prefab");
    let mut session = started(&root);
    let placed = ok(
        &mut session,
        "place_prefab",
        json!({ "prefab": "stairs", "at": [64, 0, 0], "params": { "steps": 5 } }),
    );
    assert_eq!(placed["ids"].as_array().expect("ids").len(), 5);
    let map = ok(&mut session, "validate", json!({}));
    assert!(map["faces"].as_u64().expect("faces") > 10);
    assert_eq!(map["has_start"], true);
    let described = ok(&mut session, "describe", json!({}));
    let last = &described["things"][6];
    assert_eq!(last["prefab"], "stairs");
    assert_eq!(last["cuboid"][0], 192.0, "placed at 64, and fifth along");
    assert_eq!(
        ok(&mut session, "prefabs", json!({}))["prefabs"][0],
        "stairs"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn moving_setting_and_deleting_work_by_id() {
    let root = content("edit");
    let mut session = started(&root);
    let id = ok(
        &mut session,
        "add_entity",
        json!({ "type": "light", "name": "lamp", "origin": [0, 0, 64], "fields": { "intensity": 200 } }),
    )["id"]
        .clone();
    ok(&mut session, "move", json!({ "id": id, "by": [10, 0, 0] }));
    ok(
        &mut session,
        "set_field",
        json!({ "id": id, "key": "intensity", "value": 300 }),
    );
    let described = ok(&mut session, "describe", json!({}));
    let lamp = described["things"]
        .as_array()
        .expect("things")
        .iter()
        .find(|thing| thing["name"] == "lamp")
        .expect("the lamp")
        .clone();
    assert_eq!(lamp["origin"][0], 10.0);
    assert_eq!(lamp["intensity"], 300.0);
    let before = count(&mut session);
    ok(&mut session, "delete", json!({ "id": id }));
    assert_eq!(count(&mut session), before - 1);
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_failed_operation_changes_nothing() {
    let root = content("fail");
    let mut session = started(&root);
    let before = session.text();
    let cases = [
        ("add_brush", json!({ "cuboid": [1, 2] })),
        ("add_entity", json!({ "origin": [0, 0, 0] })),
        ("move", json!({ "id": 999, "by": [1, 1, 1] })),
        ("set_field", json!({ "id": 999, "key": "a", "value": 1 })),
        ("set_field", json!({ "id": 1, "key": "id", "value": 5 })),
        ("delete", json!({ "id": 999 })),
        (
            "place_prefab",
            json!({ "prefab": "absent", "at": [0, 0, 0] }),
        ),
        (
            "place_prefab",
            json!({ "prefab": "../evil", "at": [0, 0, 0] }),
        ),
        ("no_such_operation", json!({})),
    ];
    for (name, args) in cases {
        assert!(run(&mut session, name, &args).is_err(), "{name} {args}");
        assert_eq!(session.text(), before, "{name}");
    }
    assert_eq!(ok(&mut session, "undo", json!({}))["changed"], true);
    fs::remove_dir_all(&root).expect("clean up");
}

/// The text of a saved map's block of `kind`.
fn block_of(session: &Session, kind: &str) -> String {
    let text = session.text();
    let start = text
        .find(&format!("{kind} "))
        .unwrap_or_else(|| panic!("no {kind}: {text}"));
    text[start..].to_owned()
}

/// Each feature's operation adds its block, and undo takes it away.
#[test]
fn features_have_operations_that_undo() {
    let mut session = Session::new(&content("features"));
    let before = session.text();
    ok(
        &mut session,
        "place_mesh",
        json!({ "source": "statue", "origin": [1, 2, 3], "scale": 2 }),
    );
    ok(
        &mut session,
        "add_decal",
        json!({ "material": "mark", "origin": [0, 0, 0], "normal": [0, 0, 1], "size": 32 }),
    );
    ok(
        &mut session,
        "add_stamp",
        json!({ "image": "letters", "origin": [0, 0, 0], "normal": [0, 1, 0], "depth": 8 }),
    );
    ok(
        &mut session,
        "paint",
        json!({ "points": [0, 0, 0, 64, 0, 0], "layer": 1 }),
    );
    ok(
        &mut session,
        "add_spline",
        json!({ "name": "patrol", "points": [0, 0, 0, 64, 0, 0] }),
    );
    for kind in ["mesh", "decal", "stamp", "paint", "spline"] {
        assert!(session.text().contains(kind), "{kind}");
    }
    for _ in 0..5 {
        ok(&mut session, "undo", json!({}));
    }
    assert_eq!(session.text(), before);
}

/// What each needs is asked for.
#[test]
fn features_say_what_they_need() {
    let mut session = Session::new(&content("needs"));
    let error = run(&mut session, "add_decal", &json!({ "origin": [0, 0, 0] }));
    assert!(error.expect_err("no material").contains("material"));
    let error = run(&mut session, "add_spline", &json!({ "points": [0, 0, 0] }));
    assert!(error.expect_err("no name").contains("name"));
}

/// `set_block` makes a block a map has one of, and then changes that one.
#[test]
fn set_block_keeps_one_of_a_block() {
    let mut session = Session::new(&content("once"));
    ok(
        &mut session,
        "set_block",
        json!({ "kind": "fog", "fields": { "density": 0.01 } }),
    );
    ok(
        &mut session,
        "set_block",
        json!({ "kind": "fog", "fields": { "density": 0.02 } }),
    );
    let text = session.text();
    assert_eq!(text.matches("fog ").count(), 1, "{text}");
    assert!(text.contains("0.02"), "{text}");
}

/// A line grows a point at a time.
#[test]
fn a_line_grows() {
    let mut session = Session::new(&content("line"));
    let made = ok(
        &mut session,
        "add_spline",
        json!({ "name": "a", "points": [0, 0, 0] }),
    );
    let id = made["id"].clone();
    let grown = ok(
        &mut session,
        "append_point",
        json!({ "id": id, "point": [8, 0, 0] }),
    );
    assert_eq!(grown["points"], 2);
    let error = run(
        &mut session,
        "append_point",
        &json!({ "id": id, "point": [8] }),
    );
    assert!(error.is_err());
}

/// Moving a line moves all of its points, and a stencil's light with it unless
/// that's a sun's direction.
#[test]
fn moving_carries_lines_and_lights() {
    let mut session = Session::new(&content("moves"));
    let line = ok(
        &mut session,
        "add_spline",
        json!({ "name": "a", "points": [0, 0, 0, 10, 0, 0] }),
    );
    ok(
        &mut session,
        "move",
        json!({ "id": line["id"], "by": [1, 2, 3] }),
    );
    let text = block_of(&session, "spline");
    assert!(text.contains("1 2 3") && text.contains("11 2 3"), "{text}");
    let lamp = ok(
        &mut session,
        "add_entity",
        json!({ "type": "ray_stencil", "origin": [0, 0, 0], "fields": { "light": [5, 5, 5] } }),
    );
    ok(
        &mut session,
        "move",
        json!({ "id": lamp["id"], "by": [1, 0, 0] }),
    );
    assert!(session.text().contains("6 5 5"), "{}", session.text());
    let sun = ok(
        &mut session,
        "add_entity",
        json!({
            "type": "ray_stencil",
            "origin": [0, 0, 0],
            "fields": { "light": [7, 7, 7], "sun": true },
        }),
    );
    ok(
        &mut session,
        "move",
        json!({ "id": sun["id"], "by": [1, 0, 0] }),
    );
    assert!(
        session.text().contains("7 7 7"),
        "a direction stays: {}",
        session.text()
    );
}

/// A material covers a whole brush, faces too.
#[test]
fn a_material_covers_a_brush() {
    let mut session = Session::new(&content("cover"));
    let brush = ok(
        &mut session,
        "add_brush",
        json!({ "cuboid": [0, 0, 0, 8, 8, 8], "material": "old" }),
    );
    ok(
        &mut session,
        "set_material",
        json!({ "id": brush["id"], "material": "new" }),
    );
    let text = session.text();
    assert!(text.contains("new") && !text.contains("old"), "{text}");
}

/// Materials are read and written, and one that won't parse is refused.
#[test]
fn materials_edit_safely() {
    let root = content("materials");
    fs::create_dir_all(root.join("materials")).expect("directory");
    fs::write(root.join("materials/stone.emat"), "format 1 material\n").expect("write");
    let mut session = Session::new(&root);
    let read = ok(&mut session, "read_material", json!({ "name": "stone" }));
    assert!(read["text"].as_str().expect("text").contains("material"));
    let bad = json!({ "name": "stone", "text": "not { a document" });
    assert!(run(&mut session, "write_material", &bad).is_err());
    let climb = json!({ "name": "../stone", "text": "format 1 material\n" });
    assert!(run(&mut session, "write_material", &climb).is_err());
    let good = json!({ "name": "stone", "text": "format 1 material\n\nsurface {\n}\n" });
    ok(&mut session, "write_material", good);
    let again = ok(&mut session, "read_material", json!({ "name": "stone" }));
    assert!(again["text"].as_str().expect("text").contains("surface"));
}

/// A map's script sits beside it, and needs a saved map to sit beside.
#[test]
fn a_maps_script_sits_beside_it() {
    let root = content("script");
    let mut session = Session::new(&root);
    assert!(run(&mut session, "read_script", &json!({})).is_err());
    let path = root.join("mine.emap");
    ok(
        &mut session,
        "save",
        json!({ "path": path.display().to_string() }),
    );
    ok(
        &mut session,
        "write_script",
        json!({ "text": "function thing(self) end" }),
    );
    assert!(root.join("mine.luau").exists());
    let read = ok(&mut session, "read_script", json!({}));
    assert_eq!(read["text"], "function thing(self) end");
}

/// A prefab's line lands where it's placed, like the rest of it.
#[test]
fn a_prefabs_lines_move_with_it() {
    let root = content("prefab_line");
    let prefab = r#"
return {
    build = function(params)
        return { { block = "spline", name = "p", points = { 0, 0, 0, 16, 0, 0 } } }
    end,
}
"#;
    fs::write(root.join("prefabs").join("patrol.luau"), prefab).expect("write");
    let mut session = Session::new(&root);
    ok(
        &mut session,
        "place_prefab",
        json!({ "prefab": "patrol", "at": [100, 0, 0] }),
    );
    let text = block_of(&session, "spline");
    assert!(
        text.contains("100 0 0") && text.contains("116 0 0"),
        "{text}"
    );
}

/// What the content offers can be listed.
#[test]
fn the_content_can_be_listed() {
    let mut session = Session::new(&content("list"));
    let listed = ok(&mut session, "list", json!({}));
    assert_eq!(listed["prefabs"], json!(["stairs"]));
    assert_eq!(listed["meshes"], json!([]));
}

/// Every prefab that ships builds with what it assumes, and what's shared isn't
/// listed as one.
#[test]
fn every_prefab_builds() {
    let content = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let names = prefab_names(&content);
    assert!(names.len() > 40, "{names:?}");
    assert!(!names.iter().any(|name| name.starts_with('_')), "{names:?}");
    for name in names {
        let built = build_prefab(&content, &name, &json!({}), [0.0; 3]);
        let built = built.unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(!built.blocks.is_empty(), "{name}");
    }
}

/// A prefab can be turned to face any way, shape and all.
#[test]
fn a_prefab_turns_before_it_is_placed() {
    let content = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let at = [100.0, 200.0, 0.0];
    let params = json!({ "turn": 90, "material": "stone", "size": 64, "height": 32 });
    let built = build_prefab(&content, "lift", &params, at).expect("builds");
    let brush = &built.blocks[0].children[0];
    let box_of = brush.get("cuboid").and_then(engine_format::Value::numbers);
    assert_eq!(
        box_of.expect("a box"),
        vec![36.0, 200.0, 24.0, 100.0, 264.0, 32.0],
        "turned about the origin, then moved"
    );
}

/// A material gets maps from its own picture, and asking again changes nothing
/// about what it says. What flows is left alone.
#[test]
fn a_material_gets_maps_from_its_picture() {
    let root = content("derive");
    fs::create_dir_all(root.join("materials")).expect("directory");
    fs::create_dir_all(root.join("textures")).expect("directory");
    let mut pixels = Vec::new();
    for at in 0..64_u32 * 64 {
        let grey = if at % 64 < 32 { 60 } else { 200 };
        pixels.extend_from_slice(&[grey, grey, grey, 255]);
    }
    let image = engine_compile::encode_png(64, 64, &pixels).expect("encodes");
    fs::write(root.join("textures/wall.png"), &image).expect("writes");
    let wall = "format 1 material\n\nsurface {\n    base = textures/wall.png\n}\n";
    fs::write(root.join("materials/wall.emat"), wall).expect("writes");
    let flowing =
        "format 1 material\n\nsurface {\n    base = textures/wall.png\n    kind = liquid\n}\n";
    fs::write(root.join("materials/wall_pool.emat"), flowing).expect("writes");
    let mut session = Session::new(&root);
    let answer = run(&mut session, "derive_maps", &json!({ "prefix": "wall" })).expect("runs");
    assert_eq!(answer["derived"], json!(["wall"]));
    assert!(root.join("textures/wall_normal.png").exists());
    assert!(root.join("textures/wall_specular.png").exists());
    let text = fs::read_to_string(root.join("materials/wall.emat")).expect("reads");
    assert!(text.contains("normal = textures/wall_normal.png"), "{text}");
    run(&mut session, "derive_maps", &json!({ "material": "wall" })).expect("runs");
    let again = fs::read_to_string(root.join("materials/wall.emat")).expect("reads");
    assert_eq!(again, text);
    let pool = fs::read_to_string(root.join("materials/wall_pool.emat")).expect("reads");
    assert_eq!(pool, flowing);
}

/// Materials are checked against the texture standard, and what's right says
/// nothing.
#[test]
fn check_textures_says_whats_wrong() {
    let root = content("check_textures");
    fs::create_dir_all(root.join("materials")).expect("directory");
    fs::create_dir_all(root.join("textures")).expect("directory");
    let image = engine_compile::encode_png(64, 64, &[128; 64 * 64 * 4]).expect("encodes");
    fs::write(root.join("textures/wall.png"), &image).expect("writes");
    let good = "format 1 material\n\nsurface {\n    base = textures/wall.png\n    world_size = 64 64\n    repeat = both\n}\n";
    fs::write(root.join("materials/good.emat"), good).expect("writes");
    let bare = "format 1 material\n\nsurface {\n    base = textures/wall.png\n}\n";
    fs::write(root.join("materials/bare.emat"), bare).expect("writes");
    let mut session = Session::new(&root);
    let answer = run(&mut session, "check_textures", &json!({})).expect("runs");
    assert!(answer.get("good").is_none(), "{answer}");
    let problems = answer["bare"].to_string();
    assert!(problems.contains("world_size"), "{problems}");
    assert!(problems.contains("repeats"), "{problems}");
}
