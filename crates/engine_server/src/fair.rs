//! The fair play zone: who gets how much help with their ping (proposal 0051).

use engine_core::time::{MOST_VIEW_DELAY, TICK_SECONDS};

/// How much later than a shooter's ping a view is on a clean connection, in seconds: the
/// interpolation buffer, with a tick to spare.
const STEADY_VIEW_SLACK: f64 = 3.0 * TICK_SECONDS;

/// How long a wait takes to count for half as much, which is how long a client's view
/// clock holds on to one.
const WAIT_HALF_LIFE: f64 = 6.0;

/// How far back past a shooter's ping a shot can reach, from what the server has seen.
///
/// A client on a bad connection draws further back, so its shots have to reach further.
/// Saying so isn't enough, as reaching further lets a cheat hit someone who's already behind
/// cover. So it goes by how long the client really took to say it had each snapshot.
/// Holding that back to gain reach puts its own ping up, towards the ceiling.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewSlack {
    /// The longest they've taken lately, past a tick, in seconds.
    longest_wait: f64,
    /// When they last said they had a new snapshot, on the seat's clock.
    last_heard: Option<f64>,
}

impl ViewSlack {
    /// Notes that they said they had a new snapshot, at `clock`.
    pub fn heard(&mut self, clock: f64, tick_seconds: f64) {
        if let Some(last) = self.last_heard {
            self.longest_wait = self.longest_wait.max(clock - last - tick_seconds);
        }
        self.last_heard = Some(clock);
    }

    /// Lets `seconds` go by.
    pub fn pass(&mut self, seconds: f64) {
        self.longest_wait *= 0.5_f64.powf(seconds / WAIT_HALF_LIFE);
    }

    /// How far past their ping a shot can reach now, in seconds.
    #[must_use]
    pub fn seconds(&self) -> f64 {
        (STEADY_VIEW_SLACK + self.longest_wait).min(MOST_VIEW_DELAY + TICK_SECONDS)
    }
}

/// The server's rules, in milliseconds and seconds as named.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FairSettings {
    /// `sv_fair_enable`: off means plain lag compensation.
    pub enable: bool,
    /// `sv_fair_floor`: added delay brings players up to this ping.
    pub floor: f64,
    /// `sv_fair_width`: the zone's top is the mean plus this.
    pub width: f64,
    /// `sv_fair_ceiling`: ping above which there's no compensation.
    pub ceiling: f64,
    /// `sv_fair_ceiling_margin`: how far under the ceiling ping must drop to
    /// get compensation back.
    pub ceiling_margin: f64,
    /// `sv_fair_partial`: the share of the excess compensated between the
    /// zone's top and the ceiling.
    pub partial: f64,
    /// `sv_fair_outlier`: pings over this are left out of the mean, unless
    /// they're the majority.
    pub outlier: f64,
    /// `sv_fair_interval`: seconds between zone updates.
    pub interval: f64,
    /// `sv_unlag_debug`: whether shots show the boxes they're judged against.
    pub show: bool,
    /// `sv_unlag_enable`: whether shots rewind at all.
    pub unlag: bool,
}

impl Default for FairSettings {
    fn default() -> Self {
        Self {
            enable: true,
            floor: 20.0,
            width: 20.0,
            ceiling: 90.0,
            ceiling_margin: 5.0,
            partial: 0.5,
            outlier: 100.0,
            interval: 30.0,
            unlag: true,
            show: false,
        }
    }
}

impl FairSettings {
    /// Each rule's console name and its value as text.
    #[must_use]
    pub fn cvars(&self) -> [(&'static str, String); 10] {
        let flag = |on: bool| String::from(if on { "1" } else { "0" });
        [
            ("sv_fair_enable", flag(self.enable)),
            ("sv_fair_floor", self.floor.to_string()),
            ("sv_fair_width", self.width.to_string()),
            ("sv_fair_ceiling", self.ceiling.to_string()),
            ("sv_fair_ceiling_margin", self.ceiling_margin.to_string()),
            ("sv_fair_partial", self.partial.to_string()),
            ("sv_fair_outlier", self.outlier.to_string()),
            ("sv_fair_interval", self.interval.to_string()),
            ("sv_unlag_debug", flag(self.show)),
            ("sv_unlag_enable", flag(self.unlag)),
        ]
    }

    /// The rules among a level's `settings`, with defaults for the rest.
    #[must_use]
    pub fn from_settings(settings: &[(String, f64)]) -> Self {
        let mut rules = Self::default();
        for (name, value) in settings {
            match name.as_str() {
                "sv_fair_enable" => rules.enable = *value != 0.0,
                "sv_fair_floor" => rules.floor = *value,
                "sv_fair_width" => rules.width = *value,
                "sv_fair_ceiling" => rules.ceiling = *value,
                "sv_fair_ceiling_margin" => rules.ceiling_margin = *value,
                "sv_fair_partial" => rules.partial = *value,
                "sv_fair_outlier" => rules.outlier = *value,
                "sv_fair_interval" => rules.interval = *value,
                "sv_unlag_enable" => rules.unlag = *value != 0.0,
                "sv_unlag_debug" => rules.show = *value != 0.0,
                _ => {}
            }
        }
        rules
    }
}

/// Where the lobby stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    /// The ping delay brings the lowest up to.
    pub floor: f64,
    /// The top of the zone, up to which ping is fully compensated.
    pub top: f64,
    /// The ping above which nothing is compensated.
    pub ceiling: f64,
}

impl Zone {
    /// The zone for a lobby of `pings`, by `rules`.
    #[must_use]
    pub fn of(pings: &[f64], rules: &FairSettings) -> Self {
        let high = pings.iter().filter(|ping| **ping > rules.outlier).count();
        let majority_high = high * 2 > pings.len();
        let counted: Vec<f64> = pings
            .iter()
            .copied()
            .filter(|ping| majority_high || *ping <= rules.outlier)
            .collect();
        #[expect(clippy::cast_precision_loss, reason = "a few players")]
        let mean = counted.iter().sum::<f64>() / counted.len().max(1) as f64;
        let top = rules.floor.max(mean + rules.width);
        // Nobody is slowed for no reason, as in a LAN game.
        let highest = pings.iter().copied().fold(0.0, f64::max);
        Self {
            floor: rules.floor.min(highest),
            top,
            ceiling: rules.ceiling.max(top),
        }
    }

    /// How many milliseconds a player on `ping` has added.
    #[must_use]
    pub fn added_delay(&self, ping: f64) -> f64 {
        (self.floor - ping).max(0.0)
    }

    /// How many milliseconds of a player's ping are made up for, given
    /// whether they were `over` the ceiling last time. Returns that, and
    /// whether they're over it now.
    #[must_use]
    pub fn compensated(&self, ping: f64, over: bool, rules: &FairSettings) -> (f64, bool) {
        let over = if over {
            ping >= self.ceiling - rules.ceiling_margin
        } else {
            ping > self.ceiling
        };
        if over {
            (0.0, true)
        } else if ping <= self.top {
            (ping, false)
        } else {
            (self.top + (ping - self.top) * rules.partial, false)
        }
    }
}
