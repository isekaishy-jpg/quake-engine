//! A grid of baked light for moving things (proposal 0072).

use engine_core::math::Vec3;

/// The light style of bounced light, which never changes. Keeping it apart
/// from direct light lets moving things block one without the other.
pub const BOUNCE_STYLE: u8 = 63;

/// The light style whose samples are drawn crisp, by the edges beside them.
pub const CRISP_STYLE: u8 = 62;

/// Baked lights that move through some styles in turn, blended by time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightTrack {
    /// Its first style.
    pub first_style: u8,
    /// How many styles it has, one for each keyframe.
    pub count: u8,
    /// How many seconds a trip round takes, or 0 if a script drives it.
    pub period: f32,
}

impl LightTrack {
    /// Sets its styles for `time`: each keyframe is brightest at its own moment
    /// and fades into the next, so light seems to move between them.
    pub fn blend(&self, styles: &mut [f32], time: f32) {
        if self.period <= 0.0 || self.count == 0 {
            return;
        }
        let count = f32::from(self.count);
        let place = (time / self.period).rem_euclid(1.0) * count;
        for index in 0..self.count {
            let away = (f32::from(index) - place).abs();
            let around = away.min(count - away);
            if let Some(style) = styles.get_mut(usize::from(self.first_style + index)) {
                *style = (1.0 - around).max(0.0);
            }
        }
    }
}

/// Light at points through a map's space, from six sides each.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProbeGrid {
    /// The corner of the grid.
    pub origin: Vec3,
    /// World units between probes.
    pub spacing: f32,
    /// How many probes along each axis.
    pub size: [u32; 3],
    /// Each probe's colour seen from +x, -x, +y, -y, +z and -z.
    pub colors: Vec<[[u8; 3]; 6]>,
    /// Which way each probe's strongest light comes from, as bytes.
    pub directions: Vec<[u8; 3]>,
}

/// The light at a point.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ProbeSample {
    /// The brightness of its strongest side, from 0 to 255.
    pub brightness: f32,
    /// Which way the strongest light comes from.
    pub direction: Vec3,
}

impl ProbeGrid {
    /// The index of the probe at a grid position.
    fn index(&self, at: [u32; 3]) -> Option<usize> {
        let [x, y, z] = at;
        let [sx, sy, _] = self.size;
        usize::try_from((z * sy + y) * sx + x).ok()
    }

    /// The light at `point`, from the nearest eight probes.
    #[must_use]
    pub fn sample(&self, point: Vec3) -> Option<ProbeSample> {
        if self.spacing <= 0.0 || self.colors.is_empty() {
            return None;
        }
        let local = (point - self.origin) * (1.0 / self.spacing);
        let parts = [local.x, local.y, local.z];
        let mut low = [0_u32; 3];
        let mut share = [0.0_f32; 3];
        for axis in 0..3 {
            #[expect(clippy::cast_precision_loss, reason = "grid sizes are small")]
            let top = self.size[axis].saturating_sub(1) as f32;
            let clamped = parts[axis].clamp(0.0, top);
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "clamped into the grid"
            )]
            let cell = clamped.floor() as u32;
            low[axis] = cell.min(self.size[axis].saturating_sub(2));
            #[expect(clippy::cast_precision_loss, reason = "grid sizes are small")]
            let start = low[axis] as f32;
            share[axis] = clamped - start;
        }
        let mut total = 0.0;
        let mut direction = Vec3::ZERO;
        let mut weights = 0.0;
        for corner in 0..8_u32 {
            let bit = |axis: u32| (corner >> axis) & 1;
            let at = [
                (low[0] + bit(0)).min(self.size[0].saturating_sub(1)),
                (low[1] + bit(1)).min(self.size[1].saturating_sub(1)),
                (low[2] + bit(2)).min(self.size[2].saturating_sub(1)),
            ];
            let weight = |axis: usize, on: u32| {
                if on == 1 {
                    share[axis]
                } else {
                    1.0 - share[axis]
                }
            };
            let weight = weight(0, bit(0)) * weight(1, bit(1)) * weight(2, bit(2));
            let index = self.index(at)?;
            let colors = self.colors.get(index)?;
            // A probe in solid saw nothing, and shouldn't darken its neighbours.
            let strongest = colors
                .iter()
                .map(|side| (f32::from(side[0]) + f32::from(side[1]) + f32::from(side[2])) / 3.0)
                .fold(0.0, f32::max);
            if strongest <= 0.0 {
                continue;
            }
            let sum = strongest;
            total += sum * weight;
            weights += weight;
            if let Some(bytes) = self.directions.get(index) {
                let unit = |byte: u8| f32::from(byte) / 127.5 - 1.0;
                direction =
                    direction + Vec3::new(unit(bytes[0]), unit(bytes[1]), unit(bytes[2])) * weight;
            }
        }
        (weights > 0.0).then(|| ProbeSample {
            brightness: total / weights,
            direction: direction.normalized(),
        })
    }
}
