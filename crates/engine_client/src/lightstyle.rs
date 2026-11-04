//! Animated light brightness, from `R_AnimateLight`.

/// How many styles there are.
pub const LIGHT_STYLE_COUNT: usize = 64;

/// Letters a second.
const STEPS_PER_SECOND: f64 = 10.0;

/// The letter for normal brightness.
const NORMAL: u8 = b'm';

/// Each light style's pattern of letters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LightStyles {
    /// The patterns.
    patterns: Vec<Vec<u8>>,
}

impl Default for LightStyles {
    fn default() -> Self {
        Self {
            patterns: vec![Vec::new(); LIGHT_STYLE_COUNT],
        }
    }
}

impl LightStyles {
    /// Every style at normal brightness.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets style `index`.
    pub fn set(&mut self, index: u8, pattern: &str) {
        if let Some(style) = self.patterns.get_mut(usize::from(index)) {
            *style = pattern.bytes().filter(u8::is_ascii_lowercase).collect();
        }
    }

    /// Every style's brightness at `time`, where 1 is normal.
    #[must_use]
    pub fn values(&self, time: f64) -> [f32; LIGHT_STYLE_COUNT] {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the step count is small and time is never negative"
        )]
        let step = (time.max(0.0) * STEPS_PER_SECOND) as usize;
        let mut values = [1.0; LIGHT_STYLE_COUNT];
        for (value, pattern) in values.iter_mut().zip(&self.patterns) {
            if !pattern.is_empty() {
                let letter = pattern[step % pattern.len()];
                *value = f32::from(letter - b'a') / f32::from(NORMAL - b'a');
            }
        }
        values
    }
}
