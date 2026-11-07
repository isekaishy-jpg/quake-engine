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
    let root = content("features");
    fs::create_dir_all(root.join("meshes")).expect("directory");
    fs::write(root.join("meshes").join("statue.emesh"), "").expect("write");
    let mut session = Session::new(&root);
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

#[test]
fn check_shaders_says_whats_wrong() {
    let root = content("check_shaders");
    fs::create_dir_all(root.join("shaders")).expect("directory");
    let good = "fn shader_uv(uv: vec2<f32>, world: vec3<f32>) -> vec2<f32> { return uv * 2.0; }";
    fs::write(root.join("shaders/good.wgsl"), good).expect("writes");
    let post = "fn shader_post(color: vec3<f32>, uv: vec2<f32>) -> vec3<f32> { return color; }";
    fs::write(root.join("shaders/post.wgsl"), post).expect("writes");
    fs::write(root.join("shaders/bad.wgsl"), "fn shader_uv(").expect("writes");
    let mut session = Session::new(&root);
    let answer = run(&mut session, "check_shaders", &json!({})).expect("runs");
    assert!(answer.get("good").is_none(), "{answer}");
    assert!(answer.get("post").is_none(), "{answer}");
    assert!(answer.get("bad").is_some(), "{answer}");
}

#[test]
fn the_shaders_that_come_with_the_engine_compile() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let mut session = Session::new(&root);
    let answer = run(&mut session, "check_shaders", &json!({})).expect("runs");
    assert_eq!(answer, json!({}));
}

#[test]
fn the_effects_that_come_with_the_engine_make_sense() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let mut session = Session::new(&root);
    let answer = run(&mut session, "check_effects", &json!({})).expect("runs");
    assert_eq!(answer, json!({}));
}

/// An empty map in a temporary content folder.
fn fresh(name: &str) -> Session {
    let folder = std::env::temp_dir().join(format!("checks_{name}_{}", std::process::id()));
    fs::create_dir_all(&folder).expect("a folder");
    Session::new(&folder)
}

/// What `validate` finds, as text.
fn problems(session: &mut Session) -> String {
    let answer = run(session, "validate", &json!({})).expect("it answers");
    answer["problems"].to_string()
}

#[test]
fn validating_finds_what_compiling_lets_through() {
    let mut session = fresh("problems");
    ok(
        &mut session,
        "add_entity",
        json!({ "type": "effect", "effect": "none", "fields": { "effect": "none" } }),
    );
    ok(
        &mut session,
        "add_entity",
        json!({ "type": "zone", "fields": { "title": "Hall", "music": "music/none.ogg" } }),
    );
    ok(
        &mut session,
        "set_block",
        json!({ "kind": "fog", "fields": { "densty": 0.01 } }),
    );
    let found = problems(&mut session);
    assert!(found.contains("no effect called none"), "{found}");
    assert!(found.contains("no shape"), "{found}");
    assert!(found.contains("no music"), "{found}");
    assert!(found.contains("no field densty"), "{found}");
    assert!(found.contains("nowhere to start"), "{found}");
}

#[test]
fn what_a_map_cant_hold_is_refused_at_once() {
    let mut session = fresh("refused");
    assert!(run(&mut session, "set_block", &json!({ "kind": "nonsense" })).is_err());
    assert!(run(&mut session, "place_mesh", &json!({ "source": "none" })).is_err());
    assert!(
        run(
            &mut session,
            "add_brush",
            &json!({ "cuboid": [8, 8, 8, 0, 0, 0] })
        )
        .is_err()
    );
}

#[test]
fn undo_goes_back_through_changes_of_every_kind() {
    let mut session = fresh("undo");
    let first = session.text();
    ok(
        &mut session,
        "add_brush",
        json!({ "cuboid": [0, 0, 0, 8, 8, 8] }),
    );
    let one = session.text();
    ok(
        &mut session,
        "add_brush",
        json!({ "cuboid": [16, 0, 0, 24, 8, 8] }),
    );
    let two = session.text();
    ok(
        &mut session,
        "set_field",
        json!({ "id": 1, "key": "material", "value": "stone" }),
    );
    let three = session.text();
    ok(&mut session, "delete", json!({ "id": 1 }));
    let four = session.text();
    for expected in [&three, &two, &one, &first] {
        assert_eq!(ok(&mut session, "undo", json!({}))["changed"], true);
        assert_eq!(&session.text(), expected);
    }
    assert_eq!(ok(&mut session, "undo", json!({}))["changed"], false);
    for expected in [&one, &two, &three, &four] {
        assert_eq!(ok(&mut session, "redo", json!({}))["changed"], true);
        assert_eq!(&session.text(), expected);
    }
}

#[test]
fn a_change_after_undoing_ends_what_could_be_redone() {
    let mut session = fresh("redo");
    ok(
        &mut session,
        "add_brush",
        json!({ "cuboid": [0, 0, 0, 8, 8, 8] }),
    );
    ok(&mut session, "undo", json!({}));
    ok(
        &mut session,
        "add_brush",
        json!({ "cuboid": [16, 0, 0, 24, 8, 8] }),
    );
    ok(&mut session, "undo", json!({}));
    ok(&mut session, "redo", json!({}));
    assert_eq!(ok(&mut session, "redo", json!({}))["changed"], false);
}

#[test]
fn describing_can_be_narrowed() {
    let mut session = fresh("describe");
    for index in 0..250 {
        let x = f64::from(index) * 32.0;
        ok(
            &mut session,
            "add_entity",
            json!({ "type": "light", "origin": [x, 0, 0] }),
        );
    }
    ok(
        &mut session,
        "add_entity",
        json!({ "type": "info_player_start", "origin": [0, 0, 0] }),
    );
    let all = ok(&mut session, "describe", json!({}));
    assert_eq!(all["matching"], 251);
    assert_eq!(all["things"].as_array().expect("things").len(), 200);
    let start = ok(
        &mut session,
        "describe",
        json!({ "type": "info_player_start" }),
    );
    assert_eq!(start["matching"], 1);
    let near = ok(
        &mut session,
        "describe",
        json!({ "near": [0, 0, 0], "radius": 40 }),
    );
    assert_eq!(near["matching"], 3);
    let everything = ok(&mut session, "describe", json!({ "limit": 0 }));
    assert_eq!(everything["things"].as_array().expect("things").len(), 251);
}

#[test]
fn a_broken_shader_or_effect_is_refused_and_a_good_one_written() {
    let mut session = fresh("writes");
    assert!(
        run(
            &mut session,
            "write_shader",
            &json!({ "name": "bad", "text": "nonsense" })
        )
        .is_err()
    );
    let code = "fn shader_uv(uv: vec2<f32>, world: vec3<f32>) -> vec2<f32> { return uv; }";
    ok(
        &mut session,
        "write_shader",
        json!({ "name": "good", "text": code }),
    );
    assert_eq!(
        ok(&mut session, "read_shader", json!({ "name": "good" }))["text"],
        code
    );
    assert!(
        run(
            &mut session,
            "write_effect",
            &json!({ "name": "bad", "text": "{" })
        )
        .is_err()
    );
    assert!(run(&mut session, "read_effect", &json!({ "name": "bad" })).is_err());
}

/// The land's lowest and highest, as the last change to it said.
fn heights(told: &Value) -> (f64, f64) {
    (
        told["low"].as_f64().expect("low"),
        told["high"].as_f64().expect("high"),
    )
}

#[test]
fn land_is_shaped_stamped_and_copied_in_its_file() {
    let root = content("land");
    let mut session = Session::new(&root);
    ok(
        &mut session,
        "make_terrain",
        json!({ "name": "hills", "chunks": 1, "hills": 0 }),
    );
    let brush = |tool: &str, amount: f64| json!({ "name": "hills", "tool": tool, "amount": amount, "at": [1000, 1000], "radius": 200 });
    let raised = ok(&mut session, "shape_terrain", brush("raise", 100.0));
    assert_eq!(heights(&raised), (0.0, 100.0));
    let lowered = ok(&mut session, "shape_terrain", brush("lower", 40.0));
    assert_eq!(heights(&lowered), (0.0, 60.0), "it was kept, and goes down");
    let flat = ok(&mut session, "shape_terrain", brush("flatten", 10.0));
    assert!(heights(&flat).1 < 60.0, "{flat}");
    let refused = run(
        &mut session,
        "shape_terrain",
        &json!({ "name": "hills", "tool": "flatten", "at": [0, 0], "radius": 10 }),
    );
    assert!(refused.is_err(), "a height isn't guessed");
    let wrong = run(
        &mut session,
        "shape_terrain",
        &json!({ "name": "hills", "tool": "dig", "at": [0, 0], "radius": 10 }),
    );
    assert!(wrong.is_err(), "and there's no such tool");

    ok(
        &mut session,
        "make_terrain",
        json!({ "name": "lump", "chunks": 1, "hills": 0 }),
    );
    ok(
        &mut session,
        "shape_terrain",
        json!({ "name": "lump", "tool": "raise", "amount": 300, "at": [1024, 1024], "radius": 900, "soft": 0 }),
    );
    let stamped = ok(
        &mut session,
        "stamp_terrain",
        json!({ "name": "hills", "stamp": "lump", "at": [500, 500], "radius": 300 }),
    );
    assert!(heights(&stamped).1 > 200.0, "{stamped}");
    let moved = ok(
        &mut session,
        "copy_terrain",
        json!({ "name": "hills", "from": [1000, 1000], "at": [500, 500], "radius": 300, "soft": 0 }),
    );
    assert!(
        heights(&moved).1 < 200.0,
        "the stamp's copied over: {moved}"
    );

    let unknown = run(
        &mut session,
        "add_terrain",
        &json!({ "source": "nothing", "material": "moss" }),
    );
    assert!(unknown.is_err(), "land that isn't there can't be added");
    ok(
        &mut session,
        "add_terrain",
        json!({ "source": "hills", "material": "moss" }),
    );
    assert_eq!(count(&mut session), 1);
}

#[test]
fn land_is_painted_in_its_file() {
    let root = content("paint");
    let mut session = Session::new(&root);
    ok(
        &mut session,
        "make_terrain",
        json!({ "name": "hills", "chunks": 1, "hills": 0 }),
    );
    let paint = json!({ "name": "hills", "layer": 0, "at": [100, 100], "radius": 300 });
    assert!(
        run(&mut session, "paint_terrain", &paint).is_err(),
        "there's no layer yet"
    );
    let skipping = json!({ "name": "hills", "layer": 1, "material": "moss" });
    assert!(
        run(&mut session, "set_terrain_layer", &skipping).is_err(),
        "layers go in order"
    );
    for (layer, material) in ["rock", "moss"].iter().enumerate() {
        ok(
            &mut session,
            "set_terrain_layer",
            json!({ "name": "hills", "layer": layer, "material": material, "size": 300, "hex": layer == 1 }),
        );
    }
    ok(
        &mut session,
        "paint_terrain",
        json!({ "name": "hills", "layer": 1, "at": [100, 100], "radius": 300, "soft": 0 }),
    );
    let bytes = fs::read(root.join("terrain").join("hills.eterrain")).expect("land");
    let data = engine_compile::read_terrain_data(&bytes).expect("reads");
    assert_eq!(data.layers.len(), 2);
    assert_eq!(data.layers[1].material, "moss");
    assert!((data.layers[1].size - 300.0).abs() < 0.01);
    assert!(data.layers[1].hex && !data.layers[0].hex);
    let at = 3 * data.across() as usize + 3;
    assert_eq!(data.paint[at][..2], [0, 255], "painted there");
    assert_eq!(
        *data.paint.last().expect("paint"),
        [255, 0, 0, 0, 0, 0, 0, 0]
    );
    ok(
        &mut session,
        "add_terrain",
        json!({ "source": "hills", "material": "rock" }),
    );
    let told = problems(&mut session);
    assert!(told.contains("land has no material moss"), "{told}");
}

/// Time through a sine, into a surface's own light.
const WAVE: &str = "format 1 graph
node { id = time kind = time }
node { id = wave kind = sin a = time }
outputs { emissive = wave }
";

#[test]
fn a_graph_is_written_and_says_what_it_comes_to() {
    let root = content("graph");
    let mut session = Session::new(&root);
    ok(
        &mut session,
        "write_graph",
        json!({ "name": "wave", "text": WAVE }),
    );
    let told = ok(&mut session, "describe_graph", json!({ "name": "wave" }));
    assert_eq!(told["ok"], true, "{told}");
    let code = told["code"].as_str().expect("code");
    assert!(code.contains("fn shader_surface"), "{code}");
    let checked = ok(&mut session, "check_shaders", json!({}));
    assert_eq!(checked, json!({}), "{checked}");
}

#[test]
fn a_wrong_graph_is_refused_with_what_is_wrong() {
    let root = content("wrong-graph");
    let mut session = Session::new(&root);
    let wrong = WAVE.replace("a = time", "a = nothing");
    let refused = run(
        &mut session,
        "write_graph",
        &json!({ "name": "wave", "text": wrong }),
    );
    let told = refused.expect_err("it's refused");
    assert!(told.contains("wave reads nothing"), "{told}");
    assert!(!root.join("shaders/wave.egraph").exists());
    // One put there by hand is found.
    fs::create_dir_all(root.join("shaders")).expect("folder");
    fs::write(root.join("shaders/wave.egraph"), &wrong).expect("write");
    let checked = ok(&mut session, "check_shaders", json!({}));
    assert!(checked["wave"].as_str().is_some(), "{checked}");
    let described = ok(&mut session, "describe_graph", json!({ "name": "wave" }));
    assert_eq!(described["ok"], false);
}

#[test]
fn the_kinds_of_node_are_listed() {
    let root = content("kinds");
    let mut session = Session::new(&root);
    let told = ok(&mut session, "graph_kinds", json!({}));
    let kinds = told["kinds"].as_array().expect("kinds");
    assert!(kinds.iter().any(|kind| kind["name"] == "mix"));
    assert!(told["outputs"].as_array().expect("outputs").len() >= 6);
}

#[test]
fn a_field_needs_a_shader_that_is_there_and_right() {
    let root = content("field");
    let mut session = Session::new(&root);
    let field = json!({ "name": "wet", "shader": "rain", "size": 64 });
    assert!(run(&mut session, "add_field", &field).is_err(), "no shader");
    let bad = "fn shader_field(at: vec2<i32>, size: vec2<i32>) -> vec4<f32> { return 1; }";
    let refused = run(
        &mut session,
        "write_shader",
        &json!({ "name": "rain", "text": bad }),
    );
    assert!(refused.is_err(), "it's checked as a field");
    let good = bad.replace("return 1;", "return field_read(at);");
    ok(
        &mut session,
        "write_shader",
        json!({ "name": "rain", "text": good }),
    );
    ok(&mut session, "add_field", field);
    assert_eq!(count(&mut session), 1);
}
