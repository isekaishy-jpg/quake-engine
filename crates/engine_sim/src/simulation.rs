//! Entities, the player and pusher physics.

use std::rc::Rc;
use std::sync::Arc;

use engine_core::entity::{EntityState, ModelInstance};
use engine_core::math::Vec3;
use engine_core::player::{PlayerState, UserCommand};
use engine_map::Map;
use engine_movement::{MoveVars, player_move};
use engine_physics::CollisionWorld;

use crate::flight::{FLAG_ON_GROUND, MoveKind};
use crate::table::{EntityId, SlotTable};

/// The longest a single command may claim to last, in seconds, so a client
/// can't move further than this at once.
const MAX_COMMAND_SECONDS: f32 = 0.1;

/// How fast the view recovers from a knock, in degrees a second.
const PUNCH_RECOVERY: f32 = 10.0;

/// The smallest corner of the player's box, from `PutClientInServer`.
const PLAYER_MINS: Vec3 = Vec3::new(-16.0, -16.0, -24.0);

/// The largest corner of the player's box.
const PLAYER_MAXS: Vec3 = Vec3::new(16.0, 16.0, 32.0);

/// How far `SV_LinkEdict` grows every box, so triggers and the player each
/// reach this much further.
const LINK_MARGIN: f32 = 1.0;

/// How much wider items are than their boxes.
const ITEM_MARGIN: f32 = 15.0;

/// `FL_ITEM`.
const FLAG_ITEM: u32 = 256;

/// `EF_MUZZLEFLASH`, which lasts for one snapshot.
const EFFECT_MUZZLE_FLASH: u8 = 2;

/// How an entity blocks others.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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

/// What the engine knows about an entity. Game fields live in the script.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
}

/// Something a script has to handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Callback {
    /// `id` thinks.
    Think(EntityId),
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
}

/// The game's state.
#[derive(Clone, Debug)]
pub struct Simulation {
    /// The map.
    map: Arc<Map>,
    /// Every entity.
    entities: SlotTable<Entity>,
    /// The player's entity.
    player_id: EntityId,
    /// The player.
    player: PlayerState,
    /// The rules of movement.
    vars: MoveVars,
    /// The last command run.
    last_command: u32,
    /// Game time in seconds.
    time: f64,
    /// Brush models the player hit since last asked.
    touched: Vec<u32>,
    /// The player's last yaw and pitch.
    view: (f32, f32),
    /// The collision scripts' traces use.
    collision: Option<Rc<CollisionWorld>>,
    /// Whether the player's jump button is down.
    jump_down: bool,
    /// Whether the player's attack button is down.
    attack_down: bool,
    /// The impulse waiting to be handled, or 0.
    impulse: u8,
    /// Model names, where a shape's number is its place here plus one.
    shapes: Vec<String>,
    /// How many of those the server has passed on.
    shapes_sent: usize,
    /// The number of the player's weapon model, or 0 for none.
    weapon_model: u16,
    /// The frame the player's weapon shows.
    weapon_frame: u16,
    /// Whether the player's weapon is hidden from their own view.
    weapon_hidden: bool,
    /// How far the view is knocked, in degrees.
    punch: Vec3,
    /// The state of `rand`.
    pub(crate) seed: u32,
    /// When monsters last took a look at where the player can be seen from.
    pub(crate) last_check: f64,
    /// The leaves the player can be seen from.
    pub(crate) player_view: Vec<bool>,
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
    /// A simulation of `map` with the player at `origin`.
    #[must_use]
    pub fn new(map: Arc<Map>, origin: Vec3) -> Self {
        let mut entities = SlotTable::new();
        let player_id = entities.insert(
            Entity {
                mins: PLAYER_MINS,
                maxs: PLAYER_MAXS,
                ..Entity::default()
            },
            0.0,
        );
        Self {
            map,
            entities,
            player_id,
            player: PlayerState {
                origin,
                ..PlayerState::default()
            },
            vars: MoveVars::default(),
            last_command: 0,
            time: 0.0,
            touched: Vec::new(),
            view: (0.0, 0.0),
            collision: None,
            jump_down: false,
            attack_down: false,
            impulse: 0,
            shapes: Vec::new(),
            shapes_sent: 0,
            weapon_model: 0,
            weapon_frame: 0,
            weapon_hidden: false,
            punch: Vec3::ZERO,
            seed: 1,
            last_check: 0.0,
            player_view: Vec::new(),
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

    /// Model names the client hasn't heard yet, with their numbers.
    pub fn take_new_shapes(&mut self) -> Vec<(u16, String)> {
        let first = self.shapes_sent;
        self.shapes_sent = self.shapes.len();
        self.shapes[first..]
            .iter()
            .zip(first..)
            .filter_map(|(name, index)| Some((u16::try_from(index + 1).ok()?, name.clone())))
            .collect()
    }

    /// The name of the player's weapon model, empty for none.
    #[must_use]
    pub fn weapon_model_name(&self) -> &str {
        usize::from(self.weapon_model)
            .checked_sub(1)
            .and_then(|index| self.shapes.get(index))
            .map_or("", String::as_str)
    }

    /// Sets the player's weapon model by name, where empty is none.
    pub fn set_weapon_model(&mut self, name: &str) {
        self.weapon_model = if name.is_empty() {
            0
        } else {
            self.shape_number(name)
        };
    }

    /// The map.
    pub(crate) fn map(&self) -> &Arc<Map> {
        &self.map
    }

    /// How far the view is knocked, in degrees.
    #[must_use]
    pub fn punch(&self) -> Vec3 {
        self.punch
    }

    /// Knocks the view.
    pub fn set_punch(&mut self, punch: Vec3) {
        self.punch = punch;
    }

    /// The frame the player's weapon shows.
    #[must_use]
    pub fn weapon_frame(&self) -> u16 {
        self.weapon_frame
    }

    /// Sets the frame the player's weapon shows.
    pub fn set_weapon_frame(&mut self, frame: u16) {
        self.weapon_frame = frame;
    }

    /// The number of the player's weapon model.
    #[must_use]
    pub fn weapon_model(&self) -> u16 {
        self.weapon_model
    }

    /// The number of the model the player sees, which is 0 for none.
    #[must_use]
    pub fn shown_weapon_model(&self) -> u16 {
        if self.weapon_hidden {
            0
        } else {
            self.weapon_model
        }
    }

    /// Hides the player's weapon from their own view, or shows it again.
    pub fn set_weapon_hidden(&mut self, hidden: bool) {
        self.weapon_hidden = hidden;
    }

    /// Marks the player as dead or alive.
    pub fn set_dead(&mut self, dead: bool) {
        self.player.dead = dead;
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
                origin: entity.origin,
                angles: entity.angles,
            })
            .collect()
    }

    /// Whether the player's jump button is down.
    #[must_use]
    pub fn jump_down(&self) -> bool {
        self.jump_down
    }

    /// Notes the player's buttons, for scripts to see before the move runs.
    pub fn set_buttons(&mut self, command: &UserCommand) {
        self.jump_down = command.jump;
        self.attack_down = command.attack;
        if command.impulse != 0 {
            self.impulse = command.impulse;
        }
    }

    /// Whether the player's attack button is down.
    #[must_use]
    pub fn attack_down(&self) -> bool {
        self.attack_down
    }

    /// The impulse waiting to be handled, or 0.
    #[must_use]
    pub fn impulse(&self) -> u8 {
        self.impulse
    }

    /// Sets the impulse waiting to be handled.
    pub fn set_impulse(&mut self, impulse: u8) {
        self.impulse = impulse;
    }

    /// Sets the collision scripts' traces use.
    pub fn set_world(&mut self, world: Rc<CollisionWorld>) {
        self.collision = Some(world);
    }

    /// The collision scripts' traces use.
    pub(crate) fn collision(&self) -> Option<&Rc<CollisionWorld>> {
        self.collision.as_ref()
    }

    /// The player's last yaw and pitch.
    #[must_use]
    pub fn view(&self) -> (f32, f32) {
        self.view
    }

    /// The player.
    #[must_use]
    pub fn player(&self) -> &PlayerState {
        &self.player
    }

    /// The player's entity.
    #[must_use]
    pub fn player_id(&self) -> EntityId {
        self.player_id
    }

    /// The last command run.
    #[must_use]
    pub fn last_command(&self) -> u32 {
        self.last_command
    }

    /// Game time in seconds.
    #[must_use]
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Adds an entity.
    pub fn spawn(&mut self) -> EntityId {
        self.entities.insert(Entity::default(), self.time)
    }

    /// Removes an entity. The player stays.
    pub fn remove(&mut self, id: EntityId) {
        if id != self.player_id {
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

    /// An entity's position, which for the player is the player's.
    #[must_use]
    pub fn origin(&self, id: EntityId) -> Option<Vec3> {
        if id == self.player_id {
            return Some(self.player.origin);
        }
        self.entities.get(id).map(|entity| entity.origin)
    }

    /// Moves an entity. Moving the player takes it off the ground.
    pub fn set_origin(&mut self, id: EntityId, origin: Vec3) {
        if id == self.player_id {
            self.player.origin = origin;
            self.player.on_ground = false;
            self.player.ground_model = None;
        } else if let Some(entity) = self.entities.get_mut(id) {
            entity.origin = origin;
        }
    }

    /// An entity's velocity.
    #[must_use]
    pub fn velocity(&self, id: EntityId) -> Option<Vec3> {
        if id == self.player_id {
            return Some(self.player.velocity);
        }
        self.entities.get(id).map(|entity| entity.velocity)
    }

    /// Sets an entity's velocity.
    pub fn set_velocity(&mut self, id: EntityId, velocity: Vec3) {
        if id == self.player_id {
            self.player.velocity = velocity;
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
        }
    }

    /// Where every brush model is.
    #[must_use]
    pub fn instances(&self) -> Vec<ModelInstance> {
        self.entities
            .iter()
            .filter_map(|(_, entity)| {
                let solid = entity.solid == Solid::Bsp;
                (entity.visible || solid).then_some(ModelInstance {
                    model: entity.model?,
                    origin: entity.origin,
                    visible: entity.visible,
                    solid,
                    frame: entity.frame,
                })
            })
            .collect()
    }

    /// Runs `command`, unless it's older than one already run.
    pub fn run_command(&mut self, command: &UserCommand, world: &CollisionWorld) {
        if command.sequence <= self.last_command {
            return;
        }
        self.last_command = command.sequence;
        self.view = (command.yaw, command.pitch);
        let mut command = *command;
        command.seconds = command.seconds.clamp(0.0, MAX_COMMAND_SECONDS);
        // `DropPunchAngle`.
        let knock = self.punch.length();
        self.punch = self.punch.normalized() * (knock - PUNCH_RECOVERY * command.seconds).max(0.0);
        let instances = self.instances();
        let boxes = self.solid_boxes();
        player_move(
            &mut self.player,
            &command,
            &self.vars,
            world.with(&instances).with_boxes(&boxes),
            &mut self.touched,
        );
    }

    /// Turns noclip on or off, and returns the new setting.
    pub fn toggle_noclip(&mut self) -> bool {
        self.player.noclip = !self.player.noclip;
        self.player.noclip
    }

    /// Moves the player to `origin`, at rest.
    pub fn teleport(&mut self, origin: Vec3) {
        self.set_origin(self.player_id, origin);
        self.player.velocity = Vec3::ZERO;
    }

    /// Advances `id` by `seconds`, and returns what its script has to handle,
    /// following `SV_Physics_Pusher` and `SV_RunThink`.
    pub fn advance(&mut self, id: EntityId, seconds: f64, world: &CollisionWorld) -> Vec<Callback> {
        let mut callbacks = Vec::new();
        let Some(entity) = self.entities.get(id).copied() else {
            return callbacks;
        };
        if id == self.player_id {
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
                    callbacks.push(Callback::Think(id));
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
                self.run_think(id, &entity, seconds, &mut callbacks);
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "a tick needs no more than f32"
                )]
                self.drift(id, seconds as f32);
            }
            MoveType::Fly | MoveType::Toss | MoveType::FlyMissile | MoveType::Bounce => {
                self.run_think(id, &entity, seconds, &mut callbacks);
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
            callbacks.push(Callback::Think(id));
        }
    }

    /// Everything solid but brush models.
    pub(crate) fn solid_entities(&self) -> impl Iterator<Item = (EntityId, &Entity)> {
        self.entities
            .iter()
            .filter(|(_, entity)| matches!(entity.solid, Solid::Bbox | Solid::SlideBox))
    }

    /// The first entity `test` accepts.
    pub(crate) fn entity_ids_with(&self, test: impl Fn(&Entity) -> bool) -> Option<EntityId> {
        self.entities
            .iter()
            .find(|(_, entity)| test(entity))
            .map(|(id, _)| id)
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
        let before = self.player;
        let blocker = if self.push_player(id, model, step, world) {
            self.push_entities(id, model, step, world)
        } else {
            Err(self.player_id)
        };
        if let Err(blocker) = blocker {
            self.player = before;
            if let Some(pusher) = self.entities.get_mut(id) {
                pusher.origin = start;
                pusher.local_time -= seconds;
            }
            callbacks.push(Callback::Blocked { id, other: blocker });
        }
    }

    /// Moves the player if a pusher carries or hits them, and returns false if
    /// they're stuck.
    fn push_player(
        &mut self,
        id: EntityId,
        model: u32,
        step: Vec3,
        world: &CollisionWorld,
    ) -> bool {
        if self.player.noclip {
            return true;
        }
        let Some(origin) = self.entities.get(id).map(|pusher| pusher.origin) else {
            return true;
        };
        let instances = self.instances();
        let collision = world.with(&instances);
        let riding = self.player.on_ground && self.player.ground_model == Some(model);
        if !riding && !collision.in_model(model, origin, self.player.origin) {
            return true;
        }
        let pushed = collision
            .ignoring(model)
            .trace(self.player.origin, self.player.origin + step);
        self.player.origin = pushed.end;
        !collision.in_solid(self.player.origin)
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
            if other == id || other == self.player_id || still {
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

    /// Triggers overlapping a monster, as `SV_LinkEdict` finds.
    fn monster_touches(&self, touches: &mut Vec<Callback>) {
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
            if entity.move_type != MoveType::Step || entity.solid == Solid::Not {
                continue;
            }
            let low = entity.origin + entity.mins;
            let high = entity.origin + entity.maxs;
            for (id, trigger_low, trigger_high) in &triggers {
                if boxes_touch(low, high, *trigger_low, *trigger_high) {
                    touches.push(Callback::Touch {
                        id: *id,
                        other: Some(other),
                    });
                }
            }
        }
    }

    /// Moves the clock on by `seconds`.
    pub fn end_tick(&mut self, seconds: f64) {
        self.time += seconds;
    }

    /// Touches from the player since last asked: brush models hit, then
    /// triggers overlapping its box.
    pub fn take_touches(&mut self) -> Vec<Callback> {
        let player = self.player_id;
        let mut touches = Vec::new();
        let mut touched = std::mem::take(&mut self.touched);
        touched.sort_unstable();
        touched.dedup();
        for model in touched {
            for (id, entity) in self.entities.iter() {
                if entity.model == Some(model) && entity.solid == Solid::Bsp {
                    touches.push(Callback::Touch {
                        id,
                        other: Some(player),
                    });
                }
            }
        }
        self.monster_touches(&mut touches);
        if !self.player.noclip && !self.player.dead {
            let margin = Vec3::new(LINK_MARGIN, LINK_MARGIN, LINK_MARGIN);
            let low = self.player.origin + PLAYER_MINS - margin;
            let high = self.player.origin + PLAYER_MAXS + margin;
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
                        other: Some(player),
                    });
                }
            }
        }
        touches
    }
}
