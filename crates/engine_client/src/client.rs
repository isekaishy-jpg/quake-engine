//! The client: commands, prediction and the render tick (proposal 0024).

use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::time::Duration;

use engine_core::entity::{
    BlastRule, EFFECT_MUZZLE_FLASH, Effect, EntityState, ModelInstance, ShotFeel,
};
use engine_core::math::Vec3;
use engine_core::player::{PLAYER_MAXS, PLAYER_MINS, PlayerState, UserCommand};
use engine_core::sound::{SoundStart, StaticSound};
use engine_core::time::TICK_SECONDS;
use engine_map::Contents;
use engine_movement::{MoveVars, player_move};
use engine_net::{
    ClientMessage, FireCommand, PlayerInfo, PlayerScore, ServerMessage, Snapshot, Transport,
    apply_delta, delta_baseline,
};
use engine_physics::CollisionWorld;

use crate::lightstyle::{LIGHT_STYLE_COUNT, LightStyles};
use crate::projectiles::{Blast, Flights, Launch, Timeline};
use crate::shown_shot::ShownShot;
use crate::view_clock::ViewClock;

/// How far light can start inside something and still come out of it, as a window's glass is
/// solid to everything else.
const LIGHT_EMERGES: f32 = 48.0;

/// How far it looks for what's between the sun and a place.
const SUN_LOOKS: f32 = 8192.0;

/// How far apart it looks for the way out.
const LIGHT_EMERGE_STEP: f32 = 4.0;

/// Further than this in a tick is a teleport, and isn't blended.
pub(crate) const MAX_BLENDED_MOVE: f32 = 200.0;

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
    /// A change of camera, by what it's called in text, over some seconds.
    Camera {
        /// The camera.
        camera: String,
        /// How long it takes to blend to it.
        blend: f32,
    },
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

/// The most commands waiting for the server. Older ones are forgotten.
const MAX_PENDING: usize = 512;

/// How many of the latest commands go in each move, so a lost packet costs
/// nothing, as `CL_SendMove` does in QuakeWorld.
const COMMANDS_PER_MOVE: usize = 3;

/// The share of the gap your own shots close: half, as closing all of it asks too much.
pub const DEFAULT_SHOT_CORRECTION: f64 = 0.5;

/// How long past a round trip a guess waits for the server, in seconds.
const GUESS_PATIENCE: f64 = 0.15;

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
    /// The time others are drawn at.
    clock: ViewClock,
    /// What's flying.
    flights: Flights,
    /// Numbers of models by name.
    model_numbers: HashMap<String, u16>,
    /// Knocks predicted from our own rockets, by the command they land on.
    knocks: Vec<(u32, Vec3)>,
    /// How our own blasts knock us, if that's to be predicted.
    blast: Option<BlastRule>,
    /// The share of the gap own shots are eased by.
    shot_correction: f64,
    /// Whether the player's own body is drawn, as it is from outside.
    shows_self: bool,
    /// Sounds already played for our own shots, and when to stop waiting.
    expected_sounds: Vec<(String, f64)>,
    /// Our own shot as it's shown before the server answers.
    shown: ShownShot,
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
            clock: ViewClock::default(),
            flights: Flights::default(),
            model_numbers: HashMap::new(),
            knocks: Vec::new(),
            blast: None,
            shot_correction: DEFAULT_SHOT_CORRECTION,
            shows_self: false,
            expected_sounds: Vec::new(),
            shown: ShownShot::default(),
            touched: Vec::new(),
        }
    }

    /// Runs `command` on `state`, against the latest brush model positions,
    /// with any knock that's due on it.
    fn run_on(&mut self, state: &mut PlayerState, command: &UserCommand) {
        self.touched.clear();
        for (sequence, knock) in &self.knocks {
            if *sequence == command.sequence {
                state.velocity = state.velocity + *knock;
                state.on_ground = false;
            }
        }
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

    /// Where the entity `id` is, as of the last snapshot.
    #[must_use]
    pub fn entity_origin(&self, id: u64) -> Option<Vec3> {
        self.entities
            .iter()
            .find(|entity| entity.id == id)
            .map(|entity| entity.origin)
    }

    /// Entities as they should be drawn, `fraction` of the way from the last
    /// snapshot but one to the last.
    #[must_use]
    pub fn draw_entities(&self, fraction: f32) -> Vec<DrawnEntity> {
        self.entities
            .iter()
            .filter(|entity| self.shows_self || entity.id != self.player_entity)
            .map(|entity| {
                let mut state = *entity;
                if let Some(from) = self.previous_entities.get(&entity.id)
                    && (entity.origin - from.origin).length() < MAX_BLENDED_MOVE
                {
                    state.origin = from.origin.lerp(entity.origin, fraction);
                    state.angles = blend_angles(from.angles, entity.angles, fraction);
                }
                DrawnEntity {
                    from_frame: state.frame,
                    state,
                    blend: 1.0,
                }
            })
            .collect()
    }

    /// Entities as they should be drawn online, `seconds` since the last
    /// frame: a little in the past, between the snapshots either side of it,
    /// so they move evenly however snapshots arrive.
    pub fn draw_entities_buffered(&mut self, seconds: f64) -> Vec<DrawnEntity> {
        let time = self.clock.advance(seconds, self.time);
        let after = self
            .history
            .iter()
            .position(|snapshot| snapshot.time >= time);
        let pair = after.and_then(|index| {
            let before = index.checked_sub(1)?;
            Some((self.history.get(before)?, self.history.get(index)?))
        });
        let Some((before, after)) = pair else {
            let mut drawn = self.draw_entities(1.0);
            self.place_flights(&mut drawn, time);
            return drawn;
        };
        let span = after.time - before.time;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a blend needs no more than f32"
        )]
        let fraction = if span > 0.0 {
            ((time - before.time) / span) as f32
        } else {
            1.0
        };
        let old: HashMap<u64, &EntityState> = before
            .entities
            .iter()
            .map(|entity| (entity.id, entity))
            .collect();
        let mut drawn: Vec<DrawnEntity> = after
            .entities
            .iter()
            .filter(|entity| self.shows_self || entity.id != self.player_entity)
            .map(|entity| {
                let mut state = *entity;
                if let Some(from) = old.get(&entity.id)
                    && (entity.origin - from.origin).length() < MAX_BLENDED_MOVE
                {
                    state.origin = from.origin.lerp(entity.origin, fraction);
                    state.angles = blend_angles(from.angles, entity.angles, fraction);
                }
                DrawnEntity {
                    from_frame: state.frame,
                    state,
                    blend: 1.0,
                }
            })
            .collect();
        self.place_flights(&mut drawn, time);
        drawn
    }

    /// Puts what's flying where its path says, and adds shots of our own the
    /// server hasn't confirmed.
    fn place_flights(&self, drawn: &mut Vec<DrawnEntity>, behind: f64) {
        let times = self.timeline(behind);
        for entity in drawn.iter_mut() {
            if let Some(place) = self.flights.place_of(entity.state.id, &times) {
                entity.state.origin = place;
            }
        }
        let guesses = self.flights.guesses(times);
        for (number, (model, origin, velocity)) in (0u64..).zip(guesses) {
            let flat = (velocity.x * velocity.x + velocity.y * velocity.y).sqrt();
            drawn.push(DrawnEntity {
                state: EntityState {
                    id: u64::MAX - number,
                    model,
                    origin,
                    angles: Vec3::new(
                        velocity.z.atan2(flat).to_degrees(),
                        velocity.y.atan2(velocity.x).to_degrees(),
                        0.0,
                    ),
                    ..EntityState::default()
                },
                from_frame: 0,
                blend: 1.0,
            });
        }
    }

    /// The player's weapon model and frame.
    #[must_use]
    pub fn weapon(&self) -> (u16, u16) {
        (self.weapon.0, self.shown.frame().unwrap_or(self.weapon.1))
    }

    /// The player's entity.
    #[must_use]
    pub fn player_entity(&self) -> u64 {
        self.player_entity
    }

    /// The player's effect bits.
    #[must_use]
    pub fn take_player_effects(&mut self) -> u8 {
        let mut effects = self.player_effects;
        if self.shown.is_active() {
            effects &= !EFFECT_MUZZLE_FLASH;
        }
        if self.shown.take_flash() {
            effects |= EFFECT_MUZZLE_FLASH;
        }
        effects
    }

    /// How far the view is knocked, in degrees, `fraction` of the way from the
    /// last snapshot but one to the last.
    #[must_use]
    pub fn punch(&self, fraction: f32) -> Vec3 {
        if self.shown.is_active() {
            return Vec3::new(-self.shown.recoil(fraction), 0.0, 0.0);
        }
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

    /// The level's gravity, as the server last said.
    #[must_use]
    pub fn gravity(&self) -> f32 {
        self.vars.gravity
    }

    /// Numbers `command`, sends it, and predicts its result.
    pub fn send_command(&mut self, mut command: UserCommand, seconds: f32) {
        command.sequence = self.next_sequence;
        command.seconds = seconds;
        command.view_time = self.clock.time();
        self.next_sequence = self.next_sequence.wrapping_add(1);
        let busy = self.punch != Vec3::ZERO || self.weapon.1 != 0;
        self.shown.advance(seconds, busy);
        self.follow_flights();
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

    /// Hears from the network, `elapsed` since last time.
    pub fn update_network(&mut self, elapsed: Duration) {
        self.transport.update(elapsed);
    }

    /// Sends what's waiting.
    pub fn flush_network(&mut self) {
        self.transport.flush();
    }

    /// Gives up the connection, for the next level's client.
    #[must_use]
    pub fn into_transport(self) -> Box<dyn Transport> {
        self.transport
    }

    /// How long a command takes to come back, as far as can be told.
    fn round_trip(&self) -> f64 {
        #[expect(clippy::cast_precision_loss, reason = "a few commands")]
        let waiting = self.pending.len() as f64;
        waiting * TICK_SECONDS
    }

    /// The game time at the server now, as far as can be told.
    fn present(&self) -> f64 {
        self.time + self.round_trip() / 2.0
    }

    /// The game time the player's own commands run at.
    fn own_time(&self) -> f64 {
        self.time + self.round_trip()
    }

    /// The times flights are drawn by, with others drawn at `behind`.
    fn timeline(&self, behind: f64) -> Timeline {
        Timeline {
            present: self.present(),
            own: self.own_time(),
            behind,
            correction: self.shot_correction,
        }
    }

    /// Sets the share of the gap own shots are eased by, from 0 to 1.
    pub const fn set_shot_correction(&mut self, correction: f64) {
        self.shot_correction = correction.clamp(0.0, 1.0);
    }

    /// Sets how our own blasts knock us, or that they aren't predicted.
    pub fn set_blast(&mut self, blast: Option<BlastRule>) {
        self.blast = blast;
    }

    /// Notes that `sound` has already been played for a shot, so the server's
    /// won't play over it.
    pub fn expect_own_sound(&mut self, sound: &str) {
        let give_up = self.clock.time() + self.round_trip() + GUESS_PATIENCE;
        self.expected_sounds.push((sound.to_owned(), give_up));
    }

    /// Shows the feel of our own shot, without waiting for the server.
    pub fn show_shot(&mut self, feel: ShotFeel) {
        #[expect(clippy::cast_possible_truncation, reason = "a fraction of a second")]
        let patience = (self.round_trip() + GUESS_PATIENCE) as f32;
        self.shown.start(feel, patience);
    }

    /// Notes that the button is up, so a held weapon's frames stop going round.
    pub fn let_go_of_fire(&mut self) {
        self.shown.let_go();
    }

    /// Draws our own shot before the server has confirmed it.
    pub fn guess_launch(
        &mut self,
        model: &str,
        origin: Vec3,
        velocity: Vec3,
        (bounces, blasts, fuse): (bool, bool, f32),
    ) {
        if let Some(model) = self.model_numbers.get(model).copied() {
            let launch = Launch {
                origin,
                velocity,
                time: self.own_time(),
                bounces,
                blasts,
                strength: self.own_strength(),
                fuse,
                gravity: self.vars.gravity,
            };
            let behind = self.clock.time();
            let fired_on = self.next_sequence;
            self.flights.guess(
                launch,
                model,
                fired_on,
                behind,
                &self.world,
                &self.instances,
            );
        }
    }

    /// Follows what's flying, and predicts the knock of our own rockets. Only
    /// movement is predicted, and the server's next word corrects it.
    fn follow_flights(&mut self) {
        let ids: HashSet<u64> = self.entities.iter().map(|entity| entity.id).collect();
        let give_up = self.round_trip() + GUESS_PATIENCE;
        self.flights
            .retain(|id| ids.contains(&id), self.clock.time(), give_up);
        let now = self.clock.time();
        self.expected_sounds.retain(|(_, by)| *by > now);
        let Some(rule) = self.blast else {
            return;
        };
        // What flies into us goes off first, and never gets to where it's going.
        let origin = self.predicted.origin;
        let (low, high) = (origin + PLAYER_MINS, origin + PLAYER_MAXS);
        let hits = self
            .flights
            .take_hits(self.next_sequence, (low, high), &self.boxes);
        for hit in hits {
            if hit.direct {
                let points = rule.direct_damage * hit.strength;
                self.knock(&rule, hit.place, points, self.next_sequence);
            } else {
                let blast = Blast {
                    lands_on: self.next_sequence,
                    place: hit.place,
                    own: hit.own,
                    strength: hit.strength,
                };
                self.take_blast(&rule, &blast);
            }
        }
        let answered = self.history.back().map_or(0, |last| last.acknowledged);
        for blast in self.flights.take_blasts(self.next_sequence) {
            // Otherwise, the server's numbers already have it.
            if blast.lands_on > answered.wrapping_add(1) {
                self.take_blast(&rule, &blast);
            }
        }
    }

    /// Solid bodies, from the latest snapshot.
    #[must_use]
    pub fn solid_boxes(&self) -> &[(Vec3, Vec3)] {
        &self.boxes
    }

    /// Says whether the camera is outside the player, so that they're drawn.
    pub const fn show_self(&mut self, shows: bool) {
        self.shows_self = shows;
    }

    /// What the line from `from` to `to` hits first, as a place and which way it faces, for what
    /// a client draws and nothing depends on.
    #[must_use]
    pub fn line_hit(&self, from: Vec3, to: Vec3) -> Option<(Vec3, Vec3)> {
        let trace = self.world.with(&self.instances).trace(from, to);
        (trace.fraction < 1.0 && !trace.start_solid).then_some((trace.end, trace.normal))
    }

    /// How far a line from `start` along `way` gets, up to `most`, past the world and moving
    /// models.
    #[must_use]
    pub fn room_along(&self, start: Vec3, way: Vec3, most: f32) -> f32 {
        let collision = self.world.with(&self.instances);
        let trace = collision.trace(start, start + way * most);
        if trace.start_solid {
            return 0.0;
        }
        most * trace.fraction
    }

    /// Whether a line is clear of the world, moving models and bodies,
    /// counting ourselves. With `to_sky`, reaching sky counts as clear, since
    /// sky is solid to collision.
    #[must_use]
    pub fn line_is_clear(&self, start: Vec3, end: Vec3, to_sky: bool) -> bool {
        let origin = self.predicted.origin;
        let mut boxes = self.boxes.clone();
        boxes.push((origin + PLAYER_MINS, origin + PLAYER_MAXS));
        let collision = self.world.with(&self.instances).with_boxes(&boxes);
        let trace = collision.trace(start, end);
        if trace.start_solid || trace.fraction >= 1.0 {
            return true;
        }
        let beyond = trace.end + (end - start).normalized() * 2.0;
        to_sky && trace.model.is_none() && self.world.map_contents(beyond) == Contents::Sky
    }

    /// How far along a line light gets, past what it starts in, such as the glass of its window.
    /// Characters don't stop it, as a beam is too rough for them to show.
    #[must_use]
    pub fn light_reach(&self, start: Vec3, end: Vec3) -> f32 {
        let whole = (end - start).length();
        let way = (end - start).normalized();
        let collision = self.world.with(&self.instances);
        let mut gone = 0.0;
        while gone < LIGHT_EMERGES.min(whole) {
            let trace = collision.trace(start + way * gone, end);
            if !trace.start_solid {
                return gone + trace.fraction * (whole - gone);
            }
            gone += LIGHT_EMERGE_STEP;
        }
        0.0
    }

    /// How low the sun's light gets, along a line through `place` from the way it comes.
    /// Under a roof that's the roof, and in the open it's whatever it lands on.
    #[must_use]
    pub fn sun_gets_to(&self, place: Vec3, toward_sun: Vec3) -> f32 {
        let collision = self.world.with(&self.instances);
        let up = place + toward_sun * SUN_LOOKS;
        let looked = collision.trace(place, up);
        let way = toward_sun * 2.0;
        let sky = self.world.map_contents(looked.end + way) == Contents::Sky;
        if looked.fraction < 1.0 && (looked.model.is_some() || !sky) {
            return looked.end.z;
        }
        collision.trace(place, place - toward_sun * SUN_LOOKS).end.z
    }

    /// Works out what `blast` does to us, from `T_RadiusDamage`.
    fn take_blast(&mut self, rule: &BlastRule, blast: &Blast) {
        let middle = self.predicted.origin + Vec3::new(0.0, 0.0, 4.0);
        let distance = (blast.place - middle).length();
        if distance > rule.reach {
            return;
        }
        let share = if blast.own { rule.own_share } else { 1.0 };
        let points = (rule.damage - distance * rule.falloff) * share * blast.strength;
        let collision = self.world.with(&self.instances);
        let clear = collision
            .move_box(blast.place, Vec3::ZERO, Vec3::ZERO, middle)
            .fraction
            >= 1.0;
        if points > 0.0 && clear {
            self.knock(rule, blast.place, points, blast.lands_on);
        }
    }

    /// Knocks us away from `place` by `points` of damage, on command
    /// `lands_on`, from `T_Damage`.
    fn knock(&mut self, rule: &BlastRule, place: Vec3, points: f32, lands_on: u32) {
        let direction = (self.predicted.origin - place).normalized();
        let knock = direction * (points * rule.knock_per_damage);
        self.knocks.push((lands_on, knock));
        self.replay();
    }

    /// How strong a shot of ours is, by what we're carrying.
    fn own_strength(&self) -> f32 {
        let Some(rule) = self.blast else {
            return 1.0;
        };
        let carried = self.stats.get(rule.carried_stat).copied().unwrap_or(0);
        if carried & rule.boost_bits != 0 {
            rule.boost
        } else {
            1.0
        }
    }

    /// The command that ran at game time `time`, counting from the last one
    /// the server has answered for.
    fn command_at(&self, time: f64) -> u32 {
        let answered = self.history.back().map_or(0, |last| last.acknowledged);
        #[expect(clippy::cast_possible_truncation, reason = "a few ticks either way")]
        let ticks = ((time + TICK_SECONDS - self.time) / TICK_SECONDS).round() as i64;
        u32::try_from(i64::from(answered) + ticks).unwrap_or(answered)
    }

    /// Works out the prediction again from the last snapshot.
    fn replay(&mut self) {
        let Some(mut state) = self.history.back().map(|last| last.player) else {
            return;
        };
        let pending = std::mem::take(&mut self.pending);
        for command in &pending {
            self.run_on(&mut state, command);
        }
        self.pending = pending;
        self.predicted = state;
    }

    /// Sends a shot now, ahead of the next command, `offset` of the way
    /// through the tick, out of 256. Returns the number its randomness comes
    /// from.
    pub fn send_fire(&mut self, yaw: f32, pitch: f32, offset: u8) -> u16 {
        let seed = u16::try_from(self.next_sequence & 0xFFFF).unwrap_or(0);
        let fire = FireCommand {
            yaw,
            pitch,
            view_time: self.clock.time(),
            offset,
            seed,
        };
        self.transport
            .send_unreliable(ClientMessage::Fire(fire).encode());
        self.transport.flush();
        seed
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

    /// The last command the server has answered for, and where that left the
    /// player, for measuring prediction.
    #[must_use]
    pub fn last_answer(&self) -> Option<(u32, PlayerState)> {
        let snapshot = self.history.back()?;
        Some((snapshot.acknowledged, snapshot.player))
    }

    /// The number the next command will have.
    #[must_use]
    pub fn next_sequence(&self) -> u32 {
        self.next_sequence
    }

    /// The last launch heard of, for tests of paths.
    #[must_use]
    pub fn last_launch(&self) -> Option<(u64, Launch)> {
        self.flights.last_launch()
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
        // A knock lands after the command before it, so the server's numbers
        // for that command already include it.
        let included = snapshot.acknowledged.wrapping_add(1);
        self.knocks.retain(|(sequence, _)| *sequence > included);
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
        let pending = std::mem::take(&mut self.pending);
        for command in &pending {
            self.run_on(&mut predicted, command);
        }
        self.pending = pending;
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

    /// Takes a new list of entities.
    fn note_entities(&mut self, tick: u32, entities: Vec<EntityState>) {
        let previous: HashMap<u64, EntityState> = std::mem::replace(&mut self.entities, entities)
            .into_iter()
            .map(|entity| (entity.id, entity))
            .collect();
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
                Some(ServerMessage::Effect(Effect::Launch(projectile))) => {
                    let own = projectile.owner == self.player_entity;
                    let fired_on = self.command_at(projectile.time);
                    let behind = self.clock.time();
                    self.flights.launched(
                        &projectile,
                        own,
                        fired_on,
                        (behind, self.vars.gravity),
                        &self.world,
                        &self.instances,
                    );
                }
                Some(ServerMessage::Effect(effect)) => self.effects.push(effect),
                Some(ServerMessage::ModelName { number, name }) => {
                    self.model_numbers.insert(name.clone(), number);
                    self.model_names.push((number, name));
                }
                Some(ServerMessage::Sound(sound)) => {
                    let played = sound
                        .own
                        .then(|| {
                            self.expected_sounds
                                .iter()
                                .position(|(name, _)| *name == sound.name)
                        })
                        .flatten();
                    match played {
                        Some(index) => {
                            let _played = self.expected_sounds.remove(index);
                        }
                        None => self.sounds.push(sound),
                    }
                }
                Some(ServerMessage::StaticSound(sound)) => self.static_sounds.push(sound),
                Some(ServerMessage::Gravity(gravity)) => self.vars.gravity = gravity,
                Some(ServerMessage::Music(track)) => self.music = Some(track),
                Some(ServerMessage::Camera { camera, blend }) => {
                    self.events.push(ScreenEvent::Camera { camera, blend });
                }
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
pub(crate) fn blend_angles(from: Vec3, to: Vec3, fraction: f32) -> Vec3 {
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
