//! Console variables.

/// What a cvar is used for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CvarFlags {
    /// The value is written to the user's config.
    pub saved: bool,
}

/// A named variable, stored as a string as in Quake.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cvar {
    /// The current value.
    value: String,
    /// The value it was registered with.
    default: String,
    /// What it's used for.
    flags: CvarFlags,
}

impl Cvar {
    /// A cvar holding `default`.
    #[must_use]
    pub fn new(default: &str, flags: CvarFlags) -> Self {
        Self {
            value: default.to_owned(),
            default: default.to_owned(),
            flags,
        }
    }

    /// The current value.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// The current value as a number, or 0 if it isn't one, as `atof` gives.
    #[must_use]
    pub fn number(&self) -> f32 {
        self.value.trim().parse().unwrap_or(0.0)
    }

    /// The registered default.
    #[must_use]
    pub fn default_value(&self) -> &str {
        &self.default
    }

    /// What it's used for.
    #[must_use]
    pub fn flags(&self) -> CvarFlags {
        self.flags
    }

    /// Replaces the value.
    pub fn set(&mut self, value: &str) {
        value.clone_into(&mut self.value);
    }
}
