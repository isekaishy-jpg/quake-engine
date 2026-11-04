//! The client: commands, prediction and the render tick (proposal 0024).

use std::collections::VecDeque;
use std::rc::Rc;

use engine_core::entity::ModelInstance;
use engine_core::math::Vec3;
use engine_core::player::{PlayerState, UserCommand};
use engine_movement::{MoveVars, player_move};
use engine_net::{ClientMessage, ServerMessage, Transport};
use engine_physics::CollisionWorld;

use crate::lightstyle::{LIGHT_STYLE_COUNT, LightStyles};

/// The most commands waiting for the server. Older ones are forgotten.
const MAX_PENDING: usize = 512;

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
            touched: Vec::new(),
        }
    }

    /// Runs `command` on `state`, against the latest brush model positions.
    fn run_on(&mut self, state: &mut PlayerState, command: &UserCommand) {
        self.touched.clear();
        let collision = self.world.with(&self.instances);
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

    /// Every light style's brightness at `time`.
    #[must_use]
    pub fn light_styles(&self, time: f64) -> [f32; LIGHT_STYLE_COUNT] {
        self.styles.values(time)
    }

    /// Takes text from the server.
    pub fn take_prints(&mut self) -> Vec<String> {
        std::mem::take(&mut self.prints)
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
        self.transport.send(ClientMessage::Move(command).encode());
        let mut predicted = self.predicted;
        self.run_on(&mut predicted, &command);
        self.predicted = predicted;
        if self.pending.len() == MAX_PENDING {
            let _oldest = self.pending.pop_front();
        }
        self.pending.push_back(command);
    }

    /// Sends a console command to the server.
    pub fn send_console(&mut self, text: &str) {
        self.transport
            .send(ClientMessage::Console(text.to_owned()).encode());
    }

    /// Reads what the server sent. Each snapshot resets the prediction to the
    /// server's state and replays what it hasn't run, as `CL_PredictMove`.
    pub fn receive(&mut self) {
        while let Some(bytes) = self.transport.receive() {
            match ServerMessage::decode(&bytes) {
                Some(ServerMessage::Snapshot(snapshot)) => {
                    self.pending
                        .retain(|command| command.sequence > snapshot.acknowledged);
                    self.previous_instances =
                        std::mem::replace(&mut self.instances, snapshot.instances);
                    let mut predicted = snapshot.player;
                    let pending: Vec<UserCommand> = self.pending.iter().copied().collect();
                    for command in &pending {
                        self.run_on(&mut predicted, command);
                    }
                    self.predicted = predicted;
                }
                Some(ServerMessage::LightStyle { index, pattern }) => {
                    self.styles.set(index, &pattern);
                }
                Some(ServerMessage::Print(text)) => self.prints.push(text),
                Some(ServerMessage::SetView { yaw, pitch }) => self.view = Some((yaw, pitch)),
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
