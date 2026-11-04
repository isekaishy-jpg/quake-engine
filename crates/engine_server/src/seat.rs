//! Someone connected to a game, and what the server keeps for them.

use std::collections::VecDeque;
use std::time::Duration;

use engine_core::player::UserCommand;
use engine_net::{ClientMessage, FireCommand, ServerMessage, Snapshot, Transport, delta_from};
use engine_sim::EntityId;
use serde::{Deserialize, Serialize};

/// The most seconds of movement a client can have in hand, so a client can't
/// save up time, or move faster than the clock, by sending commands in bursts.
const MAX_ALLOWANCE: f32 = 0.25;

/// The most a single command can cost.
const MAX_COMMAND_COST: f32 = 0.1;

/// How many snapshots are kept to send differences from.
const SNAPSHOT_HISTORY: usize = 64;

/// How much each sample moves a measured ping, so short spikes don't count.
const PING_SMOOTHING: f64 = 0.02;

/// Seconds of silence after which someone's gone.
const TIMEOUT_SECONDS: f64 = 10.0;

/// Who someone says they are.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatInfo {
    /// Their name.
    pub name: String,
    /// Their shirt colour in the high four bits, and pants in the low.
    pub colors: u8,
}

impl Default for SeatInfo {
    fn default() -> Self {
        Self {
            name: "player".to_owned(),
            colors: 0,
        }
    }
}

/// Someone connected.
pub struct Seat {
    /// How to reach them.
    pub(crate) transport: Box<dyn Transport>,
    /// Their player, once they've loaded the map.
    pub(crate) player: Option<EntityId>,
    /// Who they are.
    pub(crate) info: SeatInfo,
    /// What they carry to the next level.
    pub(crate) parms: Option<Vec<f64>>,
    /// How many model names they've heard.
    pub(crate) shapes_told: usize,
    /// Seconds of movement they can still use.
    allowance: f32,
    /// Seconds since anything arrived.
    quiet: f64,
    /// Recent snapshots, newest last.
    history: VecDeque<Snapshot>,
    /// The last snapshot they said they received.
    seen: u32,
    /// Game time now, as this seat's clock has it.
    clock: f64,
    /// When each recent snapshot went, by tick.
    sent: VecDeque<(u32, f64)>,
    /// Their ping as measured here, in milliseconds, smoothed.
    ping: Option<f64>,
    /// Milliseconds added to their ping, moving gently towards what it should be.
    pub(crate) added: f64,
    /// Milliseconds of their ping made up for.
    pub(crate) compensated: f64,
    /// Whether they're over the ceiling.
    pub(crate) over_ceiling: bool,
    /// Whether their ping has been used yet. A newcomer starts where they
    /// should be, since only changes during play have to be gentle.
    pub(crate) settled: bool,
    /// Commands held for the added delay.
    held_commands: VecDeque<(f64, Vec<UserCommand>)>,
    /// Snapshots held for the added delay.
    held_snapshots: VecDeque<(f64, Vec<u8>)>,
    /// Shots held for the added delay.
    held_fires: VecDeque<(f64, FireCommand)>,
    /// What arrived during a half tick, for the next full one.
    pub(crate) backlog: VecDeque<ClientMessage>,
}

impl Seat {
    /// Someone reached by `transport`.
    #[must_use]
    pub fn new(transport: Box<dyn Transport>, info: SeatInfo) -> Self {
        Self {
            transport,
            player: None,
            info,
            parms: None,
            shapes_told: 0,
            allowance: MAX_ALLOWANCE,
            quiet: 0.0,
            history: VecDeque::with_capacity(SNAPSHOT_HISTORY),
            seen: 0,
            clock: 0.0,
            sent: VecDeque::with_capacity(SNAPSHOT_HISTORY),
            ping: None,
            added: 0.0,
            compensated: 0.0,
            over_ceiling: false,
            settled: false,
            held_commands: VecDeque::new(),
            held_snapshots: VecDeque::new(),
            held_fires: VecDeque::new(),
            backlog: VecDeque::new(),
        }
    }

    /// Sends a message that has to arrive.
    pub(crate) fn send(&mut self, message: &ServerMessage) {
        self.transport.send(message.encode());
    }

    /// Sends a message that may be lost.
    pub(crate) fn send_unreliable(&mut self, message: &ServerMessage) {
        self.transport.send_unreliable(message.encode());
    }

    /// Sends what differs from the last snapshot they received.
    pub(crate) fn send_snapshot(&mut self, snapshot: Snapshot) {
        let base = self.history.iter().find(|old| old.tick == self.seen);
        let message = delta_from(base, &snapshot);
        let release = self.clock + self.hold_seconds();
        self.held_snapshots.push_back((release, message));
        if self.history.len() == SNAPSHOT_HISTORY {
            let _oldest = self.history.pop_front();
            let _oldest = self.sent.pop_front();
        }
        self.sent.push_back((snapshot.tick, self.clock));
        self.history.push_back(snapshot);
        self.release_snapshots();
    }

    /// How long each way holds what's going, as added delay is split in two.
    fn hold_seconds(&self) -> f64 {
        self.added / 2000.0
    }

    /// Sends snapshots that have been held for long enough.
    fn release_snapshots(&mut self) {
        while self
            .held_snapshots
            .front()
            .is_some_and(|(release, _)| *release <= self.clock)
        {
            if let Some((_release, bytes)) = self.held_snapshots.pop_front() {
                self.transport.send_unreliable(bytes);
            }
        }
    }

    /// Holds commands for the added delay.
    pub(crate) fn hold_commands(&mut self, commands: Vec<UserCommand>) {
        let release = self.clock + self.hold_seconds();
        self.held_commands.push_back((release, commands));
    }

    /// Holds a shot for the added delay.
    pub(crate) fn hold_fire(&mut self, fire: FireCommand) {
        let release = self.clock + self.hold_seconds();
        self.held_fires.push_back((release, fire));
    }

    /// Takes shots that have been held for long enough, by `now`.
    pub(crate) fn take_fires(&mut self, now: f64) -> Vec<FireCommand> {
        let mut fires = Vec::new();
        while self
            .held_fires
            .front()
            .is_some_and(|(release, _)| *release <= self.clock + now)
        {
            if let Some((_release, fire)) = self.held_fires.pop_front() {
                fires.push(fire);
            }
        }
        fires
    }

    /// Takes commands that have been held for long enough.
    pub(crate) fn take_commands(&mut self) -> Vec<UserCommand> {
        let mut commands = Vec::new();
        while self
            .held_commands
            .front()
            .is_some_and(|(release, _)| *release <= self.clock)
        {
            if let Some((_release, more)) = self.held_commands.pop_front() {
                commands.extend(more);
            }
        }
        commands
    }

    /// Whether their ping has been measured yet.
    pub(crate) fn has_ping(&self) -> bool {
        self.ping.is_some()
    }

    /// Their ping as measured here, in milliseconds. Nothing they say counts:
    /// it's how long their answer to a snapshot took, less the tick that
    /// waiting for a command to carry it takes.
    #[must_use]
    pub(crate) fn ping(&self) -> f64 {
        self.ping.unwrap_or(0.0)
    }

    /// Forgets what they've seen, since a new level starts from nothing.
    pub(crate) fn start_level(&mut self) {
        self.history.clear();
        self.sent.clear();
        self.held_commands.clear();
        self.held_snapshots.clear();
        self.held_fires.clear();
        self.backlog.clear();
        self.seen = 0;
        self.player = None;
        self.shapes_told = 0;
    }

    /// Notes the last snapshot they received.
    pub(crate) fn note_seen(&mut self, tick: u32, tick_seconds: f64) {
        if tick <= self.seen {
            return;
        }
        self.seen = tick;
        if let Some((_tick, sent)) = self.sent.iter().find(|(sent, _)| *sent == tick) {
            let added = self.added;
            let sample = ((self.clock - sent - tick_seconds) * 1000.0 - added).max(0.0);
            self.ping = Some(
                self.ping
                    .map_or(sample, |ping| ping + (sample - ping) * PING_SMOOTHING),
            );
        }
    }

    /// Notes that something arrived from them.
    pub(crate) fn heard(&mut self) {
        self.quiet = 0.0;
    }

    /// Passes on what's waiting to go.
    pub(crate) fn flush(&mut self) {
        self.transport.flush();
    }

    /// Moves the clock on by `seconds`.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a tick needs no more than f32"
    )]
    pub(crate) fn add_allowance(&mut self, seconds: f64) {
        self.allowance = (self.allowance + seconds as f32).min(MAX_ALLOWANCE);
        self.quiet += seconds;
        self.clock += seconds;
        self.transport.update(Duration::from_secs_f64(seconds));
        self.release_snapshots();
    }

    /// Uses `seconds` of movement, and says whether they had it. A command
    /// never lasts longer than the simulation lets it, so that's all it costs.
    pub(crate) fn spend(&mut self, seconds: f32) -> bool {
        let cost = seconds.clamp(0.0, MAX_COMMAND_COST);
        if cost > self.allowance {
            return false;
        }
        self.allowance -= cost;
        true
    }

    /// Whether they've left, or gone quiet for too long.
    pub(crate) fn is_lost(&self) -> bool {
        !self.transport.is_connected() || self.quiet > TIMEOUT_SECONDS
    }
}
