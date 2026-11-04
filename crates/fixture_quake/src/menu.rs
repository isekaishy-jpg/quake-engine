//! Quake's menus, from `menu.c` (proposal 0043).

use engine_core::canvas::{CHARACTER_SIZE, Canvas};

use crate::quit_messages::QUIT_MESSAGES;

/// The most save slots.
pub const SAVE_SLOTS: usize = 12;

/// How many pages of help there are.
const HELP_PAGES: u8 = 6;

/// A slider's width in characters.
const SLIDER_RANGE: u8 = 10;

/// How many rows the main menu's picture has.
const MAIN_ROWS: usize = 5;

/// The row of the main menu that isn't ready until online play.
const MULTIPLAYER_ROW: usize = 1;

/// How many rows the single player menu's picture has.
const SINGLE_ROWS: usize = 3;

/// What's asked to change a binding.
const KEY_ACTIONS: [(&str, &str); 18] = [
    ("+attack", "attack"),
    ("impulse 10", "change weapon"),
    ("+jump", "jump / swim up"),
    ("+forward", "walk forward"),
    ("+back", "backpedal"),
    ("+left", "turn left"),
    ("+right", "turn right"),
    ("+speed", "run"),
    ("+moveleft", "step left"),
    ("+moveright", "step right"),
    ("+strafe", "sidestep"),
    ("+lookup", "look up"),
    ("+lookdown", "look down"),
    ("centerview", "center view"),
    ("+mlook", "mouse look"),
    ("+klook", "keyboard look"),
    ("+moveup", "swim up"),
    ("+movedown", "swim down"),
];

/// What a row of options does.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Row {
    /// Goes to another screen.
    Open(Screen),
    /// Runs console text.
    Run(&'static str),
    /// Sets a number between two values, in steps.
    Slider {
        /// The setting.
        name: &'static str,
        /// Where the slider's left end is.
        low: f32,
        /// Where its right end is.
        high: f32,
        /// How far a key moves it.
        step: f32,
    },
    /// Turns a setting on and off.
    Switch(&'static str),
    /// Turns a setting's sign around.
    Sign(&'static str),
}

/// The rows of the options menu, from `M_Options_Draw`.
const OPTIONS: [(&str, Row); 10] = [
    ("    Customize controls", Row::Open(Screen::Keys)),
    ("         Go to console", Row::Run("toggleconsole")),
    ("     Reset to defaults", Row::Run("reset_defaults")),
    (
        "            Brightness",
        Row::Slider {
            name: "gamma",
            low: 1.0,
            high: 0.5,
            step: 0.05,
        },
    ),
    (
        "           Mouse Speed",
        Row::Slider {
            name: "sensitivity",
            low: 1.0,
            high: 11.0,
            step: 0.5,
        },
    ),
    (
        "          Music Volume",
        Row::Slider {
            name: "bgmvolume",
            low: 0.0,
            high: 1.0,
            step: 0.1,
        },
    ),
    (
        "          Sound Volume",
        Row::Slider {
            name: "volume",
            low: 0.0,
            high: 1.0,
            step: 0.1,
        },
    ),
    ("            Always Run", Row::Switch("cl_alwaysrun")),
    ("          Invert Mouse", Row::Sign("m_pitch")),
    ("         Video Options", Row::Open(Screen::Video)),
];

/// The rows of the video menu.
const VIDEO: [(&str, Row); 4] = [
    ("         Vertical Sync", Row::Switch("vid_vsync")),
    ("            Frame Rate", Row::Switch("show_fps")),
    (
        "         Field of View",
        Row::Slider {
            name: "fov",
            low: 60.0,
            high: 130.0,
            step: 5.0,
        },
    ),
    (
        "        Interface Size",
        Row::Slider {
            name: "ui_scale",
            low: 0.0,
            high: 6.0,
            step: 1.0,
        },
    ),
];

/// A place in the menus.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Screen {
    /// Nothing's open.
    #[default]
    None,
    /// The main menu.
    Main,
    /// Single player.
    SinglePlayer,
    /// Choosing a game to load.
    Load,
    /// Choosing a slot to save in.
    Save,
    /// Options.
    Options,
    /// Key bindings.
    Keys,
    /// Video options.
    Video,
    /// Pages of help.
    Help,
    /// Asking whether to quit.
    Quit,
    /// Asking whether to leave the game for a new one.
    NewGame,
}

/// What the menus need from the engine.
pub trait MenuHost {
    /// The value of a setting.
    fn setting(&self, name: &str) -> f32;
    /// Changes a setting.
    fn set_setting(&mut self, name: &str, value: f32);
    /// Runs console text.
    fn run(&mut self, command: &str);
    /// The keys that run `command`.
    fn keys_for(&self, command: &str) -> Vec<String>;
    /// Plays a sound from the menus.
    fn play(&mut self, sound: &str);
    /// Each save slot's description, for those in use.
    fn saves(&self) -> Vec<Option<String>>;
    /// Whether there's a game running.
    fn in_game(&self) -> bool;
    /// Whether the game can be saved now.
    fn can_save(&self) -> bool;
}

/// The menus' state.
#[derive(Clone, Debug, Default)]
pub struct Menu {
    /// The screen showing.
    screen: Screen,
    /// The screen to go back to from a question.
    behind: Screen,
    /// The main menu's cursor.
    main_cursor: usize,
    /// The single player menu's cursor.
    single_cursor: usize,
    /// The save and load menus' cursor.
    slot_cursor: usize,
    /// The options menu's cursor.
    options_cursor: usize,
    /// The video menu's cursor.
    video_cursor: usize,
    /// The keys menu's cursor.
    keys_cursor: usize,
    /// Whether the next key is to be bound.
    grabbing: bool,
    /// The page of help showing.
    help_page: u8,
    /// Which quit message shows.
    quit_message: usize,
    /// The save slots, as they were when the screen opened.
    saves: Vec<Option<String>>,
}

/// One step through `count` rows, wrapping round.
fn step(cursor: usize, count: usize, forward: bool) -> usize {
    if forward {
        (cursor + 1) % count
    } else {
        (cursor + count - 1) % count
    }
}

impl Menu {
    /// Closed menus.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a screen is showing.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.screen != Screen::None
    }

    /// Closes every screen.
    pub fn close(&mut self) {
        self.screen = Screen::None;
        self.grabbing = false;
    }

    /// Opens `screen`.
    pub fn open(&mut self, screen: Screen, host: &mut dyn MenuHost) {
        if matches!(screen, Screen::Save) && !host.can_save() {
            return;
        }
        if matches!(screen, Screen::Save | Screen::Load) {
            self.saves = host.saves();
        }
        match screen {
            Screen::Help => self.help_page = 0,
            Screen::Quit => {
                self.behind = self.screen;
                self.quit_message = (self.quit_message + 3) % QUIT_MESSAGES.len();
            }
            Screen::NewGame => self.behind = self.screen,
            _ => {}
        }
        self.screen = screen;
        host.play("misc/menu2.wav");
    }

    /// `togglemenu`, from `M_ToggleMenu_f`.
    pub fn toggle(&mut self, host: &mut dyn MenuHost) {
        match self.screen {
            Screen::None => self.open(Screen::Main, host),
            Screen::Main => self.close(),
            _ => self.open(Screen::Main, host),
        }
    }

    /// Takes a key, by its name.
    pub fn key(&mut self, key: &str, host: &mut dyn MenuHost) {
        match self.screen {
            Screen::None => {}
            Screen::Main => self.main_key(key, host),
            Screen::SinglePlayer => self.single_key(key, host),
            Screen::Load | Screen::Save => self.slot_key(key, host),
            Screen::Options => self.rows_key(key, host, &OPTIONS, Screen::Main),
            Screen::Video => self.rows_key(key, host, &VIDEO, Screen::Options),
            Screen::Keys => self.keys_key(key, host),
            Screen::Help => self.help_key(key, host),
            Screen::Quit => self.question_key(key, host, "quit"),
            Screen::NewGame => self.question_key(key, host, "newgame"),
        }
    }

    /// The main menu's keys.
    fn main_key(&mut self, key: &str, host: &mut dyn MenuHost) {
        match key {
            "ESCAPE" => self.toggle(host),
            "DOWNARROW" | "UPARROW" => {
                host.play("misc/menu1.wav");
                let forward = key == "DOWNARROW";
                self.main_cursor = step(self.main_cursor, MAIN_ROWS, forward);
                if self.main_cursor == MULTIPLAYER_ROW {
                    self.main_cursor = step(self.main_cursor, MAIN_ROWS, forward);
                }
            }
            "ENTER" => match self.main_cursor {
                0 => self.open(Screen::SinglePlayer, host),
                2 => self.open(Screen::Options, host),
                3 => self.open(Screen::Help, host),
                _ => self.open(Screen::Quit, host),
            },
            _ => {}
        }
    }

    /// The single player menu's keys.
    fn single_key(&mut self, key: &str, host: &mut dyn MenuHost) {
        match key {
            "ESCAPE" => self.open(Screen::Main, host),
            "DOWNARROW" | "UPARROW" => {
                host.play("misc/menu1.wav");
                self.single_cursor = step(self.single_cursor, SINGLE_ROWS, key == "DOWNARROW");
            }
            "ENTER" => match self.single_cursor {
                0 if host.in_game() => self.open(Screen::NewGame, host),
                0 => {
                    self.close();
                    host.run("newgame");
                }
                1 => self.open(Screen::Load, host),
                _ => self.open(Screen::Save, host),
            },
            _ => {}
        }
    }

    /// The load and save menus' keys.
    fn slot_key(&mut self, key: &str, host: &mut dyn MenuHost) {
        match key {
            "ESCAPE" => self.open(Screen::SinglePlayer, host),
            "ENTER" => {
                let slot = self.slot_cursor;
                if self.screen == Screen::Save {
                    self.close();
                    host.run(&format!("save s{slot}"));
                } else if self.saves.get(slot).is_some_and(Option::is_some) {
                    host.play("misc/menu2.wav");
                    self.close();
                    host.run(&format!("load s{slot}"));
                }
            }
            "UPARROW" | "LEFTARROW" | "DOWNARROW" | "RIGHTARROW" => {
                host.play("misc/menu1.wav");
                let forward = matches!(key, "DOWNARROW" | "RIGHTARROW");
                self.slot_cursor = step(self.slot_cursor, SAVE_SLOTS, forward);
            }
            _ => {}
        }
    }

    /// Changes `row`, by one step `up` or down.
    fn adjust(row: Row, up: bool, host: &mut dyn MenuHost) {
        match row {
            Row::Slider {
                name,
                low,
                high,
                step,
            } => {
                host.play("misc/menu3.wav");
                let toward = if high > low { step } else { -step };
                let change = if up { toward } else { -toward };
                let value = host.setting(name) + change;
                host.set_setting(name, value.clamp(low.min(high), low.max(high)));
            }
            Row::Switch(name) => {
                host.play("misc/menu3.wav");
                let on = host.setting(name) != 0.0;
                host.set_setting(name, if on { 0.0 } else { 1.0 });
            }
            Row::Sign(name) => {
                host.play("misc/menu3.wav");
                let value = host.setting(name);
                host.set_setting(name, -value);
            }
            Row::Open(_) | Row::Run(_) => {}
        }
    }

    /// The keys of a screen of rows.
    fn rows_key(&mut self, key: &str, host: &mut dyn MenuHost, rows: &[(&str, Row)], back: Screen) {
        let video = self.screen == Screen::Video;
        let cursor = if video {
            self.video_cursor
        } else {
            self.options_cursor
        };
        let mut moved = cursor;
        match key {
            "ESCAPE" => self.open(back, host),
            "UPARROW" | "DOWNARROW" => {
                host.play("misc/menu1.wav");
                moved = step(cursor, rows.len(), key == "DOWNARROW");
            }
            "LEFTARROW" => Self::adjust(rows[cursor].1, false, host),
            "RIGHTARROW" => Self::adjust(rows[cursor].1, true, host),
            "ENTER" => match rows[cursor].1 {
                Row::Open(screen) => self.open(screen, host),
                Row::Run(command) => {
                    self.close();
                    host.run(command);
                }
                row => Self::adjust(row, true, host),
            },
            _ => {}
        }
        if video {
            self.video_cursor = moved;
        } else {
            self.options_cursor = moved;
        }
    }

    /// The keys menu's keys.
    fn keys_key(&mut self, key: &str, host: &mut dyn MenuHost) {
        let action = KEY_ACTIONS[self.keys_cursor].0;
        if self.grabbing {
            host.play("misc/menu1.wav");
            self.grabbing = false;
            if key != "ESCAPE" && key != "`" {
                host.run(&format!("bind \"{key}\" \"{action}\""));
            }
            return;
        }
        match key {
            "ESCAPE" => self.open(Screen::Options, host),
            "LEFTARROW" | "UPARROW" | "DOWNARROW" | "RIGHTARROW" => {
                host.play("misc/menu1.wav");
                let forward = matches!(key, "DOWNARROW" | "RIGHTARROW");
                self.keys_cursor = step(self.keys_cursor, KEY_ACTIONS.len(), forward);
            }
            "ENTER" => {
                host.play("misc/menu2.wav");
                let bound = host.keys_for(action);
                if bound.len() > 1 {
                    Self::unbind(&bound, host);
                }
                self.grabbing = true;
            }
            "BACKSPACE" | "DEL" => {
                host.play("misc/menu2.wav");
                let bound = host.keys_for(action);
                Self::unbind(&bound, host);
            }
            _ => {}
        }
    }

    /// Removes the bindings of `keys`.
    fn unbind(keys: &[String], host: &mut dyn MenuHost) {
        for key in keys {
            host.run(&format!("unbind \"{key}\""));
        }
    }

    /// The help pages' keys.
    fn help_key(&mut self, key: &str, host: &mut dyn MenuHost) {
        match key {
            "ESCAPE" => self.open(Screen::Main, host),
            "UPARROW" | "RIGHTARROW" => {
                host.play("misc/menu2.wav");
                self.help_page = (self.help_page + 1) % HELP_PAGES;
            }
            "DOWNARROW" | "LEFTARROW" => {
                host.play("misc/menu2.wav");
                self.help_page = (self.help_page + HELP_PAGES - 1) % HELP_PAGES;
            }
            _ => {}
        }
    }

    /// The keys of a question, which runs `command` on a yes.
    fn question_key(&mut self, key: &str, host: &mut dyn MenuHost, command: &str) {
        match key {
            "Y" | "ENTER" => {
                self.close();
                host.run(command);
            }
            "N" | "ESCAPE" => {
                let behind = self.behind;
                if behind == Screen::None {
                    self.close();
                } else {
                    self.open(behind, host);
                }
            }
            _ => {}
        }
    }

    /// Draws what's showing, at seconds `time`, in a space of 320 by 200 in the
    /// middle of the screen.
    pub fn draw(&self, canvas: &mut Canvas<'_>, time: f64, host: &dyn MenuHost) {
        let x = ((canvas.width() - 320.0) / 2.0).floor();
        let y = ((canvas.height() - 200.0) / 2.0).floor();
        let area = Area { x, y, time };
        match self.screen {
            Screen::None => {}
            Screen::Main => {
                area.titled(canvas, "gfx/ttl_main.lmp", true);
                area.picture(canvas, "gfx/mainmenu.lmp", 72.0, 32.0);
                area.dot(canvas, self.main_cursor);
            }
            Screen::SinglePlayer => {
                area.titled(canvas, "gfx/ttl_sgl.lmp", true);
                area.picture(canvas, "gfx/sp_menu.lmp", 72.0, 32.0);
                area.dot(canvas, self.single_cursor);
            }
            Screen::Load | Screen::Save => self.draw_slots(canvas, &area),
            Screen::Options => {
                area.titled(canvas, "gfx/p_option.lmp", true);
                area.rows(canvas, &OPTIONS, self.options_cursor, host);
            }
            Screen::Video => {
                area.titled(canvas, "gfx/vidmodes.lmp", false);
                area.rows(canvas, &VIDEO, self.video_cursor, host);
            }
            Screen::Keys => self.draw_keys(canvas, &area, host),
            Screen::Help => {
                area.picture(canvas, &format!("gfx/help{}.lmp", self.help_page), 0.0, 0.0);
            }
            Screen::Quit => self.draw_quit(canvas, &area),
            Screen::NewGame => {
                area.text_box(canvas, 56.0, 76.0, 24, 4);
                area.print(canvas, 64.0, 84.0, "Are you sure you want to");
                area.print(canvas, 64.0, 92.0, "    start a new game?   ");
                area.print(canvas, 64.0, 108.0, "   Press Y or N         ");
            }
        }
    }

    /// Draws the load or save menu.
    fn draw_slots(&self, canvas: &mut Canvas<'_>, area: &Area) {
        let title = if self.screen == Screen::Save {
            "gfx/p_save.lmp"
        } else {
            "gfx/p_load.lmp"
        };
        area.titled(canvas, title, false);
        for slot in 0..SAVE_SLOTS {
            let text = self
                .saves
                .get(slot)
                .and_then(Option::as_deref)
                .unwrap_or("--- UNUSED SLOT ---");
            #[expect(clippy::cast_precision_loss, reason = "twelve slots")]
            let top = 32.0 + 8.0 * slot as f32;
            area.print(canvas, 16.0, top, text);
        }
        #[expect(clippy::cast_precision_loss, reason = "twelve slots")]
        let top = 32.0 + 8.0 * self.slot_cursor as f32;
        area.cursor(canvas, 8.0, top);
    }

    /// Draws the keys menu.
    fn draw_keys(&self, canvas: &mut Canvas<'_>, area: &Area, host: &dyn MenuHost) {
        area.titled(canvas, "gfx/ttl_cstm.lmp", false);
        if self.grabbing {
            area.print(canvas, 12.0, 32.0, "Press a key or button for this action");
        } else {
            area.print(canvas, 18.0, 32.0, "Enter to change, backspace to clear");
        }
        for (row, (action, name)) in (0u8..).zip(KEY_ACTIONS) {
            let top = 48.0 + 8.0 * f32::from(row);
            area.print(canvas, 16.0, top, name);
            let keys = host.keys_for(action);
            match keys.as_slice() {
                [] => area.print(canvas, 140.0, top, "???"),
                [one] => area.print(canvas, 140.0, top, one),
                [one, two, ..] => {
                    area.print(canvas, 140.0, top, &format!("{one} or {two}"));
                }
            }
        }
        #[expect(clippy::cast_precision_loss, reason = "eighteen rows")]
        let top = 48.0 + 8.0 * self.keys_cursor as f32;
        if self.grabbing {
            canvas.character(area.x + 130.0, area.y + top, b'=');
        } else {
            area.cursor(canvas, 130.0, top);
        }
    }

    /// Draws the question on quitting.
    fn draw_quit(&self, canvas: &mut Canvas<'_>, area: &Area) {
        area.text_box(canvas, 56.0, 76.0, 24, 4);
        let lines = QUIT_MESSAGES[self.quit_message];
        for (line, top) in lines.iter().zip([84.0, 92.0, 100.0, 108.0]) {
            area.print(canvas, 64.0, top, line);
        }
    }
}

/// Where the menus are on the screen.
struct Area {
    /// The left edge.
    x: f32,
    /// The top edge.
    y: f32,
    /// Seconds, for what moves.
    time: f64,
}

impl Area {
    /// Draws a picture.
    fn picture(&self, canvas: &mut Canvas<'_>, name: &str, x: f32, y: f32) {
        canvas.picture(name, self.x + x, self.y + y);
    }

    /// Draws a menu's title in the middle, with the plaque beside it if asked.
    fn titled(&self, canvas: &mut Canvas<'_>, name: &str, plaque: bool) {
        if plaque {
            self.picture(canvas, "gfx/qplaque.lmp", 16.0, 4.0);
        }
        if let Some((width, _height)) = canvas.size_of(name) {
            self.picture(canvas, name, ((320.0 - width) / 2.0).floor(), 4.0);
        }
    }

    /// Draws the spinning cursor beside picture row `row`.
    fn dot(&self, canvas: &mut Canvas<'_>, row: usize) {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a frame of animation"
        )]
        let frame = (self.time * 10.0) as u64 % 6 + 1;
        #[expect(clippy::cast_precision_loss, reason = "a few rows")]
        let top = 32.0 + 20.0 * row as f32;
        self.picture(canvas, &format!("gfx/menudot{frame}.lmp"), 54.0, top);
    }

    /// Draws text in the menus' colour.
    fn print(&self, canvas: &mut Canvas<'_>, x: f32, y: f32, text: &str) {
        canvas.bright_text(self.x + x, self.y + y, text.as_bytes());
    }

    /// Draws the blinking cursor.
    fn cursor(&self, canvas: &mut Canvas<'_>, x: f32, y: f32) {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "which half of a blink"
        )]
        let blink = (self.time * 4.0) as u64 % 2;
        canvas.character(self.x + x, self.y + y, 12 + u8::from(blink == 1));
    }

    /// Draws a slider, with its knob at `range` from 0 to 1, from
    /// `M_DrawSlider`.
    fn slider(&self, canvas: &mut Canvas<'_>, x: f32, y: f32, range: f32) {
        let (left, top) = (self.x + x, self.y + y);
        canvas.character(left - CHARACTER_SIZE, top, 128);
        for at in 0..SLIDER_RANGE {
            canvas.character(left + f32::from(at) * CHARACTER_SIZE, top, 129);
        }
        let end = f32::from(SLIDER_RANGE) * CHARACTER_SIZE;
        canvas.character(left + end, top, 130);
        let knob = (end - CHARACTER_SIZE) * range.clamp(0.0, 1.0);
        canvas.character(left + knob, top, 131);
    }

    /// Draws rows of options, from `M_Options_Draw`.
    fn rows(
        &self,
        canvas: &mut Canvas<'_>,
        rows: &[(&str, Row)],
        cursor: usize,
        host: &dyn MenuHost,
    ) {
        for (index, (name, row)) in (0u8..).zip(rows) {
            let top = 32.0 + 8.0 * f32::from(index);
            self.print(canvas, 16.0, top, name);
            match *row {
                Row::Slider {
                    name, low, high, ..
                } => {
                    let range = (host.setting(name) - low) / (high - low);
                    self.slider(canvas, 220.0, top, range);
                }
                Row::Switch(name) => {
                    let on = host.setting(name) != 0.0;
                    self.print(canvas, 220.0, top, if on { "on" } else { "off" });
                }
                Row::Sign(name) => {
                    let on = host.setting(name) < 0.0;
                    self.print(canvas, 220.0, top, if on { "on" } else { "off" });
                }
                Row::Open(_) | Row::Run(_) => {}
            }
        }
        #[expect(clippy::cast_precision_loss, reason = "a few rows")]
        let top = 32.0 + 8.0 * cursor as f32;
        self.cursor(canvas, 200.0, top);
    }

    /// Draws a box for `lines` of `width` characters, from `M_DrawTextBox`.
    fn text_box(&self, canvas: &mut Canvas<'_>, x: f32, y: f32, width: u8, lines: u8) {
        let side = |canvas: &mut Canvas<'_>, left: f32, top: &str, middle: &str, bottom: &str| {
            self.picture(canvas, top, left, y);
            for line in 1..=lines {
                self.picture(canvas, middle, left, y + 8.0 * f32::from(line));
            }
            self.picture(canvas, bottom, left, y + 8.0 * f32::from(lines + 1));
        };
        side(
            canvas,
            x,
            "gfx/box_tl.lmp",
            "gfx/box_ml.lmp",
            "gfx/box_bl.lmp",
        );
        let columns = width / 2;
        for column in 0..columns {
            let left = x + 8.0 + 16.0 * f32::from(column);
            self.picture(canvas, "gfx/box_tm.lmp", left, y);
            for line in 1..=lines {
                let name = if line == 2 {
                    "gfx/box_mm2.lmp"
                } else {
                    "gfx/box_mm.lmp"
                };
                self.picture(canvas, name, left, y + 8.0 * f32::from(line));
            }
            self.picture(
                canvas,
                "gfx/box_bm.lmp",
                left,
                y + 8.0 * f32::from(lines + 1),
            );
        }
        side(
            canvas,
            x + 8.0 + 16.0 * f32::from(columns),
            "gfx/box_tr.lmp",
            "gfx/box_mr.lmp",
            "gfx/box_br.lmp",
        );
    }
}
