//! The drop-down console and the lines over the view (proposal 0042).

use std::collections::VecDeque;

use engine_core::canvas::{CHARACTER_SIZE, Canvas, TALK_MARK};

use crate::theme::{UiTheme, draw_cells};

/// The colour of what the console says, in the engine's own look.
const CONSOLE_INK: [f32; 4] = [0.87, 0.85, 0.8, 1.0];

/// The colour of someone talking.
const CONSOLE_ACCENT: [f32; 4] = [0.73, 0.29, 0.24, 1.0];

/// The most lines kept.
const MAX_LINES: usize = 1024;

/// The most typed lines kept.
const MAX_HISTORY: usize = 64;

/// `con_notifytime`, in seconds.
const NOTIFY_SECONDS: f64 = 3.0;

/// How many lines show over the view.
const NOTIFY_LINES: usize = 4;

/// How much of the screen the console covers.
const COVER: f32 = 0.5;

/// How fast the console moves, in screens a second.
const SLIDE_SPEED: f32 = 3.0;

/// The prompt.
const PROMPT: char = ']';

/// How many lines Page Up scrolls.
const PAGE_LINES: usize = 4;

/// The characters of a key, plain and with Shift held, as `keyshift` in
/// `keys.c`. Keys here are named by their position on a US keyboard.
fn typed(key: &str, shift: bool) -> Option<char> {
    let mut letters = key.chars();
    let (first, rest) = (letters.next()?, letters.next());
    match key {
        "SPACE" => return Some(' '),
        "SEMICOLON" => return Some(if shift { ':' } else { ';' }),
        _ => {}
    }
    if rest.is_some() {
        return None;
    }
    if first.is_ascii_alphabetic() {
        return Some(if shift {
            first.to_ascii_uppercase()
        } else {
            first.to_ascii_lowercase()
        });
    }
    if !shift {
        return Some(first);
    }
    const PLAIN: &str = "1234567890-=[]\\;'`,./";
    const SHIFTED: &str = ")!@#$%^&*(";
    const SHIFTED_REST: &str = "_+{}|:\"~<>?";
    let at = PLAIN.find(first)?;
    if at < 10 {
        let digit = usize::try_from(first.to_digit(10)?).ok()?;
        SHIFTED.chars().nth(digit)
    } else {
        SHIFTED_REST.chars().nth(at - 10)
    }
}

/// The most that can be typed into a line of chat.
const MAX_CHAT: usize = 96;

/// Draws one line, in the bright font if it's someone talking.
fn draw_line(canvas: &mut Canvas<'_>, x: f32, y: f32, line: &str) {
    match line.strip_prefix(TALK_MARK) {
        Some(talk) => canvas.bright_text(x, y, talk.as_bytes()),
        None => canvas.text(x, y, line.as_bytes()),
    }
}

/// A line of chat being typed over the view.
#[derive(Clone, Debug, Default)]
struct Chat {
    /// What's been typed.
    text: String,
    /// Whether it's only for the team.
    team: bool,
}

/// The console's text and what's being typed.
#[derive(Clone, Debug, Default)]
pub struct ConsoleView {
    /// Every line kept, oldest first.
    lines: VecDeque<String>,
    /// When recent lines arrived.
    notify: VecDeque<(f64, String)>,
    /// What's being typed.
    input: String,
    /// Lines typed so far.
    history: Vec<String>,
    /// How far back through `history` the input came from.
    history_at: usize,
    /// How many lines up the view has scrolled.
    scroll: usize,
    /// Whether it's open.
    open: bool,
    /// How far down it is, from 0 to [`COVER`].
    down: f32,
    /// Whether Shift is held.
    shift: bool,
    /// A line of chat being typed, while the game goes on.
    chat: Option<Chat>,
}

impl ConsoleView {
    /// An empty console.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a line, which arrived at `time`.
    pub fn print(&mut self, line: &str, time: f64) {
        for part in line.split('\n') {
            if self.lines.len() == MAX_LINES {
                let _oldest = self.lines.pop_front();
            }
            self.lines.push_back(part.to_owned());
            if self.scroll > 0 {
                self.scroll += 1;
            }
            self.notify.push_back((time, part.to_owned()));
            if self.notify.len() > NOTIFY_LINES {
                let _oldest = self.notify.pop_front();
            }
        }
    }

    /// Whether it's open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens or closes it, starting with nothing typed.
    pub fn set_open(&mut self, open: bool) {
        self.open = open;
        self.chat = None;
        self.input.clear();
        self.history_at = 0;
        self.scroll = 0;
    }

    /// Starts a line of chat over the view, for the team or for everyone.
    pub fn open_chat(&mut self, team: bool) {
        self.chat = Some(Chat {
            text: String::new(),
            team,
        });
    }

    /// Whether a line of chat is being typed.
    #[must_use]
    pub fn is_chatting(&self) -> bool {
        self.chat.is_some()
    }

    /// Forgets a line of chat.
    pub fn cancel_chat(&mut self) {
        self.chat = None;
    }

    /// Takes a key for the line of chat, from `Key_Message`. Returns the
    /// command to run, if one was entered.
    pub fn chat_key(&mut self, key: &str) -> Option<String> {
        let shift = self.shift;
        let chat = self.chat.as_mut()?;
        match key {
            "ENTER" => {
                let command = if chat.team { "say_team" } else { "say" };
                // A quote would end the line early.
                let text = chat.text.replace('"', "'");
                self.chat = None;
                return (!text.trim().is_empty()).then(|| format!("{command} \"{text}\""));
            }
            "ESCAPE" => self.chat = None,
            "BACKSPACE" => {
                let _last = chat.text.pop();
            }
            _ => {
                if let Some(character) = typed(key, shift)
                    && chat.text.len() < MAX_CHAT
                {
                    chat.text.push(character);
                }
            }
        }
        None
    }

    /// Forgets what's over the view.
    pub fn clear_notify(&mut self) {
        self.notify.clear();
    }

    /// Empties it.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.notify.clear();
        self.scroll = 0;
    }

    /// Notes whether Shift is held.
    pub fn set_shift(&mut self, held: bool) {
        self.shift = held;
    }

    /// The character `key` types now, by whether Shift is held.
    #[must_use]
    pub fn typed(&self, key: &str) -> Option<char> {
        typed(key, self.shift)
    }

    /// What's being typed.
    #[must_use]
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Replaces what's being typed.
    pub fn set_input(&mut self, text: &str) {
        text.clone_into(&mut self.input);
    }

    /// Steps through what was typed, by `back` lines.
    fn recall(&mut self, back: bool) {
        if back && self.history_at < self.history.len() {
            self.history_at += 1;
        } else if !back && self.history_at > 0 {
            self.history_at -= 1;
        } else {
            return;
        }
        self.input = self
            .history_at
            .checked_sub(1)
            .and_then(|from_end| self.history.iter().rev().nth(from_end))
            .cloned()
            .unwrap_or_default();
    }

    /// Takes a key, by its name. Returns a line to run, if one was entered.
    pub fn key(&mut self, key: &str) -> Option<String> {
        match key {
            "ENTER" => {
                let line = std::mem::take(&mut self.input);
                self.print(&format!("{PROMPT}{line}"), f64::NEG_INFINITY);
                self.history_at = 0;
                self.scroll = 0;
                if line.is_empty() {
                    return None;
                }
                if self.history.len() == MAX_HISTORY {
                    let _oldest = self.history.remove(0);
                }
                self.history.push(line.clone());
                return Some(line);
            }
            "BACKSPACE" => {
                let _last = self.input.pop();
            }
            "UPARROW" => self.recall(true),
            "DOWNARROW" => self.recall(false),
            "PGUP" => {
                let most = self.lines.len().saturating_sub(1);
                self.scroll = (self.scroll + PAGE_LINES).min(most);
            }
            "PGDN" => self.scroll = self.scroll.saturating_sub(PAGE_LINES),
            "HOME" => self.scroll = self.lines.len().saturating_sub(1),
            "END" => self.scroll = 0,
            _ => {
                if let Some(character) = typed(key, self.shift) {
                    self.input.push(character);
                }
            }
        }
        None
    }

    /// Moves it towards where it should be, over `seconds`. `forced` fills the
    /// screen, with nothing to show behind it.
    pub fn update(&mut self, seconds: f32, forced: bool) {
        let wanted = if forced {
            1.0
        } else if self.open {
            COVER
        } else {
            0.0
        };
        let step = seconds * SLIDE_SPEED;
        self.down = if forced {
            1.0
        } else if self.down < wanted {
            (self.down + step).min(wanted)
        } else {
            (self.down - step).max(wanted)
        };
    }

    /// Draws recent lines at the top of the view, at game time `time`, and
    /// under them any line of chat, from `Con_DrawNotify`.
    pub fn draw_notify(&self, canvas: &mut Canvas<'_>, time: f64, blink: bool) {
        let mut y = 0.0;
        for (arrived, line) in &self.notify {
            if time - arrived < NOTIFY_SECONDS && time >= *arrived {
                draw_line(canvas, 0.0, y, line);
                y += CHARACTER_SIZE;
            }
        }
        if let Some(chat) = &self.chat {
            let label = if chat.team { "say_team:" } else { "say:" };
            canvas.text(CHARACTER_SIZE, y, label.as_bytes());
            #[expect(clippy::cast_precision_loss, reason = "a short label")]
            let left = (label.len() as f32 + 2.0) * CHARACTER_SIZE;
            // A long line shows its end, which is where the typing is.
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "columns on a screen"
            )]
            let room = ((canvas.width() - left) / CHARACTER_SIZE).max(2.0) as usize - 1;
            let shown = &chat.text[chat.text.len().saturating_sub(room)..];
            let mut text = shown.to_owned();
            if blink {
                text.push('\u{b}');
            }
            canvas.text(left, y, text.as_bytes());
        }
    }

    /// Draws what it says in the engine's own font.
    fn draw_lines_in(
        &self,
        canvas: &mut Canvas<'_>,
        theme: &mut UiTheme,
        bottom: f32,
        blink: bool,
    ) {
        let (size, cell, gap) = (CHARACTER_SIZE * 1.15, CHARACTER_SIZE, CHARACTER_SIZE * 1.25);
        let mut y = bottom - 2.0 * gap;
        if self.open {
            let mut text = String::with_capacity(self.input.len() + 2);
            text.push(']');
            text.push_str(&self.input);
            if blink {
                text.push('_');
            }
            draw_cells(
                canvas,
                theme,
                (CHARACTER_SIZE, y, cell),
                size,
                &text,
                CONSOLE_INK,
            );
        }
        y -= gap;
        for line in self.lines.iter().rev().skip(self.scroll) {
            if y < 0.0 {
                break;
            }
            let (ink, plain) = match line.strip_prefix(TALK_MARK) {
                Some(said) => (CONSOLE_ACCENT, said),
                None => (CONSOLE_INK, line.as_str()),
            };
            draw_cells(canvas, theme, (CHARACTER_SIZE, y, cell), size, plain, ink);
            y -= gap;
        }
    }

    /// Draws it over `background`, from `Con_DrawConsole`, with `version` in
    /// its corner.
    pub fn draw(
        &self,
        canvas: &mut Canvas<'_>,
        background: &str,
        version: &str,
        blink: bool,
        theme: Option<&mut UiTheme>,
    ) {
        if self.down <= 0.0 {
            return;
        }
        let (width, height) = (canvas.width(), canvas.height());
        let bottom = (height * self.down).floor();
        // Slide the picture down rather than squash it.
        canvas.stretched(background, [0.0, bottom - height, width, bottom], 1.0);
        if let Some(theme) = theme {
            self.draw_lines_in(canvas, theme, bottom, blink);
            return;
        }
        #[expect(clippy::cast_precision_loss, reason = "a short version")]
        let left = width - (version.len() as f32 + 1.0) * CHARACTER_SIZE;
        canvas.bright_text(left, bottom - 2.0 * CHARACTER_SIZE, version.as_bytes());
        let mut y = bottom - 2.0 * CHARACTER_SIZE;
        if self.open {
            let mut text = String::with_capacity(self.input.len() + 2);
            text.push(PROMPT);
            text.push_str(&self.input);
            if blink {
                text.push('\u{b}');
            }
            canvas.text(CHARACTER_SIZE, y, text.as_bytes());
        }
        y -= CHARACTER_SIZE;
        for line in self.lines.iter().rev().skip(self.scroll) {
            if y < 0.0 {
                break;
            }
            draw_line(canvas, CHARACTER_SIZE, y, line);
            y -= CHARACTER_SIZE;
        }
    }
}
