//! Things that fly, as the client works them out (proposal 0052).
//!
//! A projectile's path is fixed once it's fired, so a launch event is all it
//! takes to know it. Only the server's projectile hits anything. What's here
//! decides where it's drawn, and when a rocket will go off, and can be tuned
//! without touching any result.

use engine_core::entity::{ModelInstance, Projectile};
use engine_core::math::Vec3;
use engine_core::time::TICK_SECONDS;
use engine_physics::CollisionWorld;

/// A tick, which is what the server moves things by.
#[expect(clippy::cast_possible_truncation, reason = "a tick fits f32")]
const STEP: f32 = TICK_SECONDS as f32;

/// How long a path is worked out for, in seconds.
const MAX_FLIGHT: f32 = 6.0;

/// How much of a hit's speed a bounce turns round, from `SV_Physics_Toss`.
const BOUNCE: f32 = 1.5;

/// Under this upward speed, a thing that lands stays down.
const REST_SPEED: f32 = 60.0;

/// How long an enemy's projectile takes to catch up with its true place.
const CATCH_UP_SECONDS: f64 = 0.1;

/// How long your own projectile is held back at most, so it can't be felt.
const LAUNCH_DELAY: f64 = 0.008;

/// How long your own projectile takes to settle into the correction.
const SETTLE_SECONDS: f64 = 0.4;

/// The times a flight is drawn by.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Timeline {
    /// The game time at the server now, as far as can be told.
    pub present: f64,
    /// The game time the player's own commands are running at, which is
    /// a round trip past the last snapshot.
    pub own: f64,
    /// The game time others are drawn at.
    pub behind: f64,
    /// The share of the gap own shots are eased by.
    pub correction: f64,
}

/// A launch, as the server or this client's guess has it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Launch {
    /// Where it starts.
    pub origin: Vec3,
    /// How it starts moving.
    pub velocity: Vec3,
    /// The game time it starts.
    pub time: f64,
    /// Whether it falls and bounces, rather than flying straight.
    pub bounces: bool,
    /// Whether it blows up when it first hits something.
    pub blasts: bool,
    /// How many times stronger than usual its damage is.
    pub strength: f32,
    /// Seconds until it blows up by itself, or 0 if it won't.
    pub fuse: f32,
    /// `sv_gravity`, which what bounces falls by.
    pub gravity: f32,
}

/// A blast on its way, as a flight's path has it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Blast {
    /// The command it lands on.
    pub(crate) lands_on: u32,
    /// Where it is.
    pub(crate) place: Vec3,
    /// Whether it's our own.
    pub(crate) own: bool,
    /// How many times stronger than usual it is.
    pub(crate) strength: f32,
}

/// Something flying into us, or into something else.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Hit {
    /// Where it is.
    pub(crate) place: Vec3,
    /// How many times stronger than usual it is.
    pub(crate) strength: f32,
    /// Whether all its damage comes from the hit, rather than as a blast.
    pub(crate) direct: bool,
    /// Whether it's our own.
    pub(crate) own: bool,
}

/// One path through the map.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    /// Places a tick apart.
    points: Vec<Vec3>,
    /// The launch.
    launch: Launch,
    /// When and where it first hits something, and what it hit's normal.
    impact: Option<(f64, Vec3)>,
}

impl Path {
    /// Works out the path of `launch`, with `instances` where they are now.
    #[must_use]
    pub fn new(launch: Launch, world: &CollisionWorld, instances: &[ModelInstance]) -> Self {
        let collision = world.with(instances);
        let mut position = launch.origin;
        let mut velocity = launch.velocity;
        let mut points = vec![position];
        let mut impact = None;
        let mut flown = 0.0;
        while flown < MAX_FLIGHT {
            if launch.bounces {
                velocity.z -= launch.gravity * STEP;
            }
            // A point, not a player: `trace` would stop a player's box short.
            let end = position + velocity * STEP;
            let trace = collision.move_box(position, Vec3::ZERO, Vec3::ZERO, end);
            position = trace.end;
            flown += STEP;
            if trace.fraction < 1.0 {
                if impact.is_none() {
                    impact = Some((launch.time + f64::from(flown), position));
                }
                if !launch.bounces {
                    points.push(position);
                    break;
                }
                let into = velocity.dot(trace.normal);
                velocity = velocity - trace.normal * (into * BOUNCE);
                if trace.normal.z > 0.7 && velocity.z < REST_SPEED {
                    points.push(position);
                    break;
                }
            }
            points.push(position);
        }
        Self {
            points,
            launch,
            impact,
        }
    }

    /// Where it is at game time `time`.
    #[must_use]
    pub fn at(&self, time: f64) -> Vec3 {
        #[expect(clippy::cast_possible_truncation, reason = "seconds of flight")]
        let ticks = ((time - self.launch.time).max(0.0) as f32) / STEP;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a place in a short list"
        )]
        let index = ticks as usize;
        let last = self.points.len() - 1;
        if index >= last {
            return self.points[last];
        }
        self.points[index].lerp(self.points[index + 1], ticks.fract())
    }

    /// When and where it first hits something.
    #[must_use]
    pub fn impact(&self) -> Option<(f64, Vec3)> {
        self.impact
    }

    /// The launch.
    #[must_use]
    pub fn launch(&self) -> &Launch {
        &self.launch
    }
}

/// What the client knows of something flying.
#[derive(Clone, Debug, PartialEq)]
struct Flight {
    /// The entity, or `None` for a guess at our own shot.
    entity: Option<u64>,
    /// Its path.
    path: Path,
    /// Whether this player fired it.
    own: bool,
    /// The drawn time when it first appeared.
    seen_at: f64,
    /// The model, for a guess, which has no entity to go by.
    model: u16,
    /// Whether it's been used to predict a blast.
    blast_used: bool,
    /// The command running when it was fired.
    fired_on: u32,
    /// Whether a snapshot has shown its entity. A launch is reliable and a
    /// snapshot isn't, so it can be heard of first.
    seen: bool,
}

impl Flight {
    /// The game time to draw it at, from the record's defaults.
    fn drawn_time(&self, times: &Timeline) -> f64 {
        let age = (times.behind - self.seen_at).max(0.0);
        let drawn = if self.own {
            // Leaves when you fire, then eases towards what's on your screen.
            let gap = (times.own - times.behind).max(0.0);
            let settled = (age / SETTLE_SECONDS).min(1.0);
            let eased = ((gap * times.correction) - LAUNCH_DELAY).max(0.0) * settled;
            times.own - LAUNCH_DELAY.min(gap) - eased
        } else {
            // Starts at the muzzle, and catches up with the real thing.
            let gap = (times.present - times.behind).max(0.0);
            times.present - gap * (1.0 - (age / CATCH_UP_SECONDS).min(1.0))
        };
        drawn.max(self.path.launch.time)
    }
}

/// Everything known to be flying.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Flights {
    /// What's in the air.
    flights: Vec<Flight>,
}

impl Flights {
    /// Notes a launch from the server. A guess at the same shot gives way.
    pub fn launched(
        &mut self,
        projectile: &Projectile,
        own: bool,
        fired_on: u32,
        (behind, gravity): (f64, f32),
        world: &CollisionWorld,
        instances: &[ModelInstance],
    ) {
        let mut fired_on = fired_on;
        let launch = Launch {
            origin: projectile.origin,
            velocity: projectile.velocity,
            time: projectile.time,
            bounces: projectile.bounces,
            blasts: projectile.blasts,
            strength: projectile.strength,
            fuse: projectile.fuse,
            gravity,
        };
        let mut seen_at = behind;
        let mut blast_used = false;
        if own {
            let guess = self.flights.iter().position(|flight| {
                let same = &flight.path.launch;
                flight.entity.is_none()
                    && same.bounces == launch.bounces
                    && same.blasts == launch.blasts
            });
            if let Some(guess) = guess {
                let guess = self.flights.remove(guess);
                seen_at = guess.seen_at;
                blast_used = guess.blast_used;
                fired_on = guess.fired_on;
            }
        }
        self.flights.push(Flight {
            entity: Some(projectile.entity),
            path: Path::new(launch, world, instances),
            own,
            seen_at,
            model: 0,
            blast_used,
            fired_on,
            seen: false,
        });
    }

    /// Adds a guess at a shot of our own, which the server's launch replaces.
    pub fn guess(
        &mut self,
        launch: Launch,
        model: u16,
        fired_on: u32,
        behind: f64,
        world: &CollisionWorld,
        instances: &[ModelInstance],
    ) {
        self.flights.push(Flight {
            entity: None,
            path: Path::new(launch, world, instances),
            own: true,
            seen_at: behind,
            model,
            blast_used: false,
            fired_on,
            seen: false,
        });
    }

    /// Forgets what the server no longer has, and what it never showed by
    /// `give_up`, so a refused shot just goes away.
    pub fn retain(&mut self, alive: impl Fn(u64) -> bool, behind: f64, give_up: f64) {
        self.flights.retain_mut(|flight| {
            let shown = flight.entity.is_some_and(&alive);
            let waited = behind - flight.seen_at < give_up;
            let keep = shown || (!flight.seen && waited);
            flight.seen |= shown;
            keep
        });
    }

    /// Where to draw entity `id`, if its path is known.
    #[must_use]
    pub fn place_of(&self, id: u64, times: &Timeline) -> Option<Vec3> {
        let flight = self
            .flights
            .iter()
            .find(|flight| flight.entity == Some(id))?;
        Some(flight.path.at(flight.drawn_time(times)))
    }

    /// Guesses still waiting, as their models, places and directions.
    pub fn guesses(&self, times: Timeline) -> impl Iterator<Item = (u16, Vec3, Vec3)> + '_ {
        self.flights
            .iter()
            .filter(|flight| flight.entity.is_none())
            .map(move |flight| {
                let time = flight.drawn_time(&times);
                (
                    flight.model,
                    flight.path.at(time),
                    flight.path.launch.velocity,
                )
            })
    }

    /// Takes blasts that go off by command `command`, once each, whoever's
    /// they are. Something that flies for a number of ticks goes off that many
    /// commands after the one it was fired on.
    pub(crate) fn take_blasts(&mut self, command: u32) -> Vec<Blast> {
        let mut blasts = Vec::new();
        for flight in &mut self.flights {
            let launch = flight.path.launch;
            let due = if launch.fuse > 0.0 {
                let when = launch.time + f64::from(launch.fuse);
                Some((when, flight.path.at(when)))
            } else if launch.blasts {
                flight.path.impact
            } else {
                None
            };
            let Some((when, place)) = due else {
                continue;
            };
            let flown = (when - launch.time) / f64::from(STEP);
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a few hundred ticks at most"
            )]
            let lands_on = flight.fired_on + flown.round() as u32;
            if !flight.blast_used && lands_on <= command {
                flight.blast_used = true;
                blasts.push(Blast {
                    lands_on,
                    place,
                    own: flight.own,
                    strength: launch.strength,
                });
            }
        }
        blasts
    }

    /// Takes whatever of anyone else's is inside our box, from `low` to `high`,
    /// by command `command`, once each, and what's run into one of `others` on
    /// the way, which goes off there. Like a blast, it's counted in commands
    /// from the one it was fired on, a tick of flight to each.
    pub(crate) fn take_hits(
        &mut self,
        command: u32,
        (low, high): (Vec3, Vec3),
        others: &[(Vec3, Vec3)],
    ) -> Vec<Hit> {
        let within = |place: Vec3, low: Vec3, high: Vec3| {
            (low.x..=high.x).contains(&place.x)
                && (low.y..=high.y).contains(&place.y)
                && (low.z..=high.z).contains(&place.z)
        };
        let mut hits = Vec::new();
        for flight in &mut self.flights {
            let launch = flight.path.launch;
            let explosive = launch.blasts || launch.fuse > 0.0;
            if flight.blast_used || !explosive {
                continue;
            }
            let flown = command.checked_sub(flight.fired_on);
            let place = flown.and_then(|flown| flight.path.points.get(flown as usize));
            let Some(&place) = place else {
                continue;
            };
            // What fired it can't be hit by it, and an item's box is nothing.
            let blocked = others.iter().any(|(low, high)| {
                low != high && within(place, *low, *high) && !within(launch.origin, *low, *high)
            });
            let inside = !flight.own && within(place, low, high);
            if inside || blocked {
                flight.blast_used = true;
                hits.push(Hit {
                    place,
                    strength: launch.strength,
                    direct: inside && launch.blasts,
                    own: flight.own,
                });
            }
        }
        hits
    }

    /// The newest launch known, with its entity.
    #[must_use]
    pub fn last_launch(&self) -> Option<(u64, Launch)> {
        let flight = self.flights.last()?;
        Some((flight.entity?, flight.path.launch))
    }

    /// How many are known, for tests.
    #[must_use]
    pub fn len(&self) -> usize {
        self.flights.len()
    }

    /// Whether none are known.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.flights.is_empty()
    }
}
