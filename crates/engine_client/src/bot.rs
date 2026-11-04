//! Bots: real clients with something simple at the keys (proposal 0049).
//!
//! A bot only knows what a player's client knows, through the same messages,
//! so a room of them tests the whole of the network, and lets one person try
//! a game alone. Its aim is at what it draws, as a player's is.

use std::rc::Rc;
use std::time::Duration;

use engine_core::math::Vec3;
use engine_core::player::UserCommand;
use engine_net::Transport;
use engine_physics::CollisionWorld;

use crate::client::{Client, DrawnEntity};

/// How fast a bot asks to run.
const RUN_SPEED: f32 = 400.0;

/// How high a bot's eyes are above its middle.
const EYE_HEIGHT: f32 = 22.0;

/// How far a bot notices others.
const SIGHT_RANGE: f32 = 1500.0;

/// Below this speed, a bot that wants to move is stuck.
const STUCK_SPEED: f32 = 40.0;

/// How long a bot keeps a direction, in seconds.
const WANDER_SECONDS: f32 = 2.5;

/// How often a bot presses a held button again, in seconds, so that a dead bot
/// lets go and then respawns.
const RESPAWN_PRESS_SECONDS: f32 = 0.5;

/// How a bot plays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BotSkill {
    /// How far off its aim is, at most, in degrees.
    pub aim_error: f32,
    /// How fast it turns, in degrees a second.
    pub turn_speed: f32,
    /// Whether it moves at all, or only stands and shoots.
    pub moves: bool,
    /// Whether it shoots.
    pub shoots: bool,
    /// If this isn't 0, it just runs to and fro along its yaw, turning after
    /// this many seconds, as a target for measuring.
    pub patrol_seconds: f32,
}

impl Default for BotSkill {
    fn default() -> Self {
        Self {
            aim_error: 3.0,
            turn_speed: 360.0,
            moves: true,
            shoots: true,
            patrol_seconds: 0.0,
        }
    }
}

/// A bot.
pub struct Bot {
    /// Its client.
    client: Client,
    /// How it plays.
    skill: BotSkill,
    /// Where it's looking.
    yaw: f32,
    /// How far down it's looking.
    pitch: f32,
    /// Seconds left of its current direction.
    wander: f32,
    /// Seconds since it pressed a button to respawn.
    press: f32,
    /// The state of its random numbers.
    random: u32,
    /// Whether it's holding attack, to respawn.
    fired: bool,
    /// Whether attack was held last tick.
    held_attack: bool,
}

impl Bot {
    /// A bot playing through `client`.
    #[must_use]
    pub fn new(client: Client, skill: BotSkill, seed: u32) -> Self {
        Self {
            client,
            skill,
            yaw: 0.0,
            pitch: 0.0,
            wander: 0.0,
            press: 0.0,
            random: seed | 1,
            fired: false,
            held_attack: false,
        }
    }

    /// Its client.
    #[must_use]
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Its client.
    pub fn client_mut(&mut self) -> &mut Client {
        &mut self.client
    }

    /// Gives up its connection, for the next level.
    #[must_use]
    pub fn into_parts(self) -> (Box<dyn Transport>, BotSkill, u32) {
        (self.client.into_transport(), self.skill, self.random)
    }

    /// The next random number from -1 to 1.
    fn next_random(&mut self) -> f32 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 17;
        self.random ^= self.random << 5;
        #[expect(clippy::cast_precision_loss, reason = "a random share")]
        let share = (self.random >> 8) as f32 / 8_388_608.0;
        share - 1.0
    }

    /// The nearest other player it can see, among `drawn`, by `is_player`.
    fn target(&self, drawn: &[DrawnEntity], world: &CollisionWorld) -> Option<Vec3> {
        let players: Vec<u64> = self.client.scores().iter().map(|s| s.entity).collect();
        let eye = self.eye();
        drawn
            .iter()
            .filter(|entity| players.contains(&entity.state.id))
            .map(|entity| entity.state.origin)
            .filter(|&origin| (origin - eye).length() < SIGHT_RANGE)
            .filter(|&origin| {
                let collision = world.with(&[]);
                collision
                    .move_box(eye, Vec3::ZERO, Vec3::ZERO, origin)
                    .fraction
                    >= 1.0
            })
            .min_by(|a, b| (*a - eye).length().total_cmp(&(*b - eye).length()))
    }

    /// Where its eyes are.
    fn eye(&self) -> Vec3 {
        self.client.predicted().origin + Vec3::new(0.0, 0.0, EYE_HEIGHT)
    }

    /// Turns towards `point`, as far as it can over `seconds`.
    fn aim_at(&mut self, point: Vec3, seconds: f32) {
        let to = point - self.eye();
        let flat = (to.x * to.x + to.y * to.y).sqrt();
        let error = self.skill.aim_error;
        let wanted_yaw = to.y.atan2(to.x).to_degrees() + self.next_random() * error;
        let wanted_pitch = -to.z.atan2(flat).to_degrees() + self.next_random() * error;
        let reach = self.skill.turn_speed * seconds;
        let turn = (wanted_yaw - self.yaw + 180.0).rem_euclid(360.0) - 180.0;
        self.yaw += turn.clamp(-reach, reach);
        self.pitch += (wanted_pitch - self.pitch).clamp(-reach, reach);
    }

    /// Picks a new way to go when it's time to, or it's stuck.
    fn wander(&mut self, seconds: f32) {
        self.wander -= seconds;
        let velocity = self.client.predicted().velocity;
        let speed = (velocity.x * velocity.x + velocity.y * velocity.y).sqrt();
        let stuck = speed < STUCK_SPEED && self.wander < WANDER_SECONDS - 0.3;
        if self.wander <= 0.0 || stuck {
            self.yaw += 90.0 + 90.0 * self.next_random();
            self.pitch = 0.0;
            self.wander = WANDER_SECONDS;
        }
    }

    /// What it does this tick, given `drawn` and the map.
    fn decide(
        &mut self,
        seconds: f32,
        drawn: &[DrawnEntity],
        world: &CollisionWorld,
    ) -> UserCommand {
        let mut command = UserCommand {
            yaw: self.yaw,
            pitch: self.pitch,
            ..UserCommand::default()
        };
        if self.client.predicted().dead {
            // Respawning takes a press after letting go.
            self.press += seconds;
            if self.press > RESPAWN_PRESS_SECONDS {
                self.press = 0.0;
                self.fired = !self.fired;
            }
            command.attack = self.fired;
            return command;
        }
        if self.skill.patrol_seconds > 0.0 {
            self.wander -= seconds;
            if self.wander <= 0.0 {
                self.wander = self.skill.patrol_seconds;
                self.yaw += 180.0;
            }
            command.yaw = self.yaw;
            command.forward = RUN_SPEED;
            return command;
        }
        let target = self.target(drawn, world);
        if let Some(point) = target {
            self.aim_at(point, seconds);
            command.attack = self.skill.shoots;
        } else {
            self.wander(seconds);
        }
        command.yaw = self.yaw;
        command.pitch = self.pitch;
        if self.skill.moves {
            command.forward = RUN_SPEED;
            command.jump = self.next_random() > 0.995;
            if target.is_some() {
                // Strafe, as anything that wants to live does.
                let side = if self.wander.rem_euclid(2.0) < 1.0 {
                    RUN_SPEED
                } else {
                    -RUN_SPEED
                };
                command.side = side;
                self.wander -= seconds;
            }
        }
        command
    }

    /// Hears from the server, plays a tick of `seconds`, and sends its command.
    pub fn tick(&mut self, seconds: f32, world: &Rc<CollisionWorld>) {
        self.client.update_network(Duration::from_secs_f32(seconds));
        self.client.receive();
        if let Some((yaw, pitch)) = self.client.take_view() {
            self.yaw = yaw;
            self.pitch = pitch;
        }
        let drawn = self.client.draw_entities_buffered(f64::from(seconds));
        let command = self.decide(seconds, &drawn, world);
        if command.attack && !self.held_attack {
            let _seed = self.client.send_fire(command.yaw, command.pitch, 128);
        }
        self.held_attack = command.attack;
        self.client.send_command(command, seconds);
        self.client.flush_network();
        // What a bot isn't using would otherwise pile up.
        let _sounds = self.client.take_sounds();
        let _static_sounds = self.client.take_static_sounds();
        let _effects = self.client.take_effects();
        let _prints = self.client.take_prints();
        let _events = self.client.take_events();
        let _names = self.client.take_model_names();
        let _music = self.client.take_music();
    }
}
