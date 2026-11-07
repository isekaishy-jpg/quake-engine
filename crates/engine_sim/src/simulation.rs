//! Entities, players and pusher physics.

use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use engine_core::entity::{EFFECT_MUZZLE_FLASH, EntityState, ModelInstance};
use engine_core::math::Vec3;
use engine_core::player::{PLAYER_MAXS, PLAYER_MINS, PUNCH_RECOVERY, PlayerState, UserCommand};
use engine_map::Map;
use engine_movement::{MoveVars, player_move};
use engine_physics::CollisionWorld;
use serde::{Deserialize, Serialize};

use crate::flight::{FLAG_ON_GROUND, MoveKind};
use crate::table::{EntityId, SlotTable};

/// The longest a single command may claim to last, in seconds, so a client
/// can't move further than this at once.
const MAX_COMMAND_SECONDS: f32 = 0.1;

/// The furthest a shot can look into the past, in seconds, so nobody can be
/// shot from long after they've reached cover.
const MAX_REWIND: f64 = 0.25;

/// How far `SV_LinkEdict` grows every box, so triggers and the player each
/// reach this much further.
const LINK_MARGIN: f32 = 1.0;

/// How much wider items are than their boxes.
const ITEM_MARGIN: f32 = 15.0;

/// `FL_ITEM`.
pub const FLAG_ITEM: u32 = 256;

/// An entity with this flag never sleeps, for long-running jobs.
pub const FLAG_ALWAYS_AWAKE: u32 = 1 << 15;

/// How an entity blocks others.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Solid {
    /// It doesn't.
    #[default]
    Not,
    /// It notices being touched, but doesn't block.
    Trigger,
    /// A solid box.
    Bbox,
    /// A solid box that others slide off.
    SlideBox,
    /// A solid brush model.
    Bsp,
}

/// How an entity moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveType {
    /// It doesn't, by itself.
    #[default]
    None,
    /// It follows its velocity on its own clock, and pushes what's in the way.
    Push,
    /// The player, who moves by commands.
    Walk,
    /// A monster, which moves in steps.
    Step,
    /// It flies, with no gravity.
    Fly,
    /// It falls.
    Toss,
    /// It goes through everything.
    Noclip,
    /// It flies, and hits monsters from further away.
    FlyMissile,
    /// It falls and bounces.
    Bounce,
}

/// Where an entity is held: on a place on another's model.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attachment {
    /// What it's on.
    pub parent: EntityId,
    /// Which of the map's model points.
    pub point: u32,
}

/// What the engine knows about an entity. Game fields live in the script.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    /// Its position.
    pub origin: Vec3,
    /// Units a second.
    pub velocity: Vec3,
    /// The smallest corner of its box, from its origin.
    pub mins: Vec3,
    /// The largest corner of its box, from its origin.
    pub maxs: Vec3,
    /// How it blocks others.
    pub solid: Solid,
    /// How it moves.
    pub move_type: MoveType,
    /// The brush model it shows, if any.
    pub model: Option<u32>,
    /// The number of the other model it shows, or 0 for none.
    pub shape: u16,
    /// Which frame it shows.
    pub frame: u16,
    /// Which skin it shows.
    pub skin: u8,
    /// Where it last touched triggers from. Only moving under its own power
    /// or being pushed touches them again, as `SV_LinkEdict` is told.
    #[serde(default)]
    pub touched_from: Vec3,
    /// Effects, by the original's bits.
    pub effects: u8,
    /// Its pitch, yaw and roll in degrees.
    pub angles: Vec3,
    /// How fast those change, in degrees a second.
    pub angular_velocity: Vec3,
    /// Who made it, which it doesn't hit.
    pub owner: Option<EntityId>,
    /// What it's standing on, if it isn't the world.
    pub ground: Option<EntityId>,
    /// Where a script says it's looking, for what isn't the player.
    pub view_angle: Vec3,
    /// The yaw a monster is turning towards.
    pub ideal_yaw: f32,
    /// How far a monster turns at once, in degrees.
    pub yaw_speed: f32,
    /// The original's flag bits.
    pub flags: u32,
    /// Whether it was in a liquid when last checked.
    pub in_water: bool,
    /// Whether it's been checked at all.
    pub water_known: bool,
    /// Whether the model is drawn.
    pub visible: bool,
    /// A pusher's own clock, in seconds.
    pub local_time: f64,
    /// When it next thinks, or 0 for never.
    pub next_think: f64,
    /// What it's held to, if anything.
    #[serde(default)]
    pub attached: Option<Attachment>,
}

/// Something a script has to handle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Callback {
    /// `id` thinks, at a game time inside the tick, which is 0 for a tick's own
    /// time. Scripts see that time, as `SV_RunThink` arranges, so what repeats
    /// doesn't round up to whole ticks (proposal 0050).
    Think(EntityId, f64),
    /// `id` was blocked by `other`.
    Blocked {
        /// The pusher.
        id: EntityId,
        /// What's in the way.
        other: EntityId,
    },
    /// `id` landed hard.
    Land(EntityId),
    /// `id` was touched by `other`.
    Touch {
        /// What was touched.
        id: EntityId,
        /// What touched it, or `None` for the world.
        other: Option<EntityId>,
    },
    /// `id` crossed the surface of a liquid.
    Splash(EntityId),
    /// `id` woke, after sleeping this many seconds.
    Wake(EntityId, f64),
}

/// One player, by whatever connects them (proposal 0045).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Player {
    /// Their entity.
    id: EntityId,
    /// How they're moving.
    state: PlayerState,
    /// The last command run.
    last_command: u32,
    /// Their last yaw and pitch.
    view: (f32, f32),
    /// Whether their jump button is down.
    jump_down: bool,
    /// Whether their attack button is down.
    attack_down: bool,
    /// The impulse waiting to be handled, or 0.
    impulse: u8,
    /// The number of their weapon model, or 0 for none.
    weapon_model: u16,
    /// The frame their weapon shows.
    weapon_frame: u16,
    /// Whether their weapon is hidden from their own view.
    weapon_hidden: bool,
    /// How far their view is knocked, in degrees.
    punch: Vec3,
    /// Brush models they hit since last asked.
    #[serde(skip)]
    touched: Vec<u32>,
    /// The game time of what they were looking at, or 0 if it's now.
    #[serde(skip)]
    view_time: f64,
    /// Where they've been lately, oldest first, with when.
    #[serde(skip)]
    past: VecDeque<(f64, Vec3)>,
    /// How far back their shots can look, in seconds.
    #[serde(skip, default = "default_rewind_limit")]
    rewind_limit: f64,
}

/// How far back a shot looks until the server says otherwise.
fn default_rewind_limit() -> f64 {
    MAX_REWIND
}

impl Player {
    /// A player on `id`, at `origin`.
    fn new(id: EntityId, origin: Vec3) -> Self {
        Self {
            id,
            state: PlayerState {
                origin,
                ..PlayerState::default()
            },
            last_command: 0,
            view: (0.0, 0.0),
            jump_down: false,
            attack_down: false,
            impulse: 0,
            weapon_model: 0,
            weapon_frame: 0,
            weapon_hidden: false,
            punch: Vec3::ZERO,
            touched: Vec::new(),
            view_time: 0.0,
            past: VecDeque::new(),
            rewind_limit: MAX_REWIND,
        }
    }

    /// Where they were at `time`, between the two places recorded around it,
    /// from `G_TimeShiftClient`.
    fn at_time(&self, time: f64) -> Option<Vec3> {
        let after = self.past.iter().position(|(when, _)| *when >= time)?;
        let (to_time, to) = self.past[after];
        let Some(&(from_time, from)) = after.checked_sub(1).and_then(|i| self.past.get(i)) else {
            return Some(to);
        };
        let span = to_time - from_time;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a blend needs no more than f32"
        )]
        let fraction = if span > 0.0 {
            ((time - from_time) / span) as f32
        } else {
            1.0
        };
        Some(from.lerp(to, fraction))
    }
}

/// Game time when a level starts, from `SV_SpawnServer`. It isn't 0, because
/// scripts tell a field that was never set from one that has passed by comparing
/// it with the time: a teleport that's waiting to be used has 0 for its
/// `nextthink`, and that has to be in the past (proposal 0063).
const LEVEL_START_TIME: f64 = 1.0;

/// What a saved game keeps of a simulation (proposal 0044).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedSimulation {
    /// Every entity.
    entities: SlotTable<Entity>,
    /// Every player.
    players: Vec<Player>,
    /// Game time in seconds.
    time: f64,
    /// Model names, where a shape's number is its place here plus one.
    shapes: Vec<String>,
    /// The state of `rand`.
    seed: u32,
    /// When monsters last took a look at where a player can be seen from.
    last_check: f64,
    /// The player monsters are looking for.
    check_index: usize,
    /// The leaves that player can be seen from.
    player_view: Vec<bool>,
    /// How many more passes touch triggers for everything.
    #[serde(default)]
    force_retouch: u8,
    /// The level's gravity. Saves from before it was kept have the usual.
    #[serde(default = "usual_gravity")]
    gravity: f32,
}

/// `sv_gravity` for almost every level.
fn usual_gravity() -> f32 {
    MoveVars::default().gravity
}

/// The game's state.
#[derive(Clone, Debug)]
pub struct Simulation {
    /// The map.
    map: Arc<Map>,
    /// Every entity.
    entities: SlotTable<Entity>,
    /// Every player. The first is the host's, and stays.
    players: Vec<Player>,
    /// The rules of movement.
    pub(crate) vars: MoveVars,
    /// Game time in seconds.
    time: f64,
    /// The collision scripts' traces use.
    collision: Option<Rc<CollisionWorld>>,
    /// Model names, where a shape's number is its place here plus one.
    shapes: Vec<String>,
    /// What's been given a brush model, in slot order, so that traces needn't
    /// look through everything. Some may have gone since.
    brushes: Vec<EntityId>,
    /// The state of `rand`.
    pub(crate) seed: u32,
    /// When monsters last took a look at where a player can be seen from.
    pub(crate) last_check: f64,
    /// The player monsters are looking for, as `PF_newcheckclient` picks.
    pub(crate) check_index: usize,
    /// The leaves that player can be seen from.
    pub(crate) player_view: Vec<bool>,
    /// How many more passes touch triggers for everything, from
    /// `force_retouch`.
    force_retouch: u8,
    /// Where players are taken to be, while a shot looks into the past.
    rewound: Vec<(EntityId, Vec3)>,
    /// Whether shots show the boxes they're judged against.
    showing_rewind: bool,
    /// The exact time of what scripts are running for.
    event_time: Option<f64>,
    /// Where doors, lifts and monsters have been lately, oldest first.
    things_past: VecDeque<(f64, Vec<(EntityId, Vec3)>)>,
}

/// Whether two boxes overlap.
fn boxes_touch(low_a: Vec3, high_a: Vec3, low_b: Vec3, high_b: Vec3) -> bool {
    low_a.x <= high_b.x
        && low_a.y <= high_b.y
        && low_a.z <= high_b.z
        && high_a.x >= low_b.x
        && high_a.y >= low_b.y
        && high_a.z >= low_b.z
}

impl Simulation {
    /// A simulation of `map` with the first player at `origin`.
    #[must_use]
    pub fn new(map: Arc<Map>, origin: Vec3) -> Self {
        let mut simulation = Self {
            map,
            entities: SlotTable::new(),
            players: Vec::new(),
            vars: MoveVars::default(),
            time: LEVEL_START_TIME,
            collision: None,
            shapes: Vec::new(),
            brushes: Vec::new(),
            seed: 1,
            last_check: 0.0,
            check_index: 0,
            player_view: Vec::new(),
            force_retouch: 0,
            rewound: Vec::new(),
            showing_rewind: false,
            event_time: None,
            things_past: VecDeque::new(),
        };
        let _first = simulation.add_player(origin);
        simulation
    }

    /// Adds a player at `origin`.
    pub fn add_player(&mut self, origin: Vec3) -> EntityId {
        let id = self.entities.insert(
            Entity {
                mins: PLAYER_MINS,
                maxs: PLAYER_MAXS,
                ..Entity::default()
            },
            self.time,
        );
        self.players.push(Player::new(id, origin));
        id
    }

    /// Takes a player out of the game, which isn't for the first.
    pub fn remove_player(&mut self, id: EntityId) {
        if self.players.first().is_some_and(|first| first.id != id) {
            self.players.retain(|player| player.id != id);
            let _removed = self.entities.remove(id, self.time);
            self.check_index = 0;
        }
    }

    /// The players' entities.
    #[must_use]
    pub fn player_ids(&self) -> Vec<EntityId> {
        self.players.iter().map(|player| player.id).collect()
    }

    /// Whether `id` is a player.
    #[must_use]
    pub fn is_player(&self, id: EntityId) -> bool {
        self.players.iter().any(|player| player.id == id)
    }

    /// The player on `id`.
    fn slot(&self, id: EntityId) -> Option<&Player> {
        self.players.iter().find(|player| player.id == id)
    }

    /// The player on `id`.
    fn slot_mut(&mut self, id: EntityId) -> Option<&mut Player> {
        self.players.iter_mut().find(|player| player.id == id)
    }

    /// What a saved game keeps. Naming every field means a new one can't be
    /// forgotten here.
    #[must_use]
    pub fn save(&self) -> SavedSimulation {
        let Self {
            map: _,
            entities,
            players,
            vars,
            time,
            collision: _,
            shapes,
            brushes: _,
            seed,
            last_check,
            check_index,
            player_view,
            force_retouch,
            rewound: _,
            showing_rewind: _,
            event_time: _,
            things_past: _,
        } = self;
        SavedSimulation {
            entities: entities.clone(),
            players: players.clone(),
            time: *time,
            shapes: shapes.clone(),
            seed: *seed,
            last_check: *last_check,
            check_index: *check_index,
            player_view: player_view.clone(),
            force_retouch: *force_retouch,
            gravity: vars.gravity,
        }
    }

    /// A simulation of `map` as it was saved. Its commands count from the
    /// start again, and the client hears every model's name.
    #[must_use]
    pub fn restored(map: Arc<Map>, saved: SavedSimulation) -> Self {
        let mut players = saved.players;
        for player in &mut players {
            player.last_command = 0;
        }
        let brushes = saved
            .entities
            .iter()
            .filter(|(_, entity)| entity.model.is_some())
            .map(|(id, _)| id)
            .collect();
        Self {
            map,
            brushes,
            entities: saved.entities,
            players,
            vars: MoveVars {
                gravity: saved.gravity,
                ..MoveVars::default()
            },
            time: saved.time,
            collision: None,
            shapes: saved.shapes,
            seed: saved.seed,
            last_check: saved.last_check,
            force_retouch: saved.force_retouch,
            check_index: saved.check_index,
            player_view: saved.player_view,
            rewound: Vec::new(),
            showing_rewind: false,
            event_time: None,
            things_past: VecDeque::new(),
        }
    }

    /// The number of the model called `name`, giving it one if it's new.
    fn shape_number(&mut self, name: &str) -> u16 {
        let found = self.shapes.iter().position(|shape| shape == name);
        let index = found.unwrap_or_else(|| {
            self.shapes.push(name.to_owned());
            self.shapes.len() - 1
        });
        u16::try_from(index + 1).unwrap_or(0)
    }

    /// Gives the model called `name` a number now, so clients know it before
    /// anything shows it.
    pub fn precache_shape(&mut self, name: &str) {
        let _number = self.shape_number(name);
    }

    /// Shows the model called `name` on an entity, which gets a box of 32
    /// units each way until `setsize` changes it, as `PF_setmodel` gives.
    pub fn set_shape(&mut self, id: EntityId, name: &str) {
        let shape = self.shape_number(name);
        if let Some(entity) = self.entities.get_mut(id) {
            entity.shape = shape;
            entity.visible = true;
            entity.mins = Vec3::new(-16.0, -16.0, -16.0);
            entity.maxs = Vec3::new(16.0, 16.0, 16.0);
        }
    }

    /// Every model name, where a number is a name's place plus one.
    #[must_use]
    pub fn shapes(&self) -> &[String] {
        &self.shapes
    }

    /// The name of a player's weapon model, empty for none.
    #[must_use]
    pub fn weapon_model_name(&self, id: EntityId) -> &str {
        self.slot(id)
            .and_then(|player| usize::from(player.weapon_model).checked_sub(1))
            .and_then(|index| self.shapes.get(index))
            .map_or("", String::as_str)
    }

    /// Sets a player's weapon model by name, where empty is none.
    pub fn set_weapon_model(&mut self, id: EntityId, name: &str) {
        let number = if name.is_empty() {
            0
        } else {
            self.shape_number(name)
        };
        if let Some(player) = self.slot_mut(id) {
            player.weapon_model = number;
        }
    }

    /// The map.
    #[must_use]
    pub fn map(&self) -> &Arc<Map> {
        &self.map
    }

    /// How far a player's view is knocked, in degrees.
    #[must_use]
    pub fn punch(&self, id: EntityId) -> Vec3 {
        self.slot(id).map_or(Vec3::ZERO, |player| player.punch)
    }

    /// Knocks a player's view.
    pub fn set_punch(&mut self, id: EntityId, punch: Vec3) {
        if let Some(player) = self.slot_mut(id) {
            player.punch = punch;
        }
    }

    /// The frame a player's weapon shows.
    #[must_use]
    pub fn weapon_frame(&self, id: EntityId) -> u16 {
        self.slot(id).map_or(0, |player| player.weapon_frame)
    }

    /// Sets the frame a player's weapon shows.
    pub fn set_weapon_frame(&mut self, id: EntityId, frame: u16) {
        if let Some(player) = self.slot_mut(id) {
            player.weapon_frame = frame;
        }
    }

    /// The number of the model a player sees, which is 0 for none.
    #[must_use]
    pub fn shown_weapon_model(&self, id: EntityId) -> u16 {
        self.slot(id)
            .filter(|player| !player.weapon_hidden)
            .map_or(0, |player| player.weapon_model)
    }

    /// Hides a player's weapon from their own view, or shows it again.
    pub fn set_weapon_hidden(&mut self, id: EntityId, hidden: bool) {
        if let Some(player) = self.slot_mut(id) {
            player.weapon_hidden = hidden;
        }
    }

    /// Marks a player as dead or alive.
    pub fn set_dead(&mut self, id: EntityId, dead: bool) {
        if let Some(player) = self.slot_mut(id) {
            player.state.dead = dead;
        }
    }

    /// Holds a player still, or lets them go.
    pub fn set_frozen(&mut self, id: EntityId, frozen: bool) {
        if let Some(player) = self.slot_mut(id) {
            player.state.frozen = frozen;
            if frozen {
                player.state.velocity = Vec3::ZERO;
            }
        }
    }

    /// Ends every muzzle flash, which only lasts for one snapshot.
    pub fn clear_muzzle_flashes(&mut self) {
        for id in self.entities.ids() {
            if let Some(entity) = self.entities.get_mut(id) {
                entity.effects &= !EFFECT_MUZZLE_FLASH;
            }
        }
    }

    /// Every entity that shows a model.
    #[must_use]
    pub fn entity_states(&self) -> Vec<EntityState> {
        self.entities
            .iter()
            .filter(|(_, entity)| entity.visible && entity.shape != 0)
            .map(|(id, entity)| EntityState {
                id: id.to_bits(),
                model: entity.shape,
                frame: entity.frame,
                skin: entity.skin,
                effects: entity.effects,
                origin: self.origin(id).unwrap_or(entity.origin),
                angles: self.shown_angles(id, entity),
            })
            .collect()
    }

    /// How an entity faces. A player's body turns with their yaw, and leans a
    /// third of their pitch, from `SV_ClientThink`.
    fn shown_angles(&self, id: EntityId, entity: &Entity) -> Vec3 {
        self.slot(id).map_or(entity.angles, |player| {
            let (yaw, pitch) = player.view;
            Vec3::new(-pitch / 3.0, yaw, 0.0)
        })
    }

    /// Whether a player's jump button is down.
    #[must_use]
    pub fn jump_down(&self, id: EntityId) -> bool {
        self.slot(id).is_some_and(|player| player.jump_down)
    }

    /// Notes a player's buttons, for scripts to see before the move runs.
    pub fn set_buttons(&mut self, id: EntityId, command: &UserCommand) {
        if let Some(player) = self.slot_mut(id) {
            player.view_time = command.view_time;
            player.jump_down = command.jump;
            player.attack_down = command.attack;
            if command.impulse != 0 {
                player.impulse = command.impulse;
            }
        }
    }

    /// Whether a player's attack button is down.
    #[must_use]
    pub fn attack_down(&self, id: EntityId) -> bool {
        self.slot(id).is_some_and(|player| player.attack_down)
    }

    /// The impulse waiting to be handled, or 0.
    #[must_use]
    pub fn impulse(&self, id: EntityId) -> u8 {
        self.slot(id).map_or(0, |player| player.impulse)
    }

    /// Sets the impulse waiting to be handled.
    pub fn set_impulse(&mut self, id: EntityId, impulse: u8) {
        if let Some(player) = self.slot_mut(id) {
            player.impulse = impulse;
        }
    }

    /// Sets the collision scripts' traces use.
    pub fn set_world(&mut self, world: Rc<CollisionWorld>) {
        self.collision = Some(world);
    }

    /// The collision scripts' traces use.
    pub(crate) fn collision(&self) -> Option<&Rc<CollisionWorld>> {
        self.collision.as_ref()
    }

    /// A player's last yaw and pitch.
    #[must_use]
    pub fn view(&self, id: EntityId) -> (f32, f32) {
        self.slot(id).map_or((0.0, 0.0), |player| player.view)
    }

    /// The first player.
    #[must_use]
    pub fn player(&self) -> &PlayerState {
        &self.players[0].state
    }

    /// The player on `id`.
    #[must_use]
    pub fn player_of(&self, id: EntityId) -> Option<&PlayerState> {
        self.slot(id).map(|player| &player.state)
    }

    /// The first player's entity.
    #[must_use]
    pub fn player_id(&self) -> EntityId {
        self.players[0].id
    }

    /// The last command a player has run.
    #[must_use]
    pub fn last_command(&self, id: EntityId) -> u32 {
        self.slot(id).map_or(0, |player| player.last_command)
    }

    /// Game time in seconds, which is exact while something's thinking.
    #[must_use]
    pub fn time(&self) -> f64 {
        self.event_time.unwrap_or(self.time)
    }

    /// Sets the time scripts see, or goes back to the tick's own.
    pub fn set_event_time(&mut self, time: Option<f64>) {
        self.event_time = time;
    }

    /// Adds an entity.
    pub fn spawn(&mut self) -> EntityId {
        self.entities.insert(Entity::default(), self.time)
    }

    /// Removes an entity. Players stay.
    pub fn remove(&mut self, id: EntityId) {
        if !self.is_player(id) {
            let _removed = self.entities.remove(id, self.time);
        }
    }

    /// The entity `id`.
    #[must_use]
    pub fn entity(&self, id: EntityId) -> Option<&Entity> {
        self.entities.get(id)
    }

    /// The entity `id`.
    pub fn entity_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        self.entities.get_mut(id)
    }

    /// Every entity, in slot order.
    #[must_use]
    pub fn ids(&self) -> Vec<EntityId> {
        self.entities.ids()
    }

    /// An entity's position, which for a player is the player's.
    #[must_use]
    pub fn origin(&self, id: EntityId) -> Option<Vec3> {
        if let Some((_, origin)) = self.rewound.iter().find(|(player, _)| *player == id) {
            return Some(*origin);
        }
        if let Some(player) = self.slot(id) {
            return Some(player.state.origin);
        }
        self.entities.get(id).map(|entity| entity.origin)
    }

    /// Where the place called `name` on `id` is now, and how it's turned.
    pub fn place_on(&self, id: EntityId, name: &str) -> Option<(Vec3, Vec3)> {
        let parent = self.entities.get(id)?;
        let at = self.map.model_point(parent.model?, name)?;
        self.point_on(parent, at)
    }

    /// Where model point `at` is, on `parent` as it stands.
    fn point_on(&self, parent: &Entity, at: usize) -> Option<(Vec3, Vec3)> {
        let (_, point) = self.map.model_points.get(at)?;
        Some((parent.origin + point.origin, parent.angles + point.angles))
    }

    /// Holds `id` to the place called `name` on `parent`. Says if there's such a place.
    pub fn attach(&mut self, id: EntityId, parent: EntityId, name: &str) -> bool {
        let model = self.entities.get(parent).and_then(|parent| parent.model);
        let at = model.and_then(|model| self.map.model_point(model, name));
        let (Some(at), Some(entity)) = (at, self.entities.get_mut(id)) else {
            return false;
        };
        entity.attached = Some(Attachment {
            parent,
            point: u32::try_from(at).unwrap_or(u32::MAX),
        });
        true
    }

    /// Lets go of `id`, which stays where it is.
    pub fn detach(&mut self, id: EntityId) {
        if let Some(entity) = self.entities.get_mut(id) {
            entity.attached = None;
        }
    }

    /// Puts what's attached where it's held. This is at the end of a tick, once everything's
    /// moved, or it would lag behind what it's on. What's lost its parent lets go.
    fn follow_attachments(&mut self) {
        let held: Vec<(EntityId, Attachment)> = self
            .entities
            .iter()
            .filter_map(|(id, entity)| Some((id, entity.attached?)))
            .collect();
        for (id, attachment) in held {
            let parent = self.entities.get(attachment.parent);
            let place = parent.and_then(|parent| self.point_on(parent, attachment.point as usize));
            let Some((origin, angles)) = place else {
                self.detach(id);
                continue;
            };
            self.set_origin(id, origin);
            if let Some(entity) = self.entities.get_mut(id) {
                entity.angles = angles;
            }
        }
    }

    /// Moves an entity. Moving a player takes them off the ground.
    pub fn set_origin(&mut self, id: EntityId, origin: Vec3) {
        if let Some(player) = self.slot_mut(id) {
            player.state.origin = origin;
            player.state.on_ground = false;
            player.state.ground_model = None;
        } else if let Some(entity) = self.entities.get_mut(id) {
            entity.origin = origin;
            entity.touched_from = origin;
        }
    }

    /// Makes even what isn't moving touch triggers for the next two passes,
    /// from `force_retouch`.
    pub fn force_retouch(&mut self) {
        self.force_retouch = 2;
    }

    /// An entity's velocity.
    #[must_use]
    pub fn velocity(&self, id: EntityId) -> Option<Vec3> {
        if let Some(player) = self.slot(id) {
            return Some(player.state.velocity);
        }
        self.entities.get(id).map(|entity| entity.velocity)
    }

    /// Sets an entity's velocity.
    pub fn set_velocity(&mut self, id: EntityId, velocity: Vec3) {
        if let Some(player) = self.slot_mut(id) {
            player.state.velocity = velocity;
        } else if let Some(entity) = self.entities.get_mut(id) {
            entity.velocity = velocity;
        }
    }

    /// Gives an entity brush model `number`, and that model's box.
    pub fn set_model(&mut self, id: EntityId, number: u32) {
        let model = self.map.models.get(number as usize);
        if let (Some(model), Some(entity)) = (model, self.entities.get_mut(id)) {
            entity.model = Some(number);
            entity.visible = true;
            entity.mins = model.mins;
            entity.maxs = model.maxs;
            let entities = &self.entities;
            self.brushes.retain(|brush| entities.get(*brush).is_some());
            if let Err(at) = self.brushes.binary_search(&id) {
                self.brushes.insert(at, id);
            }
        }
    }

    /// What's been given a brush model and is still here.
    pub(crate) fn brushes(&self) -> impl Iterator<Item = (EntityId, &Entity)> {
        self.brushes
            .iter()
            .filter_map(|id| Some((*id, self.entities.get(*id)?)))
    }

    /// Where the solid brush models are, leaving out `ignored`.
    pub(crate) fn solid_instances(&self, ignored: Option<u32>) -> Vec<ModelInstance> {
        self.brushes()
            .filter(|(_, entity)| entity.solid == Solid::Bsp && entity.model != ignored)
            .filter_map(|(id, entity)| {
                Some(ModelInstance {
                    model: entity.model?,
                    origin: self.instance_origin(id, entity),
                    visible: entity.visible,
                    solid: true,
                    frame: entity.frame,
                })
            })
            .collect()
    }

    /// Where a brush model is for now, which a rewind can change.
    fn instance_origin(&self, id: EntityId, entity: &Entity) -> Vec3 {
        if self.rewound.is_empty() {
            entity.origin
        } else {
            self.origin(id).unwrap_or(entity.origin)
        }
    }

    /// Where every brush model is.
    #[must_use]
    pub fn instances(&self) -> Vec<ModelInstance> {
        self.entities
            .iter()
            .filter_map(|(id, entity)| {
                let solid = entity.solid == Solid::Bsp;
                (entity.visible || solid).then_some(ModelInstance {
                    model: entity.model?,
                    origin: self.instance_origin(id, entity),
                    visible: entity.visible,
                    solid,
                    frame: entity.frame,
                })
            })
            .collect()
    }

    /// Runs `command` for a player, unless it's older than one already run.
    pub fn run_command(&mut self, id: EntityId, command: &UserCommand, world: &CollisionWorld) {
        if command.sequence <= self.last_command(id) {
            return;
        }
        let instances = self.solid_instances(None);
        let boxes = self.solid_boxes(id);
        let vars = self.vars;
        let Some(player) = self.slot_mut(id) else {
            return;
        };
        player.last_command = command.sequence;
        player.view = (command.yaw, command.pitch);
        let mut command = *command;
        command.seconds = command.seconds.clamp(0.0, MAX_COMMAND_SECONDS);
        // `DropPunchAngle`.
        let knock = player.punch.length();
        player.punch =
            player.punch.normalized() * (knock - PUNCH_RECOVERY * command.seconds).max(0.0);
        player_move(
            &mut player.state,
            &command,
            &vars,
            world.with(&instances).with_boxes(&boxes),
            &mut player.touched,
        );
    }

    /// Turns noclip on or off, and returns the new setting.
    pub fn toggle_noclip(&mut self, id: EntityId) -> bool {
        self.slot_mut(id).is_some_and(|player| {
            player.state.noclip = !player.state.noclip;
            player.state.noclip
        })
    }

    /// Sets `sv_gravity`.
    pub fn set_gravity(&mut self, gravity: f32) {
        self.vars.gravity = gravity;
    }

    /// `sv_gravity`.
    #[must_use]
    pub fn gravity(&self) -> f32 {
        self.vars.gravity
    }

    /// Stops a player backing up for `seconds`.
    pub fn set_no_backing(&mut self, id: EntityId, seconds: f32) {
        if let Some(player) = self.slot_mut(id) {
            player.state.no_backing = seconds;
        }
    }

    /// Moves a player to `origin`, at rest.
    pub fn teleport(&mut self, id: EntityId, origin: Vec3) {
        self.set_origin(id, origin);
        if let Some(player) = self.slot_mut(id) {
            player.state.velocity = Vec3::ZERO;
        }
    }

    /// What `id` has to think before moving by `seconds`. Things that fly
    /// think first, in `SV_Physics_Toss` and `SV_Physics_Noclip`, so that they
    /// can stop or turn before they go on.
    pub fn think_before_moving(&mut self, id: EntityId, seconds: f64) -> Option<Callback> {
        let entity = *self.entities.get(id)?;
        let thinks_first = matches!(
            entity.move_type,
            MoveType::Noclip
                | MoveType::Fly
                | MoveType::Toss
                | MoveType::FlyMissile
                | MoveType::Bounce
        );
        if !thinks_first || self.is_player(id) {
            return None;
        }
        let mut callbacks = Vec::new();
        self.run_think(id, &entity, seconds, &mut callbacks);
        callbacks.pop()
    }

    /// Advances `id` by `seconds`, and returns what its script has to handle,
    /// following `SV_Physics_Pusher` and `SV_RunThink`. What thinks before it
    /// moves has had [`Simulation::think_before_moving`] first.
    pub fn advance(&mut self, id: EntityId, seconds: f64, world: &CollisionWorld) -> Vec<Callback> {
        let mut callbacks = Vec::new();
        let Some(entity) = self.entities.get(id).copied() else {
            return callbacks;
        };
        if self.is_player(id) {
            self.run_think(id, &entity, seconds, &mut callbacks);
            return callbacks;
        }
        match entity.move_type {
            MoveType::Push => {
                let think_time = entity.next_think;
                let old_time = entity.local_time;
                let move_time = if think_time < old_time + seconds {
                    (think_time - old_time).max(0.0)
                } else {
                    seconds
                };
                if move_time > 0.0 {
                    self.push_move(id, move_time, world, &mut callbacks);
                }
                let local_time = self.entities.get(id).map_or(old_time, |e| e.local_time);
                if think_time > old_time && think_time <= local_time {
                    if let Some(entity) = self.entities.get_mut(id) {
                        entity.next_think = 0.0;
                    }
                    callbacks.push(Callback::Think(id, 0.0));
                }
            }
            MoveType::None | MoveType::Walk => {
                self.run_think(id, &entity, seconds, &mut callbacks);
            }
            MoveType::Step => {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "a tick needs no more than f32"
                )]
                self.fall(id, seconds as f32, world, &mut callbacks);
                self.run_think(id, &entity, seconds, &mut callbacks);
                self.check_water_of(id, world, &mut callbacks);
            }
            MoveType::Noclip => {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "a tick needs no more than f32"
                )]
                self.drift(id, seconds as f32);
            }
            MoveType::Fly | MoveType::Toss | MoveType::FlyMissile | MoveType::Bounce => {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "a tick needs no more than f32"
                )]
                self.fly(id, seconds as f32, world, &mut callbacks);
            }
        }
        callbacks
    }

    /// `SV_RunThink`.
    fn run_think(
        &mut self,
        id: EntityId,
        entity: &Entity,
        seconds: f64,
        callbacks: &mut Vec<Callback>,
    ) {
        let think_time = entity.next_think;
        if think_time > 0.0 && think_time <= self.time + seconds {
            if let Some(entity) = self.entities.get_mut(id) {
                entity.next_think = 0.0;
            }
            callbacks.push(Callback::Think(id, think_time.max(self.time)));
        }
    }

    /// Everything solid but brush models.
    pub(crate) fn solid_entities(&self) -> impl Iterator<Item = (EntityId, &Entity)> {
        self.entities
            .iter()
            .filter(|(_, entity)| matches!(entity.solid, Solid::Bbox | Solid::SlideBox))
    }

    /// `SV_PushMove`: moves a pusher, and what it carries or runs into. If
    /// anything is in the way, everything goes back.
    fn push_move(
        &mut self,
        id: EntityId,
        seconds: f64,
        world: &CollisionWorld,
        callbacks: &mut Vec<Callback>,
    ) {
        let Some(pusher) = self.entities.get_mut(id) else {
            return;
        };
        if pusher.velocity == Vec3::ZERO {
            pusher.local_time += seconds;
            return;
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a move within one tick needs no more than f32"
        )]
        let step = pusher.velocity * seconds as f32;
        let start = pusher.origin;
        pusher.origin = start + step;
        pusher.local_time += seconds;
        let moved = *pusher;
        let Some(model) = moved.model.filter(|_| moved.solid == Solid::Bsp) else {
            return;
        };
        let before: Vec<PlayerState> = self.players.iter().map(|player| player.state).collect();
        let stuck = self
            .player_ids()
            .into_iter()
            .find(|&player| !self.push_player(player, id, model, step, world));
        let blocker = match stuck {
            Some(player) => Err(player),
            None => self.push_entities(id, model, step, world),
        };
        if let Err(blocker) = blocker {
            for (player, state) in self.players.iter_mut().zip(before) {
                player.state = state;
            }
            if let Some(pusher) = self.entities.get_mut(id) {
                pusher.origin = start;
                pusher.local_time -= seconds;
            }
            callbacks.push(Callback::Blocked { id, other: blocker });
        }
    }

    /// Moves a player if a pusher carries or hits them, and returns false if
    /// they're stuck.
    fn push_player(
        &mut self,
        player: EntityId,
        id: EntityId,
        model: u32,
        step: Vec3,
        world: &CollisionWorld,
    ) -> bool {
        let Some(origin) = self.entities.get(id).map(|pusher| pusher.origin) else {
            return true;
        };
        let instances = self.solid_instances(None);
        let Some(state) = self.slot_mut(player).map(|player| &mut player.state) else {
            return true;
        };
        if state.noclip {
            return true;
        }
        let collision = world.with(&instances);
        let riding = state.on_ground && state.ground_model == Some(model);
        if !riding && !collision.in_model(model, origin, state.origin) {
            return true;
        }
        let pushed = collision
            .ignoring(model)
            .trace(state.origin, state.origin + step);
        state.origin = pushed.end;
        !collision.in_solid(state.origin)
    }

    /// Whether `entity` is in something, from `SV_TestEntityPosition`.
    fn stuck(&self, id: EntityId, entity: &Entity, world: &CollisionWorld) -> bool {
        let size = (entity.mins, entity.maxs);
        let ends = (entity.origin, entity.origin);
        self.trace_move(ends, size, MoveKind::Normal, Some(id), world)
            .start_solid
    }

    /// Moves what a pusher carries or hits. Returns what's stuck, having put
    /// back what it moved.
    fn push_entities(
        &mut self,
        id: EntityId,
        model: u32,
        step: Vec3,
        world: &CollisionWorld,
    ) -> Result<(), EntityId> {
        let Some(pusher) = self.entities.get(id).copied() else {
            return Ok(());
        };
        let (low, high) = (pusher.origin + pusher.mins, pusher.origin + pusher.maxs);
        let mut moved: Vec<(EntityId, Entity)> = Vec::new();
        for other in self.entities.ids() {
            let Some(entity) = self.entities.get(other).copied() else {
                continue;
            };
            let still = matches!(
                entity.move_type,
                MoveType::Push | MoveType::None | MoveType::Noclip | MoveType::Walk
            );
            if other == id || self.is_player(other) || still {
                continue;
            }
            let riding = entity.flags & FLAG_ON_GROUND != 0 && entity.ground == Some(id);
            if !riding {
                let touching = boxes_touch(
                    entity.origin + entity.mins,
                    entity.origin + entity.maxs,
                    low,
                    high,
                );
                if !touching || !self.stuck(other, &entity, world) {
                    continue;
                }
            }
            moved.push((other, entity));
            // `SV_PushEntity`, with the pusher out of the way.
            let path = (
                (entity.origin, entity.origin + step),
                (entity.mins, entity.maxs),
            );
            let trace = self.trace_without(path, MoveKind::Normal, Some(other), world, Some(model));
            let mut pushed = entity;
            pushed.flags &= !FLAG_ON_GROUND;
            pushed.origin = trace.end;
            if let Some(stored) = self.entities.get_mut(other) {
                *stored = pushed;
            }
            if !self.stuck(other, &pushed, world) || pushed.mins.x >= pushed.maxs.x {
                continue;
            }
            if matches!(pushed.solid, Solid::Not | Solid::Trigger) {
                // A corpse, which gets out of the way.
                if let Some(stored) = self.entities.get_mut(other) {
                    stored.mins.x = 0.0;
                    stored.mins.y = 0.0;
                    stored.maxs = stored.mins;
                }
                continue;
            }
            for (back, entity) in moved {
                if let Some(stored) = self.entities.get_mut(back) {
                    *stored = entity;
                }
            }
            return Err(other);
        }
        Ok(())
    }

    /// Triggers overlapping what's moved, as `SV_LinkEdict` finds: monsters
    /// that have stepped, and anything in flight. What's come to rest links no
    /// more, unless the game has asked for everything to.
    fn moved_touches(&mut self, touches: &mut Vec<Callback>) {
        let everything = self.force_retouch > 0;
        self.force_retouch = self.force_retouch.saturating_sub(1);
        let margin = Vec3::new(LINK_MARGIN, LINK_MARGIN, LINK_MARGIN);
        let triggers: Vec<(EntityId, Vec3, Vec3)> = self
            .entities
            .iter()
            .filter(|(_, entity)| entity.solid == Solid::Trigger)
            .map(|(id, entity)| {
                (
                    id,
                    entity.origin + entity.mins - margin,
                    entity.origin + entity.maxs + margin,
                )
            })
            .collect();
        for (other, entity) in self.entities.iter() {
            let flying = matches!(
                entity.move_type,
                MoveType::Toss | MoveType::Bounce | MoveType::Fly | MoveType::FlyMissile
            ) && entity.velocity != Vec3::ZERO;
            let stepped =
                entity.move_type == MoveType::Step && entity.origin != entity.touched_from;
            if !(everything || stepped || flying) || entity.solid == Solid::Not {
                continue;
            }
            // Players touch triggers as they move. What's stored for one here stays at
            // the middle of the world, where it would touch whatever's there.
            if self.slot(other).is_some() {
                continue;
            }
            let low = entity.origin + entity.mins;
            let high = entity.origin + entity.maxs;
            for (id, trigger_low, trigger_high) in &triggers {
                if *id != other && boxes_touch(low, high, *trigger_low, *trigger_high) {
                    touches.push(Callback::Touch {
                        id: *id,
                        other: Some(other),
                    });
                }
            }
        }
        for entity in self.entities.values_mut() {
            entity.touched_from = entity.origin;
        }
    }

    /// Moves the clock on by `seconds`, and notes where players are.
    pub fn end_tick(&mut self, seconds: f64) {
        self.follow_attachments();
        self.time += seconds;
        self.rewound.clear();
        let oldest = self.time - MAX_REWIND - 2.0 * seconds;
        let time = self.time;
        for player in &mut self.players {
            player.past.push_back((time, player.state.origin));
            while player.past.front().is_some_and(|(when, _)| *when < oldest) {
                let _old = player.past.pop_front();
            }
        }
        // Only worth keeping with someone to shoot at it from the past.
        if self.players.len() > 1 {
            let things = self
                .entities
                .iter()
                .filter(|(_, entity)| {
                    matches!(entity.move_type, MoveType::Push | MoveType::Step)
                        && entity.solid != Solid::Not
                })
                .map(|(id, entity)| (id, entity.origin))
                .collect();
            self.things_past.push_back((time, things));
            while self
                .things_past
                .front()
                .is_some_and(|(when, _)| *when < oldest)
            {
                let _old = self.things_past.pop_front();
            }
        }
    }

    /// Where doors, lifts and monsters were at `time`, between the records
    /// around it.
    fn things_at(&self, time: f64) -> Vec<(EntityId, Vec3)> {
        let Some(after) = self.things_past.iter().position(|(when, _)| *when >= time) else {
            return Vec::new();
        };
        let (to_time, to) = &self.things_past[after];
        let Some((from_time, from)) = after.checked_sub(1).and_then(|i| self.things_past.get(i))
        else {
            return to.clone();
        };
        let span = to_time - from_time;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a blend needs no more than f32"
        )]
        let fraction = if span > 0.0 {
            ((time - from_time) / span) as f32
        } else {
            1.0
        };
        let before: HashMap<EntityId, Vec3> = from.iter().copied().collect();
        to.iter()
            .map(|&(id, origin)| {
                let start = before.get(&id).copied().unwrap_or(origin);
                (id, start.lerp(origin, fraction))
            })
            .collect()
    }

    /// Sets how far back a player's shots can look, in seconds.
    pub fn set_rewind_limit(&mut self, id: EntityId, seconds: f64) {
        if let Some(player) = self.slot_mut(id) {
            player.rewind_limit = seconds.clamp(0.0, MAX_REWIND);
        }
    }

    /// Takes in a shot's aim, ahead of the command it came before.
    pub fn aim_player(&mut self, id: EntityId, yaw: f32, pitch: f32, view_time: f64) {
        if let Some(player) = self.slot_mut(id) {
            player.view = (yaw, pitch);
            player.view_time = view_time;
        }
    }

    /// Takes everything else back to where `shooter` saw it, up to their
    /// limit, until [`Simulation::end_rewind`] (proposals 0048 and 0051).
    pub fn rewind(&mut self, shooter: EntityId) {
        let Some(player) = self.slot(shooter) else {
            return;
        };
        let (view_time, limit) = (player.view_time, player.rewind_limit);
        if view_time <= 0.0 || limit <= 0.0 {
            return;
        }
        let time = view_time.max(self.time - limit).min(self.time);
        let mut rewound: Vec<(EntityId, Vec3)> = self
            .players
            .iter()
            .filter(|player| player.id != shooter)
            .filter_map(|player| Some((player.id, player.at_time(time)?)))
            .collect();
        rewound.extend(self.things_at(time));
        // The shooter's own eye is exact too, not rounded to a tick.
        let at = self.event_time.and_then(|time| {
            self.slot(shooter)
                .and_then(|player| player.at_time(time))
                .map(|origin| (shooter, origin))
        });
        rewound.extend(at);
        self.rewound = rewound;
    }

    /// Sets whether shots show the boxes they're judged against.
    pub fn set_showing_rewind(&mut self, showing: bool) {
        self.showing_rewind = showing;
    }

    /// The boxes of the other players as `shooter`'s shot sees them, if those
    /// are being shown.
    #[must_use]
    pub fn shown_rewind(&self, shooter: EntityId) -> Vec<(Vec3, Vec3)> {
        if !self.showing_rewind {
            return Vec::new();
        }
        self.rewound
            .iter()
            .filter(|(id, _)| *id != shooter && self.slot(*id).is_some())
            .filter_map(|(id, origin)| {
                let entity = self.entity(*id)?;
                Some((*origin + entity.mins, *origin + entity.maxs))
            })
            .collect()
    }

    /// Puts players back after a shot.
    pub fn end_rewind(&mut self) {
        self.rewound.clear();
    }

    /// Touches from players since last asked: brush models hit, then triggers
    /// overlapping their boxes.
    pub fn take_touches(&mut self) -> Vec<Callback> {
        let mut touches = Vec::new();
        for index in 0..self.players.len() {
            let player = self.players[index].id;
            let mut touched = std::mem::take(&mut self.players[index].touched);
            touched.sort_unstable();
            touched.dedup();
            for model in touched {
                for (id, entity) in self.brushes() {
                    if entity.model == Some(model) && entity.solid == Solid::Bsp {
                        touches.push(Callback::Touch {
                            id,
                            other: Some(player),
                        });
                    }
                }
            }
        }
        self.moved_touches(&mut touches);
        for player in &self.players {
            let state = player.state;
            if state.dead || state.frozen {
                continue;
            }
            let margin = Vec3::new(LINK_MARGIN, LINK_MARGIN, LINK_MARGIN);
            let low = state.origin + PLAYER_MINS - margin;
            let high = state.origin + PLAYER_MAXS + margin;
            for (id, entity) in self.entities.iter() {
                // `SV_LinkEdict` makes items extra wide.
                let wide = if entity.flags & FLAG_ITEM != 0 {
                    Vec3::new(ITEM_MARGIN, ITEM_MARGIN, 0.0)
                } else {
                    margin
                };
                let touching = entity.solid == Solid::Trigger
                    && boxes_touch(
                        low,
                        high,
                        entity.origin + entity.mins - wide,
                        entity.origin + entity.maxs + wide,
                    );
                if touching {
                    touches.push(Callback::Touch {
                        id,
                        other: Some(player.id),
                    });
                }
            }
        }
        touches
    }
}
