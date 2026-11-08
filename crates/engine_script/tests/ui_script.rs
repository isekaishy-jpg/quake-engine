//! The status bar and menus as a script.

use engine_script::{UiCommand, UiScore, UiScript, UiState};

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
            tracking: 0.0,
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
    let _opened = script.open_menu(&told, "").expect("opens");
    assert!(!script.draw_menu(&told).expect("draws").is_empty());
    assert!(!script.draw_status(&told).expect("draws").is_empty());
    for key in ["DOWNARROW", "ENTER", "RIGHTARROW", "ESCAPE"] {
        let _heard = script.menu_key(&told, key).expect("heard");
    }
}

/// The shipped script, on a screen of `size` units with a game going or not.
fn shipped(size: (f32, f32), in_game: bool) -> (UiScript, UiState) {
    let source = include_str!("../../../content/scripts/ui.luau");
    let script = UiScript::new("ui.luau", source).expect("runs");
    let mut told = state();
    told.width = size.0;
    told.height = size.1;
    told.in_game = in_game;
    told.saves = vec!["A level".to_owned(), String::new()];
    told.keys
        .insert("+attack".to_owned(), vec!["MOUSE1".to_owned()]);
    told.texts.insert("name".to_owned(), "player".to_owned());
    told.stats.insert("can_save".to_owned(), 1.0);
    (script, told)
}

/// The room a screen can have: the least, 4:3, ultrawide, and a tall window.
const SHAPES: [(f32, f32); 4] = [
    (640.0, 360.0),
    (640.0, 480.0),
    (853.0, 360.0),
    (640.0, 1138.0),
];

/// All the menus.
const SCREENS: [&str; 14] = [
    "",
    "new",
    "load",
    "save",
    "options",
    "video",
    "audio",
    "access",
    "keys",
    "multiplayer",
    "join",
    "host",
    "setup",
    "help",
];

/// Where a command draws, with text taken as a character's width, on the wide side.
fn extent(command: &UiCommand) -> Option<[f32; 4]> {
    match command {
        UiCommand::Frame { place, .. } | UiCommand::Piece { place, .. } => Some(*place),
        UiCommand::Text {
            at,
            size,
            tracking,
            text,
            align,
            ..
        } => {
            #[expect(clippy::cast_precision_loss, reason = "a line of text")]
            let width = text.chars().count() as f32 * (size * 0.6 + tracking);
            let left = at[0] - width * align;
            Some([left, at[1], left + width, at[1] + size])
        }
        _ => None,
    }
}

/// The words on a screen, once it's drawn.
fn words(commands: &[UiCommand]) -> Vec<String> {
    commands
        .iter()
        .filter_map(|command| match command {
            UiCommand::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// Everything fits on every screen, in every shape: no part or word leaves the screen, nothing
/// is cut short, and there's never a scroll bar.
#[test]
fn nothing_scrolls_or_overflows() {
    for size in SHAPES {
        for in_game in [false, true] {
            let (script, mut told) = shipped(size, in_game);
            // As long as save names get, in every slot, and two keys that take room.
            told.saves = vec!["The Slipgate Complex   kills:  12/120".to_owned(); 12];
            told.keys.insert(
                "+forward".to_owned(),
                vec!["MWHEELDOWN".to_owned(), "BACKSPACE".to_owned()],
            );
            for screen in SCREENS {
                let _opened = script.open_menu(&told, screen).expect(screen);
                for _row in 0..20 {
                    let drawn = script.draw_menu(&told).expect(screen);
                    for command in &drawn {
                        if let UiCommand::Frame { name, .. } = command {
                            assert!(!name.starts_with("scroll"), "{screen} scrolls");
                        }
                        if let UiCommand::Text { text, .. } = command {
                            assert!(!text.contains("..."), "{screen} cut {text}");
                        }
                        if let Some([left, top, right, bottom]) = extent(command) {
                            assert!(
                                left >= 0.0 && top >= 0.0 && right <= size.0 && bottom <= size.1,
                                "{screen} at {size:?} overflows with {command:?}"
                            );
                        }
                    }
                    let _heard = script.menu_key(&told, "DOWNARROW").expect(screen);
                }
            }
        }
    }
}

/// What's over the game fits too, at its fullest: a full server's scores, long messages, and
/// the long words at the end of a game.
#[test]
fn nothing_over_the_game_overflows() {
    let inside = |drawn: &[UiCommand], size: (f32, f32), what: &str| {
        for command in drawn {
            if let Some([left, top, right, bottom]) = extent(command) {
                assert!(
                    left >= 0.0 && top >= 0.0 && right <= size.0 && bottom <= size.1,
                    "{what} at {size:?} overflows with {command:?}"
                );
            }
        }
    };
    for size in SHAPES {
        let (script, mut told) = shipped(size, true);
        told.numbers.insert("playing".to_owned(), 1.0);
        told.numbers.insert("center_age".to_owned(), 1000.0);
        told.numbers.insert("center_left".to_owned(), 1.0);
        told.numbers.insert("game_time".to_owned(), 5000.0);
        told.numbers.insert("completed_time".to_owned(), 5000.0);
        for name in [
            "health", "armor", "ammo", "shells", "nails", "rockets", "cells",
        ] {
            told.stats.insert(name.to_owned(), 200.0);
        }
        told.messages = vec![("You got the very long named thing ".repeat(5), 0.0); 4];
        told.texts.insert("center".to_owned(), "A line of words that goes on and on and on, far past what one line can hold on a screen.\n".repeat(16));
        let drawn = script.draw_status(&told).expect("draws");
        inside(&drawn, size, "playing");
        told.texts.insert(
            "level_title".to_owned(),
            "A level with a name that goes on and on and on and on and on".to_owned(),
        );
        told.numbers.insert("show_scores".to_owned(), 1.0);
        let drawn = script.draw_status(&told).expect("draws");
        inside(&drawn, size, "the level's numbers");
        told.stats.insert("deathmatch".to_owned(), 1.0);
        told.scores = (0..16)
            .map(|player| UiScore {
                name: "a player with a long name".to_owned(),
                frags: -100 - player,
                own: player == 0,
            })
            .collect();
        let drawn = script.draw_status(&told).expect("draws");
        inside(&drawn, size, "scores");
        for ending in ["results", "finale", "cutscene"] {
            told.texts
                .insert("intermission".to_owned(), ending.to_owned());
            let drawn = script.draw_status(&told).expect("draws");
            inside(&drawn, size, ending);
        }
    }
}

/// All of a long list shows at once, with no row left for a scroll bar to reach.
#[test]
fn long_lists_show_every_row_at_once() {
    let (script, mut told) = shipped((640.0, 360.0), true);
    let _opened = script.open_menu(&told, "keys").expect("opens");
    let drawn = words(&script.draw_menu(&told).expect("draws"));
    for action in ["Attack", "Sidestep", "Keyboard look", "Swim down", "Back"] {
        assert!(drawn.iter().any(|word| word == action), "{action}");
    }
    told.saves = (0..12).map(|slot| format!("Game {slot}")).collect();
    let _opened = script.open_menu(&told, "load").expect("opens");
    let drawn = words(&script.draw_menu(&told).expect("draws"));
    for slot in 0..12 {
        let name = format!("GAME {slot}");
        assert!(drawn.contains(&name), "{name}");
    }
}

/// A keyboard reaches every page of settings, and both columns of a long list.
#[test]
fn a_keyboard_reaches_every_tab_and_column() {
    let (script, told) = shipped((640.0, 360.0), false);
    let _opened = script.open_menu(&told, "options").expect("opens");
    let _drawn = script.draw_menu(&told).expect("draws");
    for heading in ["Video", "Audio", "Controls", "Accessibility", "Game"] {
        let _heard = script.menu_key(&told, "PGDN").expect("heard");
        let drawn = words(&script.draw_menu(&told).expect("draws"));
        assert!(
            drawn.iter().filter(|word| *word == heading).count() >= 2,
            "{heading}"
        );
    }
    let _opened = script.open_menu(&told, "keys").expect("opens");
    let _drawn = script.draw_menu(&told).expect("draws");
    let _heard = script.menu_key(&told, "RIGHTARROW").expect("heard");
    let _heard = script.menu_key(&told, "ENTER").expect("heard");
    let drawn = words(&script.draw_menu(&told).expect("draws"));
    assert!(
        drawn.iter().any(|word| word == "Press a key for Run"),
        "{drawn:?}"
    );
}

/// Every screen, by keys and by the pointer, on screens of each shape. A script's error is
/// only seen on the screen that has it, so all of them are walked.
#[test]
fn every_screen_of_the_shipped_menus_works() {
    for size in SHAPES {
        for in_game in [false, true] {
            let (script, told) = shipped(size, in_game);
            for screen in SCREENS {
                let _opened = script.open_menu(&told, screen).expect(screen);
                assert!(
                    !script.draw_menu(&told).expect(screen).is_empty(),
                    "{screen}"
                );
                for key in [
                    "DOWNARROW",
                    "RIGHTARROW",
                    "LEFTARROW",
                    "UPARROW",
                    "HOME",
                    "END",
                ] {
                    let _heard = script.menu_key(&told, key).expect(key);
                    let _drawn = script.draw_menu(&told).expect(key);
                }
                for y in (0..360).step_by(30) {
                    #[expect(clippy::cast_precision_loss, reason = "a few hundred")]
                    let place = (size.0 * 0.6, y as f32);
                    let _moved = script.menu_pointer(&told, place, false).expect(screen);
                }
                let _drawn = script.draw_menu(&told).expect(screen);
            }
        }
    }
}

/// Keys go to what's chosen, and what's on a key now is said to have lost it.
#[test]
fn taking_a_key_from_another_action_says_so() {
    let (script, told) = shipped((640.0, 360.0), false);
    let _opened = script.open_menu(&told, "keys").expect("opens");
    let _down = script.menu_key(&told, "DOWNARROW").expect("moves");
    let _grab = script.menu_key(&told, "ENTER").expect("grabs");
    let commands = script.menu_key(&told, "MOUSE1").expect("binds");
    assert!(
        commands.iter().any(
            |command| matches!(command, UiCommand::Run(line) if line.contains("bind \"MOUSE1\""))
        ),
        "{commands:?}"
    );
    let drawn = script.draw_menu(&told).expect("draws");
    let said = drawn.iter().any(
        |command| matches!(command, UiCommand::Text { text, .. } if text.contains("was on Attack")),
    );
    assert!(said, "{drawn:?}");
}

#[test]
fn escape_while_grabbing_binds_nothing() {
    let (script, told) = shipped((640.0, 360.0), false);
    let _opened = script.open_menu(&told, "keys").expect("opens");
    let _grab = script.menu_key(&told, "ENTER").expect("grabs");
    let commands = script.menu_key(&told, "ESCAPE").expect("cancels");
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, UiCommand::Run(_)))
    );
}

#[test]
fn quitting_asks_first_and_no_is_what_is_chosen() {
    let (script, told) = shipped((640.0, 360.0), false);
    let _opened = script.open_menu(&told, "").expect("opens");
    let _last = script.menu_key(&told, "END").expect("moves");
    let asked = script.menu_key(&told, "ENTER").expect("asks");
    assert!(
        !asked
            .iter()
            .any(|command| matches!(command, UiCommand::Run(_)))
    );
    let answered = script.menu_key(&told, "ENTER").expect("answers");
    assert!(
        !answered
            .iter()
            .any(|command| matches!(command, UiCommand::Run(_)))
    );
}

#[test]
fn a_game_in_progress_resumes_first() {
    let (script, told) = shipped((640.0, 360.0), true);
    let _opened = script.open_menu(&told, "").expect("opens");
    let commands = script.menu_key(&told, "ENTER").expect("chooses");
    assert!(commands.contains(&UiCommand::Close), "{commands:?}");
}

/// The video screen on a tall window, so its rows are in one column, with `antialias` and
/// `msaa` as they're set, moved down to row `row`.
fn video_at(antialias: f64, msaa: f64, row: usize) -> (UiScript, UiState) {
    let (script, mut told) = shipped((640.0, 1138.0), false);
    told.settings.insert("r_antialias".to_owned(), antialias);
    told.settings.insert("r_msaa".to_owned(), msaa);
    let _opened = script.open_menu(&told, "video").expect("opens");
    for _ in 0..row {
        let _down = script.menu_key(&told, "DOWNARROW").expect("moves");
    }
    (script, told)
}

/// What a key on the video screen runs.
fn runs(script: &UiScript, told: &UiState, key: &str) -> Vec<String> {
    script
        .menu_key(told, key)
        .expect("heard")
        .into_iter()
        .filter_map(|command| match command {
            UiCommand::Run(line) => Some(line),
            _ => None,
        })
        .collect()
}

/// Anti-aliasing and multisampling are on the video screen by names a player knows, showing what
/// they're set to.
#[test]
fn anti_aliasing_is_in_the_options() {
    // In one column, a row's name is drawn and then what it's set to.
    let shows = |antialias: f64, msaa: f64, name: &str, value: &str| {
        let (script, told) = video_at(antialias, msaa, 0);
        let drawn = words(&script.draw_menu(&told).expect("draws"));
        let at = drawn.iter().position(|text| text == name);
        assert_eq!(
            at.and_then(|at| drawn.get(at + 1)).map(String::as_str),
            Some(value),
            "{drawn:?}"
        );
    };
    shows(2.0, 4.0, "Anti-aliasing", "SMAA");
    shows(3.0, 4.0, "Anti-aliasing", "TAA");
    shows(2.0, 4.0, "Multisampling", "4x");
    shows(0.0, 1.0, "Anti-aliasing", "Off");
    shows(0.0, 1.0, "Multisampling", "Off");
    let (script, mut told) = video_at(2.0, 1.0, 0);
    told.settings.insert("r_scale".to_owned(), 0.5);
    let drawn = words(&script.draw_menu(&told).expect("draws"));
    let at = drawn.iter().position(|text| text == "Render Scale");
    assert_eq!(
        at.and_then(|at| drawn.get(at + 1)).map(String::as_str),
        Some("50%")
    );
}

/// Turning them sets the setting to the next of its own values, going round.
#[test]
fn turning_anti_aliasing_sets_it() {
    let (script, told) = video_at(2.0, 1.0, 3);
    assert_eq!(runs(&script, &told, "RIGHTARROW"), vec!["r_antialias 3"]);
    let (script, told) = video_at(3.0, 1.0, 3);
    assert_eq!(runs(&script, &told, "RIGHTARROW"), vec!["r_antialias 0"]);
    let (script, told) = video_at(0.0, 1.0, 3);
    assert_eq!(runs(&script, &told, "LEFTARROW"), vec!["r_antialias 3"]);
    let (script, told) = video_at(2.0, 1.0, 4);
    assert_eq!(runs(&script, &told, "RIGHTARROW"), vec!["r_msaa 2"]);
    let (script, told) = video_at(2.0, 8.0, 4);
    assert_eq!(runs(&script, &told, "RIGHTARROW"), vec!["r_msaa 1"]);
    let (script, mut told) = video_at(2.0, 1.0, 5);
    told.settings.insert("r_scale".to_owned(), 1.0);
    assert_eq!(runs(&script, &told, "RIGHTARROW"), vec!["r_scale 1.5"]);
    let (script, told) = video_at(2.0, 3.0, 4);
    assert_eq!(
        runs(&script, &told, "RIGHTARROW"),
        vec!["r_msaa 2"],
        "a value it doesn't list counts as the first"
    );
}
