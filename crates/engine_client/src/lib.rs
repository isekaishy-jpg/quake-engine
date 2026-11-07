//! Client state: prediction, interpolation and input commands.
//!
//! May depend on `engine_core`, `engine_console`, `engine_map`, `engine_physics`, `engine_movement`, `engine_net`. See proposal 0008.

mod ambience;
mod bindings;
mod bot;
mod client;
mod effect_system;
mod effects;
mod input;
mod lightstyle;
mod look;
mod particles;
mod projectiles;
mod shown_shot;
mod view;
mod view_clock;
mod view_tint;
mod zones;

pub use ambience::AmbientSounds;
pub use bindings::{Bindings, canonical_key};
pub use bot::{Bot, BotSkill};
pub use client::{Client, DEFAULT_SHOT_CORRECTION, DrawnEntity, ScreenEvent};
pub use effect_system::{EffectDraw, EffectLight, EffectSystem, GpuEmitter, MAX_EFFECT_PARTICLES};
pub use effects::{BeamPiece, DynamicLight, EffectSource, Effects, MAX_LIGHTS};
pub use input::Buttons;
pub use lightstyle::{LIGHT_STYLE_COUNT, LightStyles};
pub use look::{Look, LookScale};
pub use particles::{Particle, Particles, Trail};
pub use projectiles::{Flights, Launch, Path, own_shot_time};
pub use shown_shot::ShotClock;
pub use view::{StepSmoother, VIEW_HEIGHT, ViewKick, dead_roll, view_bob, view_idle, view_roll};
pub use view_clock::ViewClock;
pub use view_tint::{Shift, ViewTint};
pub use zones::{ZoneChange, ZoneTracker};
