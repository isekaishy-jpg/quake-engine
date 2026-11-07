//! Reading the engine's own look from content (proposal 0117). A game without it
//! goes on with what it has.

use std::collections::BTreeMap;

use engine_assets::FileSystem;
use engine_script::UiScript;
use engine_ui::{FontStyle, KitPiece, UiAnimation, UiKit, UiTheme};
use serde_json::Value;

/// Where the fonts are.
const FONTS: [(FontStyle, &str); 3] = [
    (
        FontStyle::Regular,
        "fonts/cultist_sans/CultistSans-Regular.ttf",
    ),
    (FontStyle::Bold, "fonts/cultist_sans/CultistSans-Bold.ttf"),
    (
        FontStyle::Display,
        "fonts/cultist_sans/CultistSansDisplay-Regular.ttf",
    ),
];

/// The frames and icons, at twice their size, for a sharp look on a large screen.
const KIT_PICTURE: &str = "ui/kit/atlas/cultist-ui-2x.png";

/// What says where they are.
const KIT_INDEX: &str = "ui/kit/atlas/cultist-ui-2x.json";

/// Pixels of the kit's picture to a unit of the kit.
const KIT_DENSITY: f32 = 2.0;

/// The moving pictures.
const ANIMATIONS: [&str; 2] = ["impact-pop", "smear-swipe"];

/// The script of the status bar and menus.
const UI_SCRIPT: &str = "scripts/ui.luau";

/// The picture shown as the engine starts.
pub(crate) const SPLASH_PICTURE: &str = "ui/cultist-engine-splash-4k.png";

/// The picture behind the console.
pub(crate) const CONSOLE_PICTURE: &str = "ui/cultist-engine-conback-4k.png";

/// Four numbers, from a list.
#[expect(clippy::cast_possible_truncation, reason = "pictures are small")]
fn four(value: Option<&Value>) -> Option<[f32; 4]> {
    let list = value?.as_array()?;
    let number = |at: usize| list.get(at)?.as_f64().map(|number| number as f32);
    Some([number(0)?, number(1)?, number(2)?, number(3)?])
}

/// The kit described by `text`, or `None` if it doesn't make sense.
pub(crate) fn read_kit(text: &[u8]) -> Option<UiKit> {
    let document: Value = serde_json::from_slice(text).ok()?;
    let frames = document.get("frames")?.as_object()?;
    let pieces = frames
        .iter()
        .filter_map(|(name, frame)| {
            Some((
                name.clone(),
                KitPiece {
                    rect: four(frame.get("rect"))?,
                    slices: four(frame.get("slices")).unwrap_or([0.0; 4]),
                },
            ))
        })
        .collect();
    Some(UiKit {
        picture: KIT_PICTURE.to_owned(),
        density: KIT_DENSITY,
        pieces,
    })
}

/// The moving picture in the folder `folder`, as `text` describes it.
#[expect(clippy::cast_possible_truncation, reason = "a frame's time")]
pub(crate) fn read_animation(folder: &str, text: &[u8]) -> Option<UiAnimation> {
    let document: Value = serde_json::from_slice(text).ok()?;
    let frames = document
        .get("frames")?
        .as_array()?
        .iter()
        .filter_map(|frame| {
            let file = frame.get("file")?.as_str()?;
            let seconds = frame.get("durationMs")?.as_f64()? / 1000.0;
            Some((format!("{folder}/{file}"), seconds as f32))
        })
        .collect();
    let loops = document.get("loop").and_then(Value::as_bool);
    Some(UiAnimation {
        frames,
        loops: loops.unwrap_or(false),
    })
}

/// The status bar and menus, if the content has them and the look they need. One
/// that fails says so, and the game's own are used.
#[expect(clippy::print_stderr, reason = "the console isn't up yet")]
pub(crate) fn load_ui_script(filesystem: &FileSystem, theme: &UiTheme) -> Option<UiScript> {
    if !theme.has_fonts() {
        return None;
    }
    let bytes = filesystem.read(UI_SCRIPT).ok()?;
    let source = String::from_utf8(bytes).ok()?;
    match UiScript::new(UI_SCRIPT, &source) {
        Ok(script) => {
            script.set_text_widths(BTreeMap::from([
                (
                    "regular".to_owned(),
                    theme.character_widths(FontStyle::Regular),
                ),
                ("bold".to_owned(), theme.character_widths(FontStyle::Bold)),
                (
                    "display".to_owned(),
                    theme.character_widths(FontStyle::Display),
                ),
            ]));
            Some(script)
        }
        Err(error) => {
            eprintln!("{UI_SCRIPT}: {error}");
            None
        }
    }
}

/// What `filesystem` holds of the engine's own look.
pub(crate) fn load_theme(filesystem: &FileSystem) -> UiTheme {
    let mut theme = UiTheme::default();
    for (style, path) in FONTS {
        if let Ok(bytes) = filesystem.read(path) {
            let _read = theme.add_font(style, &bytes);
        }
    }
    if let Some(kit) = filesystem
        .read(KIT_INDEX)
        .ok()
        .and_then(|text| read_kit(&text))
    {
        theme.kit = kit;
    }
    for name in ANIMATIONS {
        let folder = format!("ui/art/effects/{name}");
        let index = filesystem.read(&format!("{folder}/animation.json"));
        if let Some(animation) = index.ok().and_then(|text| read_animation(&folder, &text)) {
            theme.animations.insert(name.to_owned(), animation);
        }
    }
    theme
}
