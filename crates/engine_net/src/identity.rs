//! Who players and hosts are, from one connection to the next (proposal 0059).
//!
//! Each has a key pair of their own. The private half never leaves their
//! machine: they prove who they are by signing each connection's handshake, so
//! there's nothing on the wire worth stealing. A client remembers a host's
//! public key, and a host can tell who's come back.
//!
//! A host can change its key. It keeps giving out a link from the old key to
//! the new, signed by the old, until every player it knows of has had it.

use std::collections::BTreeSet;

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use renet_netcode::generate_random_bytes;

/// The public half of a key pair.
pub type PublicKey = [u8; 32];

/// How long a signature is.
pub(crate) const SIGNATURE_BYTES: usize = 64;

/// How long a link is, once encoded.
pub(crate) const LINK_BYTES: usize = 32 + 32 + SIGNATURE_BYTES;

/// How many links a host keeps at most. Anyone further behind has to be given
/// the host's code again.
const MAX_LINKS: usize = 8;

/// What's signed for a link, so that no other signature can pass for one.
const LINK_CONTEXT: &[u8] = b"quake-engine new host key";

/// The start of an encoded host identity.
const HOST_FILE_MAGIC: &[u8; 8] = b"QEHOSTK1";

/// The letters of a host's code, leaving out those that look like others.
const CODE_LETTERS: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// How many bytes of a key a code shows: enough that nobody could make a key
/// to match, and few enough to read out.
const CODE_BYTES: usize = 10;

/// Whether `signature` of `message` is `key`'s.
pub(crate) fn verify(key: &PublicKey, message: &[u8], signature: &[u8]) -> bool {
    let (Ok(key), Ok(signature)) = (
        VerifyingKey::from_bytes(key),
        Signature::from_slice(signature),
    ) else {
        return false;
    };
    key.verify(message, &signature).is_ok()
}

/// The code people can read out to be sure of the host behind `key`.
#[must_use]
pub fn host_code(key: &PublicKey) -> String {
    let mut bits = 0_u32;
    let mut held = 0;
    let mut code = String::new();
    for byte in &key[..CODE_BYTES] {
        bits = (bits << 8) | u32::from(*byte);
        held += 8;
        while held >= 5 {
            held -= 5;
            if !code.is_empty() && code.len() % 5 == 4 {
                code.push('-');
            }
            code.push(char::from(CODE_LETTERS[((bits >> held) & 31) as usize]));
        }
    }
    code
}

/// `code` as it would be written, whatever case or spacing it was typed in.
#[must_use]
pub fn tidy_code(code: &str) -> String {
    let letters: Vec<char> = code
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|letter| letter.to_ascii_uppercase())
        .collect();
    let groups: Vec<String> = letters
        .chunks(4)
        .map(|group| group.iter().collect())
        .collect();
    groups.join("-")
}

/// A player's lasting identity.
#[derive(Clone)]
pub struct PlayerIdentity {
    /// The key pair.
    key: SigningKey,
}

impl PlayerIdentity {
    /// A new identity.
    #[must_use]
    pub fn generate() -> Self {
        Self::from_secret(&generate_random_bytes())
    }

    /// The identity kept as `secret`.
    #[must_use]
    pub fn from_secret(secret: &[u8; 32]) -> Self {
        Self {
            key: SigningKey::from_bytes(secret),
        }
    }

    /// What to keep, and never give to anyone.
    #[must_use]
    pub fn secret(&self) -> [u8; 32] {
        self.key.to_bytes()
    }

    /// What others know them by.
    #[must_use]
    pub fn public(&self) -> PublicKey {
        self.key.verifying_key().to_bytes()
    }

    /// Signs `message`.
    pub(crate) fn sign(&self, message: &[u8]) -> [u8; SIGNATURE_BYTES] {
        self.key.sign(message).to_bytes()
    }
}

/// A host's word that its key has changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Link {
    /// The key it was.
    pub(crate) old: PublicKey,
    /// The key it became.
    pub(crate) new: PublicKey,
    /// What the old key says about that.
    signature: [u8; SIGNATURE_BYTES],
    /// Players who knew the old key and haven't been told yet.
    waiting: BTreeSet<PublicKey>,
}

impl Link {
    /// Whether the old key really did say this.
    fn is_sound(&self) -> bool {
        verify(&self.old, &link_message(&self.new), &self.signature)
    }

    /// What goes across to clients.
    fn to_wire(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(LINK_BYTES);
        out.extend_from_slice(&self.old);
        out.extend_from_slice(&self.new);
        out.extend_from_slice(&self.signature);
        out
    }

    /// A link as `bytes` have it, with nobody waiting.
    fn from_wire(bytes: &[u8]) -> Option<Self> {
        Some(Self {
            old: bytes.get(..32)?.try_into().ok()?,
            new: bytes.get(32..64)?.try_into().ok()?,
            signature: bytes.get(64..LINK_BYTES)?.try_into().ok()?,
            waiting: BTreeSet::new(),
        })
    }
}

/// What's signed to say the key is now `new`.
fn link_message(new: &PublicKey) -> Vec<u8> {
    [LINK_CONTEXT, new.as_slice()].concat()
}

/// Whether a player who trusts `from` can trust `to`, through `links` signed
/// one after the other. Links that don't check out don't count.
pub(crate) fn follows(from: &PublicKey, links: &[Link], to: &PublicKey) -> bool {
    let mut trusted = *from;
    for link in links {
        if link.old == trusted && link.is_sound() {
            trusted = link.new;
        }
    }
    trusted == *to
}

/// Reads the links in `bytes`, which are `count` long.
pub(crate) fn links_from_wire(bytes: &[u8], count: usize) -> Option<Vec<Link>> {
    let links: Option<Vec<Link>> = bytes
        .as_chunks::<LINK_BYTES>()
        .0
        .iter()
        .take(count)
        .map(|link| Link::from_wire(link))
        .collect();
    links.filter(|links| links.len() == count)
}

/// A host's lasting identity.
#[derive(Clone)]
pub struct HostIdentity {
    /// The key pair now.
    key: SigningKey,
    /// How it got here, oldest first, for as long as anyone needs telling.
    links: Vec<Link>,
    /// Every player who's joined.
    players: BTreeSet<PublicKey>,
}

impl HostIdentity {
    /// A new identity.
    #[must_use]
    pub fn generate() -> Self {
        Self {
            key: SigningKey::from_bytes(&generate_random_bytes()),
            links: Vec::new(),
            players: BTreeSet::new(),
        }
    }

    /// What players know it by.
    #[must_use]
    pub fn public(&self) -> PublicKey {
        self.key.verifying_key().to_bytes()
    }

    /// The code to give out with an address.
    #[must_use]
    pub fn code(&self) -> String {
        host_code(&self.public())
    }

    /// Signs `message`.
    pub(crate) fn sign(&self, message: &[u8]) -> [u8; SIGNATURE_BYTES] {
        self.key.sign(message).to_bytes()
    }

    /// Changes to a new key. Everyone known has to be told, and until they
    /// have been, each is given a link from the old key.
    ///
    /// This is for keeping keys fresh. A key that's been stolen can't be saved
    /// this way, since the thief can make links too: that takes a new identity
    /// and a new code.
    pub fn rotate(&mut self) {
        let new = SigningKey::from_bytes(&generate_random_bytes());
        let new_public = new.verifying_key().to_bytes();
        let link = Link {
            old: self.public(),
            new: new_public,
            signature: self.sign(&link_message(&new_public)),
            waiting: self.players.clone(),
        };
        self.key = new;
        if !link.waiting.is_empty() {
            self.links.push(link);
        }
        if self.links.len() > MAX_LINKS {
            let _oldest = self.links.remove(0);
        }
    }

    /// Notes that `player` has joined, having been given every link. Links
    /// nobody's waiting for any more are dropped. Says if anything changed.
    pub fn note_player(&mut self, player: PublicKey) -> bool {
        let mut changed = self.players.insert(player);
        for link in &mut self.links {
            changed |= link.waiting.remove(&player);
        }
        self.links.retain(|link| !link.waiting.is_empty());
        changed
    }

    /// How many links are still being given out.
    #[must_use]
    pub fn link_count(&self) -> usize {
        self.links.len()
    }

    /// The links, for a client.
    pub(crate) fn links_to_wire(&self) -> Vec<u8> {
        self.links.iter().flat_map(Link::to_wire).collect()
    }

    /// The whole identity as bytes, to keep. This holds the private key.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = HOST_FILE_MAGIC.to_vec();
        out.extend_from_slice(&self.key.to_bytes());
        put_keys(&mut out, &self.players);
        put_count(&mut out, self.links.len());
        for link in &self.links {
            out.extend_from_slice(&link.to_wire());
            put_keys(&mut out, &link.waiting);
        }
        out
    }

    /// An identity from what [`HostIdentity::encode`] made.
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let mut rest = bytes.strip_prefix(HOST_FILE_MAGIC.as_slice())?;
        let secret: [u8; 32] = take(&mut rest, 32)?.try_into().ok()?;
        let players = take_keys(&mut rest)?;
        let count = take_count(&mut rest)?;
        let mut links = Vec::new();
        for _ in 0..count {
            let mut link = Link::from_wire(take(&mut rest, LINK_BYTES)?)?;
            link.waiting = take_keys(&mut rest)?;
            links.push(link);
        }
        rest.is_empty().then(|| Self {
            key: SigningKey::from_bytes(&secret),
            links,
            players,
        })
    }
}

/// Adds a count.
fn put_count(out: &mut Vec<u8>, count: usize) {
    out.extend_from_slice(&u32::try_from(count).unwrap_or(u32::MAX).to_le_bytes());
}

/// Adds a set of keys.
fn put_keys(out: &mut Vec<u8>, keys: &BTreeSet<PublicKey>) {
    put_count(out, keys.len());
    for key in keys {
        out.extend_from_slice(key);
    }
}

/// Takes `length` bytes off the front.
fn take<'a>(bytes: &mut &'a [u8], length: usize) -> Option<&'a [u8]> {
    let (front, rest) = bytes.split_at_checked(length)?;
    *bytes = rest;
    Some(front)
}

/// Takes a count.
fn take_count(bytes: &mut &[u8]) -> Option<usize> {
    let count: [u8; 4] = take(bytes, 4)?.try_into().ok()?;
    usize::try_from(u32::from_le_bytes(count)).ok()
}

/// Takes a set of keys.
fn take_keys(bytes: &mut &[u8]) -> Option<BTreeSet<PublicKey>> {
    let count = take_count(bytes)?;
    let mut keys = BTreeSet::new();
    for _ in 0..count {
        let _new = keys.insert(take(bytes, 32)?.try_into().ok()?);
    }
    Some(keys)
}
