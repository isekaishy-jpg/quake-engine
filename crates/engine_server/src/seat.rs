//! Someone connected to a game, and what the server keeps for them.

use std::collections::VecDeque;

use engine_net::{ServerMessage, Snapshot, Transport, delta_from};
use engine_sim::EntityId;

/// The most seconds of movement a client can have in hand, so a client can't
/// save up time, or move faster than the clock, by sending commands in bursts.
const MAX_ALLOWANCE: f32 = 0.25;

/// The most a single command can cost.
const MAX_COMMAND_COST: f32 = 0.1;

/// How many snapshots are kept to send differences from.
const SNAPSHOT_HISTORY: usize = 64;

/// Seconds of silence after which someone's gone.
const TIMEOUT_SECONDS: f64 = 10.0;

/// Who someone says they are.
#[derive(Clone, Debug, PartialEq, Eq)]
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
        self.transport.send_unreliable(message);
        if self.history.len() == SNAPSHOT_HISTORY {
            let _oldest = self.history.pop_front();
        }
        self.history.push_back(snapshot);
    }

    /// Notes the last snapshot they received.
    pub(crate) fn note_seen(&mut self, tick: u32) {
        if tick > self.seen {
            self.seen = tick;
        }
    }

    /// Notes that something arrived from them.
    pub(crate) fn heard(&mut self) {
        self.quiet = 0.0;
    }

    /// Moves the clock on by `seconds`.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a tick needs no more than f32"
    )]
    pub(crate) fn add_allowance(&mut self, seconds: f64) {
        self.allowance = (self.allowance + seconds as f32).min(MAX_ALLOWANCE);
        self.quiet += seconds;
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
