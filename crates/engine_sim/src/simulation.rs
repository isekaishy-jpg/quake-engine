//! Entities, the player and pusher physics.

use std::sync::Arc;

use engine_core::entity::ModelInstance;
use engine_core::math::Vec3;
use engine_core::player::{PlayerState, UserCommand};
use engine_map::Map;
use engine_movement::{MoveVars, player_move};
use engine_physics::CollisionWorld;

use crate::table::{EntityId, SlotTable};

/// The longest a single command may claim to last, in seconds, so a client
/// can't move further than this at once.
const MAX_COMMAND_SECONDS: f32 = 0.1;

/// The smallest corner of the player's box, from `PutClientInServer`.
const PLAYER_MINS: Vec3 = Vec3::new(-16.0, -16.0, -24.0);

/// The largest corner of the player's box.
const PLAYER_MAXS: Vec3 = Vec3::new(16.0, 16.0, 32.0);

/// How far `SV_LinkEdict` grows every box, so triggers and the player each
/// reach this much further.
const LINK_MARGIN: f32 = 1.0;

/// How an entity blocks others.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Solid {
    /// It doesn't.
    #[default]
    Not,
    /// It notices being touched, but doesn't block.
    Trigger,
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
    /// `id` was touched by `other`.
    Touch {
        /// What was touched.
        id: EntityId,
        /// What touched it.
        other: EntityId,
    },
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
    /// Whether the player's jump button is down.
    jump_down: bool,
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
            jump_down: false,
        }
    }

    /// Whether the player's jump button is down.
    #[must_use]
    pub fn jump_down(&self) -> bool {
        self.jump_down
    }

    /// Notes whether the player's jump button is down, for scripts to see
    /// before the move runs.
    pub fn set_jump_down(&mut self, down: bool) {
        self.jump_down = down;
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
        let instances = self.instances();
        player_move(
            &mut self.player,
            &command,
            &self.vars,
            world.with(&instances),
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
            MoveType::None => {
                let think_time = entity.next_think;
                if think_time > 0.0 && think_time <= self.time + seconds {
                    if let Some(entity) = self.entities.get_mut(id) {
                        entity.next_think = 0.0;
                    }
                    callbacks.push(Callback::Think(id));
                }
            }
        }
        callbacks
    }

    /// `SV_PushMove`, for the one thing that can be pushed: the player.
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
        if self.player.noclip {
            return;
        }
        let instances = self.instances();
        let collision = world.with(&instances);
        let riding = self.player.on_ground && self.player.ground_model == Some(model);
        let hit = collision.in_model(model, moved.origin, self.player.origin);
        if !riding && !hit {
            return;
        }
        let before = self.player;
        let pushed = collision
            .ignoring(model)
            .trace(self.player.origin, self.player.origin + step);
        self.player.origin = pushed.end;
        if collision.in_solid(self.player.origin) {
            self.player = before;
            if let Some(pusher) = self.entities.get_mut(id) {
                pusher.origin = start;
                pusher.local_time -= seconds;
            }
            callbacks.push(Callback::Blocked {
                id,
                other: self.player_id,
            });
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
                    touches.push(Callback::Touch { id, other: player });
                }
            }
        }
        if !self.player.noclip {
            let margin = Vec3::new(LINK_MARGIN, LINK_MARGIN, LINK_MARGIN);
            let low = self.player.origin + PLAYER_MINS - margin;
            let high = self.player.origin + PLAYER_MAXS + margin;
            for (id, entity) in self.entities.iter() {
                let touching = entity.solid == Solid::Trigger
                    && boxes_touch(
                        low,
                        high,
                        entity.origin + entity.mins - margin,
                        entity.origin + entity.maxs + margin,
                    );
                if touching {
                    touches.push(Callback::Touch { id, other: player });
                }
            }
        }
        touches
    }
}
