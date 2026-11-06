//! Making the editor's words and buttons bigger.

use egui::{Event, Pos2, Vec2};
use engine_editor::ui_bridge::UiInput;
use engine_platform::InputEvent;

/// What `egui` is told at `scale`, with the pointer at 400, 200 on a window of 800 by 600.
fn taken(scale: f32) -> egui::RawInput {
    let mut input = UiInput::default();
    input.event(&InputEvent::Pointer { x: 400.0, y: 200.0 });
    input.take((800, 600), 0.0, scale)
}

#[test]
fn at_the_usual_size_a_point_is_a_pixel() {
    let raw = taken(1.0);
    let rect = raw.screen_rect.expect("a screen");
    assert_eq!(rect.size(), Vec2::new(800.0, 600.0));
    assert!(matches!(raw.events[0], Event::PointerMoved(at) if at == Pos2::new(400.0, 200.0)));
}

#[test]
fn twice_the_size_has_half_the_room() {
    let rect = taken(2.0).screen_rect.expect("a screen");
    assert_eq!(rect.size(), Vec2::new(400.0, 300.0));
}

/// Without this, clicks land on whatever's somewhere else once things are bigger.
#[test]
fn the_pointer_stays_on_what_it_points_at() {
    let raw = taken(2.0);
    assert!(matches!(raw.events[0], Event::PointerMoved(at) if at == Pos2::new(200.0, 100.0)));
}
