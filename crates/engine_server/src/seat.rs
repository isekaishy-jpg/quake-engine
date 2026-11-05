//! Someone connected to a game, and what the server keeps for them.

use std::collections::VecDeque;
use std::time::Duration;

use engine_core::player::UserCommand;
use engine_core::time::TICK_SECONDS;
use engine_net::{
    ClientMessage, FireCommand, MAX_NAME_BYTES, PublicKey, ServerMessage, Snapshot, Transport,
    delta_from,
};
use engine_sim::EntityId;
use serde::{Deserialize, Serialize};

use crate::fair::{FairSettings, Zone};

/// The most seconds of movement a client can have in hand, so a client can't
/// save up time, or move faster than the clock, by sending commands in bursts.
const MAX_ALLOWANCE: f32 = 0.25;

/// How much later than a shooter's ping a view can be, in seconds: the
/// interpolation buffer, with a tick to spare.
const VIEW_SLACK: f64 = 3.0 * TICK_SECONDS;

/// The most a single command can cost.
const MAX_COMMAND_COST: f32 = 0.1;

/// The least a single command can cost, because even one that lasts no time
/// runs the game's scripts.
const MIN_COMMAND_COST: f32 = 0.001;

/// How many snapshots are kept to send differences from.
const SNAPSHOT_HISTORY: usize = 64;

/// How much each sample moves a measured ping, so short spikes don't count.
const PING_SMOOTHING: f64 = 0.02;

/// How long someone put out gets to hear why, in seconds.
const KICK_SECONDS: f64 = 0.25;

/// How many answers a level's first measure of ping takes. The lowest counts,
/// since a client that's just finished loading can stall for a while, and
/// that's no measure of its network.
const PING_WARM_UP: u32 = 8;

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

/// `text` with nothing in it that could pass for something else, and no longer
/// than `limit` bytes, which is what the wire counts.
pub(crate) fn plain_text(text: &str, limit: usize) -> String {
    let mut plain = String::new();
    for character in text.trim().chars().filter(|c| !c.is_control()) {
        if plain.len() + character.len_utf8() > limit {
            break;
        }
        plain.push(character);
    }
    plain
}

impl SeatInfo {
    /// This with a name that's safe to show and send, or the usual one if
    /// nothing's left of it.
    #[must_use]
    pub fn tidied(self) -> Self {
        let name = plain_text(&self.name, MAX_NAME_BYTES);
        if name.is_empty() {
            Self {
                name: Self::default().name,
                ..self
            }
        } else {
            Self { name, ..self }
        }
    }
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
    /// The key they've proved they hold, if they came over the network.
    pub(crate) identity: Option<PublicKey>,
    /// Seconds left before they're put out, so they get to hear why.
    kicked: Option<f64>,
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
    /// How many answers have counted this level, and the lowest of them.
    warm_up: (u32, f64),
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
        let identity = transport.peer_identity();
        Self {
            transport,
            player: None,
            info: info.tidied(),
            identity,
            kicked: None,
            parms: None,
            shapes_told: 0,
            allowance: MAX_ALLOWANCE,
            quiet: 0.0,
            history: VecDeque::with_capacity(SNAPSHOT_HISTORY),
            seen: 0,
            clock: 0.0,
            sent: VecDeque::with_capacity(SNAPSHOT_HISTORY),
            ping: None,
            warm_up: (0, f64::INFINITY),
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

    /// Sends a message that has to arrive, already encoded.
    pub(crate) fn send_bytes(&mut self, bytes: Vec<u8>) {
        self.transport.send(bytes);
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

    /// Puts them out, once they've had time to hear why.
    pub(crate) fn kick(&mut self) {
        self.kicked = self.kicked.or(Some(KICK_SECONDS));
    }

    /// Whether their ping has been measured yet.
    pub(crate) fn has_ping(&self) -> bool {
        self.ping.is_some()
    }

    /// Their ping as measured here, in milliseconds. It's how long their
    /// answer to a snapshot took, less the tick that waiting for a command to
    /// carry it takes, and less what this server added itself. They can't claim
    /// a better one than they have. Holding answers back makes a worse one,
    /// which can't be told from a worse connection, so fairness's ceiling is
    /// what bounds it.
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
        self.warm_up = (0, f64::INFINITY);
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
            let (count, lowest) = self.warm_up;
            if count < PING_WARM_UP {
                self.warm_up = (count + 1, lowest.min(sample));
                if count + 1 == PING_WARM_UP {
                    self.ping = Some(self.warm_up.1);
                }
            } else {
                self.ping = self
                    .ping
                    .map(|ping| ping + (sample - ping) * PING_SMOOTHING);
            }
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
        if let Some(left) = &mut self.kicked {
            *left -= seconds;
        }
        self.quiet += seconds;
        self.clock += seconds;
        self.transport.update(Duration::from_secs_f64(seconds));
        self.release_snapshots();
    }

    /// Moves them towards where `zone` puts them, by no more than `step`
    /// milliseconds, and says how far back their shots can reach, in seconds.
    pub(crate) fn follow_zone(&mut self, zone: &Zone, rules: &FairSettings, step: f64) -> f64 {
        let ping = self.ping();
        let (compensated, over) = if rules.enable {
            zone.compensated(ping, self.over_ceiling, rules)
        } else {
            (ping, false)
        };
        let added = if rules.enable {
            zone.added_delay(ping)
        } else {
            0.0
        };
        if over != self.over_ceiling {
            self.over_ceiling = over;
            let text = if over {
                "Your ping is over this server's limit: lead your shots"
            } else {
                "Your ping is back within this server's limit"
            };
            self.send(&ServerMessage::Print(text.to_owned()));
        }
        if !self.settled && self.has_ping() {
            self.settled = true;
            self.added = added;
            self.compensated = compensated;
        }
        self.added += (added - self.added).clamp(-step, step);
        // Losing help all at once is the point of the ceiling.
        self.compensated = if over {
            0.0
        } else {
            self.compensated + (compensated - self.compensated).clamp(-step, step)
        };
        if !rules.unlag || over {
            0.0
        } else if rules.enable {
            self.compensated / 1000.0 + VIEW_SLACK
        } else {
            1.0
        }
    }

    /// Uses `seconds` of movement, and says whether they had it. A command
    /// never lasts longer than the simulation lets it, so that's all it costs.
    pub(crate) fn spend(&mut self, seconds: f32) -> bool {
        let cost = seconds.clamp(MIN_COMMAND_COST, MAX_COMMAND_COST);
        if cost > self.allowance {
            return false;
        }
        self.allowance -= cost;
        true
    }

    /// Whether they've left, or gone quiet for too long.
    pub(crate) fn is_lost(&self) -> bool {
        self.kicked.is_some_and(|left| left <= 0.0)
            || !self.transport.is_connected()
            || self.quiet > TIMEOUT_SECONDS
    }
}
