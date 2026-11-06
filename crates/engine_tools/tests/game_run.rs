//! Opening the game from the tools.

use std::path::PathBuf;

use engine_tools::{GameRunner, data_in};

/// A runner for a map called `mine`.
fn runner() -> GameRunner {
    GameRunner::new(
        PathBuf::from("game"),
        PathBuf::from("data"),
        PathBuf::from("content"),
        "mine".to_owned(),
    )
}

#[test]
fn playing_opens_on_the_map_with_sound() {
    let command = runner().play_command();
    let words: Vec<String> = command
        .get_args()
        .map(|word| word.to_string_lossy().into_owned())
        .collect();
    let map = words.iter().position(|word| word == "+map").expect("+map");
    assert_eq!(words[map + 1], "mine");
    assert!(words.contains(&"-window".to_owned()), "{words:?}");
    assert!(!words.contains(&"-nosound".to_owned()), "{words:?}");
}

#[test]
fn the_data_is_where_the_game_data_is() {
    let root = std::env::temp_dir().join(format!("engine-data-{}", std::process::id()));
    let (empty, full) = (root.join("empty"), root.join("full"));
    std::fs::create_dir_all(&empty).expect("folder");
    std::fs::create_dir_all(full.join("id1")).expect("folder");
    assert_eq!(data_in(&[&empty, &full]), None);
    std::fs::write(full.join("id1").join("pak0.pak"), b"").expect("file");
    assert_eq!(data_in(&[&empty, &full]), Some(full.clone()));
    std::fs::remove_dir_all(&root).expect("clean up");
}
