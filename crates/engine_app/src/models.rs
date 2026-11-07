//! Models by name, and how an entity becomes something to draw.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use engine_assets::FileSystem;
use engine_client::{BeamPiece, DrawnEntity};
use engine_core::entity::EntityState;
use engine_core::math::Vec3;
use engine_map::{
    AliasModel, JointPose, MOVING_MESH_PREFIX, Map, MovingMesh, NO_MOVE, OccluderSphere, Rows,
    Skeleton, Sprite, blend_poses, moved, placed_at, pose_occlusion,
};
use engine_render::{
    EntityDraw, LightDraw, MovingDraw, Renderer, SpriteDraw, choose_level, height_share,
    model_matrix,
};
use fixture_quake::{load_palette, parse_colored_mdl, parse_item_bsp, parse_mdl, parse_spr};

/// How fast bonus items spin, in degrees a second.
const SPIN_SPEED: f32 = 100.0;

/// The most ambient light on a model, from `R_DrawEntitiesOnList`.
const MAX_AMBIENT: f32 = 128.0;

/// The most light on a model, ambient and directional together.
const MAX_LIGHT: f32 = 192.0;

/// The least light on the player's weapon.
const MIN_WEAPON_LIGHT: f32 = 24.0;

/// How long a change of clip takes to blend, in seconds.
const CLIP_BLEND_SECONDS: f32 = 0.2;

/// What the map calls full light, for what moves.
const MOVING_FULL_LIGHT: f32 = 255.0;

/// Light at which a skin shows as it is.
const NEUTRAL_LIGHT: f32 = 128.0;

/// How long what's far off goes between poses, in seconds, by its level of detail. Where it is
/// goes on changing every frame: this is only how often its limbs do.
const POSE_GAPS: [f32; 4] = [0.0, 0.0, 1.0 / 30.0, 1.0 / 15.0];

/// What a client last saw of an entity's clip.
#[derive(Clone, Debug)]
struct ClipSeen {
    /// The clip, plus one.
    clip: u16,
    /// When it began.
    start: f32,
    /// What it's changing from, as the clip, when that began, and when the change came.
    from: Option<(u16, f32, f32)>,
    /// Which level of detail it's at.
    level: usize,
    /// When it's next posed.
    next_pose: f32,
    /// Its joints, as last posed.
    palette: Vec<Rows>,
    /// Its morph weights, as last posed.
    weights: Vec<f32>,
    /// Its middle and reach, as last posed.
    bounds: (Vec3, f32),
}

impl ClipSeen {
    /// What's seen of something playing `state`'s clip, that's yet to be posed.
    fn new(state: &EntityState) -> Self {
        Self {
            clip: state.clip,
            start: state.clip_start,
            from: None,
            level: 0,
            next_pose: f32::MIN,
            palette: Vec::new(),
            weights: Vec::new(),
            bounds: (Vec3::ZERO, 0.0),
        }
    }
}

/// How `state`'s skeleton stands at `time`, blending out of what it played before.
fn pose_at(
    skeleton: &Skeleton,
    state: &EntityState,
    time: f32,
    seen: &mut ClipSeen,
) -> Vec<JointPose> {
    if seen.clip != state.clip || seen.start.to_bits() != state.clip_start.to_bits() {
        seen.from = Some((seen.clip, seen.start, time));
        seen.clip = state.clip;
        seen.start = state.clip_start;
    }
    let clip_of = |number: u16| usize::from(number).checked_sub(1);
    let pose = skeleton.pose(clip_of(state.clip), time - state.clip_start);
    let Some((clip, start, changed)) = seen.from else {
        return pose;
    };
    let share = (time - changed) / CLIP_BLEND_SECONDS;
    if !(0.0..1.0).contains(&share) {
        seen.from = None;
        return pose;
    }
    blend_poses(&skeleton.pose(clip_of(clip), time - start), &pose, share)
}

/// How much of each morph target shows on `state`: what its clip says, then what a script has set.
fn weights_of(skeleton: &Skeleton, state: &EntityState, time: f32) -> Vec<f32> {
    let clip = usize::from(state.clip).checked_sub(1);
    let mut weights = skeleton.weights(clip, time - state.clip_start);
    for [target, value] in state.morphs {
        if value != 0
            && let Some(weight) = weights.get_mut(usize::from(target))
        {
            *weight = f32::from(value) / 255.0;
        }
    }
    weights
}

/// Works out `state`'s pose, if it's time to.
fn pose_if_due(skeleton: &Skeleton, state: &EntityState, time: f32, seen: &mut ClipSeen) {
    if time < seen.next_pose && seen.clip == state.clip {
        return;
    }
    let gap = POSE_GAPS[seen.level.min(POSE_GAPS.len() - 1)];
    // Spread out by who it is, so that a crowd doesn't all pose in the one frame.
    #[expect(clippy::cast_precision_loss, reason = "only a share of a gap")]
    let share = (state.id % 16) as f32 / 16.0;
    let late = if seen.palette.is_empty() { share } else { 1.0 };
    seen.next_pose = time + gap * late;
    let placed = skeleton.placed(&pose_at(skeleton, state, time, seen));
    seen.bounds = skeleton.bounds(&placed);
    // A mesh with morphs and no joints stays where it is, as a joint that moves nothing.
    seen.palette = if placed.is_empty() {
        vec![NO_MOVE]
    } else {
        skeleton.palette(&placed)
    };
    seen.weights = weights_of(skeleton, state, time);
}

/// Adds `moving`, the map's mesh number `mesh`, as `state` has it, to what's drawn.
fn draw_moving(
    (mesh, moving): (usize, &MovingMesh),
    state: &EntityState,
    (light, lighting): (f32, &Lighting<'_>),
    seen: &mut ClipSeen,
    lists: &mut DrawLists,
) {
    let skeleton = &moving.skeleton;
    pose_if_due(skeleton, state, lighting.time, seen);
    let (middle, radius) = seen.bounds;
    let rows = placed_at(state.origin, state.angles);
    let center = moved(&rows, middle);
    let share = height_share(
        radius,
        (center - lighting.eye).length(),
        lighting.height_scale,
    );
    let shared = &moving.mesh;
    seen.level = choose_level(seen.level, share, &shared.level_sizes, shared.levels.len());
    let first_joint = u32::try_from(lists.palette.len()).unwrap_or(0);
    lists.palette.extend_from_slice(&seen.palette);
    let joint_count = u32::try_from(seen.palette.len()).unwrap_or(0);
    let first_weight = u32::try_from(lists.morph_weights.len()).unwrap_or(0);
    let showing = seen.weights.iter().any(|weight| *weight != 0.0);
    let weight_count = if showing {
        u32::try_from(seen.weights.len()).unwrap_or(0)
    } else {
        0
    };
    if showing {
        lists.morph_weights.extend_from_slice(&seen.weights);
    }
    let level = (light / MOVING_FULL_LIGHT).clamp(0.0, 2.0);
    lists.moving.push(MovingDraw {
        mesh: u32::try_from(mesh).unwrap_or(0),
        rows,
        first_joint,
        joint_count,
        first_weight,
        weight_count,
        light: [level; 3],
        level: u32::try_from(seen.level).unwrap_or(0),
        center,
        radius,
    });
}

/// A model the renderer holds.
struct Loaded {
    /// Its number in the renderer.
    number: u32,
    /// Its data, for frames and flags.
    model: Arc<AliasModel>,
    /// The shade of each pose.
    occlusion: Vec<Vec<OccluderSphere>>,
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
    /// Where the view was last.
    pub(crate) eye: Vec3,
    /// What makes a size a share of the view's height.
    pub(crate) height_scale: f32,
    /// How much of a sun that isn't baked gets to a point.
    pub(crate) sun: &'a dyn Fn(Vec3) -> f32,
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

/// Where the spheres of a model's pose are, as `draw` places them.
fn posed_shade(occlusion: &[Vec<OccluderSphere>], draw: &EntityDraw) -> Vec<(Vec3, f32)> {
    let matrix = model_matrix(draw.origin, draw.angles);
    let place = |sphere: &OccluderSphere| {
        let [x, y, z] = sphere.center;
        let row =
            |at: usize| matrix[at] * x + matrix[at + 4] * y + matrix[at + 8] * z + matrix[at + 12];
        (Vec3::new(row(0), row(1), row(2)), sphere.radius)
    };
    let spheres = occlusion.get(draw.next_pose as usize);
    spheres.into_iter().flatten().map(place).collect()
}

/// What a frame draws, besides the map.
#[derive(Default)]
pub(crate) struct DrawLists {
    /// The models.
    pub(crate) entities: Vec<EntityDraw>,
    /// The sprites.
    pub(crate) sprites: Vec<SpriteDraw>,
    /// Moving meshes.
    pub(crate) moving: Vec<MovingDraw>,
    /// The joints of all of those.
    pub(crate) palette: Vec<Rows>,
    /// The morph weights of all of those.
    pub(crate) morph_weights: Vec<f32>,
    /// The shade of each model, as it's posed: the whole, then its parts.
    pub(crate) shades: Vec<Vec<(Vec3, f32)>>,
}

/// Every model asked for so far, by name, and the server's numbers for them.
#[derive(Default)]
pub(crate) struct ModelBank {
    /// Each name's asset, or `None` if it wouldn't load.
    by_name: HashMap<String, Option<Arc<Asset>>>,
    /// The server's numbers, in this game.
    by_number: HashMap<u32, Arc<Asset>>,
    /// The names of those numbers.
    names: HashMap<u32, String>,
    /// Models in a player's colours, by number and colours.
    colored: HashMap<(u32, u8), Arc<Asset>>,
    /// What each entity showing a moving mesh last played, for blending between clips. It's
    /// only for looks, so it's kept here, and the server's poses don't blend.
    clips: RefCell<HashMap<u64, ClipSeen>>,
}

impl ModelBank {
    /// Forgets the server's numbers, for a new level.
    pub(crate) fn clear_numbers(&mut self) {
        self.by_number.clear();
        self.names.clear();
        self.colored.clear();
        self.clips.borrow_mut().clear();
    }

    /// Loads model `number` in `colors`, the first time.
    pub(crate) fn prepare_colors(
        &mut self,
        number: u16,
        colors: u8,
        filesystem: &FileSystem,
        renderer: Option<&mut Renderer>,
    ) {
        let key = (u32::from(number), colors);
        if self.colored.contains_key(&key) {
            return;
        }
        let name = self.names.get(&key.0).cloned().unwrap_or_default();
        if let Some(asset) = load_colored(&name, colors, filesystem, renderer) {
            self.colored.insert(key, asset);
        }
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
        // A moving mesh is in the map, with no file to load.
        if name.starts_with(MOVING_MESH_PREFIX) {
            self.names.insert(number, name.to_owned());
            return;
        }
        if let Some(asset) = self.asset(name, filesystem, renderer) {
            self.by_number.insert(number, asset);
            self.names.insert(number, name.to_owned());
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

    /// Adds what to draw for `entity`, in `colors` if it wears any.
    pub(crate) fn draw(
        &self,
        entity: &DrawnEntity,
        colors: Option<u8>,
        lighting: &Lighting<'_>,
        lists: &mut DrawLists,
    ) {
        let state = &entity.state;
        let number = u32::from(state.model);
        let name = self.names.get(&number);
        if let Some((mesh, moving)) = name.and_then(|name| lighting.map.moving_mesh(name)) {
            // Only what's baked: the shader adds moving lights to what it's given, as it does
            // for everything else, and adding them here too would count them twice.
            let light =
                lighting.map.light_at(state.origin, lighting.styles) + (lighting.sun)(state.origin);
            let mut clips = self.clips.borrow_mut();
            let seen = clips
                .entry(state.id)
                .or_insert_with(|| ClipSeen::new(state));
            draw_moving((mesh, moving), state, (light, lighting), seen, lists);
            return;
        }
        let colored = colors.and_then(|colors| self.colored.get(&(number, colors)));
        let Some(asset) = colored.or_else(|| self.by_number.get(&number)) else {
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
        let light = (lighting.map.light_at(state.origin, lighting.styles)
            + (lighting.sun)(state.origin))
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
            let draw = EntityDraw {
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
            };
            lists.entities.push(draw);
            lists.shades.push(posed_shade(&loaded.occlusion, &draw));
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

    /// What to draw for the player's weapon, `model` at `frame`, lit where it
    /// is, as `R_DrawViewModel`.
    pub(crate) fn draw_weapon(
        &self,
        model: u32,
        frame: u16,
        placed: (Vec3, Vec3),
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
            .light_at(placed.0, lighting.styles)
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

/// Loads the model `name` in `colors`, and hands it to `renderer`.
fn load_colored(
    name: &str,
    colors: u8,
    filesystem: &FileSystem,
    renderer: Option<&mut Renderer>,
) -> Option<Arc<Asset>> {
    let renderer = renderer?;
    let palette = load_palette(filesystem).ok()?;
    let bytes = filesystem.read(name).ok()?;
    let model = Arc::new(parse_colored_mdl(&bytes, &palette, colors)?);
    let number = renderer.add_model(Arc::clone(&model)).ok()?;
    let occlusion = pose_occlusion(&model);
    let loaded = Loaded {
        number,
        model,
        occlusion,
    };
    Some(Arc::new(Asset::Models(vec![loaded])))
}

/// Loads `name`, and hands it to `renderer`.
fn load(
    name: &str,
    filesystem: &FileSystem,
    renderer: Option<&mut Renderer>,
) -> Option<Arc<Asset>> {
    let renderer = renderer?;
    let palette = load_palette(filesystem).ok()?;
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
        let occlusion = pose_occlusion(&model);
        parts.push(Loaded {
            number,
            model,
            occlusion,
        });
    }
    Some(Arc::new(Asset::Models(parts)))
}
