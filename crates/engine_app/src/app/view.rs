//! What's seen: the view, its effects and the picture they make.

use std::collections::HashMap;

use engine_audio::Listener;
use engine_client::{
    DrawnEntity, EffectSource, ScreenEvent, VIEW_HEIGHT, dead_roll, view_bob, view_idle, view_roll,
};
use engine_core::canvas::Quad;
use engine_core::entity::ModelInstance;
use engine_core::math::{Vec3, view_axes};
use engine_map::Contents;
use engine_render::{
    CapsuleDraw, Drawables, EntityDraw, LightDraw, ParticleColor, ParticleDraw, SpriteDraw, View,
};
use fixture_quake::Intermission;

use super::World;
use super::shading::{StencilMask, capsule_draws};
use crate::models::{DrawLists, Lighting};

/// A frame's picture.
pub(crate) struct Picture {
    /// The view.
    pub(crate) view: View,
    /// Where brush models are.
    instances: Vec<ModelInstance>,
    /// The other models.
    entities: Vec<EntityDraw>,
    /// The sprites.
    sprites: Vec<SpriteDraw>,
    /// The particles.
    particles: Vec<ParticleDraw>,
    /// The dynamic lights.
    lights: Vec<LightDraw>,
    /// What's drawn over all of that.
    pub(crate) quads: Vec<Quad>,
    /// Where stencils' lights are.
    stencil_lights: Vec<Vec3>,
    /// How open stencils are.
    stencil_masks: Vec<StencilMask>,
    /// Characters shading the light.
    capsules: Vec<CapsuleDraw>,
}

/// How far the eye sits above a dead player's middle.
pub(super) const DEAD_VIEW_HEIGHT: f32 = -8.0;

/// How wide the view is with no `fov` set, in degrees.
pub(super) const DEFAULT_FOV: f32 = 90.0;

impl Picture {
    /// What's in it, as the renderer wants it.
    pub(crate) fn drawables(&self) -> Drawables<'_> {
        Drawables {
            instances: &self.instances,
            entities: &self.entities,
            sprites: &self.sprites,
            particles: &self.particles,
            lights: &self.lights,
            quads: &self.quads,
            stencil_lights: &self.stencil_lights,
            stencil_masks: &self.stencil_masks,
            capsules: &self.capsules,
        }
    }
}

impl World {
    /// Turns the view by the keys held.
    pub(super) fn turn_view(&mut self, seconds: f32) {
        let (yaw, pitch) = self.buttons.turn(seconds);
        let held = self
            .client
            .as_ref()
            .is_some_and(|client| client.predicted().frozen);
        if !held && (yaw != 0.0 || pitch != 0.0) {
            let (yaw, pitch) = (
                self.look.yaw() + f64::from(yaw),
                self.look.pitch() + f64::from(pitch),
            );
            self.look.set_view(yaw, pitch);
        }
    }

    /// Shows what the game asked to.
    pub(super) fn show_events(&mut self, events: Vec<ScreenEvent>) {
        for event in events {
            match event {
                ScreenEvent::CenterPrint(text) => self.hud.center_print(text),
                ScreenEvent::Command(text) => self.commands.push(text),
                ScreenEvent::Damage { armor, blood, from } => self.hurt(armor, blood, from),
                ScreenEvent::Intermission(kind) => {
                    self.hud.set_intermission(Intermission::from_number(kind));
                }
                ScreenEvent::LevelTitle(text) => {
                    self.console_view.print(&text, f64::NEG_INFINITY);
                    self.hud.set_level_title(text);
                }
            }
        }
    }

    /// Shows the player being hurt, from `V_ParseDamage`.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "f32 keeps angles to a few thousandths of a degree"
    )]
    pub(super) fn hurt(&mut self, armor: u8, blood: u8, from: Vec3) {
        self.hud.damage(armor, blood);
        let Some(client) = &self.client else {
            return;
        };
        let count = (f32::from(blood) * 0.5 + f32::from(armor) * 0.5).max(10.0);
        let direction = (from - client.predicted().origin).normalized();
        let [forward, right, _up] = view_axes(self.look.yaw() as f32, self.look.pitch() as f32);
        self.kick
            .hit(count, direction.dot(right), direction.dot(forward));
    }

    /// What's at the eye, for the colour of the view.
    pub(super) fn contents_at(&self, eye: Vec3) -> Contents {
        self.map.as_ref().map_or(Contents::Empty, |map| {
            map.tree
                .leaf(map.tree.leaf_at(eye))
                .map_or(Contents::Empty, |leaf| leaf.contents)
        })
    }

    /// Follows what's on the client for effects, and returns the lights
    /// shining now.
    pub(super) fn update_effects(
        &mut self,
        time: f32,
        drawn: &[DrawnEntity],
        player: EffectSource,
    ) -> Vec<LightDraw> {
        if let Some(client) = &self.client {
            self.effects.set_gravity(client.gravity());
        }
        self.effects.update(time);
        let mut sources: Vec<EffectSource> = drawn
            .iter()
            .map(|entity| EffectSource {
                id: entity.state.id,
                origin: entity.state.origin,
                angles: entity.state.angles,
                effects: entity.state.effects,
                model_flags: self.models.flags(entity.state.model),
            })
            .collect();
        sources.push(player);
        self.effects.follow(&sources);
        self.effects
            .lights()
            .map(|light| LightDraw {
                origin: light.origin,
                radius: light.radius,
                min_light: light.min_light,
            })
            .collect()
    }

    /// The particles to draw, in their colours.
    pub(super) fn particle_draws(&self) -> Vec<ParticleDraw> {
        let Some(palette) = &self.palette else {
            return Vec::new();
        };
        self.effects
            .particles()
            .live()
            .iter()
            .map(|particle| {
                let [red, green, blue] = palette.color(particle.color);
                ParticleDraw {
                    origin: particle.origin,
                    color: ParticleColor { red, green, blue },
                }
            })
            .collect()
    }

    /// The picture, `leftover` seconds past the last tick. `frame_seconds` is
    /// the time since the last view, for smoothing.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "angles and map time need no more than f32 for drawing"
    )]
    pub(crate) fn view(&mut self, leftover: f32, fraction: f32, frame_seconds: f32) -> Picture {
        let command = self.command();
        let mut origin = Vec3::ZERO;
        let mut instances = Vec::new();
        let mut lists = DrawLists::default();
        let mut lights = Vec::new();
        let mut styles = [1.0; engine_client::LIGHT_STYLE_COUNT];
        let mut punch = Vec3::ZERO;
        let online = self.is_online();
        let mut sway = (0.0, 0.0, 0.0);
        let mut roll = 0.0;
        let mut capsules = Vec::new();
        let time = self.map_time as f32;
        if let Some(client) = &mut self.client {
            let state = client.render_state(&command, leftover);
            let body = state.origin + client.ground_lag(&state, fraction);
            let step = self.step.update(body.z, state.on_ground, frame_seconds);
            let speed = Vec3::new(state.velocity.x, state.velocity.y, 0.0).length();
            let bob = view_bob(time, speed);
            let frozen = state.frozen;
            let eye = if frozen {
                0.0
            } else if state.dead {
                DEAD_VIEW_HEIGHT
            } else {
                VIEW_HEIGHT
            };
            origin = body + Vec3::new(0.0, 0.0, eye + step + bob);
            let (sin, cos) = self.look.yaw().to_radians().sin_cos();
            self.listener = Listener {
                origin,
                right: Vec3::new(sin as f32, -cos as f32, 0.0),
            };
            instances = client.draw_instances(fraction);
            styles = client.light_styles(self.map_time);
            if let Some(map) = &self.map {
                for track in &map.light_tracks {
                    track.blend(&mut styles, time);
                }
            }
            punch = client.punch(fraction);
            let (kick_pitch, kick_roll) = self.kick.update(frame_seconds);
            let right = self.listener.right;
            roll = view_roll(state.velocity.dot(right)) + kick_roll;
            punch.x += kick_pitch;
            if state.dead {
                roll = dead_roll();
            }
            if frozen {
                roll = 0.0;
                sway = view_idle(time);
            }
            let drawn = if online {
                client.draw_entities_buffered(f64::from(frame_seconds))
            } else {
                client.draw_entities(fraction)
            };
            let wearing: HashMap<u64, u8> = client
                .scores()
                .iter()
                .map(|score| (score.entity, score.colors))
                .collect();
            let (model, frame) = client.weapon();
            let bodies = client.solid_boxes().to_vec();
            let player = EffectSource {
                id: client.player_entity(),
                origin: body,
                angles: Vec3::new(self.look.pitch() as f32, self.look.yaw() as f32, 0.0),
                effects: client.take_player_effects(),
                model_flags: 0,
            };
            lights = self.update_effects(time + leftover, &drawn, player);
            if let Some(map) = &self.map {
                let (flames, sparks) = self.flames.update(map, origin, time, frame_seconds);
                lights.extend(flames);
                for spark in sparks {
                    self.effects.spark(spark);
                }
            }
            if let Some(map) = self.map.clone() {
                let lighting = Lighting {
                    map: &map,
                    styles: &styles,
                    lights: &lights,
                    time,
                };
                for entity in &drawn {
                    let colors = wearing.get(&entity.state.id).copied();
                    if let Some(colors) = colors {
                        self.models.prepare_colors(
                            entity.state.model,
                            colors,
                            &self.filesystem,
                            self.renderer.as_mut(),
                        );
                    }
                    self.models.draw(entity, colors, &lighting, &mut lists);
                }
                for piece in self.effects.beam_pieces(player.id, body) {
                    self.models.draw_beam(
                        &piece,
                        &self.filesystem,
                        self.renderer.as_mut(),
                        &mut lists,
                    );
                }
                let yaw = self.look.yaw() as f32;
                let pitch = self.look.pitch() as f32;
                let [forward, ..] = view_axes(yaw, pitch);
                // From `V_CalcRefdef`, with the status bar's 2 units.
                let placed = origin + forward * (bob * 0.4) + Vec3::new(0.0, 0.0, 2.0);
                let angles = Vec3::new(-pitch, yaw, 0.0);
                capsules = capsule_draws(&map, body, origin, &bodies, &instances);
                if !frozen {
                    lists.entities.extend(self.models.draw_weapon(
                        u32::from(model),
                        frame,
                        (placed, angles),
                        &lighting,
                    ));
                }
            }
        }
        if let (Some(map), Some(client)) = (&self.map, &self.client) {
            self.gates.update(map, client, time, frame_seconds);
        }
        let contents = self.contents_at(origin);
        let fade = if self.is_running() {
            frame_seconds
        } else {
            0.0
        };
        self.hud.update_view(fade, contents);
        let view = View {
            origin,
            yaw: self.look.yaw() as f32 + punch.y + sway.1,
            pitch: self.look.pitch() as f32 + punch.x + sway.0,
            roll: roll + sway.2,
            fov: self.fov,
            time: self.map_time as f32,
            styles,
            gamma: self.gamma,
        };
        Picture {
            view,
            instances,
            entities: lists.entities,
            sprites: lists.sprites,
            particles: self.particle_draws(),
            lights,
            quads: Vec::new(),
            stencil_lights: self.gates.lights().to_vec(),
            stencil_masks: self.gates.masks().to_vec(),
            capsules,
        }
    }
}
