//! Moving bytes between client and server.

use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::Duration;

/// Something that carries messages to the other side.
pub trait Transport {
    /// Sends one message that has to arrive, in order. A closed connection
    /// loses it.
    fn send(&mut self, bytes: Vec<u8>);

    /// Sends one message that may be lost, or arrive out of order.
    fn send_unreliable(&mut self, bytes: Vec<u8>) {
        self.send(bytes);
    }

    /// The next message, if one has arrived.
    fn receive(&mut self) -> Option<Vec<u8>>;

    /// Whether the other end is still there.
    fn is_connected(&self) -> bool {
        true
    }

    /// Why the other end turned this connection down, if it did.
    fn refusal(&self) -> Option<String> {
        None
    }

    /// Whether messages can flow yet, since a connection takes time to open.
    fn is_ready(&self) -> bool {
        true
    }

    /// Hears from the network, `elapsed` since last time.
    fn update(&mut self, _elapsed: Duration) {}

    /// Sends what's waiting.
    fn flush(&mut self) {}
}

/// One end of a connection inside this process.
#[derive(Debug)]
pub struct Loopback {
    /// To the other end.
    outgoing: Sender<Vec<u8>>,
    /// From the other end.
    incoming: Receiver<Vec<u8>>,
    /// Whether the other end has gone.
    closed: bool,
}

/// A connected pair of ends.
#[must_use]
pub fn loopback() -> (Loopback, Loopback) {
    let (to_second, from_first) = mpsc::channel();
    let (to_first, from_second) = mpsc::channel();
    (
        Loopback {
            outgoing: to_second,
            incoming: from_second,
            closed: false,
        },
        Loopback {
            outgoing: to_first,
            incoming: from_first,
            closed: false,
        },
    )
}

impl Transport for Loopback {
    fn send(&mut self, bytes: Vec<u8>) {
        self.closed |= self.outgoing.send(bytes).is_err();
    }

    fn receive(&mut self) -> Option<Vec<u8>> {
        match self.incoming.try_recv() {
            Ok(bytes) => Some(bytes),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.closed = true;
                None
            }
        }
    }

    fn is_connected(&self) -> bool {
        !self.closed
    }
}
