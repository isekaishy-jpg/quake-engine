//! The platform's operations, run headless.

use std::fs;
use std::path::{Path, PathBuf};

use engine_tools::{Session, run};
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
