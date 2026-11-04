//! Client state: prediction, interpolation and input commands.
//!
//! May depend on `engine_core`, `engine_console`, `engine_map`, `engine_physics`, `engine_movement`, `engine_net`. See proposal 0008.

mod ambience;
mod bindings;
mod bot;
mod client;
mod effects;
mod input;
mod lightstyle;
mod look;
mod particles;
mod projectiles;
mod view;
mod view_clock;

pub use ambience::AmbientSounds;
pub use bindings::{Bindings, canonical_key};
pub use bot::{Bot, BotSkill};
pub use client::{Client, DrawnEntity, ScreenEvent};
pub use effects::{BeamPiece, DynamicLight, EffectSource, Effects, MAX_LIGHTS};
pub use input::Buttons;
pub use lightstyle::{LIGHT_STYLE_COUNT, LightStyles};
pub use look::{Look, LookScale};
pub use particles::{Particle, Particles, Trail};
pub use projectiles::{Flights, Launch, Path};
pub use view::{StepSmoother, VIEW_HEIGHT, ViewKick, dead_roll, view_bob, view_idle, view_roll};
pub use view_clock::ViewClock;
