//! The status bar and menus as a script.

use engine_script::{UiCommand, UiScript, UiState};

/// A screen with a game going.
fn state() -> UiState {
    UiState {
        width: 640.0,
        height: 360.0,
        in_game: true,
        ..UiState::default()
    }
}

#[test]
fn a_script_says_what_to_draw() {
    let source = "function draw_status(state)
        ui.fill({ 0, 0, state.width, 10 }, { 1, 0, 0, 1 })
        ui.text('hello', { x = 5, y = 6, size = 12, align = 0.5 })
    end";
    let script = UiScript::new("test", source).expect("runs");
    assert!(script.draws_status());
    let commands = script.draw_status(&state()).expect("draws");
    assert_eq!(
        commands[0],
        UiCommand::Fill {
            place: [0.0, 0.0, 640.0, 10.0],
            color: [1.0, 0.0, 0.0, 1.0],
        }
    );
    assert_eq!(
        commands[1],
        UiCommand::Text {
            style: "regular".to_owned(),
            at: [5.0, 6.0],
            size: 12.0,
            text: "hello".to_owned(),
            color: [1.0; 4],
            align: 0.5,
        }
    );
}

#[test]
fn a_script_can_be_told_numbers() {
    let source = "function draw_status(state) ui.run(tostring(state.stats.health)) end";
    let script = UiScript::new("test", source).expect("runs");
    let mut told = state();
    told.stats.insert("health".to_owned(), 75.0);
    let commands = script.draw_status(&told).expect("draws");
    assert_eq!(commands, [UiCommand::Run("75".to_owned())]);
}

#[test]
fn a_menu_hears_keys_and_can_close() {
    let source = "function menu_key(state, key)
        if key == 'ESCAPE' then ui.close() end
    end";
    let script = UiScript::new("test", source).expect("runs");
    assert_eq!(script.menu_key(&state(), "A").expect("runs"), []);
    assert_eq!(
        script.menu_key(&state(), "ESCAPE").expect("runs"),
        [UiCommand::Close]
    );
}

#[test]
fn a_script_says_which_settings_it_wants() {
    let script = UiScript::new("test", "settings = { 'volume', 'gamma' }").expect("runs");
    assert_eq!(script.wanted_settings(), ["volume", "gamma"]);
}

#[test]
fn what_a_script_doesnt_have_does_nothing() {
    let script = UiScript::new("test", "").expect("runs");
    assert!(!script.draws_status());
    assert_eq!(script.draw_menu(&state()).expect("nothing"), []);
}

#[test]
fn a_broken_script_is_refused() {
    assert!(UiScript::new("test", "function (").is_err());
}

#[test]
fn a_script_has_no_reach_beyond_its_sandbox() {
    let source = "function draw_status(state) ui.run(tostring(os.remove)) end";
    let script = UiScript::new("test", source).expect("runs");
    let commands = script.draw_status(&state()).expect("draws");
    assert_eq!(commands, [UiCommand::Run("nil".to_owned())]);
}

#[test]
fn the_engines_own_script_runs() {
    let source = include_str!("../../../content/scripts/ui.luau");
    let script = UiScript::new("ui.luau", source).expect("runs");
    let mut told = state();
    told.stats.insert("health".to_owned(), 100.0);
    told.stats.insert("armor".to_owned(), 0.0);
    told.stats.insert("ammo".to_owned(), 25.0);
    let _opened = script.open_menu(&told).expect("opens");
    assert!(!script.draw_menu(&told).expect("draws").is_empty());
    assert!(!script.draw_status(&told).expect("draws").is_empty());
    for key in ["DOWNARROW", "ENTER", "RIGHTARROW", "ESCAPE"] {
        let _heard = script.menu_key(&told, key).expect("heard");
    }
}
