//! Command lines the engine refuses. These stop before a window opens.

use std::process::{Command, Output};

/// Runs the engine with `arguments`.
fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_quake-engine"))
        .args(arguments)
        .output()
        .expect("the engine starts")
}

/// Asserts that the engine failed and said `message`.
fn assert_refused(arguments: &[&str], message: &str) {
    let output = run(arguments);
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{arguments:?}");
    assert!(error.contains(message), "{error}");
}

#[test]
fn a_base_directory_is_required() {
    assert_refused(&[], "usage:");
}

#[test]
fn options_need_their_values() {
    assert_refused(&["-basedir"], "-basedir needs a path");
    assert_refused(&["-game"], "-game needs a name");
}

/// Getting as far as asking for a base directory means it was understood.
#[test]
fn window_is_an_option() {
    assert_refused(&["-window"], "usage:");
}

#[test]
fn unknown_options_are_refused() {
    assert_refused(&["-nosuch"], "unknown option: -nosuch");
}

#[test]
fn a_missing_game_directory_is_reported() {
    assert_refused(&["-basedir", "no-such-directory-here"], "file not found");
}
