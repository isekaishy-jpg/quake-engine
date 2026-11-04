//! Things that fly, as the client works them out (proposal 0052).
//!
//! A projectile's path is fixed once it's fired, so a launch event is all it
//! takes to know it. Only the server's projectile hits anything. What's here
//! decides where it's drawn, and when a rocket will go off, and can be tuned
//! without touching any result.

use engine_core::entity::{ModelInstance, Projectile};
use engine_core::math::Vec3;
use engine_physics::CollisionWorld;

/// `sv_gravity`.
const GRAVITY: f32 = 800.0;

/// How long a path is worked out for, in seconds.
const MAX_FLIGHT: f32 = 6.0;

/// A tick, which is what the server moves things by.
const STEP: f32 = 1.0 / 72.0;

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
                velocity.z -= GRAVITY * STEP;
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
    /// The command it was fired on, if it's our own.
    fired_on: u32,
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
        fired_on: Option<u32>,
        behind: f64,
        world: &CollisionWorld,
        instances: &[ModelInstance],
    ) {
        let own = fired_on.is_some();
        let mut fired_on = fired_on.unwrap_or(0);
        let launch = Launch {
            origin: projectile.origin,
            velocity: projectile.velocity,
            time: projectile.time,
            bounces: projectile.bounces,
        };
        let mut seen_at = behind;
        let mut blast_used = false;
        if own {
            let guess = self.flights.iter().position(|flight| {
                flight.entity.is_none() && flight.path.launch.bounces == launch.bounces
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
        });
    }

    /// Forgets what the server no longer has, and guesses it never confirmed
    /// by `give_up`, so a refused shot just goes away.
    pub fn retain(&mut self, alive: impl Fn(u64) -> bool, behind: f64, give_up: f64) {
        self.flights.retain(|flight| match flight.entity {
            Some(id) => alive(id),
            None => behind - flight.seen_at < give_up,
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

    /// Takes the commands and places of own rockets that go off by command
    /// `command`, once each. A rocket that flies for a number of ticks knocks
    /// the command that many after the one that fired it.
    pub fn take_blasts(&mut self, command: u32) -> Vec<(u32, Vec3)> {
        let mut blasts = Vec::new();
        for flight in &mut self.flights {
            let Some((when, place)) = flight.path.impact else {
                continue;
            };
            let flown = (when - flight.path.launch.time) / f64::from(STEP);
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a few hundred ticks at most"
            )]
            let lands_on = flight.fired_on + flown.round() as u32;
            let mine = flight.own && !flight.path.launch.bounces;
            if mine && !flight.blast_used && lands_on <= command {
                flight.blast_used = true;
                blasts.push((lands_on, place));
            }
        }
        blasts
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
