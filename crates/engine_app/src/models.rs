//! Models by name, and how an entity becomes something to draw.

use std::collections::HashMap;
use std::sync::Arc;

use engine_assets::FileSystem;
use engine_client::DrawnEntity;
use engine_core::math::Vec3;
use engine_map::{AliasModel, Map};
use engine_render::{EntityDraw, Renderer};
use fixture_quake::{Palette, parse_mdl};

/// How fast bonus items spin, in degrees a second.
const SPIN_SPEED: f32 = 100.0;

/// The most ambient light on a model, from `R_DrawEntitiesOnList`.
const MAX_AMBIENT: f32 = 128.0;

/// The most light on a model, ambient and directional together.
const MAX_LIGHT: f32 = 192.0;

/// The least light on the player's weapon.
const MIN_WEAPON_LIGHT: f32 = 24.0;

/// Light at which a skin shows as it is.
const NEUTRAL_LIGHT: f32 = 128.0;

/// A model the renderer holds.
struct Loaded {
    /// Its number in the renderer.
    number: u32,
    /// Its data, for frames and flags.
    model: Arc<AliasModel>,
}

/// Every model asked for so far, by name, and the server's numbers for them.
#[derive(Default)]
pub(crate) struct ModelBank {
    /// Each name's model, or `None` if it wouldn't load.
    by_name: HashMap<String, Option<Arc<Loaded>>>,
    /// The server's numbers, in this game.
    by_number: HashMap<u32, Arc<Loaded>>,
}

impl ModelBank {
    /// Forgets the server's numbers, for a new level.
    pub(crate) fn clear_numbers(&mut self) {
        self.by_number.clear();
    }

    /// Notes that the server calls `name` by `number`, loading it if needed.
    pub(crate) fn name_number(
        &mut self,
        number: u32,
        name: &str,
        filesystem: &FileSystem,
        renderer: Option<&mut Renderer>,
    ) {
        let loaded = match self.by_name.get(name) {
            Some(loaded) => loaded.clone(),
            None => {
                let loaded = load(name, filesystem, renderer);
                if loaded.is_some() || !name.is_empty() {
                    self.by_name.insert(name.to_owned(), loaded.clone());
                }
                loaded
            }
        };
        if let Some(loaded) = loaded {
            self.by_number.insert(number, loaded);
        }
    }

    /// How many models are loaded.
    pub(crate) fn len(&self) -> usize {
        self.by_name.values().flatten().count()
    }

    /// How many names wouldn't load.
    pub(crate) fn missing(&self) -> usize {
        self.by_name
            .values()
            .filter(|model| model.is_none())
            .count()
    }

    /// What to draw for `entity`, lit by `map` at `time`.
    pub(crate) fn draw(
        &self,
        entity: &DrawnEntity,
        map: &Map,
        styles: &[f32],
        time: f32,
    ) -> Option<EntityDraw> {
        let state = &entity.state;
        let loaded = self.by_number.get(&u32::from(state.model))?;
        let model = &loaded.model;
        let pose_of = |frame: u16| {
            model
                .frames
                .get(usize::from(frame))
                .or(model.frames.first())
                .map_or(0, |frame| frame.pose_at(time))
        };
        let light = map.light_at(state.origin, styles);
        let ambient = light.min(MAX_AMBIENT);
        let shade = light.min(MAX_LIGHT - ambient);
        let mut angles = state.angles;
        if model.spins {
            angles.y = (time * SPIN_SPEED).rem_euclid(360.0);
        }
        Some(EntityDraw {
            model: loaded.number,
            pose: pose_of(entity.from_frame),
            next_pose: pose_of(state.frame),
            blend: entity.blend,
            skin: u32::from(state.skin),
            origin: state.origin,
            angles,
            ambient: ambient / NEUTRAL_LIGHT,
            shade: shade / NEUTRAL_LIGHT,
            in_front: false,
        })
    }

    /// What to draw for the player's weapon, `model` at `frame`.
    pub(crate) fn draw_weapon(
        &self,
        model: u32,
        frame: u16,
        placed: (Vec3, Vec3),
        light: f32,
        time: f32,
    ) -> Option<EntityDraw> {
        let loaded = self.by_number.get(&model)?;
        let pose = loaded
            .model
            .frames
            .get(usize::from(frame))
            .map_or(0, |frame| frame.pose_at(time));
        let light = light.max(MIN_WEAPON_LIGHT);
        let ambient = light.min(MAX_AMBIENT);
        let shade = light.min(MAX_LIGHT - ambient);
        Some(EntityDraw {
            model: loaded.number,
            pose,
            next_pose: pose,
            blend: 0.0,
            skin: 0,
            origin: placed.0,
            angles: placed.1,
            ambient: ambient / NEUTRAL_LIGHT,
            shade: shade / NEUTRAL_LIGHT,
            in_front: true,
        })
    }
}

/// Loads `name`, and hands it to `renderer`.
fn load(
    name: &str,
    filesystem: &FileSystem,
    renderer: Option<&mut Renderer>,
) -> Option<Arc<Loaded>> {
    let renderer = renderer?;
    let palette = filesystem.read("gfx/palette.lmp").ok()?;
    let palette = Palette::parse(&palette)?;
    let bytes = filesystem.read(name).ok()?;
    let model = Arc::new(parse_mdl(&bytes, &palette)?);
    let number = renderer.add_model(Arc::clone(&model)).ok()?;
    Some(Arc::new(Loaded { number, model }))
}
