//! Mouse look.
//!
//! The view is a pure function of the ordered raw counts and the settings, so
//! frame rate and tick rate have no effect on it (proposal 0015).

/// The highest the view can look, in degrees. Negative is up, as in Quake.
const PITCH_UP_LIMIT: f64 = -70.0;

/// The lowest the view can look, in degrees.
const PITCH_DOWN_LIMIT: f64 = 80.0;

/// Degrees turned per raw mouse count.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LookScale {
    /// Degrees of yaw per horizontal count.
    pub yaw: f64,
    /// Degrees of pitch per vertical count.
    pub pitch: f64,
}

impl LookScale {
    /// The scale from Quake's `sensitivity`, `m_yaw` and `m_pitch`.
    #[must_use]
    pub fn from_settings(sensitivity: f64, m_yaw: f64, m_pitch: f64) -> Self {
        Self {
            yaw: sensitivity * m_yaw,
            pitch: sensitivity * m_pitch,
        }
    }
}

/// The view, kept as count totals from an origin.
///
/// Totals of whole counts are exact in `f64`, so moving out and back returns
/// to exactly the same angle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// Degrees per count.
    scale: LookScale,
    /// Yaw when `yaw_counts` was zero.
    yaw_origin: f64,
    /// Pitch when `pitch_counts` was zero.
    pitch_origin: f64,
    /// Horizontal counts since the origin.
    yaw_counts: f64,
    /// Vertical counts since the origin.
    pitch_counts: f64,
}

impl Look {
    /// A level view facing yaw 0.
    #[must_use]
    pub fn new(scale: LookScale) -> Self {
        Self {
            scale,
            yaw_origin: 0.0,
            pitch_origin: 0.0,
            yaw_counts: 0.0,
            pitch_counts: 0.0,
        }
    }

    /// Applies one raw motion event. Call this for each event as it arrives,
    /// because pitch is clamped per event, so that a batch of events can't
    /// give a different result.
    pub fn motion(&mut self, x: f64, y: f64) {
        self.yaw_counts += x;
        let counts = self.pitch_counts + y;
        let pitch = self.pitch_origin + counts * self.scale.pitch;
        self.pitch_counts = if pitch < PITCH_UP_LIMIT {
            (PITCH_UP_LIMIT - self.pitch_origin) / self.scale.pitch
        } else if pitch > PITCH_DOWN_LIMIT {
            (PITCH_DOWN_LIMIT - self.pitch_origin) / self.scale.pitch
        } else {
            counts
        };
    }

    /// Changes the scale. The view stays where it is.
    pub fn set_scale(&mut self, scale: LookScale) {
        if scale != self.scale {
            *self = Self {
                scale,
                yaw_origin: self.yaw(),
                pitch_origin: self.pitch(),
                yaw_counts: 0.0,
                pitch_counts: 0.0,
            };
        }
    }

    /// The yaw in degrees, from 0 up to, but not including, 360. Moving the
    /// mouse right turns right, which lowers yaw, as in Quake.
    #[must_use]
    pub fn yaw(&self) -> f64 {
        (self.yaw_origin - self.yaw_counts * self.scale.yaw).rem_euclid(360.0)
    }

    /// The pitch in degrees. Negative is up.
    #[must_use]
    pub fn pitch(&self) -> f64 {
        self.pitch_origin + self.pitch_counts * self.scale.pitch
    }
}
