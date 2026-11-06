//! Making a level as a floor plan.

use std::path::PathBuf;

use engine_tools::{Session, run};
use serde_json::{Value, json};

/// A session with nothing in it.
fn session() -> Session {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content");
    Session::new(&root)
}

/// Paints `cells`.
fn paint(session: &mut Session, cells: &[[i32; 2]], kind: &str) -> Value {
    run(
        session,
        "plan_paint",
        &json!({ "cells": cells, "kind": kind }),
    )
    .expect("paints")
}

/// Puts `thing` on `cell`.
fn put(session: &mut Session, cell: [i32; 2], thing: &str) -> Value {
    run(
        session,
        "plan_thing",
        &json!({ "cells": [cell], "thing": thing }),
    )
    .expect("puts")
}

/// What's in the map as text.
fn text(session: &Session) -> String {
    session.text()
}

/// A room of two squares, ready to play.
fn small_level() -> Session {
    let mut session = session();
    let _room = paint(&mut session, &[[0, 0], [1, 0]], "floor");
    let _start = put(&mut session, [0, 0], "start");
    let _light = put(&mut session, [1, 0], "light");
    session
}

#[test]
fn an_empty_plan_says_what_to_do_first() {
    let mut session = session();
    let plan = run(&mut session, "plan_read", &json!({})).expect("reads");
    let missing = plan["missing"].to_string();
    assert!(missing.contains("Paint a room"), "{missing}");
    assert!(missing.contains("where you start"), "{missing}");
    assert!(missing.contains("light"), "{missing}");
}

#[test]
fn a_finished_plan_needs_nothing_more() {
    let mut session = small_level();
    let plan = run(&mut session, "plan_read", &json!({})).expect("reads");
    assert_eq!(plan["missing"], json!([]));
}

#[test]
fn what_a_plan_makes_compiles() {
    let mut session = small_level();
    let answer = run(&mut session, "validate", &json!({})).expect("validates");
    assert_eq!(answer["missing_materials"], json!([]), "{answer}");
}

#[test]
fn squares_side_by_side_share_a_floor() {
    let mut session = session();
    let _room = paint(&mut session, &[[0, 0], [1, 0], [2, 0]], "floor");
    let floors = text(&session).matches("floor_flags").count();
    assert_eq!(floors, 1, "one long brush");
}

#[test]
fn a_wall_only_goes_where_a_room_ends() {
    let mut one = session();
    let _room = paint(&mut one, &[[0, 0]], "floor");
    assert_eq!(text(&one).matches("wall_blocks").count(), 4);
    let mut two = session();
    let _room = paint(&mut two, &[[0, 0], [1, 0]], "floor");
    assert_eq!(
        text(&two).matches("wall_blocks").count(),
        6,
        "none between them"
    );
}

#[test]
fn clearing_a_square_takes_what_was_on_it() {
    let mut session = small_level();
    let plan = paint(&mut session, &[[1, 0]], "none");
    assert_eq!(plan["floor"], json!([[0, 0]]));
    assert!(plan["missing"].to_string().contains("light"));
    assert!(!text(&session).contains("\"light\""));
}

#[test]
fn there_is_only_one_start() {
    let mut session = small_level();
    let _moved = put(&mut session, [1, 0], "start");
    assert_eq!(text(&session).matches("info_player_start").count(), 1);
}

#[test]
fn a_thing_needs_a_floor() {
    let mut session = session();
    let args = json!({ "cells": [[5, 5]], "thing": "light" });
    let error = run(&mut session, "plan_thing", &args).expect_err("refused");
    assert!(error.contains("no floor"), "{error}");
}

#[test]
fn a_pool_has_water_in_it() {
    let mut session = session();
    let _room = paint(&mut session, &[[0, 0]], "floor");
    let _pool = paint(&mut session, &[[1, 0]], "water");
    assert!(text(&session).contains("contents = water") || text(&session).contains("\"water\""));
    let answer = run(&mut session, "validate", &json!({})).expect("validates");
    assert_eq!(answer["missing_materials"], json!([]), "{answer}");
}

#[test]
fn a_door_blocks_the_way_through_and_opens() {
    let mut session = session();
    let _room = paint(&mut session, &[[0, 0], [1, 0], [2, 0]], "floor");
    let _door = put(&mut session, [1, 0], "door");
    assert!(text(&session).contains("func_door"));
    let answer = run(&mut session, "validate", &json!({})).expect("validates");
    assert_eq!(answer["models"], json!(2), "a door's a model of its own");
    assert_eq!(answer["missing_materials"], json!([]), "{answer}");
}

#[test]
fn a_plan_comes_back_from_the_map() {
    let mut session = small_level();
    let dir = std::env::temp_dir().join(format!("engine-plan-{}", std::process::id()));
    let path = dir.join("level.emap");
    let args = json!({ "path": path.display().to_string() });
    let _saved = run(&mut session, "save", &args).expect("saves");
    let _opened = run(&mut session, "open", &args).expect("opens");
    let plan = run(&mut session, "plan_read", &json!({})).expect("reads");
    assert_eq!(plan["floor"], json!([[0, 0], [1, 0]]));
    assert_eq!(plan["missing"], json!([]));
    std::fs::remove_dir_all(&dir).expect("clean up");
}

#[test]
fn undo_takes_a_stroke_back() {
    let mut session = small_level();
    let _more = paint(&mut session, &[[2, 0]], "floor");
    let _undone = run(&mut session, "undo", &json!({})).expect("undoes");
    let plan = run(&mut session, "plan_read", &json!({})).expect("reads");
    assert_eq!(plan["floor"], json!([[0, 0], [1, 0]]));
}

#[test]
fn a_plan_can_change_its_look() {
    let mut session = small_level();
    let args = json!({ "look": "qg01_ashen_abbey" });
    let _changed = run(&mut session, "plan_look", &args).expect("changes");
    assert!(text(&session).contains("qg01_ashen_abbey_floor_flags"));
    assert!(!text(&session).contains("uc01_blight_sewers_floor_flags"));
}

#[test]
fn what_someone_built_by_hand_is_left_alone() {
    let mut session = session();
    let args = json!({ "cuboid": [500, 500, 0, 564, 564, 64], "material": "mine" });
    let _brush = run(&mut session, "add_brush", &args).expect("adds");
    let _room = paint(&mut session, &[[0, 0]], "floor");
    let _again = paint(&mut session, &[[0, 1]], "floor");
    assert_eq!(
        text(&session).matches("\"mine\"").count() + text(&session).matches("= mine").count(),
        1
    );
}
