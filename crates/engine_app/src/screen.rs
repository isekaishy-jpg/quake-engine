//! What's drawn over the view, and what the menus can reach.

use std::collections::{BTreeMap, HashMap};

use engine_assets::FileSystem;
use engine_compile::decode_png;
use engine_console::Console;
use engine_core::canvas::{Canvas, Image, Picture, PictureSource, Quad};
use engine_core::math::Vec3;
use engine_core::sound::SoundStart;
use engine_render::Renderer;
use engine_script::{UiCommand, UiScript, UiState};
use engine_ui::{
    FontStyle, UiTheme, canvas_scale, draw_frame, draw_piece, draw_plaque, draw_rate, draw_text,
    measure_text,
};
use fixture_quake::{
    BACKGROUND_PICTURE, Hud, Intermission, LOADING_PICTURE, MenuHost, PAUSE_PICTURE, PICTURE_WAD,
    Palette, SAVE_SLOTS, Wad, load_palette, load_picture,
};

use crate::app::World;
use crate::look::{CONSOLE_PICTURE, SPLASH_PICTURE};
use crate::saves;

/// The sound channel of the menus.
const MENU_CHANNEL: u8 = 0;

/// The entity of a sound from the menus, which is no entity.
const MENU_SOUND_ENTITY: u64 = u64::MAX;

/// What's shown in the corner of the console.
const VERSION: &str = concat!("Quake Engine ", env!("CARGO_PKG_VERSION"));

/// What dims the view behind the menus, from `Draw_FadeScreen`.
const MENU_FADE: [f32; 4] = [0.0, 0.0, 0.0, 0.8];

/// Pictures loaded so far, by name.
#[derive(Default)]
pub(crate) struct PictureBank {
    /// Each name's picture, or `None` if it wouldn't load.
    by_name: HashMap<String, Option<Picture>>,
    /// `gfx.wad` and the palette, once they've been read.
    source: Option<(Wad, Palette)>,
}

impl PictureBank {
    /// Reads `gfx.wad` and the palette, the first time.
    fn source(&mut self, filesystem: &FileSystem) -> Option<&(Wad, Palette)> {
        if self.source.is_none() {
            let wad = Wad::parse(&filesystem.read(PICTURE_WAD).ok()?)?;
            let palette = load_palette(filesystem).ok()?;
            self.source = Some((wad, palette));
        }
        self.source.as_ref()
    }
}

/// What's told to the status bar and menu script.
#[expect(clippy::cast_possible_truncation, reason = "a menu's clock")]
fn ui_state(
    size: (f32, f32),
    run_time: f64,
    in_game: bool,
    hud: &Hud,
    script: &UiScript,
    settings: &BTreeMap<String, f64>,
) -> UiState {
    let mut stats = BTreeMap::new();
    if in_game {
        stats.insert("health".to_owned(), f64::from(hud.health()));
        stats.insert("armor".to_owned(), f64::from(hud.armor()));
        stats.insert("ammo".to_owned(), f64::from(hud.ammo()));
    }
    let wanted = script.wanted_settings();
    UiState {
        width: size.0,
        height: size.1,
        time: run_time as f32,
        in_game,
        stats,
        settings: wanted
            .iter()
            .filter_map(|name| Some((name.clone(), *settings.get(name)?)))
            .collect(),
    }
}

/// Carries out what the script asks to draw. What it asks to do waits for
/// [`World::run_ui`].
fn draw_ui(canvas: &mut Canvas<'_>, theme: &mut UiTheme, commands: &[UiCommand]) {
    for command in commands {
        match command {
            UiCommand::Picture { name, at } => canvas.picture(name, at[0], at[1]),
            UiCommand::Stretched { name, place, alpha } => canvas.stretched(name, *place, *alpha),
            UiCommand::Fill { place, color } => canvas.fill(*place, *color),
            UiCommand::Frame { name, place, color } => {
                draw_frame(canvas, theme, name, *place, *color)
            }
            UiCommand::Piece { name, place, color } => {
                draw_piece(canvas, theme, name, *place, *color)
            }
            UiCommand::Moving { name, place, age } => {
                let frame = theme
                    .animations
                    .get(name)
                    .and_then(|moving| moving.frame_at(*age));
                if let Some(frame) = frame.map(str::to_owned) {
                    canvas.stretched(&frame, *place, 1.0);
                }
            }
            UiCommand::Text {
                style,
                at,
                size,
                text,
                color,
                align,
            } => {
                let style = match style.as_str() {
                    "bold" => FontStyle::Bold,
                    "display" => FontStyle::Display,
                    _ => FontStyle::Regular,
                };
                let width = measure_text(canvas, theme, style, *size, text);
                let place = (at[0] - width * align, at[1], *size);
                draw_text(canvas, theme, style, place, text, *color);
            }
            UiCommand::Run(_) | UiCommand::Sound(_) | UiCommand::Close => {}
        }
    }
}

/// How long the splash shows, then how long it takes to fade.
const SPLASH_SECONDS: [f32; 2] = [1.5, 0.6];

/// How long a zone's title takes to come up, stays, and takes to go.
const TITLE_SECONDS: [f32; 3] = [0.5, 2.5, 1.0];

/// How tall a zone's title is, in units.
const TITLE_SIZE: f32 = 26.0;

/// The colour of titles.
const TITLE_INK: [f32; 3] = [0.87, 0.85, 0.8];

/// How long each picture of the spinner shows.
const SPINNER_SECONDS: f32 = 0.08;

/// How many pictures the spinner has.
const SPINNER_PICTURES: u32 = 8;

/// How wide the spinner is, in units.
const SPINNER_SIZE: f32 = 24.0;

/// Draws the picture the engine starts with, if the content has it. It covers the
/// screen, keeping its shape.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a fade needs no more than f32"
)]
fn draw_splash(canvas: &mut Canvas<'_>, run_time: f64) {
    let [hold, fade] = SPLASH_SECONDS;
    let age = run_time as f32;
    if age >= hold + fade {
        return;
    }
    let Some((width, height)) = canvas.size_of(SPLASH_PICTURE) else {
        return;
    };
    let (across, down) = (canvas.width(), canvas.height());
    let grow = (across / width).max(down / height);
    let (shown_width, shown_height) = (width * grow, height * grow);
    let (left, top) = ((across - shown_width) * 0.5, (down - shown_height) * 0.5);
    let alpha = (1.0 - (age - hold) / fade).clamp(0.0, 1.0);
    canvas.fill([0.0, 0.0, across, down], [0.0, 0.0, 0.0, alpha]);
    canvas.stretched(
        SPLASH_PICTURE,
        [left, top, left + shown_width, top + shown_height],
        alpha,
    );
}

/// Draws the spinner in the corner, for as long as something's loading.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "which picture it's on"
)]
fn draw_spinner(canvas: &mut Canvas<'_>, theme: &UiTheme, run_time: f64) {
    let number = (run_time as f32 / SPINNER_SECONDS) as u32 % SPINNER_PICTURES;
    let (right, bottom) = (canvas.width() - 12.0, canvas.height() - 12.0);
    let place = [right - SPINNER_SIZE, bottom - SPINNER_SIZE, right, bottom];
    draw_piece(
        canvas,
        theme,
        &format!("spinner-{number:02}"),
        place,
        [1.0; 4],
    );
}

/// Draws a zone's title, `age` seconds after going in.
fn draw_zone_title(canvas: &mut Canvas<'_>, theme: &mut UiTheme, title: &str, age: f32) {
    let [rise, hold, fall] = TITLE_SECONDS;
    let alpha = if age < rise {
        age / rise
    } else {
        1.0 - (age - rise - hold) / fall
    };
    let alpha = alpha.clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return;
    }
    let width = measure_text(canvas, theme, FontStyle::Display, TITLE_SIZE, title);
    let x = (canvas.width() - width) * 0.5;
    let y = canvas.height() * 0.2;
    let [red, green, blue] = TITLE_INK;
    let place = (x, y, TITLE_SIZE);
    let color = [red, green, blue, alpha];
    draw_text(canvas, theme, FontStyle::Display, place, title, color);
}

/// The PNG at `path`, as a picture.
fn read_png(filesystem: &FileSystem, path: &str) -> Option<Image> {
    let bytes = filesystem.read(path).ok()?;
    let (width, height, rgba) = decode_png(&bytes).ok()?;
    Some(Image {
        width,
        height,
        rgba,
    })
}

/// A bank of pictures, with what it takes to load more.
pub(crate) struct PictureLoader<'a> {
    /// The bank.
    pub(crate) bank: &'a mut PictureBank,
    /// Where they're read from.
    pub(crate) filesystem: &'a FileSystem,
    /// Where they're sent.
    pub(crate) renderer: &'a mut Renderer,
}

impl PictureSource for PictureLoader<'_> {
    fn add(&mut self, name: &str, image: &Image) {
        let number = self.renderer.add_picture(image).ok();
        let picture = number.map(|number| Picture {
            number,
            width: image.width,
            height: image.height,
        });
        self.bank.by_name.insert(name.to_owned(), picture);
    }

    fn picture(&mut self, name: &str) -> Option<Picture> {
        if let Some(picture) = self.bank.by_name.get(name) {
            return *picture;
        }
        let picture = if name.ends_with(".png") {
            read_png(self.filesystem, name)
        } else {
            self.bank
                .source(self.filesystem)
                .and_then(|(wad, palette)| load_picture(name, self.filesystem, wad, palette))
        };
        let picture = picture.and_then(|image| {
            let number = self.renderer.add_picture(&image).ok()?;
            Some(Picture {
                number,
                width: image.width,
                height: image.height,
            })
        });
        self.bank.by_name.insert(name.to_owned(), picture);
        picture
    }
}

impl World {
    /// What a script's told outside of drawing, when the size doesn't matter.
    fn ui_state_now(&self, script: &UiScript) -> UiState {
        ui_state(
            (0.0, 0.0),
            self.run_time.as_secs_f64(),
            self.client.is_some(),
            &self.hud,
            script,
            &self.console_settings,
        )
    }

    /// Opens or closes the script's menu.
    pub(crate) fn toggle_script_menu(&mut self) {
        self.script_menu = !self.script_menu;
        if self.script_menu
            && let Some(script) = &self.ui_script
        {
            let state = self.ui_state_now(script);
            let result = script.open_menu(&state);
            self.after_ui(result);
        }
    }

    /// Hands a key to the script's menu.
    pub(crate) fn menu_key_to_script(&mut self, key: &str) {
        if let Some(script) = &self.ui_script {
            let state = self.ui_state_now(script);
            let result = script.menu_key(&state, key);
            self.after_ui(result);
        }
    }

    /// Does what a script asked to, past drawing.
    /// Carries out what the script asked for, or says what went wrong.
    fn after_ui<E: std::fmt::Display>(&mut self, result: Result<Vec<UiCommand>, E>) {
        match result {
            Ok(commands) => self.run_ui(commands),
            Err(error) => self.note_ui_error(&error),
        }
    }

    /// Says what went wrong in the UI script, once, as it runs every frame.
    fn note_ui_error(&mut self, error: &impl std::fmt::Display) {
        let text = format!("ui script: {error}");
        if self.ui_error.as_ref() != Some(&text) {
            self.ui_error = Some(text.clone());
            self.ui_news.push(text);
        }
    }

    fn run_ui(&mut self, commands: Vec<UiCommand>) {
        for command in commands {
            match command {
                UiCommand::Run(line) => self.commands.push(line),
                UiCommand::Sound(name) => self.play_local(&name),
                UiCommand::Close => self.script_menu = false,
                _ => {}
            }
        }
    }

    /// What's drawn over the view, on a screen of `size` pixels.
    pub(crate) fn overlay(&mut self, size: (u32, u32)) -> Vec<Quad> {
        let playing = self.is_playing_screen();
        let running = self.client.is_some();
        let (time, run_time) = (self.map_time, self.run_time.as_secs_f64());
        let scale = canvas_scale(size.1, self.ui_scale);
        let Some(renderer) = &mut self.renderer else {
            return Vec::new();
        };
        let mut loader = PictureLoader {
            bank: &mut self.pictures,
            filesystem: &self.filesystem,
            renderer,
        };
        let mut canvas = Canvas::new(&mut loader, size, scale);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "which half of a blink"
        )]
        let blink = ((run_time * 4.0) as u64).is_multiple_of(2);
        let mut failed = None;
        if running {
            self.hud.draw_tint(&mut canvas);
            if self.crosshair && playing && self.hud.intermission() == Intermission::None {
                let (x, y) = (canvas.width() / 2.0 - 4.0, canvas.height() / 2.0 - 4.0);
                canvas.character(x, y, b'+');
            }
            match self
                .ui_script
                .as_ref()
                .filter(|script| script.draws_status())
            {
                Some(script) => {
                    self.hud.draw_messages(&mut canvas, playing);
                    if self.hud.intermission() == Intermission::None {
                        let state = ui_state(
                            (canvas.width(), canvas.height()),
                            run_time,
                            running,
                            &self.hud,
                            script,
                            &self.console_settings,
                        );
                        match script.draw_status(&state) {
                            Ok(commands) => draw_ui(&mut canvas, &mut self.theme, &commands),
                            Err(error) => failed = Some(error.to_string()),
                        }
                    }
                }
                None => self.hud.draw(&mut canvas, playing),
            }
            if self.paused {
                draw_plaque(&mut canvas, PAUSE_PICTURE);
            }
            if playing {
                self.console_view.draw_notify(&mut canvas, time, blink);
            }
        }
        let own_look = self.theme.has_fonts() && canvas.size_of(CONSOLE_PICTURE).is_some();
        let (background, theme) = if own_look {
            (CONSOLE_PICTURE, Some(&mut self.theme))
        } else {
            (BACKGROUND_PICTURE, None)
        };
        self.console_view
            .draw(&mut canvas, background, VERSION, blink, theme);
        if self.menu.is_open() {
            // From `M_Draw`: the menus need something quiet behind them.
            let whole = [0.0, 0.0, canvas.width(), canvas.height()];
            if running {
                canvas.fill(whole, MENU_FADE);
            } else {
                canvas.stretched(BACKGROUND_PICTURE, whole, 1.0);
            }
        }
        // The menus only read what they're told, so this doesn't need them to
        // borrow the whole of the world.
        let view = MenuView {
            settings: &self.menu_settings,
            keys: &self.bindings,
        };
        self.menu.draw(&mut canvas, run_time, &view);
        if self.script_menu
            && let Some(script) = &self.ui_script
        {
            let state = ui_state(
                (canvas.width(), canvas.height()),
                run_time,
                running,
                &self.hud,
                script,
                &self.console_settings,
            );
            match script.draw_menu(&state) {
                Ok(commands) => draw_ui(&mut canvas, &mut self.theme, &commands),
                Err(error) => failed = Some(error.to_string()),
            }
        }
        if self.level_pending.is_some() {
            if self.theme.kit.pieces.contains_key("spinner-00") {
                draw_spinner(&mut canvas, &self.theme, run_time);
            } else {
                draw_plaque(&mut canvas, LOADING_PICTURE);
            }
        }
        if let Some((title, began)) = &self.zone_title {
            #[expect(clippy::cast_possible_truncation, reason = "a fade's time")]
            let age = (time - *began) as f32;
            draw_zone_title(&mut canvas, &mut self.theme, title, age);
        }
        draw_splash(&mut canvas, run_time);
        if !self.rate_text.is_empty() {
            draw_rate(&mut canvas, &self.rate_text);
        }
        let quads = canvas.into_quads();
        if let Some(error) = failed {
            self.note_ui_error(&error);
        }
        quads
    }

    /// Plays a sound with no place, as the menus do.
    pub(crate) fn play_local(&mut self, name: &str) {
        let clip = self.sounds.clip(name, &self.filesystem, &mut self.audio);
        if let Some(clip) = clip {
            let sound = SoundStart {
                entity: MENU_SOUND_ENTITY,
                channel: MENU_CHANNEL,
                origin: Vec3::ZERO,
                volume: 1.0,
                attenuation: 0.0,
                own: true,
                name: name.to_owned(),
            };
            self.audio.start(&sound, clip);
        }
    }
}

/// The settings menus draw, copied once a tick.
pub(crate) type MenuSettings = HashMap<&'static str, f32>;

/// The settings the menus show.
pub(crate) const MENU_SETTINGS: [&str; 12] = [
    "topcolor",
    "bottomcolor",
    "gamma",
    "sensitivity",
    "bgmvolume",
    "volume",
    "cl_alwaysrun",
    "m_pitch",
    "vid_vsync",
    "show_fps",
    "fov",
    "ui_scale",
];

/// What the menus see while they're being drawn.
struct MenuView<'a> {
    /// The settings.
    settings: &'a MenuSettings,
    /// The bindings.
    keys: &'a engine_client::Bindings,
}

impl MenuHost for MenuView<'_> {
    fn setting(&self, name: &str) -> f32 {
        self.settings.get(name).copied().unwrap_or(0.0)
    }

    fn set_setting(&mut self, _name: &str, _value: f32) {}

    fn run(&mut self, _command: &str) {}

    fn keys_for(&self, command: &str) -> Vec<String> {
        self.keys.keys_for(command)
    }

    fn play(&mut self, _sound: &str) {}

    fn saves(&self) -> Vec<Option<String>> {
        Vec::new()
    }

    fn in_game(&self) -> bool {
        false
    }

    fn can_save(&self) -> bool {
        false
    }

    fn name(&self) -> String {
        String::new()
    }
}

/// What the menus can do when a key arrives.
pub(crate) struct MenuActions<'a> {
    /// The console.
    pub(crate) console: &'a mut Console<World>,
    /// The world.
    pub(crate) world: &'a mut World,
}

impl MenuHost for MenuActions<'_> {
    fn setting(&self, name: &str) -> f32 {
        self.console
            .cvar(name)
            .map_or(0.0, engine_console::Cvar::number)
    }

    fn set_setting(&mut self, name: &str, value: f32) {
        // Rounded, so steps of a tenth stay tidy in the config.
        let value = (value * 1000.0).round() / 1000.0;
        self.console.add(&format!("{name} {value}"));
    }

    fn run(&mut self, command: &str) {
        self.console.add(command);
    }

    fn keys_for(&self, command: &str) -> Vec<String> {
        self.world.bindings.keys_for(command)
    }

    fn play(&mut self, sound: &str) {
        self.world.play_local(sound);
    }

    fn saves(&self) -> Vec<Option<String>> {
        (0..SAVE_SLOTS)
            .map(|slot| saves::describe(&self.world.user_directory, &format!("s{slot}")))
            .collect()
    }

    fn in_game(&self) -> bool {
        self.world.client.is_some()
    }

    fn can_save(&self) -> bool {
        self.world.can_save()
    }

    fn name(&self) -> String {
        self.world.identity.name.clone()
    }
}
