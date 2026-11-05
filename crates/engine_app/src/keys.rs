//! The keys and lists kept between games (proposal 0059).
//!
//! The two private keys never leave these files, and a file that can't be read
//! is moved aside rather than written over, so an identity isn't lost to a
//! passing fault.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use engine_net::{HostIdentity, PlayerIdentity, PublicKey};

/// The player's private key.
const PLAYER_FILE: &str = "player.key";

/// The host's private key, and what it knows of players.
const HOST_FILE: &str = "host.key";

/// Hosts met before, as an address and a key on each line.
const KNOWN_HOSTS_FILE: &str = "known_hosts.txt";

/// Banned players, as a key on each line.
const BANS_FILE: &str = "banned_players.txt";

/// `key` in hexadecimal.
pub(crate) fn to_hex(key: &PublicKey) -> String {
    let mut text = String::with_capacity(64);
    for byte in key {
        let _written = write!(text, "{byte:02x}");
    }
    text
}

/// A key from hexadecimal.
fn from_hex(text: &str) -> Option<PublicKey> {
    let mut key = [0; 32];
    if text.len() != 64 || !text.is_ascii() {
        return None;
    }
    for (index, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(key)
}

/// The start of `key`, which is enough to tell players apart.
pub(crate) fn short_id(key: &PublicKey) -> String {
    to_hex(key)[..8].to_owned()
}

/// Writes `bytes` whole or not at all, for only this user to read.
fn write_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let waiting = path.with_extension("new");
    fs::write(&waiting, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&waiting, fs::Permissions::from_mode(0o600))?;
    }
    fs::rename(&waiting, path)
}

/// Moves a file that can't be read out of the way.
fn set_aside(path: &Path) {
    let _moved = fs::rename(path, path.with_extension("bad"));
}

/// Who this player and host are, and who they know.
pub(crate) struct Keys {
    /// Where it's all kept.
    directory: PathBuf,
    /// Who this player is.
    pub(crate) player: PlayerIdentity,
    /// Who this host is.
    pub(crate) host: HostIdentity,
    /// The key of each host met, by address.
    known_hosts: BTreeMap<String, PublicKey>,
    /// Players not let in.
    pub(crate) banned: BTreeSet<PublicKey>,
}

impl Keys {
    /// What's in `directory`, making anything that isn't there yet.
    pub(crate) fn load(directory: &Path) -> Self {
        let mut keys = Self {
            directory: directory.to_path_buf(),
            player: PlayerIdentity::generate(),
            host: HostIdentity::generate(),
            known_hosts: BTreeMap::new(),
            banned: BTreeSet::new(),
        };
        keys.load_player();
        keys.load_host();
        let text = |name: &str| fs::read_to_string(directory.join(name)).unwrap_or_default();
        keys.known_hosts = text(KNOWN_HOSTS_FILE)
            .lines()
            .filter_map(|line| {
                let (address, key) = line.split_once(' ')?;
                Some((address.to_owned(), from_hex(key)?))
            })
            .collect();
        keys.banned = text(BANS_FILE).lines().filter_map(from_hex).collect();
        keys
    }

    /// Reads the player's key, or keeps the new one.
    fn load_player(&mut self) {
        let path = self.directory.join(PLAYER_FILE);
        match fs::read(&path).map(|bytes| <[u8; 32]>::try_from(bytes.as_slice())) {
            Ok(Ok(secret)) => self.player = PlayerIdentity::from_secret(&secret),
            Ok(Err(_)) => {
                set_aside(&path);
                let _kept = write_file(&path, &self.player.secret());
            }
            Err(_) => {
                let _kept = write_file(&path, &self.player.secret());
            }
        }
    }

    /// Reads the host's key, or keeps the new one.
    fn load_host(&mut self) {
        let path = self.directory.join(HOST_FILE);
        match fs::read(&path).map(|bytes| HostIdentity::decode(&bytes)) {
            Ok(Some(host)) => self.host = host,
            Ok(None) => {
                set_aside(&path);
                self.save_host();
            }
            Err(_) => self.save_host(),
        }
    }

    /// Keeps the host's identity.
    pub(crate) fn save_host(&self) {
        let _kept = write_file(&self.directory.join(HOST_FILE), &self.host.encode());
    }

    /// The key the host at `address` had last time.
    pub(crate) fn known_host(&self, address: &str) -> Option<PublicKey> {
        self.known_hosts.get(address).copied()
    }

    /// Remembers who's at `address`.
    pub(crate) fn remember_host(&mut self, address: String, key: PublicKey) {
        if self.known_hosts.insert(address, key) != Some(key) {
            self.save_known_hosts();
        }
    }

    /// Forgets who's at `address`, and says if anyone was.
    pub(crate) fn forget_host(&mut self, address: &str) -> bool {
        let known = self.known_hosts.remove(address).is_some();
        self.save_known_hosts();
        known
    }

    /// Writes the hosts down.
    fn save_known_hosts(&self) {
        let mut text = String::new();
        for (address, key) in &self.known_hosts {
            let _written = writeln!(text, "{address} {}", to_hex(key));
        }
        let _kept = write_file(&self.directory.join(KNOWN_HOSTS_FILE), text.as_bytes());
    }

    /// Writes the bans down.
    pub(crate) fn save_bans(&self) {
        let mut text = String::new();
        for key in &self.banned {
            let _written = writeln!(text, "{}", to_hex(key));
        }
        let _kept = write_file(&self.directory.join(BANS_FILE), text.as_bytes());
    }
}
