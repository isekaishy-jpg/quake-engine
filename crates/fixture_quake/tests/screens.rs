//! The status bar and the menus, measured by what they ask to draw.

use std::collections::HashMap;

use engine_core::canvas::{Canvas, Picture, PictureSource, Quad};
use fixture_quake::{Hud, Intermission, Menu, MenuHost, SAVE_SLOTS, Score, Screen};

/// Pictures of 16 by 16, each with its own number, remembering what was asked.
#[derive(Default)]
struct Names {
    /// Every name, in order of first asking.
    seen: Vec<String>,
}

impl PictureSource for Names {
    fn picture(&mut self, name: &str) -> Option<Picture> {
        let found = self.seen.iter().position(|seen| seen == name);
        let number = found.unwrap_or_else(|| {
            self.seen.push(name.to_owned());
            self.seen.len() - 1
        });
        Some(Picture {
            number: u32::try_from(number).expect("small") + 1,
            width: 16,
            height: 16,
        })
    }
}

/// What drawing `draw` asks for: the names used, and every quad.
fn drawn(draw: impl FnOnce(&mut Canvas<'_>)) -> (Vec<String>, Vec<Quad>) {
    let mut names = Names::default();
    let mut canvas = Canvas::new(&mut names, (640, 400), 1);
    draw(&mut canvas);
    let quads = canvas.into_quads();
    (names.seen, quads)
}

/// Stats for a player with `health`.
fn stats(health: i32, items: u32) -> [i32; 16] {
    let mut stats = [0; 16];
    stats[0] = health;
    stats[3] = 25;
    stats[6] = 25;
    stats[10] = 1;
    stats[15] = i32::from_ne_bytes(items.to_ne_bytes());
    stats
}

/// What the status bar shows.
fn bar(hud: &Hud) -> Vec<String> {
    drawn(|canvas| hud.draw(canvas, true)).0
}

/// Whether `names` has `name`.
fn has(names: &[String], name: &str) -> bool {
    names.iter().any(|seen| seen == name)
}

#[test]
fn the_bar_shows_the_face_for_the_health_left() {
    let mut hud = Hud::new();
    for (health, face) in [
        (100, "face1"),
        (85, "face1"),
        (65, "face2"),
        (45, "face3"),
        (5, "face5"),
    ] {
        hud.set_stats(&stats(health, 0), 100.0);
        assert!(has(&bar(&hud), face), "{health}");
    }
}

#[test]
fn being_hurt_pains_the_face_then_it_passes() {
    let mut hud = Hud::new();
    hud.set_stats(&stats(100, 0), 10.0);
    hud.damage();
    assert!(has(&bar(&hud), "face_p1"));
    hud.set_stats(&stats(100, 0), 10.3);
    assert!(has(&bar(&hud), "face1"));
}

#[test]
fn a_low_number_turns_to_the_warning_colour() {
    let mut hud = Hud::new();
    hud.set_stats(&stats(100, 0), 1.0);
    assert!(has(&bar(&hud), "num_1"));
    hud.set_stats(&stats(20, 0), 1.0);
    assert!(has(&bar(&hud), "anum_2"));
}

#[test]
fn a_weapon_flashes_when_picked_up_then_settles() {
    let shotgun = 1;
    let mut hud = Hud::new();
    hud.set_stats(&stats(100, shotgun), 5.0);
    assert!(bar(&hud).iter().any(|name| name.starts_with("inva")));
    hud.set_stats(&stats(100, shotgun), 7.0);
    let names = bar(&hud);
    assert!(has(&names, "inv2_shotgun"), "it's the one in use");
    assert!(!names.iter().any(|name| name.starts_with("inva")));
}

#[test]
fn powerups_and_sigils_show_on_the_bar() {
    let (quad, first_sigil) = (1 << 22, 1 << 28);
    let mut hud = Hud::new();
    hud.set_stats(&stats(100, quad | first_sigil), 1.0);
    let names = bar(&hud);
    assert!(has(&names, "sb_quad"));
    assert!(has(&names, "sb_sigil1"));
    assert!(has(&names, "face_quad"));
}

#[test]
fn the_dead_see_the_scores() {
    let mut hud = Hud::new();
    hud.set_stats(&stats(0, 0), 1.0);
    assert!(hud.is_dead());
    let names = bar(&hud);
    assert!(has(&names, "scorebar"));
    assert!(!has(&names, "sbar"));
}

/// Stats for a healthy player, with `deathmatch` set.
fn deathmatch_stats(deathmatch: i32) -> Vec<i32> {
    let mut numbers = stats(100, 0).to_vec();
    numbers.push(deathmatch);
    numbers
}

/// Eight players with their frags in order, the one at `own` being us.
fn eight_players(own: usize) -> Vec<Score> {
    (0..8)
        .map(|place| Score {
            name: format!("p{place}"),
            frags: 8 - i32::try_from(place).expect("small"),
            colors: 0,
            own: place == own,
        })
        .collect()
}

/// How many lines of scores are beside the bar, with `deathmatch` set, on a
/// screen `width` wide.
fn lines_beside_the_bar(hud: &Hud, width: u32) -> usize {
    let mut names = Names::default();
    let mut canvas = Canvas::new(&mut names, (width, 400), 1);
    hud.draw(&mut canvas, true);
    let beside = ((width - 320) / 2 + 324) as f32;
    let quads = canvas.into_quads();
    let colours = quads.iter().filter(|quad| {
        quad.place[0] == beside && quad.place[2] == beside + 40.0 && quad.place[3] <= 400.0
    });
    colours.count() / 2
}

#[test]
fn deathmatch_shows_scores_beside_the_bar() {
    let mut hud = Hud::new();
    hud.set_stats(&deathmatch_stats(1), 1.0);
    hud.set_scores(eight_players(7));
    assert_eq!(lines_beside_the_bar(&hud, 640), 6, "as many as fit");
    assert_eq!(lines_beside_the_bar(&hud, 480), 0, "if there's room");
    hud.set_stats(&deathmatch_stats(0), 1.0);
    assert_eq!(lines_beside_the_bar(&hud, 640), 0, "and only in deathmatch");
}

#[test]
fn the_scores_beside_the_bar_stay_around_you() {
    for own in 0..8 {
        let mut hud = Hud::new();
        hud.set_stats(&deathmatch_stats(1), 1.0);
        hud.set_scores(eight_players(own));
        let mut names = Names::default();
        let mut canvas = Canvas::new(&mut names, (640, 400), 1);
        hud.draw(&mut canvas, true);
        // The marks either side of your frags.
        let marked = canvas
            .into_quads()
            .iter()
            .filter(|quad| quad.place[0] == 484.0 && quad.place[2] == 492.0)
            .count();
        assert_eq!(marked, 1, "you're always there, in place {own}");
    }
}

#[test]
fn the_results_replace_the_bar() {
    let mut hud = Hud::new();
    hud.set_stats(&stats(100, 0), 75.0);
    hud.set_intermission(Intermission::Results);
    let names = bar(&hud);
    assert!(has(&names, "gfx/complete.lmp"));
    assert!(!has(&names, "sbar"));
}

#[test]
fn a_menu_over_the_results_hides_them() {
    let mut hud = Hud::new();
    hud.set_stats(&stats(100, 0), 75.0);
    hud.set_intermission(Intermission::Results);
    let names = drawn(|canvas| hud.draw(canvas, false)).0;
    assert!(!has(&names, "gfx/complete.lmp"));
}

#[test]
fn center_text_goes_after_two_seconds() {
    let mut hud = Hud::new();
    hud.set_stats(&stats(100, 0), 10.0);
    hud.center_print("Hi".to_owned());
    let count = |hud: &Hud| drawn(|canvas| hud.draw(canvas, true)).1.len();
    let with = count(&hud);
    hud.set_stats(&stats(100, 0), 12.5);
    assert_eq!(count(&hud), with - 2);
}

#[test]
fn the_finale_types_its_text_out() {
    let mut hud = Hud::new();
    hud.set_stats(&stats(100, 0), 10.0);
    hud.set_intermission(Intermission::Finale);
    hud.center_print("Long text\nover lines".to_owned());
    let count = |hud: &Hud| drawn(|canvas| hud.draw(canvas, true)).1.len();
    let start = count(&hud);
    hud.set_stats(&stats(100, 0), 10.5);
    assert_eq!(count(&hud), start + 4, "eight characters a second");
    hud.set_stats(&stats(100, 0), 100.0);
    assert_eq!(count(&hud), 1 + 19, "the title and all of it");
}

#[test]
fn a_powerup_says_what_colour_it_puts_over_the_view() {
    let mut hud = Hud::new();
    hud.set_stats(&stats(100, 0), 0.0);
    assert_eq!(hud.powerup_tint()[3], 0.0);
    hud.set_stats(&stats(100, 1 << 22), 0.0);
    let tint = hud.powerup_tint();
    assert!(tint[2] > 0.9 && tint[3] > 0.0, "{tint:?}");
}

/// A pretend engine.
#[derive(Default)]
struct Host {
    /// Settings.
    settings: HashMap<String, f32>,
    /// Commands run.
    commands: Vec<String>,
    /// Whether a game is running.
    in_game: bool,
    /// Whether it can be saved.
    can_save: bool,
    /// The first save slot's description.
    first_save: Option<String>,
}

impl MenuHost for Host {
    fn setting(&self, name: &str) -> f32 {
        self.settings.get(name).copied().unwrap_or(0.0)
    }

    fn set_setting(&mut self, name: &str, value: f32) {
        self.settings.insert(name.to_owned(), value);
    }

    fn run(&mut self, command: &str) {
        self.commands.push(command.to_owned());
    }

    fn keys_for(&self, _command: &str) -> Vec<String> {
        vec!["A".to_owned(), "B".to_owned()]
    }

    fn play(&mut self, _sound: &str) {}

    fn saves(&self) -> Vec<Option<String>> {
        let mut saves = vec![None; SAVE_SLOTS];
        saves[0].clone_from(&self.first_save);
        saves
    }

    fn in_game(&self) -> bool {
        self.in_game
    }

    fn can_save(&self) -> bool {
        self.can_save
    }

    fn name(&self) -> String {
        "tester".to_owned()
    }
}

/// Presses `keys` in turn.
fn press(menu: &mut Menu, host: &mut Host, keys: &[&str]) {
    for key in keys {
        let text = match *key {
            "SEMICOLON" => Some(':'),
            _ => key.chars().next().filter(|_| key.len() == 1),
        };
        let text = text.map(|character| character.to_ascii_lowercase());
        menu.key(key, text, host);
    }
}

#[test]
fn a_name_can_have_capitals_but_no_quotes() {
    let mut menu = Menu::new();
    let mut host = Host::default();
    menu.open(Screen::Setup, &mut host);
    press(&mut menu, &mut host, &["BACKSPACE"; 6]);
    for text in ['R', '_', '"', '9'] {
        menu.key("X", Some(text), &mut host);
    }
    press(&mut menu, &mut host, &["ENTER"]);
    assert!(
        host.commands.iter().any(|line| line == "name \"R_9\""),
        "{:?}",
        host.commands
    );
}

#[test]
fn a_new_game_starts_from_the_menus() {
    let mut menu = Menu::new();
    let mut host = Host::default();
    menu.toggle(&mut host);
    press(&mut menu, &mut host, &["ENTER", "ENTER"]);
    assert_eq!(host.commands, ["newgame"]);
    assert!(!menu.is_open());
}

#[test]
fn a_new_game_asks_first_if_one_is_running() {
    let mut menu = Menu::new();
    let mut host = Host {
        in_game: true,
        ..Host::default()
    };
    menu.toggle(&mut host);
    press(&mut menu, &mut host, &["ENTER", "ENTER", "N"]);
    assert!(host.commands.is_empty());
    assert!(menu.is_open());
    press(&mut menu, &mut host, &["ENTER", "Y"]);
    assert_eq!(host.commands, ["newgame"]);
}

#[test]
fn options_can_be_reached_from_the_main_menu() {
    let mut menu = Menu::new();
    let mut host = Host::default();
    menu.toggle(&mut host);
    press(&mut menu, &mut host, &["DOWNARROW", "DOWNARROW", "ENTER"]);
    press(
        &mut menu,
        &mut host,
        &["DOWNARROW", "DOWNARROW", "DOWNARROW"],
    );
    host.set_setting("gamma", 0.8);
    press(&mut menu, &mut host, &["RIGHTARROW"]);
    assert!(
        (host.setting("gamma") - 0.75).abs() < 0.001,
        "it reached the options"
    );
}

#[test]
fn a_slider_stops_at_its_ends() {
    let mut menu = Menu::new();
    let mut host = Host::default();
    host.set_setting("volume", 0.95);
    menu.open(Screen::Options, &mut host);
    let down = ["DOWNARROW"; 6];
    press(&mut menu, &mut host, &down);
    press(&mut menu, &mut host, &["RIGHTARROW"]);
    assert_eq!(host.setting("volume"), 1.0);
}

#[test]
fn invert_mouse_flips_the_sign() {
    let mut menu = Menu::new();
    let mut host = Host::default();
    host.set_setting("m_pitch", 0.022);
    menu.open(Screen::Options, &mut host);
    let down = ["DOWNARROW"; 8];
    press(&mut menu, &mut host, &down);
    press(&mut menu, &mut host, &["ENTER"]);
    assert_eq!(host.setting("m_pitch"), -0.022);
}

#[test]
fn only_a_used_slot_loads() {
    let mut menu = Menu::new();
    let mut host = Host {
        first_save: Some("the Slipgate Complex".to_owned()),
        ..Host::default()
    };
    menu.open(Screen::Load, &mut host);
    press(&mut menu, &mut host, &["DOWNARROW", "ENTER"]);
    assert!(host.commands.is_empty(), "the second slot is empty");
    press(&mut menu, &mut host, &["UPARROW", "ENTER"]);
    assert_eq!(host.commands, ["load s0"]);
}

#[test]
fn the_save_menu_needs_a_game_that_can_be_saved() {
    let mut menu = Menu::new();
    let mut host = Host::default();
    menu.open(Screen::Save, &mut host);
    assert!(!menu.is_open());
    host.can_save = true;
    menu.open(Screen::Save, &mut host);
    press(&mut menu, &mut host, &["ENTER"]);
    assert_eq!(host.commands, ["save s0"]);
}

#[test]
fn a_key_is_bound_after_clearing_two_old_ones() {
    let mut menu = Menu::new();
    let mut host = Host::default();
    menu.open(Screen::Keys, &mut host);
    press(&mut menu, &mut host, &["ENTER", "F"]);
    assert_eq!(
        host.commands,
        ["unbind \"A\"", "unbind \"B\"", "bind \"F\" \"+attack\""]
    );
}

#[test]
fn escape_leaves_binding_a_key_alone() {
    let mut menu = Menu::new();
    let mut host = Host::default();
    menu.open(Screen::Keys, &mut host);
    press(&mut menu, &mut host, &["ENTER", "ESCAPE"]);
    assert!(
        host.commands
            .iter()
            .all(|command| !command.contains("bind \"ESCAPE\""))
    );
    assert!(menu.is_open());
}

#[test]
fn quitting_asks_and_no_goes_back() {
    let mut menu = Menu::new();
    let mut host = Host::default();
    menu.open(Screen::Quit, &mut host);
    press(&mut menu, &mut host, &["N"]);
    assert!(!menu.is_open(), "opened from the game, it goes back there");
    menu.open(Screen::Quit, &mut host);
    press(&mut menu, &mut host, &["Y"]);
    assert_eq!(host.commands, ["quit"]);
}

#[test]
fn every_screen_draws_without_a_gap() {
    let mut host = Host::default();
    for screen in [
        Screen::Main,
        Screen::SinglePlayer,
        Screen::Load,
        Screen::Options,
        Screen::Video,
        Screen::Keys,
        Screen::Help,
        Screen::Quit,
        Screen::Multiplayer,
        Screen::Join,
        Screen::Host,
        Screen::Setup,
    ] {
        let mut menu = Menu::new();
        menu.open(screen, &mut host);
        let (_names, quads) = drawn(|canvas| menu.draw(canvas, 1.0, &host));
        assert!(!quads.is_empty(), "{screen:?}");
    }
}

#[test]
fn an_address_can_be_typed_and_joined() {
    let mut menu = Menu::new();
    let mut host = Host::default();
    menu.open(Screen::Multiplayer, &mut host);
    press(&mut menu, &mut host, &["ENTER"]);
    press(&mut menu, &mut host, &["ENTER"]);
    assert!(host.commands.is_empty(), "there's nothing typed yet");
    let keys = [
        "1",
        "0",
        ".",
        "0",
        ".",
        "0",
        ".",
        "9",
        "9",
        "SEMICOLON",
        "2",
        "6",
    ];
    press(&mut menu, &mut host, &keys);
    press(&mut menu, &mut host, &["BACKSPACE", "ENTER"]);
    assert_eq!(host.commands, ["password \"\"; connect \"10.0.0.99:2\""]);
}

#[test]
fn a_password_can_be_typed_to_join_with() {
    let mut menu = Menu::new();
    let mut host = Host::default();
    menu.open(Screen::Join, &mut host);
    press(
        &mut menu,
        &mut host,
        &["1", "DOWNARROW", "A", "B", "BACKSPACE", "ENTER"],
    );
    assert_eq!(host.commands, ["password \"a\"; connect \"1\""]);
}

#[test]
fn a_host_code_can_be_typed_to_be_sure_of_the_host() {
    let mut menu = Menu::new();
    let mut host = Host::default();
    menu.open(Screen::Join, &mut host);
    press(&mut menu, &mut host, &["1", "UPARROW", "A", "2", "ENTER"]);
    assert_eq!(host.commands, ["password \"\"; connect \"1\" \"a2\""]);
}

#[test]
fn a_game_can_be_hosted_with_a_password() {
    let mut menu = Menu::new();
    let mut host = Host::default();
    menu.open(Screen::Host, &mut host);
    press(
        &mut menu,
        &mut host,
        &["UPARROW", "UPARROW", "X", "Y", "DOWNARROW", "ENTER"],
    );
    let command = host.commands.last().expect("a command");
    assert!(command.contains("sv_password \"xy\""), "{command}");
    assert!(command.ends_with("host"), "{command}");
}

#[test]
fn hosting_sets_the_rules_then_starts() {
    let mut menu = Menu::new();
    let mut host = Host::default();
    menu.open(Screen::Multiplayer, &mut host);
    press(&mut menu, &mut host, &["DOWNARROW", "ENTER"]);
    press(
        &mut menu,
        &mut host,
        &["DOWNARROW", "DOWNARROW", "DOWNARROW"],
    );
    press(&mut menu, &mut host, &["RIGHTARROW", "RIGHTARROW"]);
    press(
        &mut menu,
        &mut host,
        &["DOWNARROW", "DOWNARROW", "DOWNARROW", "DOWNARROW", "ENTER"],
    );
    let command = &host.commands[0];
    assert!(command.contains("deathmatch 1"), "{command}");
    assert!(command.contains("fraglimit 20"), "{command}");
    assert!(command.ends_with("map dm1; host"), "{command}");
    assert!(!menu.is_open());
}

#[test]
fn setup_keeps_a_name_and_steps_through_colours() {
    let mut menu = Menu::new();
    let mut host = Host::default();
    menu.open(Screen::Setup, &mut host);
    press(&mut menu, &mut host, &["BACKSPACE", "Z"]);
    press(&mut menu, &mut host, &["DOWNARROW", "LEFTARROW"]);
    assert_eq!(host.setting("topcolor"), 13.0, "it wraps round");
    press(&mut menu, &mut host, &["ESCAPE"]);
    assert_eq!(host.commands, ["name \"testez\""]);
}
