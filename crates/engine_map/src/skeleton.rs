//! Skeletons, and the poses clips put them in (proposal 0140).
//!
//! The server and the client both work out poses, so this is shared: where a joint is decides
//! where something held to it is, and they have to agree.

use engine_core::math::Vec3;

/// The rows of a matrix that places things, whose last row is always 0, 0, 0, 1.
pub type Rows = [[f32; 4]; 3];

/// What leaves things where they are.
pub const NO_MOVE: Rows = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];

/// `a` after `b`.
#[must_use]
pub fn after(a: &Rows, b: &Rows) -> Rows {
    let mut out = [[0.0; 4]; 3];
    for (row, values) in out.iter_mut().enumerate() {
        for (column, value) in values.iter_mut().enumerate() {
            let mut sum = if column == 3 { a[row][3] } else { 0.0 };
            for (at, part) in a[row].iter().take(3).enumerate() {
                sum += part * b[at][column];
            }
            *value = sum;
        }
    }
    out
}

/// Where `point` goes under `rows`.
#[must_use]
pub fn moved(rows: &Rows, point: Vec3) -> Vec3 {
    let row = |row: [f32; 4]| row[0] * point.x + row[1] * point.y + row[2] * point.z + row[3];
    Vec3::new(row(rows[0]), row(rows[1]), row(rows[2]))
}

/// Which way `direction` points under `rows`, wherever it is.
#[must_use]
pub fn turned(rows: &Rows, direction: Vec3) -> Vec3 {
    let row = |row: [f32; 4]| row[0] * direction.x + row[1] * direction.y + row[2] * direction.z;
    Vec3::new(row(rows[0]), row(rows[1]), row(rows[2]))
}

/// What places something at `origin`, turned by `angles`: its pitch, yaw and roll in degrees.
#[must_use]
pub fn placed_at(origin: Vec3, angles: Vec3) -> Rows {
    let (sin_pitch, cos_pitch) = angles.x.to_radians().sin_cos();
    let (sin_yaw, cos_yaw) = angles.y.to_radians().sin_cos();
    let (sin_roll, cos_roll) = angles.z.to_radians().sin_cos();
    let forward = Vec3::new(cos_pitch * cos_yaw, cos_pitch * sin_yaw, -sin_pitch);
    let left = Vec3::new(
        sin_roll * sin_pitch * cos_yaw - cos_roll * sin_yaw,
        sin_roll * sin_pitch * sin_yaw + cos_roll * cos_yaw,
        sin_roll * cos_pitch,
    );
    let up = forward.cross(left);
    [
        [forward.x, left.x, up.x, origin.x],
        [forward.y, left.y, up.y, origin.y],
        [forward.z, left.z, up.z, origin.z],
    ]
}

/// The pitch, yaw and roll in degrees that `rows` turns by.
#[must_use]
pub fn angles_of_rows(rows: &Rows) -> Vec3 {
    let forward = turned(rows, Vec3::new(1.0, 0.0, 0.0)).normalized();
    let left = turned(rows, Vec3::new(0.0, 1.0, 0.0)).normalized();
    let up = forward.cross(left);
    let pitch = (-forward.z).clamp(-1.0, 1.0).asin();
    // Straight up or down, yaw and roll are the same turn, so it's all yaw.
    let (yaw, roll) = if forward.x.hypot(forward.y) > 1e-5 {
        (forward.y.atan2(forward.x), left.z.atan2(up.z))
    } else {
        ((-left.x).atan2(left.y), 0.0)
    };
    Vec3::new(pitch.to_degrees(), yaw.to_degrees(), roll.to_degrees())
}

/// Where something is, relative to what it's on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JointPose {
    /// How far it's moved.
    pub translation: [f32; 3],
    /// How it's turned, as a quaternion: x, y, z, w.
    pub rotation: [f32; 4],
    /// How much it's stretched along each axis.
    pub scale: [f32; 3],
}

impl Default for JointPose {
    fn default() -> Self {
        Self {
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        }
    }
}

impl JointPose {
    /// This as a matrix: stretched, then turned, then moved.
    #[must_use]
    pub fn rows(&self) -> Rows {
        let [x, y, z, w] = self.rotation;
        let [sx, sy, sz] = self.scale;
        let [tx, ty, tz] = self.translation;
        [
            [
                (1.0 - 2.0 * (y * y + z * z)) * sx,
                2.0 * (x * y - w * z) * sy,
                2.0 * (x * z + w * y) * sz,
                tx,
            ],
            [
                2.0 * (x * y + w * z) * sx,
                (1.0 - 2.0 * (x * x + z * z)) * sy,
                2.0 * (y * z - w * x) * sz,
                ty,
            ],
            [
                2.0 * (x * z - w * y) * sx,
                2.0 * (y * z + w * x) * sy,
                (1.0 - 2.0 * (x * x + y * y)) * sz,
                tz,
            ],
        ]
    }
}

/// A joint of a skeleton.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Joint {
    /// What it's called.
    pub name: String,
    /// The joint it hangs from, which always comes before it.
    pub parent: Option<u16>,
    /// Where it is on that when no clip says otherwise.
    pub rest: JointPose,
    /// What takes a corner from the mesh's space to the joint's.
    pub inverse_bind: Rows,
}

/// What a channel of a clip moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChannelPath {
    /// Where a joint is.
    #[default]
    Translation,
    /// How a joint is turned.
    Rotation,
    /// How a joint is stretched.
    Scale,
    /// How much of each morph target shows, which is the whole mesh's, and on no joint.
    Weights,
}

/// How a channel goes between its keys.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Interpolation {
    /// It holds each key until the next.
    Step,
    /// It goes evenly from each to the next.
    #[default]
    Linear,
    /// It follows a curve, with tangents kept beside each key.
    Cubic,
}

/// One thing a clip moves over time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Channel {
    /// Which joint.
    pub joint: u16,
    /// What of it.
    pub path: ChannelPath,
    /// How it goes between keys.
    pub interpolation: Interpolation,
    /// When each key is, in seconds, in order.
    pub times: Vec<f32>,
    /// The keys' values, one after the other. A curve keeps three for each key: the tangent in,
    /// the value, and the tangent out.
    pub values: Vec<f32>,
}

/// A named animation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clip {
    /// What it's called.
    pub name: String,
    /// How long it is, in seconds.
    pub length: f32,
    /// What it moves.
    pub channels: Vec<Channel>,
}

/// `a` to `b` by `share`, the short way round, at a length of one.
#[must_use]
fn turn_between(a: [f32; 4], b: [f32; 4], share: f32) -> [f32; 4] {
    let dot: f32 = a.iter().zip(&b).map(|(a, b)| a * b).sum();
    // A quaternion and its opposite are the same turn, and only one way is short.
    let (b, dot) = if dot < 0.0 {
        (b.map(|part| -part), -dot)
    } else {
        (b, dot)
    };
    let (from, to) = if dot > 0.9995 {
        // So close that slerp would divide by nearly nothing.
        (1.0 - share, share)
    } else {
        let angle = dot.acos();
        let sine = angle.sin();
        (
            ((1.0 - share) * angle).sin() / sine,
            (share * angle).sin() / sine,
        )
    };
    normalized_turn([0, 1, 2, 3].map(|at| a[at] * from + b[at] * to))
}

/// `turn` at a length of one.
fn normalized_turn(turn: [f32; 4]) -> [f32; 4] {
    let length = turn.iter().map(|part| part * part).sum::<f32>().sqrt();
    if length > 0.0 {
        turn.map(|part| part / length)
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

impl Channel {
    /// How many numbers one of its values has.
    fn width(&self) -> usize {
        match self.path {
            ChannelPath::Rotation => 4,
            ChannelPath::Translation | ChannelPath::Scale => 3,
            ChannelPath::Weights => {
                let parts = if self.interpolation == Interpolation::Cubic {
                    3
                } else {
                    1
                };
                self.values.len() / (self.times.len() * parts).max(1)
            }
        }
    }

    /// The value of key `key`, which for a curve is `part` of its three: 0 the tangent in, 1 the
    /// value, and 2 the tangent out.
    fn key(&self, key: usize, part: usize) -> &[f32] {
        let width = self.width();
        let at = match self.interpolation {
            Interpolation::Cubic => (key * 3 + part) * width,
            _ => key * width,
        };
        self.values.get(at..at + width).unwrap_or(&[])
    }

    /// Its value at `time`, into `out`. Before the first key and after the last it holds.
    fn sample(&self, time: f32, out: &mut [f32]) {
        let width = self.width();
        let count = self.times.len();
        if count == 0 {
            return;
        }
        let next = self.times.partition_point(|key| *key <= time);
        let width = width.min(out.len());
        let copy = |values: &[f32], out: &mut [f32]| {
            for (slot, value) in out.iter_mut().zip(values) {
                *slot = *value;
            }
        };
        if next == 0 || next == count {
            let key = if next == 0 { 0 } else { count - 1 };
            copy(self.key(key, 1), out);
            return;
        }
        let last = next - 1;
        let span = self.times[next] - self.times[last];
        let share = if span > 0.0 {
            (time - self.times[last]) / span
        } else {
            0.0
        };
        let (from, to) = (self.key(last, 1), self.key(next, 1));
        if from.len() < width || to.len() < width {
            return;
        }
        match self.interpolation {
            Interpolation::Step => copy(from, out),
            Interpolation::Linear if self.path == ChannelPath::Rotation => {
                let (a, b) = (
                    [from[0], from[1], from[2], from[3]],
                    [to[0], to[1], to[2], to[3]],
                );
                out[..4].copy_from_slice(&turn_between(a, b, share));
            }
            Interpolation::Linear => {
                for at in 0..width {
                    out[at] = from[at] + (to[at] - from[at]) * share;
                }
            }
            Interpolation::Cubic => {
                let (leaving, arriving) = (self.key(last, 2), self.key(next, 0));
                let (t, t2, t3) = (share, share * share, share * share * share);
                for at in 0..width {
                    out[at] = (2.0 * t3 - 3.0 * t2 + 1.0) * from[at]
                        + (t3 - 2.0 * t2 + t) * span * leaving.get(at).copied().unwrap_or(0.0)
                        + (-2.0 * t3 + 3.0 * t2) * to[at]
                        + (t3 - t2) * span * arriving.get(at).copied().unwrap_or(0.0);
                }
                if self.path == ChannelPath::Rotation {
                    let turn = normalized_turn([out[0], out[1], out[2], out[3]]);
                    out[..4].copy_from_slice(&turn);
                }
            }
        }
    }
}

/// `from` going to `to` by `share`, joint by joint, which is how a change of clip is smoothed.
#[must_use]
pub fn blend_poses(from: &[JointPose], to: &[JointPose], share: f32) -> Vec<JointPose> {
    let between = |a: [f32; 3], b: [f32; 3]| [0, 1, 2].map(|at| a[at] + (b[at] - a[at]) * share);
    from.iter()
        .zip(to)
        .map(|(a, b)| JointPose {
            translation: between(a.translation, b.translation),
            rotation: turn_between(a.rotation, b.rotation, share),
            scale: between(a.scale, b.scale),
        })
        .collect()
}

/// A named place on a skeleton.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkeletonPlace {
    /// What it's called.
    pub name: String,
    /// The joint that carries it, or none if it stays with the whole.
    pub joint: Option<u16>,
    /// Where it is on that.
    pub offset: Rows,
}

/// Joints and what moves them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Skeleton {
    /// Its joints, each after the one it hangs from.
    pub joints: Vec<Joint>,
    /// Its clips.
    pub clips: Vec<Clip>,
    /// Its named places.
    pub places: Vec<SkeletonPlace>,
    /// How far any corner gets from the joints that carry it, for bounds that follow a pose.
    pub reach: f32,
    /// What each morph target is called.
    pub morph_names: Vec<String>,
    /// How much of each shows with no clip saying otherwise.
    pub rest_weights: Vec<f32>,
}

impl Skeleton {
    /// The clip called `name`.
    #[must_use]
    pub fn clip_named(&self, name: &str) -> Option<usize> {
        self.clips.iter().position(|clip| clip.name == name)
    }

    /// Where each joint is, on what it hangs from, `time` seconds into `clip`. A clip goes
    /// round, and with none, it's at rest.
    #[must_use]
    pub fn pose(&self, clip: Option<usize>, time: f32) -> Vec<JointPose> {
        let mut pose: Vec<JointPose> = self.joints.iter().map(|joint| joint.rest).collect();
        let Some(clip) = clip.and_then(|clip| self.clips.get(clip)) else {
            return pose;
        };
        let time = if clip.length > 0.0 {
            time.rem_euclid(clip.length)
        } else {
            0.0
        };
        for channel in &clip.channels {
            let Some(joint) = pose.get_mut(usize::from(channel.joint)) else {
                continue;
            };
            let mut value = [0.0; 4];
            match channel.path {
                ChannelPath::Translation => {
                    channel.sample(time, &mut value);
                    joint.translation = [value[0], value[1], value[2]];
                }
                ChannelPath::Scale => {
                    value = [joint.scale[0], joint.scale[1], joint.scale[2], 0.0];
                    channel.sample(time, &mut value);
                    joint.scale = [value[0], value[1], value[2]];
                }
                ChannelPath::Rotation => {
                    value = joint.rotation;
                    channel.sample(time, &mut value);
                    joint.rotation = value;
                }
                ChannelPath::Weights => {}
            }
        }
        pose
    }

    /// How much of each morph target shows, `time` seconds into `clip`.
    #[must_use]
    pub fn weights(&self, clip: Option<usize>, time: f32) -> Vec<f32> {
        let mut weights = self.rest_weights.clone();
        weights.resize(self.morph_names.len(), 0.0);
        let Some(clip) = clip.and_then(|clip| self.clips.get(clip)) else {
            return weights;
        };
        let time = if clip.length > 0.0 {
            time.rem_euclid(clip.length)
        } else {
            0.0
        };
        let channels = clip.channels.iter();
        for channel in channels.filter(|channel| channel.path == ChannelPath::Weights) {
            channel.sample(time, &mut weights);
        }
        weights
    }

    /// What places each joint in the mesh's space, for `pose`.
    #[must_use]
    pub fn placed(&self, pose: &[JointPose]) -> Vec<Rows> {
        let mut placed: Vec<Rows> = Vec::with_capacity(self.joints.len());
        for (joint, local) in self.joints.iter().zip(pose) {
            let parent = joint.parent.and_then(|at| placed.get(usize::from(at)));
            let rows = local.rows();
            placed.push(parent.map_or(rows, |parent| after(parent, &rows)));
        }
        placed
    }

    /// What takes each corner to where `placed` says, for skinning.
    #[must_use]
    pub fn palette(&self, placed: &[Rows]) -> Vec<Rows> {
        self.joints
            .iter()
            .zip(placed)
            .map(|(joint, rows)| after(rows, &joint.inverse_bind))
            .collect()
    }

    /// The named place `name`, in the mesh's space, for `placed`.
    #[must_use]
    pub fn place(&self, name: &str, placed: &[Rows]) -> Option<Rows> {
        let place = self.places.iter().find(|place| place.name == name)?;
        Some(match place.joint {
            Some(joint) => after(placed.get(usize::from(joint))?, &place.offset),
            None => place.offset,
        })
    }

    /// The middle of `placed` and how far the mesh reaches from it, for seeing if it's in view.
    /// Bounds go by the pose, or a mesh that's moved a long way from its rest pops out of view.
    #[must_use]
    pub fn bounds(&self, placed: &[Rows]) -> (Vec3, f32) {
        let places: Vec<Vec3> = placed.iter().map(|rows| moved(rows, Vec3::ZERO)).collect();
        let Some(first) = places.first().copied() else {
            return (Vec3::ZERO, self.reach);
        };
        let (mut low, mut high) = (first, first);
        for place in &places {
            low = Vec3::new(low.x.min(place.x), low.y.min(place.y), low.z.min(place.z));
            high = Vec3::new(
                high.x.max(place.x),
                high.y.max(place.y),
                high.z.max(place.z),
            );
        }
        ((low + high) * 0.5, (high - low).length() * 0.5 + self.reach)
    }
}
