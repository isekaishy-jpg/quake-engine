//! The easy way to make a level: a floor plan, plain words and big buttons
//! (proposal 0119).
//!
//! Anyone should be able to make a level here. So there's one thing to do at a
//! time, nothing has to be held down or dragged, nothing depends on a right button
//! or on speed, every action works from the keyboard alone, and text can be made
//! bigger and plainer. Advanced mode has everything else.

use egui::{Align2, Color32, FontId, Key, Pos2, Rect, RichText, Sense, Stroke, Vec2};

/// What a click on the plan does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Brush {
    /// Paints floor.
    Room,
    /// Paints a pool.
    Water,
    /// Clears a square.
    Erase,
    /// Puts a thing down, by its name in a plan.
    Thing(&'static str),
}

/// The tools, in order, with their words and what they say on the plan.
pub(crate) const TOOLS: [(Brush, &str, &str, &str); 10] = [
    (Brush::Room, "Room", "", "Click squares to make a room."),
    (Brush::Water, "Water", "~", "Click squares to make a pool."),
    (
        Brush::Thing("start"),
        "Start",
        "S",
        "Click where the player begins.",
    ),
    (
        Brush::Thing("light"),
        "Light",
        "L",
        "Click a floor to light it.",
    ),
    (
        Brush::Thing("door"),
        "Door",
        "D",
        "Click a square in a gap to put a door there.",
    ),
    (
        Brush::Thing("health"),
        "Health",
        "+",
        "Click a floor to leave health there.",
    ),
    (
        Brush::Thing("ammo"),
        "Ammo",
        "A",
        "Click a floor to leave ammo there.",
    ),
    (
        Brush::Thing("enemy"),
        "Enemy",
        "E",
        "Click a floor to put an enemy there.",
    ),
    (
        Brush::Thing("fire"),
        "Fire",
        "F",
        "Click a floor to light a fire there.",
    ),
    (Brush::Erase, "Rub out", "", "Click a square to clear it."),
];

/// The looks a level can have, by the start of their materials' names.
pub(crate) const LOOKS: [(&str, &str); 6] = [
    ("Sewer", "uc01_blight_sewers"),
    ("Tomb", "uc02_royal_necropolis"),
    ("Workshop", "uc03_apothecary"),
    ("Abbey", "qg01_ashen_abbey"),
    ("Cathedral", "qg02_iron_cathedral"),
    ("Crypt", "qg03_elder_crypt"),
];

/// What the plan shows, as the editor last heard it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PlanView {
    /// Squares of floor.
    pub(crate) floor: Vec<(i32, i32)>,
    /// Squares of water.
    pub(crate) water: Vec<(i32, i32)>,
    /// What's on squares, by the word on the plan.
    pub(crate) things: Vec<((i32, i32), String)>,
    /// What's still to do.
    pub(crate) missing: Vec<String>,
    /// The look it has.
    pub(crate) look: String,
}

/// How the easy view stands.
#[derive(Clone, Debug)]
pub(crate) struct Easy {
    /// Whether it's showing, not advanced mode.
    pub(crate) on: bool,
    /// What a click does.
    pub(crate) brush: Brush,
    /// The square the keyboard is on.
    pub(crate) cursor: (i32, i32),
    /// Whether it's showing the level in 3D, not the plan.
    pub(crate) seeing: bool,
    /// How big everything is.
    pub(crate) size: f32,
    /// Whether colours are at their plainest.
    pub(crate) contrast: bool,
    /// How wide a square is on screen.
    pub(crate) square: f32,
    /// The plan.
    pub(crate) plan: PlanView,
}

impl Default for Easy {
    fn default() -> Self {
        Self {
            on: true,
            brush: Brush::Room,
            cursor: (0, 0),
            seeing: false,
            size: 1.25,
            contrast: false,
            square: 44.0,
            plan: PlanView::default(),
        }
    }
}

/// Colours of the plan.
struct Ink {
    /// Behind everything.
    paper: Color32,
    /// The grid.
    grid: Color32,
    /// Floor.
    floor: Color32,
    /// Water.
    water: Color32,
    /// Words and the cursor.
    mark: Color32,
}

impl Ink {
    /// The colours for `contrast`.
    const fn of(contrast: bool) -> Self {
        if contrast {
            Self {
                paper: Color32::BLACK,
                grid: Color32::from_gray(90),
                floor: Color32::WHITE,
                water: Color32::from_rgb(0, 160, 255),
                mark: Color32::YELLOW,
            }
        } else {
            Self {
                paper: Color32::from_rgb(0x14, 0x18, 0x1d),
                grid: Color32::from_rgb(0x2c, 0x33, 0x3c),
                floor: Color32::from_rgb(0x8a, 0x80, 0x6c),
                water: Color32::from_rgb(0x2e, 0x6f, 0x8e),
                mark: Color32::from_rgb(0xe8, 0xa3, 0x3d),
            }
        }
    }
}

/// What the plan has asked to be done to a square.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlanClick {
    /// Do the tool's work on this square.
    Use((i32, i32)),
}

/// Draws the plan over `rect`, with the middle of it at square (0, 0). Returns the
/// square clicked, if one was.
pub(crate) fn draw_plan(ui: &mut egui::Ui, rect: Rect, easy: &Easy) -> Option<PlanClick> {
    let ink = Ink::of(easy.contrast);
    let response = ui.allocate_rect(rect, Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, ink.paper);
    let size = easy.square;
    let middle = rect.center();
    let to_screen = |(x, y): (i32, i32)| -> Rect {
        #[expect(clippy::cast_precision_loss, reason = "plans are small")]
        let corner = middle + Vec2::new(x as f32 * size, -(y as f32 + 1.0) * size);
        Rect::from_min_size(corner, Vec2::splat(size))
    };
    let shown = (rect.width().max(rect.height()) / size / 2.0).ceil();
    #[expect(clippy::cast_possible_truncation, reason = "a screen's squares")]
    let reach = shown as i32 + 1;
    for at in -reach..=reach {
        #[expect(clippy::cast_precision_loss, reason = "plans are small")]
        let line = at as f32 * size;
        let stroke = Stroke::new(1.0, ink.grid);
        painter.line_segment(
            [
                Pos2::new(middle.x + line, rect.top()),
                Pos2::new(middle.x + line, rect.bottom()),
            ],
            stroke,
        );
        painter.line_segment(
            [
                Pos2::new(rect.left(), middle.y + line),
                Pos2::new(rect.right(), middle.y + line),
            ],
            stroke,
        );
    }
    for square in &easy.plan.floor {
        painter.rect_filled(to_screen(*square).shrink(1.0), 0.0, ink.floor);
    }
    for square in &easy.plan.water {
        painter.rect_filled(to_screen(*square).shrink(1.0), 0.0, ink.water);
    }
    for (square, word) in &easy.plan.things {
        painter.text(
            to_screen(*square).center(),
            Align2::CENTER_CENTER,
            word,
            FontId::proportional(size * 0.55),
            Color32::BLACK,
        );
    }
    painter.rect_stroke(
        to_screen(easy.cursor),
        0.0,
        Stroke::new(3.0, ink.mark),
        egui::StrokeKind::Inside,
    );
    if response.clicked()
        && let Some(point) = response.interact_pointer_pos()
    {
        let offset = point - middle;
        #[expect(clippy::cast_possible_truncation, reason = "a screen's squares")]
        let square = (
            (offset.x / size).floor() as i32,
            (-offset.y / size).floor() as i32,
        );
        return Some(PlanClick::Use(square));
    }
    None
}

/// The word a plan shows for the thing called `name`.
pub(crate) fn word_for(name: &str) -> String {
    TOOLS
        .iter()
        .find(|(brush, ..)| matches!(brush, Brush::Thing(known) if *known == name))
        .map_or_else(|| "?".to_owned(), |(_, _, word, _)| (*word).to_owned())
}

/// What the keyboard asked for this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct KeyActions {
    /// Do the tool's work on the cursor's square.
    pub(crate) use_tool: bool,
    /// Take the last thing back.
    pub(crate) undo: bool,
    /// Play the level.
    pub(crate) play: bool,
}

/// Takes the keys for the plan: arrows to move, Space or Enter to use the tool, a
/// number to choose one, and U to undo. One key at a time, and none held.
pub(crate) fn plan_keys(ui: &egui::Ui, easy: &mut Easy) -> KeyActions {
    let mut actions = KeyActions::default();
    // Whatever has the keyboard's focus takes its keys, so one press does one thing.
    let focused = ui.ctx().memory(|memory| memory.focused().is_some());
    if focused || ui.ctx().egui_wants_keyboard_input() {
        return actions;
    }
    ui.input(|input| {
        if input.key_pressed(Key::ArrowLeft) {
            easy.cursor.0 -= 1;
        }
        if input.key_pressed(Key::ArrowRight) {
            easy.cursor.0 += 1;
        }
        if input.key_pressed(Key::ArrowUp) {
            easy.cursor.1 += 1;
        }
        if input.key_pressed(Key::ArrowDown) {
            easy.cursor.1 -= 1;
        }
        actions.use_tool = input.key_pressed(Key::Space) || input.key_pressed(Key::Enter);
        actions.undo = input.key_pressed(Key::U);
        actions.play = input.key_pressed(Key::P);
        let numbers = [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
            Key::Num0,
        ];
        for (key, tool) in numbers.iter().zip(&TOOLS) {
            if input.key_pressed(*key) {
                easy.brush = tool.0;
            }
        }
    });
    actions
}

/// A big button with plain words.
pub(crate) fn big_button(ui: &mut egui::Ui, words: &str, on: bool, width: f32) -> bool {
    let text = RichText::new(words).size(16.0);
    let button = egui::Button::new(text)
        .min_size(Vec2::new(width, 36.0))
        .selected(on);
    ui.add(button).clicked()
}
