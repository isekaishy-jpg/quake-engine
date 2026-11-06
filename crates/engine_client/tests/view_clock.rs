//! How far behind others are drawn, on networks good and bad (proposal 0128).

#![expect(clippy::print_stdout, reason = "the measurements are the point")]

use engine_client::ViewClock;

/// A tick.
const TICK: f64 = 1.0 / 72.0;

/// A frame.
const FRAME: f64 = 1.0 / 120.0;

/// A network between the server and what's drawn.
#[derive(Clone, Copy)]
struct Network {
    /// The most that's added at random to a snapshot's trip, in seconds.
    jitter: f64,
    /// The share of snapshots lost.
    loss: f64,
    /// The chance the next is lost too, after a loss.
    burst: f64,
}

/// What a run of `seconds` showed.
struct Run {
    /// The share of frames with nothing newer to draw towards.
    starved: f64,
    /// How far behind the newest snapshot it was on average, in seconds.
    delay: f64,
    /// The largest jump in what's drawn from one frame to the next, as a share of a frame.
    worst_step: f64,
}

/// A small random number generator, so a run repeats.
struct Random(u64);

impl Random {
    /// The next number from 0 up to 1.
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        #[expect(clippy::cast_precision_loss, reason = "a random share")]
        let share = (self.0 >> 11) as f64 / (1_u64 << 53) as f64;
        share
    }
}

/// Draws frames over `network` for `seconds`.
fn run(network: Network, seconds: f64) -> Run {
    let mut random = Random(0x2545_F491_4F6C_DD1D);
    // Every snapshot that gets through: when it arrives, and its time.
    let mut arrivals = Vec::new();
    let mut lost_last = false;
    let mut sent = 0.0;
    while sent < seconds {
        let chance = if lost_last {
            network.burst
        } else {
            network.loss
        };
        lost_last = random.next() < chance;
        if !lost_last {
            arrivals.push((sent + random.next() * network.jitter, sent));
        }
        sent += TICK;
    }
    arrivals.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut clock = ViewClock::default();
    let (mut now, mut newest, mut next) = (0.0, 0.0_f64, 0);
    let (mut starved, mut frames) = (0_u32, 0_u32);
    let mut last_drawn = None;
    let mut worst_step: f64 = 0.0;
    let mut delay = 0.0;
    while now < seconds {
        now += FRAME;
        while let Some((arrives, time)) = arrivals.get(next).copied() {
            if arrives > now {
                break;
            }
            newest = newest.max(time);
            next += 1;
        }
        let drawn = clock.advance(FRAME, newest);
        // The first seconds are the clock finding its feet.
        if now > 5.0 {
            frames += 1;
            starved += u32::from(drawn >= newest);
            if let Some(last) = last_drawn {
                let step: f64 = (drawn - last) / FRAME;
                worst_step = worst_step.max(step);
            }
            delay += newest - drawn;
        }
        last_drawn = Some(drawn);
    }
    Run {
        starved: f64::from(starved) / f64::from(frames.max(1)),
        delay: delay / f64::from(frames.max(1)),
        worst_step,
    }
}

/// A network with `jitter` in milliseconds, losing 2% in ones and twos.
const fn usual(jitter: f64) -> Network {
    Network {
        jitter: jitter / 1000.0,
        loss: 0.02,
        burst: 0.3,
    }
}

#[test]
fn a_clean_connection_stays_close_to_the_server() {
    let clean = Network {
        jitter: 0.0,
        loss: 0.0,
        burst: 0.0,
    };
    let run = run(clean, 60.0);
    assert!(run.starved < 0.001, "{}", run.starved);
    assert!(run.delay < 0.04, "{}", run.delay);
}

#[test]
fn two_percent_loss_rarely_leaves_nothing_to_draw_towards() {
    let run = run(usual(5.0), 120.0);
    assert!(run.starved < 0.01, "{}", run.starved);
}

#[test]
fn a_jittery_connection_buys_itself_more_time() {
    let without_loss = |jitter: f64| Network {
        jitter,
        loss: 0.0,
        burst: 0.0,
    };
    let steady = run(without_loss(0.0), 120.0);
    let jittery = run(without_loss(0.04), 120.0);
    assert!(
        jittery.delay > steady.delay + 0.015,
        "{} {}",
        jittery.delay,
        steady.delay
    );
    assert!(jittery.starved < 0.02, "{}", jittery.starved);
}

#[test]
fn even_a_bad_connection_is_not_left_far_behind() {
    let awful = Network {
        jitter: 0.2,
        loss: 0.05,
        burst: 0.5,
    };
    let run = run(awful, 120.0);
    assert!(run.delay < 0.2, "{}", run.delay);
}

#[test]
fn what_is_drawn_moves_evenly() {
    let run = run(usual(20.0), 120.0);
    assert!(run.worst_step < 1.3, "{}", run.worst_step);
}

/// The table in proposal 0128.
#[test]
#[ignore = "prints a table"]
fn table() {
    for jitter in [0.0, 5.0, 10.0, 20.0, 40.0, 80.0] {
        for (loss, burst) in [(0.0, 0.0), (0.02, 0.0), (0.02, 0.3), (0.05, 0.5)] {
            let network = Network {
                jitter: jitter / 1000.0,
                loss,
                burst,
            };
            let run = run(network, 300.0);
            println!(
                "jitter {jitter:>2} ms loss {loss:.2} burst {burst:.1}: starved {:>6.2}% delay {:>5.1} ms worst step {:.2}",
                run.starved * 100.0,
                run.delay * 1000.0,
                run.worst_step
            );
        }
    }
}

/// Loading isn't the network, so it doesn't leave others drawn far back.
#[test]
fn the_wait_for_the_first_snapshot_is_not_held_against_the_network() {
    let mut clock = ViewClock::default();
    let _waiting = clock.advance(3.0, 0.0);
    let mut newest = 100.0;
    let mut drawn = clock.advance(FRAME, newest);
    for _ in 0..200 {
        newest += TICK;
        drawn = clock.advance(TICK, newest);
    }
    assert!(newest - drawn < 0.05, "{}", newest - drawn);
}

/// A new level's clock starts again from nothing.
#[test]
fn a_new_level_starts_the_clock_again() {
    let mut clock = ViewClock::default();
    let mut newest = 100.0;
    for _ in 0..100 {
        newest += TICK;
        let _drawn = clock.advance(TICK, newest);
    }
    let drawn = clock.advance(TICK, 1.0);
    assert!((drawn - 1.0).abs() < 0.05, "{drawn}");
    let next = clock.advance(TICK, 1.0 + TICK);
    assert!(next > drawn, "{next} after {drawn}");
}
