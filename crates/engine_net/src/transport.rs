//! Moving bytes between client and server.

use std::sync::mpsc::{self, Receiver, Sender};

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
}

/// One end of a connection inside this process.
#[derive(Debug)]
pub struct Loopback {
    /// To the other end.
    outgoing: Sender<Vec<u8>>,
    /// From the other end.
    incoming: Receiver<Vec<u8>>,
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
        },
        Loopback {
            outgoing: to_first,
            incoming: from_first,
        },
    )
}

impl Transport for Loopback {
    fn send(&mut self, bytes: Vec<u8>) {
        let _closed = self.outgoing.send(bytes);
    }

    fn receive(&mut self) -> Option<Vec<u8>> {
        self.incoming.try_recv().ok()
    }
}
