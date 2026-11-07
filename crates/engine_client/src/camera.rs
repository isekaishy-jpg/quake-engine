//! Cameras (proposal 0138). The engine does the work of a camera, every drawn frame: following,
//! not going through walls, blending from one to another. What's wanted is said in a line of
//! text, by a script, a level or the console, and mouse look never goes through any of that.

use engine_core::math::Vec3;
use engine_map::{Map, Spline};

use crate::view::view_idle;

/// How far a boom reaches behind the player, unless it's said.
const USUAL_BOOM: f32 = 96.0;

/// How far above the eye a boom's pivot is, unless it's said.
const USUAL_RISE: f32 = 8.0;

/// How long a boom takes to come halfway back out, in seconds. Coming in isn't eased: a wall
/// is a wall, and a camera that eases into one shows what's behind it.
const BOOM_HALF_LIFE: f32 = 0.08;

/// How long the pivot takes to catch half of the way up to the player, in seconds.
const PIVOT_HALF_LIFE: f32 = 0.02;

/// How far a camera stays from what's behind it.
const CLEARANCE: f32 = 10.0;

/// How short a boom gets before the camera snaps in to the player's own eyes.
const SNAP_IN: f32 = 28.0;

/// How far the player can move in a frame before the camera jumps with them, not after them.
const TELEPORT: f32 = 256.0;

/// What a camera is.
#[derive(Clone, Debug, PartialEq)]
pub enum CameraKind {
    /// From the player's own eyes.
    FirstPerson,
    /// Behind the player, on a boom.
    ThirdPerson {
        /// How far behind.
        boom: f32,
        /// How far over the eye.
        rise: f32,
        /// How far to the side.
        side: f32,
    },
    /// In one place.
    Fixed {
        /// Where.
        origin: Vec3,
        /// Its yaw, in degrees.
        yaw: f32,
        /// Its pitch, in degrees.
        pitch: f32,
        /// How much it sways, with 1 as much as Quake's intermission.
        idle: f32,
    },
    /// A camera in the level, by its name.
    Scene(String),
}

/// What's asked of the cameras: which, and how long to take getting there.
#[derive(Clone, Debug, PartialEq)]
pub struct CameraOrder {
    /// The camera.
    pub kind: CameraKind,
    /// How long it takes to blend to it, in seconds.
    pub blend: f32,
}

impl CameraKind {
    /// Reads a camera from `text`: `first`, `third`, `third 96 8 0`, `fixed x y z yaw pitch idle`
    /// or `scene name`. Left out numbers keep their usual values.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut words = text.split_whitespace();
        let kind = words.next()?;
        let rest: Vec<&str> = words.collect();
        let numbers: Option<Vec<f32>> = rest.iter().map(|word| word.parse().ok()).collect();
        let number =
            |values: &[f32], index: usize, usual: f32| values.get(index).copied().unwrap_or(usual);
        match kind {
            "first" => Some(Self::FirstPerson),
            "third" => {
                let values = numbers?;
                Some(Self::ThirdPerson {
                    boom: number(&values, 0, USUAL_BOOM),
                    rise: number(&values, 1, USUAL_RISE),
                    side: number(&values, 2, 0.0),
                })
            }
            "fixed" => {
                let values = numbers?;
                Some(Self::Fixed {
                    origin: Vec3::new(*values.first()?, *values.get(1)?, *values.get(2)?),
                    yaw: number(&values, 3, 0.0),
                    pitch: number(&values, 4, 0.0),
                    idle: number(&values, 5, 0.0),
                })
            }
            "scene" => rest.first().map(|name| Self::Scene((*name).to_owned())),
            _ => None,
        }
    }
}

/// What the player is doing, as a camera needs to know it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraSubject {
    /// Where their eyes are, with all that moves them.
    pub eye: Vec3,
    /// Where they look, in degrees.
    pub yaw: f32,
    /// Where they look, in degrees.
    pub pitch: f32,
    /// How the view rolls, in degrees.
    pub roll: f32,
    /// The game's time.
    pub time: f32,
    /// The seconds since the last frame.
    pub seconds: f32,
}

/// Where a camera is this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraShot {
    /// Where it is.
    pub origin: Vec3,
    /// Its yaw, in degrees.
    pub yaw: f32,
    /// Its pitch, in degrees.
    pub pitch: f32,
    /// Its roll, in degrees.
    pub roll: f32,
    /// Whether it's in the player's own eyes, so that they see their weapon and not themselves.
    pub first_person: bool,
}

/// The angle from `from` to `to` by the short way, in degrees.
fn turn(from: f32, to: f32) -> f32 {
    (to - from + 180.0).rem_euclid(360.0) - 180.0
}

/// What's left after `seconds` of something halving each `half_life`.
fn left_after(seconds: f32, half_life: f32) -> f32 {
    0.5_f32.powf(seconds / half_life)
}

/// The way a view faces.
fn forward_of(yaw: f32, pitch: f32) -> Vec3 {
    let (yaw_sin, yaw_cos) = yaw.to_radians().sin_cos();
    let (pitch_sin, pitch_cos) = pitch.to_radians().sin_cos();
    Vec3::new(pitch_cos * yaw_cos, pitch_cos * yaw_sin, -pitch_sin)
}

/// A camera in a level.
#[derive(Clone, Debug, PartialEq)]
struct Scene {
    /// Where it is, if it stays there.
    origin: Vec3,
    /// Which way it looks.
    yaw: f32,
    /// Which way it looks.
    pitch: f32,
    /// How much it sways.
    idle: f32,
    /// The line it moves along, if it does.
    path: Option<Spline>,
    /// How long that takes, in seconds.
    seconds: f32,
    /// What it keeps looking at, if anything.
    look_at: Option<Vec3>,
}

/// The `camera` entity called `name` in `map`.
fn scene_in(map: &Map, name: &str) -> Option<Scene> {
    let entity = map.entities.iter().find(|entity| {
        entity.get("classname") == Some("camera") && entity.get("name") == Some(name)
    })?;
    let point = |key: &str| {
        let text = entity.get(key)?;
        let mut numbers = text.split_whitespace().map(|word| word.parse::<f32>().ok());
        Some(Vec3::new(
            numbers.next()??,
            numbers.next()??,
            numbers.next()??,
        ))
    };
    let number = |key: &str, usual: f32| {
        entity
            .get(key)
            .and_then(|text| text.parse().ok())
            .unwrap_or(usual)
    };
    let path = entity
        .get("path")
        .and_then(|line| map.splines.iter().find(|spline| spline.name == line))
        .cloned();
    Some(Scene {
        origin: point("origin").unwrap_or(Vec3::ZERO),
        yaw: number("yaw", 0.0),
        pitch: number("pitch", 0.0),
        idle: number("idle", 0.0),
        path,
        seconds: number("seconds", 10.0).max(0.01),
        look_at: point("look_at"),
    })
}

/// The cameras: which is in use, and what it remembers from frame to frame.
#[derive(Clone, Debug)]
pub struct Cameras {
    /// The camera in use.
    current: CameraKind,
    /// The level's camera, if that's what's in use.
    scene: Option<Scene>,
    /// When it started, by the game's time.
    started: f32,
    /// What the last frame showed, to blend from.
    last: Option<CameraShot>,
    /// Where it's blending from, and how long that has left and took in all.
    blending: Option<(CameraShot, f32, f32)>,
    /// How long the boom is now.
    boom_now: f32,
    /// Where the boom turns.
    pivot: Option<Vec3>,
}

impl Default for Cameras {
    fn default() -> Self {
        Self {
            current: CameraKind::FirstPerson,
            scene: None,
            started: 0.0,
            last: None,
            blending: None,
            boom_now: 0.0,
            pivot: None,
        }
    }
}

impl Cameras {
    /// Changes to the camera in `order`, in `map`.
    pub fn order(&mut self, order: &CameraOrder, map: Option<&Map>, time: f32) {
        self.scene = match &order.kind {
            CameraKind::Scene(name) => map.and_then(|map| scene_in(map, name)),
            _ => None,
        };
        self.current = order.kind.clone();
        self.started = time;
        self.boom_now = 0.0;
        self.pivot = None;
        self.blending = self
            .last
            .filter(|_| order.blend > 0.0)
            .map(|last| (last, order.blend, order.blend));
    }

    /// The shot of a boom behind `subject`, by how far `room` says it can reach.
    fn boom_shot(
        &mut self,
        subject: &CameraSubject,
        (boom, rise, side): (f32, f32, f32),
        room: &dyn Fn(Vec3, Vec3, f32) -> f32,
    ) -> CameraShot {
        let wanted_pivot = subject.eye + Vec3::new(0.0, 0.0, rise);
        let pivot = self
            .pivot
            .filter(|pivot| (*pivot - wanted_pivot).length() < TELEPORT)
            .map_or(wanted_pivot, |pivot| {
                wanted_pivot + (pivot - wanted_pivot) * left_after(subject.seconds, PIVOT_HALF_LIFE)
            });
        self.pivot = Some(pivot);
        let back = forward_of(subject.yaw, subject.pitch) * -1.0;
        let right = forward_of(subject.yaw - 90.0, 0.0);
        let shoulder = pivot + right * side;
        // A few lines, so that the camera's own width clears what it passes.
        let up = Vec3::new(0.0, 0.0, CLEARANCE);
        let across = right * CLEARANCE;
        let reach = [Vec3::ZERO, up, up * -1.0, across, across * -1.0]
            .iter()
            .map(|offset| room(shoulder + *offset, back, boom))
            .fold(boom, f32::min);
        let allowed = (reach - CLEARANCE).max(0.0);
        self.boom_now = if allowed < self.boom_now {
            allowed
        } else {
            allowed + (self.boom_now - allowed) * left_after(subject.seconds, BOOM_HALF_LIFE)
        };
        if self.boom_now < SNAP_IN {
            return CameraShot {
                origin: subject.eye,
                yaw: subject.yaw,
                pitch: subject.pitch,
                roll: subject.roll,
                first_person: true,
            };
        }
        CameraShot {
            origin: shoulder + back * self.boom_now,
            yaw: subject.yaw,
            pitch: subject.pitch,
            roll: 0.0,
            first_person: false,
        }
    }

    /// The shot of the level's camera at `time`.
    fn scene_shot(scene: &Scene, time: f32, started: f32) -> CameraShot {
        let share = ((time - started) / scene.seconds).clamp(0.0, 1.0);
        let origin = scene
            .path
            .as_ref()
            .and_then(|path| path.point_at(share))
            .unwrap_or(scene.origin);
        let (yaw, pitch) = scene.look_at.map_or((scene.yaw, scene.pitch), |target| {
            let way = target - origin;
            let flat = (way.x * way.x + way.y * way.y).sqrt();
            (
                way.y.atan2(way.x).to_degrees(),
                -way.z.atan2(flat).to_degrees(),
            )
        });
        Self::still_shot(origin, yaw, pitch, scene.idle, time)
    }

    /// The shot of a camera in one place, swaying by `idle`.
    fn still_shot(origin: Vec3, yaw: f32, pitch: f32, idle: f32, time: f32) -> CameraShot {
        let (sway_pitch, sway_yaw, sway_roll) = view_idle(time);
        CameraShot {
            origin,
            yaw: yaw + sway_yaw * idle,
            pitch: pitch + sway_pitch * idle,
            roll: sway_roll * idle,
            first_person: false,
        }
    }

    /// Where the camera is this frame. `room` says how far a line from a place along a way
    /// can reach, up to a limit.
    pub fn shot(
        &mut self,
        subject: &CameraSubject,
        room: &dyn Fn(Vec3, Vec3, f32) -> f32,
    ) -> CameraShot {
        let shot = match self.current.clone() {
            CameraKind::ThirdPerson { boom, rise, side } => {
                self.boom_shot(subject, (boom, rise, side), room)
            }
            CameraKind::Fixed {
                origin,
                yaw,
                pitch,
                idle,
            } => Self::still_shot(origin, yaw, pitch, idle, subject.time),
            CameraKind::Scene(_) if self.scene.is_some() => {
                let scene = self
                    .scene
                    .as_ref()
                    .map(|scene| Self::scene_shot(scene, subject.time, self.started));
                scene.unwrap_or(CameraShot {
                    origin: subject.eye,
                    yaw: subject.yaw,
                    pitch: subject.pitch,
                    roll: subject.roll,
                    first_person: true,
                })
            }
            CameraKind::FirstPerson | CameraKind::Scene(_) => CameraShot {
                origin: subject.eye,
                yaw: subject.yaw,
                pitch: subject.pitch,
                roll: subject.roll,
                first_person: true,
            },
        };
        let shown = self.blended(shot, subject.seconds);
        self.last = Some(shown);
        shown
    }

    /// `shot`, partway from where the last camera was, while that's going on.
    fn blended(&mut self, shot: CameraShot, seconds: f32) -> CameraShot {
        let Some((from, left, whole)) = self.blending else {
            return shot;
        };
        let left = left - seconds;
        if left <= 0.0 {
            self.blending = None;
            return shot;
        }
        self.blending = Some((from, left, whole));
        let linear = 1.0 - left / whole;
        let eased = linear * linear * (3.0 - 2.0 * linear);
        CameraShot {
            origin: from.origin + (shot.origin - from.origin) * eased,
            yaw: from.yaw + turn(from.yaw, shot.yaw) * eased,
            pitch: from.pitch + (shot.pitch - from.pitch) * eased,
            roll: from.roll + (shot.roll - from.roll) * eased,
            first_person: false,
        }
    }
}
