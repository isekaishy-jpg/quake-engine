//! A poor network on purpose, for testing (proposal 0049).

use std::collections::VecDeque;
use std::time::Duration;

use crate::identity::PublicKey;
use crate::transport::Transport;

/// What a connection suffers, each way it's applied.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Conditions {
    /// How long what's sent takes to leave.
    pub send_delay: Duration,
    /// How long what arrives takes to be seen.
    pub receive_delay: Duration,
    /// The most that's added at random to what may arrive out of order.
    pub jitter: Duration,
    /// The share of what's sent unreliably that's lost, from 0 to 1.
    pub loss: f64,
    /// The chance that what follows a loss is lost too, from 0 to 1, as real networks lose
    /// things in bursts. Past [`Self::loss`], it's what decides how long they are.
    pub burst: f64,
}

/// A message on its way.
struct Delayed {
    /// When it arrives.
    due: Duration,
    /// Whether it may be lost or overtaken.
    unreliable: bool,
    /// The message.
    bytes: Vec<u8>,
}

/// Wraps a transport in [`Conditions`]. Time only passes through `update`, so
/// a test can run hours of network in moments, the same way each time.
pub struct Conditioned {
    /// The real connection.
    inner: Box<dyn Transport>,
    /// What it suffers.
    conditions: Conditions,
    /// Time so far.
    clock: Duration,
    /// The last step of the clock, since messages only move on steps.
    step: Duration,
    /// What's been sent, not yet passed on.
    outgoing: VecDeque<Delayed>,
    /// What's arrived, not yet seen.
    incoming: VecDeque<(Duration, Vec<u8>)>,
    /// The state of the random numbers.
    random: u64,
    /// Whether the last unreliable message was lost.
    lost_last: bool,
}

impl Conditioned {
    /// `inner` in `conditions`. The same `seed` gives the same losses.
    #[must_use]
    pub fn new(inner: Box<dyn Transport>, conditions: Conditions, seed: u64) -> Self {
        Self {
            inner,
            conditions,
            clock: Duration::ZERO,
            step: Duration::ZERO,
            outgoing: VecDeque::new(),
            incoming: VecDeque::new(),
            random: seed | 1,
            lost_last: false,
        }
    }

    /// The next number from 0 up to 1, by xorshift.
    fn next_random(&mut self) -> f64 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        #[expect(clippy::cast_precision_loss, reason = "a random share")]
        let value = (self.random >> 11) as f64 / (1_u64 << 53) as f64;
        value
    }

    /// When what's sent now arrives.
    fn due(&mut self, unreliable: bool) -> Duration {
        let mut due = self.clock + self.conditions.send_delay;
        if unreliable {
            due += self.conditions.jitter.mul_f64(self.next_random());
        } else if let Some(last) = self.outgoing.iter().rfind(|message| !message.unreliable) {
            // What has to arrive in order still does.
            due = due.max(last.due);
        }
        due
    }
}

impl Transport for Conditioned {
    fn send(&mut self, bytes: Vec<u8>) {
        let due = self.due(false);
        self.outgoing.push_back(Delayed {
            due,
            unreliable: false,
            bytes,
        });
    }

    fn send_unreliable(&mut self, bytes: Vec<u8>) {
        let chance = if self.lost_last {
            self.conditions.burst.max(self.conditions.loss)
        } else {
            self.conditions.loss
        };
        self.lost_last = self.next_random() < chance;
        if self.lost_last {
            return;
        }
        let due = self.due(true);
        self.outgoing.push_back(Delayed {
            due,
            unreliable: true,
            bytes,
        });
    }

    fn receive(&mut self) -> Option<Vec<u8>> {
        let (due, _bytes) = self.incoming.front()?;
        if *due > self.clock {
            return None;
        }
        self.incoming.pop_front().map(|(_due, bytes)| bytes)
    }

    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    fn refusal(&self) -> Option<String> {
        self.inner.refusal()
    }

    fn peer_identity(&self) -> Option<PublicKey> {
        self.inner.peer_identity()
    }

    fn is_ready(&self) -> bool {
        self.inner.is_ready()
    }

    fn update(&mut self, elapsed: Duration) {
        self.clock += elapsed;
        self.step = elapsed;
        self.inner.update(elapsed);
        while let Some(bytes) = self.inner.receive() {
            let due = self.clock + self.conditions.receive_delay;
            self.incoming.push_back((due, bytes));
        }
    }

    fn flush(&mut self) {
        // To the nearest step, or every delay would round up by half of one.
        let clock = self.clock + self.step / 2;
        let mut index = 0;
        while index < self.outgoing.len() {
            if self.outgoing[index].due <= clock {
                if let Some(message) = self.outgoing.remove(index) {
                    if message.unreliable {
                        self.inner.send_unreliable(message.bytes);
                    } else {
                        self.inner.send(message.bytes);
                    }
                }
            } else {
                index += 1;
            }
        }
        self.inner.flush();
    }
}
