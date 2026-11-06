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

/// The picture height that gives each canvas unit a pixel.
const HEIGHT_PER_SCALE: u32 = 360;

/// How many pixels make a canvas unit on a screen `height` tall, for a setting
/// of `wanted`, where 0 picks one to suit the screen.
#[must_use]
pub fn canvas_scale(height: u32, wanted: f32) -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a setting of a few units"
    )]
    let wanted = wanted.max(0.0) as u32;
    if wanted > 0 {
        wanted
    } else {
        (height / HEIGHT_PER_SCALE).max(1)
    }
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
