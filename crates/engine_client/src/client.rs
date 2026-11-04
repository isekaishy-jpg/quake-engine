//! The client: commands, prediction and the render tick (proposal 0024).

use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use engine_core::entity::{Effect, EntityState, ModelInstance};
use engine_core::math::Vec3;
use engine_core::player::{PlayerState, UserCommand};
use engine_core::sound::{SoundStart, StaticSound};
use engine_movement::{MoveVars, player_move};
use engine_net::{
    ClientMessage, PlayerInfo, PlayerScore, ServerMessage, Snapshot, Transport, apply_delta,
    delta_baseline,
};
use engine_physics::CollisionWorld;

use crate::lightstyle::{LIGHT_STYLE_COUNT, LightStyles};

/// Further than this in a tick is a teleport, and isn't blended.
const MAX_BLENDED_MOVE: f32 = 200.0;

/// How many ticks a model takes to blend to a new frame, which is the original's
/// 10 frames a second.
const FRAME_BLEND_TICKS: f32 = 7.2;

/// Something the game wants the player to see.
#[derive(Clone, Debug, PartialEq)]
pub enum ScreenEvent {
    /// Text for the middle of the screen.
    CenterPrint(String),
    /// A command for the console.
    Command(String),
    /// The player was hurt.
    Damage {
        /// How much their armour took.
        armor: u8,
        /// How much they took.
        blood: u8,
        /// Where it came from.
        from: Vec3,
    },
    /// An end of level screen starts, by the game's number.
    Intermission(u8),
    /// The level's title.
    LevelTitle(String),
}

/// An entity as it should be drawn now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawnEntity {
    /// Where it is now, showing the frame it's blending to.
    pub state: EntityState,
    /// The frame it's blending from.
    pub from_frame: u16,
    /// How far to `state`'s frame, from 0 to 1.
    pub blend: f32,
}

/// An entity's change of frame.
#[derive(Clone, Copy, Debug, Default)]
struct FrameChange {
    /// The frame before.
    from: u16,
    /// The tick it changed on.
    tick: u32,
}

/// The most commands waiting for the server. Older ones are forgotten.
const MAX_PENDING: usize = 512;

/// How many of the latest commands go in each move, so a lost packet costs
/// nothing, as `CL_SendMove` does in QuakeWorld.
const COMMANDS_PER_MOVE: usize = 3;

/// How many snapshots are kept to apply differences to.
const SNAPSHOT_HISTORY: usize = 64;

/// The local player's side of a game.
pub struct Client {
    /// The connection to the server.
    transport: Box<dyn Transport>,
    /// The map's collision.
    world: Rc<CollisionWorld>,
    /// The rules of movement.
    vars: MoveVars,
    /// The next command's number.
    next_sequence: u32,
    /// Commands the server hasn't run yet.
    pending: VecDeque<UserCommand>,
    /// The player after every command so far.
    predicted: PlayerState,
    /// Brush models in the latest snapshot.
    instances: Vec<ModelInstance>,
    /// Brush models in the snapshot before it.
    previous_instances: Vec<ModelInstance>,
    /// The light styles.
    styles: LightStyles,
    /// Text from the server, waiting to be shown.
    prints: Vec<String>,
    /// A view the server wants, waiting to be applied.
    view: Option<(f32, f32)>,
    /// Solid boxes in the latest snapshot.
    boxes: Vec<(Vec3, Vec3)>,
    /// Entities in the latest snapshot.
    entities: Vec<EntityState>,
    /// Entities in the snapshot before it, by id.
    previous_entities: HashMap<u64, EntityState>,
    /// The last change of frame of each entity.
    frame_changes: HashMap<u64, FrameChange>,
    /// The latest snapshot's tick.
    tick: u32,
    /// The player's weapon model and frame.
    weapon: (u16, u16),
    /// The player's entity.
    player_entity: u64,
    /// The player's effect bits.
    player_effects: u8,
    /// The view's knock in the latest snapshot.
    punch: Vec3,
    /// The view's knock in the snapshot before it.
    previous_punch: Vec3,
    /// Model names waiting to be loaded.
    model_names: Vec<(u16, String)>,
    /// Effects from the server, waiting to be shown.
    effects: Vec<Effect>,
    /// Sounds from the server, waiting to be played.
    sounds: Vec<SoundStart>,
    /// Sounds at fixed places, waiting to be added.
    static_sounds: Vec<StaticSound>,
    /// A music track waiting to be played.
    music: Option<u8>,
    /// Game time in the latest snapshot.
    time: f64,
    /// The game's numbers for its status display.
    stats: Vec<i32>,
    /// What the game has to show, waiting to be taken.
    events: Vec<ScreenEvent>,
    /// Recent snapshots, newest last.
    history: VecDeque<Snapshot>,
    /// A level the server wants loaded.
    level: Option<String>,
    /// The latest scores.
    scores: Vec<PlayerScore>,
    /// Scratch space for what a move hit, which only the server acts on.
    touched: Vec<u32>,
}

impl Client {
    /// A client with its player at `origin`.
    #[must_use]
    pub fn new(transport: Box<dyn Transport>, world: Rc<CollisionWorld>, origin: Vec3) -> Self {
        Self {
            transport,
            world,
            vars: MoveVars::default(),
            next_sequence: 1,
            pending: VecDeque::new(),
            predicted: PlayerState {
                origin,
                ..PlayerState::default()
            },
            instances: Vec::new(),
            previous_instances: Vec::new(),
            styles: LightStyles::new(),
            prints: Vec::new(),
            view: None,
            boxes: Vec::new(),
            entities: Vec::new(),
            previous_entities: HashMap::new(),
            frame_changes: HashMap::new(),
            tick: 0,
            weapon: (0, 0),
            player_entity: 0,
            player_effects: 0,
            punch: Vec3::ZERO,
            previous_punch: Vec3::ZERO,
            model_names: Vec::new(),
            effects: Vec::new(),
            sounds: Vec::new(),
            static_sounds: Vec::new(),
            music: None,
            time: 0.0,
            stats: Vec::new(),
            events: Vec::new(),
            history: VecDeque::with_capacity(SNAPSHOT_HISTORY),
            level: None,
            scores: Vec::new(),
            touched: Vec::new(),
        }
    }

    /// Runs `command` on `state`, against the latest brush model positions.
    fn run_on(&mut self, state: &mut PlayerState, command: &UserCommand) {
        self.touched.clear();
        let collision = self.world.with(&self.instances).with_boxes(&self.boxes);
        player_move(state, command, &self.vars, collision, &mut self.touched);
    }

    /// Where brush models should be drawn, `fraction` of the way from the
    /// last snapshot but one to the last, so they run one tick behind.
    #[must_use]
    pub fn draw_instances(&self, fraction: f32) -> Vec<ModelInstance> {
        if self.previous_instances.len() != self.instances.len() {
            return self.instances.clone();
        }
        self.previous_instances
            .iter()
            .zip(&self.instances)
            .map(|(from, to)| ModelInstance {
                origin: if from.model == to.model {
                    from.origin.lerp(to.origin, fraction)
                } else {
                    to.origin
                },
                ..*to
            })
            .collect()
    }

    /// How far behind the last snapshot `state`'s ground is drawn. Adding this
    /// keeps a player riding a brush model on it, because the model is drawn
    /// between ticks while the player only moves with it on them.
    #[must_use]
    pub fn ground_lag(&self, state: &PlayerState, fraction: f32) -> Vec3 {
        let origin_in = |instances: &[ModelInstance]| {
            instances
                .iter()
                .find(|instance| Some(instance.model) == state.ground_model)
                .map(|instance| instance.origin)
        };
        match (
            origin_in(&self.previous_instances),
            origin_in(&self.instances),
        ) {
            (Some(from), Some(to)) => from.lerp(to, fraction) - to,
            _ => Vec3::ZERO,
        }
    }

    /// Entities as they should be drawn, `fraction` of the way from the last
    /// snapshot but one to the last.
    #[must_use]
    pub fn draw_entities(&self, fraction: f32) -> Vec<DrawnEntity> {
        self.entities
            .iter()
            .map(|entity| {
                let mut state = *entity;
                if let Some(from) = self.previous_entities.get(&entity.id)
                    && (entity.origin - from.origin).length() < MAX_BLENDED_MOVE
                {
                    state.origin = from.origin.lerp(entity.origin, fraction);
                    state.angles = blend_angles(from.angles, entity.angles, fraction);
                }
                let change = self.frame_changes.get(&entity.id).copied();
                let (from_frame, blend) = change.map_or((entity.frame, 1.0), |change| {
                    let ticks = self.tick.wrapping_sub(change.tick) as f32 + fraction;
                    (change.from, (ticks / FRAME_BLEND_TICKS).min(1.0))
                });
                DrawnEntity {
                    state,
                    from_frame,
                    blend,
                }
            })
            .collect()
    }

    /// The player's weapon model and frame.
    #[must_use]
    pub fn weapon(&self) -> (u16, u16) {
        self.weapon
    }

    /// The player's entity.
    #[must_use]
    pub fn player_entity(&self) -> u64 {
        self.player_entity
    }

    /// The player's effect bits.
    #[must_use]
    pub fn player_effects(&self) -> u8 {
        self.player_effects
    }

    /// How far the view is knocked, in degrees, `fraction` of the way from the
    /// last snapshot but one to the last.
    #[must_use]
    pub fn punch(&self, fraction: f32) -> Vec3 {
        self.previous_punch.lerp(self.punch, fraction)
    }

    /// Game time in the latest snapshot.
    #[must_use]
    pub fn time(&self) -> f64 {
        self.time
    }

    /// The game's numbers for its status display.
    #[must_use]
    pub fn stats(&self) -> &[i32] {
        &self.stats
    }

    /// Takes what the game has asked to show.
    pub fn take_events(&mut self) -> Vec<ScreenEvent> {
        std::mem::take(&mut self.events)
    }

    /// Takes the effects that have happened.
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    /// Takes the model names the server has given.
    pub fn take_model_names(&mut self) -> Vec<(u16, String)> {
        std::mem::take(&mut self.model_names)
    }

    /// Every light style's brightness at `time`.
    #[must_use]
    pub fn light_styles(&self, time: f64) -> [f32; LIGHT_STYLE_COUNT] {
        self.styles.values(time)
    }

    /// Takes text from the server.
    pub fn take_prints(&mut self) -> Vec<String> {
        std::mem::take(&mut self.prints)
    }

    /// Takes the sounds that have started.
    pub fn take_sounds(&mut self) -> Vec<SoundStart> {
        std::mem::take(&mut self.sounds)
    }

    /// Takes the sounds at fixed places that have been added.
    pub fn take_static_sounds(&mut self) -> Vec<StaticSound> {
        std::mem::take(&mut self.static_sounds)
    }

    /// Takes the music track the server wants.
    pub fn take_music(&mut self) -> Option<u8> {
        self.music.take()
    }

    /// Takes a yaw and pitch the server wants.
    pub fn take_view(&mut self) -> Option<(f32, f32)> {
        self.view.take()
    }

    /// The player after every command so far.
    #[must_use]
    pub fn predicted(&self) -> &PlayerState {
        &self.predicted
    }

    /// How many commands the server hasn't run yet.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Numbers `command`, sends it, and predicts its result.
    pub fn send_command(&mut self, mut command: UserCommand, seconds: f32) {
        command.sequence = self.next_sequence;
        command.seconds = seconds;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        let mut predicted = self.predicted;
        self.run_on(&mut predicted, &command);
        self.predicted = predicted;
        if self.pending.len() == MAX_PENDING {
            let _oldest = self.pending.pop_front();
        }
        self.pending.push_back(command);
        let first = self.pending.len().saturating_sub(COMMANDS_PER_MOVE);
        let commands: Vec<UserCommand> = self.pending.iter().skip(first).copied().collect();
        let seen = self.history.back().map_or(0, |snapshot| snapshot.tick);
        self.transport
            .send_unreliable(ClientMessage::Move(commands, seen).encode());
    }

    /// Takes a level the server wants loaded.
    pub fn take_level(&mut self) -> Option<String> {
        self.level.take()
    }

    /// Says the level is loaded.
    pub fn send_ready(&mut self) {
        self.transport.send(ClientMessage::Ready.encode());
    }

    /// Says who this client is.
    pub fn send_info(&mut self, name: &str, colors: u8) {
        let info = PlayerInfo {
            name: name.to_owned(),
            colors,
        };
        self.transport.send(ClientMessage::Info(info).encode());
    }

    /// The latest scores.
    #[must_use]
    pub fn scores(&self) -> &[PlayerScore] {
        &self.scores
    }

    /// Whether the server is still there.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.transport.is_connected()
    }

    /// Takes in a snapshot, unless a newer one has already arrived.
    fn take_snapshot(&mut self, snapshot: Snapshot) {
        if self
            .history
            .back()
            .is_some_and(|last| last.tick >= snapshot.tick)
        {
            return;
        }
        if self.history.len() == SNAPSHOT_HISTORY {
            let _oldest = self.history.pop_front();
        }
        self.history.push_back(snapshot.clone());
        self.pending
            .retain(|command| command.sequence > snapshot.acknowledged);
        self.previous_instances = std::mem::replace(&mut self.instances, snapshot.instances);
        self.boxes = snapshot.boxes;
        self.note_entities(snapshot.tick, snapshot.entities);
        self.weapon = (snapshot.weapon_model, snapshot.weapon_frame);
        self.player_entity = snapshot.player_entity;
        self.player_effects = snapshot.player_effects;
        self.previous_punch = self.punch;
        self.punch = snapshot.punch;
        self.time = snapshot.time;
        self.stats = snapshot.stats;
        let mut predicted = snapshot.player;
        let pending: Vec<UserCommand> = self.pending.iter().copied().collect();
        for command in &pending {
            self.run_on(&mut predicted, command);
        }
        self.predicted = predicted;
    }

    /// Applies a delta, if its baseline is still here.
    fn take_delta(&mut self, bytes: &[u8]) {
        let snapshot = delta_baseline(bytes)
            .and_then(|tick| self.history.iter().find(|old| old.tick == tick))
            .and_then(|base| apply_delta(base, bytes));
        if let Some(snapshot) = snapshot {
            self.take_snapshot(snapshot);
        }
    }

    /// Sends a console command to the server.
    pub fn send_console(&mut self, text: &str) {
        self.transport
            .send(ClientMessage::Console(text.to_owned()).encode());
    }

    /// Takes a new list of entities, noting which changed frame.
    fn note_entities(&mut self, tick: u32, entities: Vec<EntityState>) {
        let previous: HashMap<u64, EntityState> = std::mem::replace(&mut self.entities, entities)
            .into_iter()
            .map(|entity| (entity.id, entity))
            .collect();
        for entity in &self.entities {
            if let Some(from) = previous.get(&entity.id)
                && from.frame != entity.frame
            {
                self.frame_changes.insert(
                    entity.id,
                    FrameChange {
                        from: from.frame,
                        tick,
                    },
                );
            }
        }
        self.previous_entities = previous;
        self.tick = tick;
    }

    /// Reads what the server sent. Each snapshot resets the prediction to the
    /// server's state and replays what it hasn't run, as `CL_PredictMove`.
    pub fn receive(&mut self) {
        while let Some(bytes) = self.transport.receive() {
            match ServerMessage::decode(&bytes) {
                Some(ServerMessage::Snapshot(snapshot)) => self.take_snapshot(snapshot),
                Some(ServerMessage::SnapshotDelta(delta)) => self.take_delta(&delta),
                Some(ServerMessage::Level(name)) => self.level = Some(name),
                Some(ServerMessage::Scores(scores)) => self.scores = scores,
                Some(ServerMessage::LightStyle { index, pattern }) => {
                    self.styles.set(index, &pattern);
                }
                Some(ServerMessage::Print(text)) => self.prints.push(text),
                Some(ServerMessage::Effect(effect)) => self.effects.push(effect),
                Some(ServerMessage::ModelName { number, name }) => {
                    self.model_names.push((number, name));
                }
                Some(ServerMessage::Sound(sound)) => self.sounds.push(sound),
                Some(ServerMessage::StaticSound(sound)) => self.static_sounds.push(sound),
                Some(ServerMessage::Music(track)) => self.music = Some(track),
                Some(ServerMessage::SetView { yaw, pitch }) => self.view = Some((yaw, pitch)),
                Some(ServerMessage::CenterPrint(text)) => {
                    self.events.push(ScreenEvent::CenterPrint(text));
                }
                Some(ServerMessage::Stuff(text)) => self.events.push(ScreenEvent::Command(text)),
                Some(ServerMessage::Damage { armor, blood, from }) => {
                    self.events.push(ScreenEvent::Damage { armor, blood, from });
                }
                Some(ServerMessage::Intermission(kind)) => {
                    self.events.push(ScreenEvent::Intermission(kind));
                }
                Some(ServerMessage::LevelTitle(text)) => {
                    self.events.push(ScreenEvent::LevelTitle(text));
                }
                None => {}
            }
        }
    }

    /// The state to draw: a copy of the prediction, moved on by `seconds`
    /// with the latest input. The copy is thrown away, so the simulation
    /// stays on its fixed tick.
    #[must_use]
    pub fn render_state(&mut self, command: &UserCommand, seconds: f32) -> PlayerState {
        let mut state = self.predicted;
        if seconds > 0.0 {
            let mut command = *command;
            command.seconds = seconds;
            self.run_on(&mut state, &command);
        }
        state
    }
}

/// Angles `fraction` of the way from `from` to `to`, the short way round.
fn blend_angles(from: Vec3, to: Vec3, fraction: f32) -> Vec3 {
    let blend = |from: f32, to: f32| {
        let turn = (to - from + 180.0).rem_euclid(360.0) - 180.0;
        from + turn * fraction
    };
    Vec3::new(
        blend(from.x, to.x),
        blend(from.y, to.y),
        blend(from.z, to.z),
    )
}
