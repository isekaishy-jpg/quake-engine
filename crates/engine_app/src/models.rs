//! Models by name, and how an entity becomes something to draw.

use std::collections::HashMap;
use std::sync::Arc;

use engine_assets::FileSystem;
use engine_client::{BeamPiece, DrawnEntity};
use engine_core::math::Vec3;
use engine_map::{AliasModel, Map, Sprite};
use engine_render::{EntityDraw, LightDraw, Renderer, SpriteDraw};
use fixture_quake::{Palette, parse_item_bsp, parse_mdl, parse_spr};

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

/// A sprite the renderer holds.
struct LoadedSprite {
    /// Its number in the renderer.
    number: u32,
    /// Its data, for frames.
    sprite: Arc<Sprite>,
}

/// What a name turned out to be.
enum Asset {
    /// One or more models, drawn together.
    Models(Vec<Loaded>),
    /// A sprite.
    Sprite(LoadedSprite),
}

/// What decides how things look this frame.
#[derive(Clone, Copy)]
pub(crate) struct Lighting<'a> {
    /// The map.
    pub(crate) map: &'a Map,
    /// Light style brightnesses.
    pub(crate) styles: &'a [f32],
    /// The dynamic lights.
    pub(crate) lights: &'a [LightDraw],
    /// Seconds of map time.
    pub(crate) time: f32,
}

impl Lighting<'_> {
    /// The dynamic light at `point`, from `R_DrawAliasModel`.
    fn dynamic_at(&self, point: Vec3) -> f32 {
        self.lights
            .iter()
            .map(|light| (light.radius - (point - light.origin).length()).max(0.0))
            .sum()
    }

    /// The ambient and directional light of `light`, as multiples of neutral.
    fn split(light: f32) -> (f32, f32) {
        let ambient = light.min(MAX_AMBIENT);
        let shade = light.min(MAX_LIGHT - ambient);
        (ambient / NEUTRAL_LIGHT, shade / NEUTRAL_LIGHT)
    }
}

/// What a frame draws, besides the map.
#[derive(Default)]
pub(crate) struct DrawLists {
    /// The models.
    pub(crate) entities: Vec<EntityDraw>,
    /// The sprites.
    pub(crate) sprites: Vec<SpriteDraw>,
}

/// Every model asked for so far, by name, and the server's numbers for them.
#[derive(Default)]
pub(crate) struct ModelBank {
    /// Each name's asset, or `None` if it wouldn't load.
    by_name: HashMap<String, Option<Arc<Asset>>>,
    /// The server's numbers, in this game.
    by_number: HashMap<u32, Arc<Asset>>,
}

impl ModelBank {
    /// Forgets the server's numbers, for a new level.
    pub(crate) fn clear_numbers(&mut self) {
        self.by_number.clear();
    }

    /// The asset called `name`, loading it if this is the first time.
    fn asset(
        &mut self,
        name: &str,
        filesystem: &FileSystem,
        renderer: Option<&mut Renderer>,
    ) -> Option<Arc<Asset>> {
        if let Some(asset) = self.by_name.get(name) {
            return asset.clone();
        }
        let asset = load(name, filesystem, renderer);
        if asset.is_some() || !name.is_empty() {
            self.by_name.insert(name.to_owned(), asset.clone());
        }
        asset
    }

    /// Notes that the server calls `name` by `number`, loading it if needed.
    pub(crate) fn name_number(
        &mut self,
        number: u32,
        name: &str,
        filesystem: &FileSystem,
        renderer: Option<&mut Renderer>,
    ) {
        if let Some(asset) = self.asset(name, filesystem, renderer) {
            self.by_number.insert(number, asset);
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

    /// The flags of model `number`, which say what trail it leaves.
    pub(crate) fn flags(&self, number: u16) -> u32 {
        match self.by_number.get(&u32::from(number)).map(Arc::as_ref) {
            Some(Asset::Models(parts)) => parts.first().map_or(0, |loaded| loaded.model.flags),
            _ => 0,
        }
    }

    /// Adds what to draw for `entity`.
    pub(crate) fn draw(
        &self,
        entity: &DrawnEntity,
        lighting: &Lighting<'_>,
        lists: &mut DrawLists,
    ) {
        let state = &entity.state;
        let Some(asset) = self.by_number.get(&u32::from(state.model)) else {
            return;
        };
        let parts = match asset.as_ref() {
            Asset::Models(parts) => parts,
            Asset::Sprite(loaded) => {
                let sprite = &loaded.sprite;
                let frame = sprite
                    .frames
                    .get(usize::from(state.frame))
                    .or(sprite.frames.first());
                if let Some(frame) = frame {
                    lists.sprites.push(SpriteDraw {
                        sprite: loaded.number,
                        image: frame.image_at(lighting.time),
                        origin: state.origin,
                        angles: state.angles,
                    });
                }
                return;
            }
        };
        let light = lighting.map.light_at(state.origin, lighting.styles)
            + lighting.dynamic_at(state.origin);
        let (ambient, shade) = Lighting::split(light);
        let time = lighting.time;
        for loaded in parts {
            let model = &loaded.model;
            let pose_of = |frame: u16| {
                model
                    .frames
                    .get(usize::from(frame))
                    .or(model.frames.first())
                    .map_or(0, |frame| frame.pose_at(time))
            };
            let mut angles = state.angles;
            if model.spins {
                angles.y = (time * SPIN_SPEED).rem_euclid(360.0);
            }
            lists.entities.push(EntityDraw {
                model: loaded.number,
                pose: pose_of(entity.from_frame),
                next_pose: pose_of(state.frame),
                blend: entity.blend,
                skin: u32::from(state.skin),
                origin: state.origin,
                angles,
                ambient,
                shade,
                in_front: false,
            });
        }
    }

    /// Adds a piece of a beam, loading its model the first time.
    pub(crate) fn draw_beam(
        &mut self,
        piece: &BeamPiece,
        filesystem: &FileSystem,
        renderer: Option<&mut Renderer>,
        lists: &mut DrawLists,
    ) {
        let Some(asset) = self.asset(piece.model, filesystem, renderer) else {
            return;
        };
        let Asset::Models(parts) = asset.as_ref() else {
            return;
        };
        for loaded in parts {
            lists.entities.push(EntityDraw {
                model: loaded.number,
                origin: piece.origin,
                angles: piece.angles,
                ambient: 1.0,
                ..EntityDraw::default()
            });
        }
    }

    /// What to draw for the player's weapon, `model` at `frame`, with the eye
    /// at `eye` and the body at `body`.
    pub(crate) fn draw_weapon(
        &self,
        model: u32,
        frame: u16,
        placed: (Vec3, Vec3),
        body: Vec3,
        lighting: &Lighting<'_>,
    ) -> Option<EntityDraw> {
        let Asset::Models(parts) = self.by_number.get(&model)?.as_ref() else {
            return None;
        };
        let loaded = parts.first()?;
        let pose = loaded
            .model
            .frames
            .get(usize::from(frame))
            .map_or(0, |frame| frame.pose_at(lighting.time));
        let light = lighting
            .map
            .light_at(body, lighting.styles)
            .max(MIN_WEAPON_LIGHT)
            + lighting.dynamic_at(placed.0);
        let (ambient, shade) = Lighting::split(light);
        Some(EntityDraw {
            model: loaded.number,
            pose,
            next_pose: pose,
            blend: 0.0,
            skin: 0,
            origin: placed.0,
            angles: placed.1,
            ambient,
            shade,
            in_front: true,
        })
    }
}

/// Loads `name`, and hands it to `renderer`.
fn load(
    name: &str,
    filesystem: &FileSystem,
    renderer: Option<&mut Renderer>,
) -> Option<Arc<Asset>> {
    let renderer = renderer?;
    let palette = filesystem.read("gfx/palette.lmp").ok()?;
    let palette = Palette::parse(&palette)?;
    let bytes = filesystem.read(name).ok()?;
    if name.ends_with(".spr") {
        let sprite = Arc::new(parse_spr(&bytes, &palette)?);
        let number = renderer.add_sprite(Arc::clone(&sprite)).ok()?;
        return Some(Arc::new(Asset::Sprite(LoadedSprite { number, sprite })));
    }
    let models = if name.ends_with(".bsp") {
        parse_item_bsp(&bytes, &palette)?
    } else {
        vec![parse_mdl(&bytes, &palette)?]
    };
    let mut parts = Vec::with_capacity(models.len());
    for model in models {
        let model = Arc::new(model);
        let number = renderer.add_model(Arc::clone(&model)).ok()?;
        parts.push(Loaded { number, model });
    }
    Some(Arc::new(Asset::Models(parts)))
}
