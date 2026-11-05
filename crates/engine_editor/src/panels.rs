//! The editor's look and panels, in the flat style of Campfire and Scopework.

use egui::{Color32, Context, CornerRadius, Margin, Stroke, Style, TextStyle, Visuals};

/// The darkest surface.
const BACKGROUND: Color32 = Color32::from_rgb(0x12, 0x15, 0x19);
/// Panels.
const PANEL: Color32 = Color32::from_rgb(0x17, 0x1b, 0x20);
/// Raised controls.
const CONTROL: Color32 = Color32::from_rgb(0x22, 0x27, 0x2e);
/// Hovered controls.
const HOVER: Color32 = Color32::from_rgb(0x2c, 0x33, 0x3c);
/// Thin lines.
const LINE: Color32 = Color32::from_rgb(0x2a, 0x30, 0x38);
/// Text.
const TEXT: Color32 = Color32::from_rgb(0xd6, 0xda, 0xdf);
/// The accent, on what's active.
pub(crate) const ACCENT: Color32 = Color32::from_rgb(0xe8, 0xa3, 0x3d);

/// Applies the look.
pub(crate) fn apply_style(context: &Context) {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = BACKGROUND;
    visuals.faint_bg_color = CONTROL;
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = ACCENT.linear_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.hyperlink_color = ACCENT;
    let square = CornerRadius::ZERO;
    visuals.window_corner_radius = square;
    visuals.menu_corner_radius = square;
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = square;
        widget.expansion = 0.0;
    }
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    visuals.widgets.inactive.weak_bg_fill = CONTROL;
    visuals.widgets.inactive.bg_fill = CONTROL;
    visuals.widgets.hovered.weak_bg_fill = HOVER;
    visuals.widgets.hovered.bg_fill = HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT.linear_multiply(0.5));
    visuals.widgets.active.weak_bg_fill = HOVER;
    visuals.widgets.active.bg_fill = HOVER;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    let mut style = Style {
        visuals,
        ..Style::default()
    };
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(8.0, 3.0);
    style.spacing.window_margin = Margin::same(6);
    style.override_text_style = Some(TextStyle::Monospace);
    context.set_global_style(style);
}
