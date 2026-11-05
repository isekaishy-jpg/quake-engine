//! What the console keeps, and how it's typed into.

use engine_core::canvas::{Canvas, Picture, PictureSource, Quad, TALK_MARK};
use engine_ui::{ConsoleView, canvas_scale};

/// A font and nothing else, at any size.
struct Font;

impl PictureSource for Font {
    fn picture(&mut self, _name: &str) -> Option<Picture> {
        Some(Picture {
            number: 1,
            width: 128,
            height: 128,
        })
    }
}

/// What `draw` asks to draw on a screen of 320 by 200.
fn drawn(draw: impl FnOnce(&mut Canvas<'_>)) -> Vec<Quad> {
    let mut font = Font;
    let mut canvas = Canvas::new(&mut font, (320, 200), 1);
    draw(&mut canvas);
    canvas.into_quads()
}

/// Types `keys`, returning the last line entered.
fn typed(view: &mut ConsoleView, keys: &[&str]) -> Option<String> {
    keys.iter().filter_map(|key| view.key(key)).last()
}

#[test]
fn a_line_comes_out_when_enter_is_pressed() {
    let mut view = ConsoleView::new();
    let line = typed(&mut view, &["M", "A", "P", "SPACE", "1", "ENTER"]);
    assert_eq!(line.as_deref(), Some("map 1"));
    assert_eq!(view.input(), "");
}

#[test]
fn shift_gives_the_other_character() {
    let mut view = ConsoleView::new();
    view.set_shift(true);
    typed(&mut view, &["A", "1", "-", "SEMICOLON"]);
    view.set_shift(false);
    typed(&mut view, &["SEMICOLON", "'"]);
    assert_eq!(view.input(), "A!_:;'");
}

#[test]
fn names_of_keys_type_nothing() {
    let mut view = ConsoleView::new();
    typed(&mut view, &["F1", "CTRL", "ESCAPE"]);
    assert_eq!(view.input(), "");
}

#[test]
fn backspace_takes_a_character_back() {
    let mut view = ConsoleView::new();
    typed(&mut view, &["A", "B", "BACKSPACE"]);
    assert_eq!(view.input(), "a");
}

#[test]
fn old_lines_come_back_with_the_arrows() {
    let mut view = ConsoleView::new();
    typed(&mut view, &["A", "ENTER", "B", "ENTER"]);
    typed(&mut view, &["UPARROW"]);
    assert_eq!(view.input(), "b");
    typed(&mut view, &["UPARROW"]);
    assert_eq!(view.input(), "a");
    typed(&mut view, &["UPARROW"]);
    assert_eq!(view.input(), "a", "no further");
    typed(&mut view, &["DOWNARROW", "DOWNARROW"]);
    assert_eq!(view.input(), "");
}

#[test]
fn new_lines_show_over_the_view_then_go() {
    let mut view = ConsoleView::new();
    view.print("Hi", 10.0);
    assert_eq!(
        drawn(|canvas| view.draw_notify(canvas, 11.0, false)).len(),
        2
    );
    assert!(drawn(|canvas| view.draw_notify(canvas, 14.0, false)).is_empty());
}

#[test]
fn only_the_last_four_lines_show_over_the_view() {
    let mut view = ConsoleView::new();
    for _ in 0..9 {
        view.print("x", 10.0);
    }
    assert_eq!(
        drawn(|canvas| view.draw_notify(canvas, 11.0, false)).len(),
        4
    );
}

#[test]
fn a_line_with_breaks_is_kept_as_several() {
    let mut view = ConsoleView::new();
    view.print("a\nb", 1.0);
    assert_eq!(
        drawn(|canvas| view.draw_notify(canvas, 1.5, false)).len(),
        2
    );
}

#[test]
fn it_slides_down_to_half_the_screen() {
    let mut view = ConsoleView::new();
    assert!(drawn(|canvas| view.draw(canvas, "back", "1", false)).is_empty());
    view.set_open(true);
    view.update(0.05, false);
    let part = drawn(|canvas| view.draw(canvas, "back", "1", false))[0].place[3];
    view.update(10.0, false);
    let whole = drawn(|canvas| view.draw(canvas, "back", "1", false))[0].place[3];
    assert!(part < whole);
    assert_eq!(whole, 100.0);
    view.update(10.0, true);
    assert_eq!(
        drawn(|canvas| view.draw(canvas, "back", "1", false))[0].place[3],
        200.0
    );
}

#[test]
fn the_scale_suits_the_screen_unless_a_setting_says() {
    assert_eq!(canvas_scale(720, 0.0), 2);
    assert_eq!(canvas_scale(1080, 0.0), 3);
    assert_eq!(canvas_scale(200, 0.0), 1);
    assert_eq!(canvas_scale(1080, 5.0), 5);
}

#[test]
fn chat_is_one_line_over_the_view() {
    let mut view = ConsoleView::new();
    view.open_chat(false);
    assert!(view.is_chatting());
    assert!(!view.is_open(), "the game stays in view");
    assert_eq!(view.chat_key("H"), None);
    assert_eq!(view.chat_key("I"), None);
    assert_eq!(view.chat_key("SEMICOLON"), None);
    assert_eq!(view.chat_key("ENTER").as_deref(), Some("say \"hi;\""));
    assert!(!view.is_chatting());
}

#[test]
fn team_chat_goes_to_the_team() {
    let mut view = ConsoleView::new();
    view.open_chat(true);
    assert_eq!(view.chat_key("A"), None);
    assert_eq!(view.chat_key("ENTER").as_deref(), Some("say_team \"a\""));
}

#[test]
fn escape_or_nothing_says_nothing() {
    let mut view = ConsoleView::new();
    view.open_chat(false);
    assert_eq!(view.chat_key("A"), None);
    assert_eq!(view.chat_key("ESCAPE"), None);
    assert!(!view.is_chatting());
    view.open_chat(false);
    assert_eq!(view.chat_key("SPACE"), None);
    assert_eq!(view.chat_key("ENTER"), None);
    assert!(!view.is_chatting());
}

#[test]
fn a_quote_cant_end_a_line_of_chat() {
    let mut view = ConsoleView::new();
    view.open_chat(false);
    view.set_shift(true);
    assert_eq!(view.chat_key("'"), None);
    view.set_shift(false);
    assert_eq!(view.chat_key("ENTER").as_deref(), Some("say \"'\""));
}

#[test]
fn a_long_line_of_chat_shows_its_end_and_stops_growing() {
    let mut view = ConsoleView::new();
    view.open_chat(false);
    for _ in 0..200 {
        assert_eq!(view.chat_key("A"), None);
    }
    let quads = drawn(|canvas| view.draw_notify(canvas, 0.0, false));
    let right = quads.iter().map(|quad| quad.place[2]).fold(0.0, f32::max);
    assert!(right <= 320.0, "it stays on the screen: {right}");
    let line = view.chat_key("ENTER").expect("a line");
    assert_eq!(line.len(), "say \"\"".len() + 96);
}

#[test]
fn someone_talking_is_bright_and_loses_the_mark() {
    let mut view = ConsoleView::new();
    view.print(&format!("{TALK_MARK}ab"), 0.0);
    view.print("ab", 0.0);
    let quads = drawn(|canvas| view.draw_notify(canvas, 1.0, false));
    assert_eq!(quads.len(), 4, "the mark isn't drawn");
    assert_ne!(quads[0].source, quads[2].source, "it's the bright half");
}
