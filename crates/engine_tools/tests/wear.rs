//! Making a level look used.

use std::path::PathBuf;

use engine_tools::{Session, run};
use serde_json::{Value, json};

/// A room 1024 wide and 256 high, with its inside from 0 to 1024 and 0 to 256.
fn room() -> Session {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let mut session = Session::new(&root);
    let walls = "qg03_elder_crypt_wall_blocks";
    for (cuboid, material) in [
        (
            [-32, -32, -32, 1056, 1056, 0],
            "qg03_elder_crypt_floor_flags",
        ),
        (
            [-32, -32, 256, 1056, 1056, 288],
            "qg03_elder_crypt_ceiling_coffer",
        ),
        ([-32, -32, 0, 0, 1056, 256], walls),
        ([1024, -32, 0, 1056, 1056, 256], walls),
        ([0, -32, 0, 1024, 0, 256], walls),
        ([0, 1024, 0, 1024, 1056, 256], walls),
    ] {
        let args = json!({ "cuboid": cuboid, "material": material });
        let _brush = run(&mut session, "add_brush", &args).expect("adds");
    }
    let start = json!({ "type": "info_player_start", "origin": [512, 512, 24] });
    let _start = run(&mut session, "add_entity", &start).expect("adds");
    session
}

/// The numbers after `name = [` in each block of `kind`.
fn lists(session: &Session, kind: &str, name: &str) -> Vec<Vec<f64>> {
    let text = session.text();
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if line.starts_with(kind) {
            inside = true;
        } else if line.starts_with('}') {
            inside = false;
        }
        let wanted = format!("{name} =");
        if inside && line.trim_start().starts_with(&wanted) {
            let (_, numbers) = line.split_once('=').expect("a value");
            out.push(
                numbers
                    .split(|c: char| !(c.is_ascii_digit() || "-.e".contains(c)))
                    .filter_map(|word| word.parse().ok())
                    .collect(),
            );
        }
    }
    out
}

/// Wears `session` and says how much it made.
fn wear(session: &mut Session, args: &Value) -> (u64, u64) {
    let answer = run(session, "wear", args).expect("wears");
    let count = |key: &str| answer[key].as_u64().expect("a count");
    (count("strokes"), count("decals"))
}

#[test]
fn wear_gathers_where_walls_meet_floors() {
    let mut session = room();
    let (strokes, _) = wear(&mut session, &json!({}));
    assert!(strokes > 4, "{strokes}");
    let lines = lists(&session, "paint", "points");
    let near_floor = lines
        .iter()
        .filter(|line| line.len() == 9 && line[2].abs() < 1.0 && line[8].abs() < 1.0)
        .count();
    assert!(near_floor > 4, "{near_floor} of {}", lines.len());
}

#[test]
fn a_decal_faces_into_the_room() {
    let mut session = room();
    let (_, decals) = wear(&mut session, &json!({ "seed": 3 }));
    assert!(decals > 0);
    let places = lists(&session, "decal", "origin");
    let ways = lists(&session, "decal", "normal");
    assert_eq!(places.len(), ways.len());
    for (place, way) in places.iter().zip(&ways) {
        // A short step out of a surface must go towards the middle of the room.
        let middle = [512.0, 512.0, 128.0];
        let towards: f64 = (0..3)
            .map(|axis| (middle[axis] - place[axis]) * way[axis])
            .sum();
        assert!(towards > 0.0, "{place:?} faces {way:?}");
    }
}

#[test]
fn wearing_again_replaces_what_it_made() {
    let mut session = room();
    let first = wear(&mut session, &json!({}));
    let again = wear(&mut session, &json!({}));
    assert_eq!(first, again);
    let count = lists(&session, "paint", "points").len();
    assert_eq!(count as u64, first.0, "only one lot");
}

#[test]
fn what_someone_painted_stays() {
    let mut session = room();
    let args = json!({ "points": [1, 2, 3], "radius": 9 });
    let _mine = run(&mut session, "paint", &args).expect("paints");
    let (strokes, _) = wear(&mut session, &json!({}));
    let _again = wear(&mut session, &json!({}));
    let count = lists(&session, "paint", "points").len() as u64;
    assert_eq!(count, strokes + 1);
}

#[test]
fn no_amount_is_no_wear() {
    let mut session = room();
    assert_eq!(wear(&mut session, &json!({ "amount": 0 })), (0, 0));
}

#[test]
fn a_seed_changes_it_and_the_same_seed_does_not() {
    let (mut one, mut two, mut other) = (room(), room(), room());
    let _one = wear(&mut one, &json!({ "seed": 5 }));
    let _two = wear(&mut two, &json!({ "seed": 5 }));
    let _other = wear(&mut other, &json!({ "seed": 6 }));
    let lines = |session: &Session| lists(session, "paint", "points");
    assert_eq!(lines(&one), lines(&two));
    assert_ne!(lines(&one), lines(&other));
}

#[test]
fn undo_takes_wear_away() {
    let mut session = room();
    let _worn = wear(&mut session, &json!({}));
    let _undone = run(&mut session, "undo", &json!({})).expect("undoes");
    assert!(lists(&session, "paint", "points").is_empty());
}

/// The outsides of a level's walls are real faces, and nobody sees them.
#[test]
fn nothing_goes_on_the_outside_of_a_level() {
    let mut session = room();
    let _worn = wear(&mut session, &json!({ "seed": 7 }));
    for place in lists(&session, "decal", "origin") {
        let inside = (0..2).all(|axis| (-1.0..=1025.0).contains(&place[axis]));
        assert!(inside && (-1.0..=257.0).contains(&place[2]), "{place:?}");
    }
}

#[test]
fn metal_stays_clean() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let mut session = Session::new(&root);
    let args =
        json!({ "cuboid": [0, 0, 0, 512, 512, 64], "material": "qg03_elder_crypt_trim_metal" });
    let _brush = run(&mut session, "add_brush", &args).expect("adds");
    assert_eq!(wear(&mut session, &json!({})), (0, 0));
}
