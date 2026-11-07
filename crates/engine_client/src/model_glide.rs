//! Models that move in steps, shown going evenly.
//!
//! A monster only moves when it thinks, ten times a second, and a tick is far shorter than that.
//! Blending across one tick left it hopping and then standing still, for all that its body
//! animated smoothly. So what moves after standing still is shown going there over a step's
//! length. What moves every tick isn't touched, and it's only for what's on screen: collision,
//! bots and everything else go by where things are.

use std::collections::HashMap;

use engine_core::math::Vec3;

use crate::client::{DrawnEntity, MAX_BLENDED_MOVE, blend_angles};

/// How long a step takes, and how long a model takes to blend to a new frame. Both are the
/// original's ten a second.
const STEP_SECONDS: f64 = 0.1;

/// How long something has to stand still for its next move to be a step. It's longer than a tick,
/// so that what moves every tick never counts.
const STILL_SECONDS: f64 = 0.03;

/// How many frames something has to stand still for, at the least, so that a slow machine doesn't
/// take its own long frames for standing still.
const STILL_FRAMES: f64 = 2.0;

/// What's known of something on screen.
#[derive(Clone, Copy, Debug)]
struct Glide {
    /// Where it's going, and which way it's turning.
    place: Vec3,
    angles: Vec3,
    /// Where this step began, and which way it faced.
    from_place: Vec3,
    from_angles: Vec3,
    /// When this step began.
    began: f64,
    /// When it last moved.
    moved: f64,
    /// Where it was last shown, and which way it faced.
    shown_place: Vec3,
    shown_angles: Vec3,
    /// The frame it's showing, the one before, and when that changed.
    frame: u16,
    from_frame: u16,
    frame_began: f64,
}

impl Glide {
    /// Something first seen at `drawn`, which has nothing to catch up with.
    fn new(drawn: &DrawnEntity, now: f64) -> Self {
        let (place, angles) = (drawn.state.origin, drawn.state.angles);
        Self {
            place,
            angles,
            from_place: place,
            from_angles: angles,
            began: now,
            moved: now - STEP_SECONDS,
            shown_place: place,
            shown_angles: angles,
            frame: drawn.state.frame,
            from_frame: drawn.state.frame,
            frame_began: now - STEP_SECONDS,
        }
    }

    /// Takes where it should be now, and works out if that starts a step.
    fn follow(&mut self, place: Vec3, angles: Vec3, now: f64, seconds: f64) {
        if place == self.place && angles == self.angles {
            return;
        }
        if (place - self.shown_place).length() >= MAX_BLENDED_MOVE {
            // A teleport.
            self.from_place = place;
            self.from_angles = angles;
            self.began = now - STEP_SECONDS;
        } else if now - self.moved > STILL_SECONDS.max(seconds * STILL_FRAMES) {
            self.from_place = self.shown_place;
            self.from_angles = self.shown_angles;
            // A frame back, or the first frame of each step would stand still.
            self.began = now - seconds;
        }
        self.moved = now;
        self.place = place;
        self.angles = angles;
    }
}

/// How far along something is, from 0 to 1, that began at `began`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a blend needs no more than f32"
)]
fn along(now: f64, began: f64) -> f32 {
    ((now - began) / STEP_SECONDS).clamp(0.0, 1.0) as f32
}

/// Shows models going evenly, whatever way they got there.
#[derive(Clone, Debug, Default)]
pub struct ModelGlide {
    /// What's on screen, by who it is.
    seen: HashMap<u64, Glide>,
    /// How long it's been going.
    now: f64,
}

impl ModelGlide {
    /// Puts each of `drawn` where it should show, `seconds` after the last frame.
    pub fn smooth(&mut self, drawn: &mut [DrawnEntity], seconds: f32) {
        let seconds = f64::from(seconds);
        self.now += seconds;
        let now = self.now;
        let mut seen = HashMap::with_capacity(drawn.len());
        for entity in drawn {
            let id = entity.state.id;
            let mut glide = self
                .seen
                .get(&id)
                .copied()
                .unwrap_or_else(|| Glide::new(entity, now));
            glide.follow(entity.state.origin, entity.state.angles, now, seconds);
            let blend = along(now, glide.began);
            entity.state.origin = glide.from_place.lerp(glide.place, blend);
            entity.state.angles = blend_angles(glide.from_angles, glide.angles, blend);
            glide.shown_place = entity.state.origin;
            glide.shown_angles = entity.state.angles;
            if entity.state.frame != glide.frame {
                glide.from_frame = glide.frame;
                glide.frame = entity.state.frame;
                glide.frame_began = now;
            }
            entity.from_frame = glide.from_frame;
            entity.blend = along(now, glide.frame_began);
            seen.insert(id, glide);
        }
        self.seen = seen;
    }
}
