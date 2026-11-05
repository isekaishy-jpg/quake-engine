//! How a client gets a connect token from the host itself (proposal 0058).
//!
//! `netcode` encrypts everything, with keys from a token that a backend would
//! normally hand out. Direct play has no backend, so the host does it, over the
//! game's own port. Both sides agree a key, and the token goes across sealed
//! under it.
//!
//! An open game agrees it with plain X25519. A game with a password uses SPAKE2
//! instead, which is only there for that: someone without the password can't
//! open the token, and can't try passwords against a recording either, since
//! each guess needs a live exchange, and those are limited.
//!
//! Someone watching learns nothing either way, because the keys are new each
//! time.
//!
//! Each side also signs what was said, with its lasting key (proposal 0059).
//! Anyone in the middle has to change what's said to read anything, and then
//! the signatures don't match.
//!
//! A request is always bigger than its answer, so this can't be used to aim
//! more traffic at someone else than they could send themselves.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::io::Cursor;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use renet_netcode::{ConnectToken, generate_random_bytes};
use sha2::{Digest, Sha256};
use spake2::{Ed25519Group, Identity, Password, Spake2};
use x25519_dalek::{PublicKey as AgreementKey, StaticSecret};

use crate::identity::{
    HostIdentity, LINK_BYTES, Link, PlayerIdentity, PublicKey, SIGNATURE_BYTES, follows, host_code,
    links_from_wire, verify,
};

/// What a request starts with.
const REQUEST_MAGIC: &[u8; 8] = b"QEKEYREQ";

/// What an answer starts with.
const ANSWER_MAGIC: &[u8; 8] = b"QEKEYANS";

/// What a refusal starts with.
const REFUSAL_MAGIC: &[u8; 8] = b"QEKEYNOT";

/// A request or refusal's word for someone who's banned.
const REFUSED_BANNED: u8 = 0;

/// A refusal's word for an open request to a game with a password.
const REFUSED_NEEDS_PASSWORD: u8 = 1;

/// A refusal's word for a password given to an open game. A client doesn't fall
/// back to an open exchange, so nobody can talk it out of a password.
const REFUSED_NO_PASSWORD: u8 = 2;

/// A request's word for an open exchange.
const KIND_OPEN: u8 = 0;

/// A request's word for an exchange with a password.
const KIND_PASSWORD: u8 = 1;

/// What the key of an open exchange is made from, besides what's agreed.
const OPEN_CONTEXT: &[u8] = b"quake-engine open exchange";

/// What a player's signature starts with.
const PLAYER_CONTEXT: &[u8] = b"quake-engine player handshake";

/// What a host's signature starts with.
const HOST_CONTEXT: &[u8] = b"quake-engine host handshake";

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

/// What a host signs: everything said on both sides.
fn host_message(
    asked: &[u8; MESSAGE_BYTES],
    answer: &[u8; MESSAGE_BYTES],
    player: &PublicKey,
) -> Vec<u8> {
    [
        HOST_CONTEXT,
        asked.as_slice(),
        answer.as_slice(),
        player.as_slice(),
    ]
    .concat()
}

/// What a player signs.
fn player_message(asked: &[u8; MESSAGE_BYTES]) -> Vec<u8> {
    [PLAYER_CONTEXT, asked.as_slice()].concat()
}

/// A key and a message for an open exchange, from a side's `secret`, what the
/// other said, and everything said. Nothing agreed with a weak key counts.
fn open_key(
    secret: &StaticSecret,
    theirs: &[u8; MESSAGE_BYTES],
    asked: &[u8; MESSAGE_BYTES],
    answer: &[u8; MESSAGE_BYTES],
) -> Option<Vec<u8>> {
    let mut key = [0; 32];
    key.copy_from_slice(&theirs[..32]);
    let shared = secret.diffie_hellman(&AgreementKey::from(key));
    shared.was_contributory().then(|| {
        Sha256::new()
            .chain_update(OPEN_CONTEXT)
            .chain_update(shared.as_bytes())
            .chain_update(asked)
            .chain_update(answer)
            .finalize()
            .to_vec()
    })
}

/// An open exchange's message for `secret`. It's shorter than SPAKE2's.
fn open_message(secret: &StaticSecret) -> [u8; MESSAGE_BYTES] {
    let mut message = [0; MESSAGE_BYTES];
    message[..32].copy_from_slice(AgreementKey::from(secret).as_bytes());
    message
}

/// One side of agreeing a key.
enum Agreement {
    /// An open game's.
    Open(StaticSecret),
    /// A game with a password's.
    Password(Spake2<Ed25519Group>),
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
    /// Who this host is.
    pub(crate) identity: HostIdentity,
    /// Players who aren't let in.
    pub(crate) banned: BTreeSet<PublicKey>,
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
    /// A doorman for a game with `password`, run by `identity`.
    pub(crate) fn new(password: &str, identity: HostIdentity, banned: BTreeSet<PublicKey>) -> Self {
        Self {
            password: password.to_owned(),
            identity,
            banned,
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

    /// Agrees a key with what was `asked`, and says what to answer.
    fn agree(&self, asked: &[u8; MESSAGE_BYTES]) -> Option<(Vec<u8>, [u8; MESSAGE_BYTES])> {
        if self.password.is_empty() {
            let secret = StaticSecret::from(generate_random_bytes::<32>());
            let answer = open_message(&secret);
            return Some((open_key(&secret, asked, asked, &answer)?, answer));
        }
        let (state, answer) = Spake2::<Ed25519Group>::start_b(
            &Password::new(self.password.as_bytes()),
            &Identity::new(CLIENT_NAME),
            &Identity::new(HOST_NAME),
        );
        let key = state.finish(asked).ok()?;
        Some((key, answer.try_into().ok()?))
    }

    /// Starts an exchange with what `player` asked, sealing the token that
    /// `mint` makes along with who this host is.
    fn start(
        &self,
        asked: [u8; MESSAGE_BYTES],
        player: PublicKey,
        now: Duration,
        mint: impl FnOnce(PublicKey) -> Option<Vec<u8>>,
    ) -> Option<Exchange> {
        let (key, answer) = self.agree(&asked)?;
        let mut sealing = self.identity.public().to_vec();
        sealing.extend_from_slice(&self.identity.sign(&host_message(&asked, &answer, &player)));
        sealing.push(u8::try_from(self.identity.link_count()).ok()?);
        sealing.extend_from_slice(&self.identity.links_to_wire());
        sealing.extend_from_slice(&mint(player)?);
        let payload = Payload {
            msg: &sealing,
            aad: REQUEST_MAGIC,
        };
        let sealed = cipher(&key)?
            .encrypt(Nonce::from_slice(&[0; 12]), payload)
            .ok()?;
        Some(Exchange {
            asked,
            answer,
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
        mint: impl FnOnce(PublicKey) -> Option<Vec<u8>>,
    ) -> Option<Vec<u8>> {
        if request.len() != REQUEST_BYTES || !is_request(request) {
            return None;
        }
        self.forget_old(now);
        let part = *request.get(REQUEST_MAGIC.len())?;
        let kind = *request.get(REQUEST_MAGIC.len() + 1)?;
        let first = REQUEST_MAGIC.len() + 2;
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
        let keys = first + MESSAGE_BYTES;
        let player: PublicKey = request.get(keys..keys + 32)?.try_into().ok()?;
        let signature = request.get(keys + 32..keys + 32 + SIGNATURE_BYTES)?;
        if !verify(&player, &player_message(&asked), signature) {
            return None;
        }
        let refusal = if self.banned.contains(&player) {
            Some(REFUSED_BANNED)
        } else if kind == KIND_OPEN && !self.password.is_empty() {
            Some(REFUSED_NEEDS_PASSWORD)
        } else if kind == KIND_PASSWORD && self.password.is_empty() {
            Some(REFUSED_NO_PASSWORD)
        } else {
            None
        };
        if let Some(reason) = refusal {
            return Some([REFUSAL_MAGIC.as_slice(), &asked[..ECHO_BYTES], &[reason]].concat());
        }
        let exchange = self.start(asked, player, now, mint)?;
        let answer = exchange.part(part);
        let _replaced = self.exchanges.insert(from, exchange);
        answer
    }
}

/// What's come of an answer.
pub(crate) enum Heard {
    /// Nothing yet.
    Nothing,
    /// The token, from the host with this key.
    Token(Box<ConnectToken>, PublicKey),
    /// Turned down, and why.
    Refused(String),
}

/// What a client expects of a host, and who it is.
pub(crate) struct Expected {
    /// The password.
    pub(crate) password: String,
    /// Who this player is.
    pub(crate) identity: PlayerIdentity,
    /// The key this host had last time.
    pub(crate) known_host: Option<PublicKey>,
    /// The code the host gave out.
    pub(crate) code: Option<String>,
}

/// The client's side.
pub(crate) struct Asker {
    /// The exchange, until it's finished.
    state: Option<Agreement>,
    /// Which kind it is.
    kind: u8,
    /// What this side said.
    asked: [u8; MESSAGE_BYTES],
    /// What the host said.
    answer: Option<[u8; MESSAGE_BYTES]>,
    /// The parts of the sealed token.
    parts: Vec<Option<Vec<u8>>>,
    /// What's expected of the host.
    expected: Expected,
}

impl Asker {
    /// Starts asking.
    pub(crate) fn new(expected: Expected) -> Self {
        let (state, kind, asked) = if expected.password.is_empty() {
            let secret = StaticSecret::from(generate_random_bytes::<32>());
            let message = open_message(&secret);
            (Agreement::Open(secret), KIND_OPEN, message)
        } else {
            let (state, asked) = Spake2::<Ed25519Group>::start_a(
                &Password::new(expected.password.as_bytes()),
                &Identity::new(CLIENT_NAME),
                &Identity::new(HOST_NAME),
            );
            let mut message = [0; MESSAGE_BYTES];
            message.copy_from_slice(&asked);
            (Agreement::Password(state), KIND_PASSWORD, message)
        };
        Self {
            state: Some(state),
            kind,
            asked,
            answer: None,
            parts: Vec::new(),
            expected,
        }
    }

    /// Why `host` isn't who it should be, given its `links`, if it isn't.
    fn doubt(&self, host: &PublicKey, links: &[Link]) -> Option<String> {
        if let Some(code) = &self.expected.code {
            // A code from before a change of key is still good.
            let keys = links.iter().map(|link| &link.old).chain([host]);
            let vouched = keys
                .filter(|key| host_code(key) == *code)
                .any(|key| follows(key, links, host));
            return (!vouched).then(|| "that isn't the host this code belongs to".to_owned());
        }
        let known = self.expected.known_host?;
        (!follows(&known, links, host)).then(|| {
            "this host isn't who it was last time: join with its code to be sure".to_owned()
        })
    }

    /// A request for `part`.
    fn request(&self, part: u8) -> Vec<u8> {
        let mut out = Vec::with_capacity(REQUEST_BYTES);
        out.extend_from_slice(REQUEST_MAGIC);
        out.push(part);
        out.push(self.kind);
        out.extend_from_slice(&self.asked);
        out.extend_from_slice(&self.expected.identity.public());
        out.extend_from_slice(&self.expected.identity.sign(&player_message(&self.asked)));
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
        let kind = self.kind;
        let wrong = || {
            let reason = if kind == KIND_OPEN {
                "the host's answer was spoiled"
            } else {
                "wrong password"
            };
            Heard::Refused(reason.to_owned())
        };
        let (Some(state), Some(answer)) = (self.state.take(), self.answer) else {
            return wrong();
        };
        let sealed: Vec<u8> = self.parts.iter().flatten().flatten().copied().collect();
        let key = match state {
            Agreement::Open(secret) => open_key(&secret, &answer, &self.asked, &answer),
            Agreement::Password(state) => state.finish(&answer).ok(),
        };
        let Some(key) = key else {
            return wrong();
        };
        let payload = Payload {
            msg: &sealed,
            aad: REQUEST_MAGIC,
        };
        let opened = cipher(&key)
            .and_then(|cipher| cipher.decrypt(Nonce::from_slice(&[0; 12]), payload).ok());
        let Some(opened) = opened else {
            return wrong();
        };
        self.check(&opened, &answer)
            .unwrap_or_else(|| Heard::Refused("the host's answer made no sense".to_owned()))
    }

    /// Checks who's sent `opened`, in answer to `answer`.
    fn check(&self, opened: &[u8], answer: &[u8; MESSAGE_BYTES]) -> Option<Heard> {
        let host: PublicKey = opened.get(..32)?.try_into().ok()?;
        let signature = opened.get(32..32 + SIGNATURE_BYTES)?;
        let links_at = 32 + SIGNATURE_BYTES + 1;
        let count = usize::from(*opened.get(links_at - 1)?);
        let token_at = links_at + count * LINK_BYTES;
        let links = links_from_wire(opened.get(links_at..token_at)?, count)?;
        let player = self.expected.identity.public();
        if !verify(
            &host,
            &host_message(&self.asked, answer, &player),
            signature,
        ) {
            return Some(Heard::Refused("the host's answer isn't its own".to_owned()));
        }
        if let Some(doubt) = self.doubt(&host, &links) {
            return Some(Heard::Refused(doubt));
        }
        let token = ConnectToken::read(&mut Cursor::new(opened.get(token_at..)?)).ok()?;
        Some(Heard::Token(Box::new(token), host))
    }

    /// Takes in a `packet` from the host.
    pub(crate) fn hear(&mut self, packet: &[u8]) -> Heard {
        let refusal = [REFUSAL_MAGIC.as_slice(), &self.asked[..ECHO_BYTES]].concat();
        if let Some([reason]) = packet.strip_prefix(refusal.as_slice()) {
            let reason = match *reason {
                REFUSED_BANNED => "you're banned from this server",
                REFUSED_NEEDS_PASSWORD => "this game needs a password",
                REFUSED_NO_PASSWORD => "this game has no password: clear yours to join",
                _ => "refused",
            };
            return Heard::Refused(reason.to_owned());
        }
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
