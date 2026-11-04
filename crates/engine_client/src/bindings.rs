//! Key bindings, following `keys.c`.

use std::collections::BTreeMap;

/// Names that differ between the platform and config files, as the name
/// bindings are stored under.
const KEY_NAMES: [(&str, &str); 16] = [
    ("UP", "UPARROW"),
    ("DOWN", "DOWNARROW"),
    ("LEFT", "LEFTARROW"),
    ("RIGHT", "RIGHTARROW"),
    ("LEFT CTRL", "CTRL"),
    ("RIGHT CTRL", "CTRL"),
    ("LEFT SHIFT", "SHIFT"),
    ("RIGHT SHIFT", "SHIFT"),
    ("LEFT ALT", "ALT"),
    ("RIGHT ALT", "ALT"),
    ("PAGE UP", "PGUP"),
    ("PAGE DOWN", "PGDN"),
    ("INSERT", "INS"),
    ("DELETE", "DEL"),
    (";", "SEMICOLON"),
    ("KEYPAD ENTER", "ENTER"),
];

/// The name a key is bound under, whichever name it arrived with.
#[must_use]
pub fn canonical_key(name: &str) -> String {
    let upper = name.to_ascii_uppercase();
    KEY_NAMES
        .iter()
        .find(|(from, _)| *from == upper)
        .map_or(upper, |(_, to)| (*to).to_owned())
}

/// What each key runs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bindings {
    /// Commands by canonical key name.
    keys: BTreeMap<String, String>,
}

impl Bindings {
    /// No bindings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds `key` to `command`.
    pub fn bind(&mut self, key: &str, command: &str) {
        let _previous = self.keys.insert(canonical_key(key), command.to_owned());
    }

    /// Removes the binding of `key`.
    pub fn unbind(&mut self, key: &str) {
        let _previous = self.keys.remove(&canonical_key(key));
    }

    /// Removes every binding.
    pub fn clear(&mut self) {
        self.keys.clear();
    }

    /// The command bound to `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.keys.get(&canonical_key(key)).map(String::as_str)
    }

    /// The command to run when `key` goes down.
    #[must_use]
    pub fn press(&self, key: &str) -> Option<&str> {
        self.get(key)
    }

    /// The command to run when `key` comes up: the matching `-` of a `+`
    /// binding, and nothing otherwise.
    #[must_use]
    pub fn release(&self, key: &str) -> Option<String> {
        let button = self.get(key)?.strip_prefix('+')?;
        Some(format!("-{button}"))
    }
}
