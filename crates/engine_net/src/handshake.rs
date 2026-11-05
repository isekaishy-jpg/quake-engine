//! How a client gets a connect token from the host itself (proposal 0058).
//!
//! `netcode` encrypts everything, with keys from a token that a backend would
//! normally hand out. Direct play has no backend, so the host does it, over the
//! game's own port. Both sides run SPAKE2 with the password, which is empty if
//! the game is open, and the token goes across sealed under what comes out.
//!
//! Someone watching learns nothing, because the keys are new each time. Someone
//! without the password can't open the token, and can't try passwords against
//! a recording either: each guess needs a live exchange, and those are limited.
//!
//! A request is always bigger than its answer, so this can't be used to aim
//! more traffic at someone else than they could send themselves.

use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use renet_netcode::ConnectToken;
use spake2::{Ed25519Group, Identity, Password, Spake2};

/// What a request starts with.
const REQUEST_MAGIC: &[u8; 8] = b"QEKEYREQ";

/// What an answer starts with.
const ANSWER_MAGIC: &[u8; 8] = b"QEKEYANS";

/// The length of a SPAKE2 message.
const MESSAGE_BYTES: usize = 33;

/// How much of the client's message an answer repeats, so that nobody can
/// answer in the host's place without having seen the request.
const ECHO_BYTES: usize = 8;

/// The length of every request. It's more than any answer, and small enough to
/// pass through any network whole.
pub(crate) const REQUEST_BYTES: usize = 1200;

/// How much of the sealed token goes in one answer.
const PART_BYTES: usize = 1100;

/// How long a host remembers an exchange, so that lost answers can be sent
/// again.
const EXCHANGE_LIFE: Duration = Duration::from_secs(10);

/// How long one address waits between exchanges, which limits guessing.
const ADDRESS_WAIT: Duration = Duration::from_secs(1);

/// How many exchanges can start in a second, from everyone.
const MAX_STARTS_A_SECOND: usize = 16;

/// How many exchanges are remembered at once.
const MAX_EXCHANGES: usize = 64;

/// What the client calls itself in the exchange.
const CLIENT_NAME: &[u8] = b"quake-engine client";

/// What the host calls itself in the exchange.
const HOST_NAME: &[u8] = b"quake-engine host";

/// Seals or opens the token with the exchange's `key`. The key is only ever
/// used once, so the nonce can stay the same.
fn cipher(key: &[u8]) -> Option<ChaCha20Poly1305> {
    (key.len() == 32).then(|| ChaCha20Poly1305::new(Key::from_slice(key)))
}

/// One exchange a host has answered.
struct Exchange {
    /// What the client said.
    asked: [u8; MESSAGE_BYTES],
    /// What the host said.
    answer: [u8; MESSAGE_BYTES],
    /// The token, sealed.
    sealed: Vec<u8>,
    /// When it started.
    started: Duration,
}

impl Exchange {
    /// Answers with `part` of the token.
    fn part(&self, part: u8) -> Option<Vec<u8>> {
        let chunk = self.sealed.chunks(PART_BYTES).nth(usize::from(part))?;
        let count = self.sealed.len().div_ceil(PART_BYTES);
        let mut out = Vec::with_capacity(REQUEST_BYTES);
        out.extend_from_slice(ANSWER_MAGIC);
        out.extend_from_slice(&self.asked[..ECHO_BYTES]);
        out.push(part);
        out.push(u8::try_from(count).ok()?);
        out.extend_from_slice(&self.answer);
        out.extend_from_slice(chunk);
        Some(out)
    }
}

/// The host's side.
pub(crate) struct Doorman {
    /// The password, empty for an open game.
    password: String,
    /// Exchanges still remembered.
    exchanges: HashMap<SocketAddr, Exchange>,
    /// When recent exchanges started.
    starts: VecDeque<Duration>,
    /// When each address last started one.
    addresses: HashMap<IpAddr, Duration>,
}

/// Whether `packet` is a request, rather than something for `netcode`.
pub(crate) fn is_request(packet: &[u8]) -> bool {
    packet.starts_with(REQUEST_MAGIC)
}

impl Doorman {
    /// A doorman for a game with `password`.
    pub(crate) fn new(password: &str) -> Self {
        Self {
            password: password.to_owned(),
            exchanges: HashMap::new(),
            starts: VecDeque::new(),
            addresses: HashMap::new(),
        }
    }

    /// Forgets what's too old to matter at time `now`.
    fn forget_old(&mut self, now: Duration) {
        self.exchanges
            .retain(|_, exchange| now.saturating_sub(exchange.started) < EXCHANGE_LIFE);
        self.addresses
            .retain(|_, last| now.saturating_sub(*last) < ADDRESS_WAIT);
        while self
            .starts
            .front()
            .is_some_and(|start| now.saturating_sub(*start) >= Duration::from_secs(1))
        {
            let _old = self.starts.pop_front();
        }
    }

    /// Whether `from` can start an exchange now.
    fn can_start(&self, from: SocketAddr) -> bool {
        self.starts.len() < MAX_STARTS_A_SECOND
            && self.exchanges.len() < MAX_EXCHANGES
            && !self.addresses.contains_key(&from.ip())
    }

    /// Starts an exchange with what the client `asked`, sealing the token that
    /// `mint` makes.
    fn start(
        &self,
        asked: [u8; MESSAGE_BYTES],
        now: Duration,
        mint: impl FnOnce() -> Option<Vec<u8>>,
    ) -> Option<Exchange> {
        let (state, answer) = Spake2::<Ed25519Group>::start_b(
            &Password::new(self.password.as_bytes()),
            &Identity::new(CLIENT_NAME),
            &Identity::new(HOST_NAME),
        );
        let key = state.finish(&asked).ok()?;
        let token = mint()?;
        let payload = Payload {
            msg: &token,
            aad: REQUEST_MAGIC,
        };
        let sealed = cipher(&key)?
            .encrypt(Nonce::from_slice(&[0; 12]), payload)
            .ok()?;
        Some(Exchange {
            asked,
            answer: answer.try_into().ok()?,
            sealed,
            started: now,
        })
    }

    /// Answers `request` from `from` at time `now`, if it should be.
    pub(crate) fn answer(
        &mut self,
        request: &[u8],
        from: SocketAddr,
        now: Duration,
        mint: impl FnOnce() -> Option<Vec<u8>>,
    ) -> Option<Vec<u8>> {
        if request.len() != REQUEST_BYTES || !is_request(request) {
            return None;
        }
        self.forget_old(now);
        let part = *request.get(REQUEST_MAGIC.len())?;
        let first = REQUEST_MAGIC.len() + 1;
        let asked: [u8; MESSAGE_BYTES] =
            request.get(first..first + MESSAGE_BYTES)?.try_into().ok()?;
        if let Some(exchange) = self.exchanges.get(&from)
            && exchange.asked == asked
        {
            return exchange.part(part);
        }
        if !self.can_start(from) {
            return None;
        }
        // Anything that gets this far has cost a little work, and counts.
        self.starts.push_back(now);
        let _last = self.addresses.insert(from.ip(), now);
        let exchange = self.start(asked, now, mint)?;
        let answer = exchange.part(part);
        let _replaced = self.exchanges.insert(from, exchange);
        answer
    }
}

/// What's come of an answer.
pub(crate) enum Heard {
    /// Nothing yet.
    Nothing,
    /// The token.
    Token(Box<ConnectToken>),
    /// The token couldn't be opened, so the passwords differ.
    Refused,
}

/// The client's side.
pub(crate) struct Asker {
    /// The exchange, until it's finished.
    state: Option<Spake2<Ed25519Group>>,
    /// What this side said.
    asked: [u8; MESSAGE_BYTES],
    /// What the host said.
    answer: Option<[u8; MESSAGE_BYTES]>,
    /// The parts of the sealed token.
    parts: Vec<Option<Vec<u8>>>,
}

impl Asker {
    /// Starts asking, with `password`.
    pub(crate) fn new(password: &str) -> Self {
        let (state, asked) = Spake2::<Ed25519Group>::start_a(
            &Password::new(password.as_bytes()),
            &Identity::new(CLIENT_NAME),
            &Identity::new(HOST_NAME),
        );
        let mut message = [0; MESSAGE_BYTES];
        message.copy_from_slice(&asked);
        Self {
            state: Some(state),
            asked: message,
            answer: None,
            parts: Vec::new(),
        }
    }

    /// A request for `part`.
    fn request(&self, part: u8) -> Vec<u8> {
        let mut out = Vec::with_capacity(REQUEST_BYTES);
        out.extend_from_slice(REQUEST_MAGIC);
        out.push(part);
        out.extend_from_slice(&self.asked);
        out.resize(REQUEST_BYTES, 0);
        out
    }

    /// Requests for everything still missing.
    pub(crate) fn requests(&self) -> Vec<Vec<u8>> {
        if self.parts.is_empty() {
            return vec![self.request(0)];
        }
        (0u8..)
            .zip(&self.parts)
            .filter(|(_, part)| part.is_none())
            .map(|(index, _)| self.request(index))
            .collect()
    }

    /// Opens the token, with everything here.
    fn open(&mut self) -> Heard {
        let (Some(state), Some(answer)) = (self.state.take(), self.answer) else {
            return Heard::Refused;
        };
        let sealed: Vec<u8> = self.parts.iter().flatten().flatten().copied().collect();
        let Some(key) = state.finish(&answer).ok() else {
            return Heard::Refused;
        };
        let payload = Payload {
            msg: &sealed,
            aad: REQUEST_MAGIC,
        };
        let token = cipher(&key).and_then(|cipher| {
            let bytes = cipher.decrypt(Nonce::from_slice(&[0; 12]), payload).ok()?;
            ConnectToken::read(&mut Cursor::new(bytes)).ok()
        });
        token.map_or(Heard::Refused, |token| Heard::Token(Box::new(token)))
    }

    /// Takes in a `packet` from the host.
    pub(crate) fn hear(&mut self, packet: &[u8]) -> Heard {
        let header = ANSWER_MAGIC.len() + ECHO_BYTES + 2 + MESSAGE_BYTES;
        if packet.len() <= header
            || !packet.starts_with(ANSWER_MAGIC)
            || packet[ANSWER_MAGIC.len()..ANSWER_MAGIC.len() + ECHO_BYTES]
                != self.asked[..ECHO_BYTES]
        {
            return Heard::Nothing;
        }
        let part = usize::from(packet[ANSWER_MAGIC.len() + ECHO_BYTES]);
        let count = usize::from(packet[ANSWER_MAGIC.len() + ECHO_BYTES + 1]);
        let Ok(answer) = <[u8; MESSAGE_BYTES]>::try_from(&packet[header - MESSAGE_BYTES..header])
        else {
            return Heard::Nothing;
        };
        if part >= count || self.answer.is_some_and(|known| known != answer) {
            return Heard::Nothing;
        }
        self.answer = Some(answer);
        self.parts.resize(count, None);
        self.parts[part] = Some(packet[header..].to_vec());
        if self.parts.iter().all(Option::is_some) {
            return self.open();
        }
        Heard::Nothing
    }
}
