//! Client state: prediction, interpolation and input commands.
//!
//! May depend on `engine_core`, `engine_console`, `engine_map`, `engine_physics`, `engine_movement`, `engine_net`. See proposal 0008.

mod ambience;
mod bindings;
mod bot;
mod camera;
mod client;
mod effect_system;
mod effects;
mod fixture_bursts;
mod input;
mod lightstyle;
mod look;
mod model_glide;
mod projectiles;
mod shown_shot;
mod view;
mod view_clock;
mod view_tint;
mod water_play;
mod zones;

pub use ambience::AmbientSounds;
pub use bindings::{Bindings, canonical_key};
pub use bot::{Bot, BotSkill};
pub use camera::{CameraKind, CameraOrder, CameraShot, CameraSubject, Cameras};
pub use client::{Client, DEFAULT_SHOT_CORRECTION, DrawnEntity, ScreenEvent};
pub use effect_system::{
    EffectLight, EffectSystem, EffectWorld, MAX_EFFECT_PARTICLES, PlacedParticle,
};
pub use effect_system::{follow_model, followed_model};
pub use effects::{BeamPiece, DynamicLight, EffectSource, Effects, MAX_LIGHTS};
pub use engine_map::{EffectDraw, EffectSettings, GpuEmitter, SimEmitter};
pub use input::Buttons;
pub use lightstyle::{LIGHT_STYLE_COUNT, LightStyles};
pub use look::{Look, LookScale};
pub use model_glide::ModelGlide;
pub use projectiles::{Flights, Launch, Path, own_shot_time};
pub use shown_shot::ShotClock;
pub use view::{StepSmoother, VIEW_HEIGHT, ViewKick, dead_roll, view_bob, view_idle, view_roll};
pub use view_clock::ViewClock;
pub use view_tint::{Shift, ViewTint};
pub use water_play::{WaterEvents, WaterPlay};
pub use zones::{ZoneChange, ZoneTracker};
