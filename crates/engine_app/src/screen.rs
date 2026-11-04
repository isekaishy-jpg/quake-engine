//! What's drawn over the view, and what the menus can reach.

use std::collections::HashMap;

use engine_assets::FileSystem;
use engine_console::Console;
use engine_core::canvas::{Canvas, Picture, PictureSource, Quad};
use engine_core::math::Vec3;
use engine_core::sound::SoundStart;
use engine_render::Renderer;
use engine_ui::{canvas_scale, draw_plaque, draw_rate};
use fixture_quake::{Intermission, MenuHost, Palette, SAVE_SLOTS, Wad, load_picture};

use crate::app::World;
use crate::saves;

/// The sound channel of the menus.
const MENU_CHANNEL: u8 = 0;

/// The entity of a sound from the menus, which is no entity.
const MENU_SOUND_ENTITY: u64 = u64::MAX;

/// What's shown in the corner of the console.
const VERSION: &str = concat!("Quake Engine ", env!("CARGO_PKG_VERSION"));

/// What dims the view behind the menus, from `Draw_FadeScreen`.
const MENU_FADE: [f32; 4] = [0.0, 0.0, 0.0, 0.8];

/// What's behind the menus with no game running.
const BACKGROUND_PICTURE: &str = "gfx/conback.lmp";

/// The picture shown while paused.
const PAUSE_PICTURE: &str = "gfx/pause.lmp";

/// The picture shown while a level loads.
const LOADING_PICTURE: &str = "gfx/loading.lmp";

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
            let wad = Wad::parse(&filesystem.read("gfx.wad").ok()?)?;
            let palette = Palette::parse(&filesystem.read("gfx/palette.lmp").ok()?)?;
            self.source = Some((wad, palette));
        }
        self.source.as_ref()
    }
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
    fn picture(&mut self, name: &str) -> Option<Picture> {
        if let Some(picture) = self.bank.by_name.get(name) {
            return *picture;
        }
        let picture = self
            .bank
            .source(self.filesystem)
            .and_then(|(wad, palette)| load_picture(name, self.filesystem, wad, palette));
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
        if running {
            self.hud.draw_tint(&mut canvas);
            if self.crosshair && playing && self.hud.intermission() == Intermission::None {
                let (x, y) = (canvas.width() / 2.0 - 4.0, canvas.height() / 2.0 - 4.0);
                canvas.character(x, y, b'+');
            }
            self.hud.draw(&mut canvas, playing);
            if self.paused {
                draw_plaque(&mut canvas, PAUSE_PICTURE);
            }
            if playing {
                self.console_view.draw_notify(&mut canvas, time);
            }
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "which half of a blink"
        )]
        let blink = ((run_time * 4.0) as u64).is_multiple_of(2);
        self.console_view.draw(&mut canvas, VERSION, blink);
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
        if self.level_pending.is_some() {
            draw_plaque(&mut canvas, LOADING_PICTURE);
        }
        if !self.rate_text.is_empty() {
            draw_rate(&mut canvas, &self.rate_text);
        }
        canvas.into_quads()
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
