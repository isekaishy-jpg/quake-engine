//! Console display and what any game shows over the view.
//!
//! What a particular game draws lives with that game, and comes here as a
//! canvas (proposal 0039).
//!
//! May depend on `engine_core`, `engine_console`, `engine_render`. See proposal 0008.

mod console_view;
mod theme;

pub use console_view::ConsoleView;
pub use theme::{
    BakedFont, FontStyle, Glyph, KitPiece, TextMeasure, UiAnimation, UiColor, UiKit, UiTheme,
    draw_cells, draw_frame, draw_piece, draw_text, measure_text, nine_slices, text_width, wrapped,
};

use engine_core::canvas::{CHARACTER_SIZE, Canvas};

/// The room in units that every screen has, at the least. A UI is laid out for this.
const UNITS: (f32, f32) = (640.0, 360.0);

/// The fewest pixels to a unit, so that a window with no size still has a scale.
const LEAST_SCALE: f32 = 0.25;

/// How many pixels make a canvas unit on a screen of `size`. It's relative to the picture, so
/// every screen has the same room and what's on it only grows with the resolution. Nothing
/// can ask for less room than a layout was made for.
#[must_use]
pub fn canvas_scale(size: (u32, u32)) -> f32 {
    #[expect(clippy::cast_precision_loss, reason = "a screen's size")]
    let (width, height) = (size.0 as f32 / UNITS.0, size.1 as f32 / UNITS.1);
    width.min(height).max(LEAST_SCALE)
}

/// Draws the frame rate in the top right corner.
pub fn draw_rate(canvas: &mut Canvas<'_>, rate: &str) {
    #[expect(clippy::cast_precision_loss, reason = "a few characters")]
    let left = canvas.width() - (rate.len() as f32 + 1.0) * CHARACTER_SIZE;
    canvas.text(left, CHARACTER_SIZE, rate.as_bytes());
}

/// Draws a picture in the middle of the screen, as the pause and loading
/// plaques are.
pub fn draw_plaque(canvas: &mut Canvas<'_>, name: &str) {
    if let Some((width, height)) = canvas.size_of(name) {
        let x = ((canvas.width() - width) / 2.0).floor();
        let y = ((canvas.height() - 48.0 - height) / 2.0).floor();
        canvas.picture(name, x, y);
    }
}
