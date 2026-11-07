//! What's seen: the view, its effects and the picture they make.

use std::collections::HashMap;

use crate::game_screen::Intermission;
use engine_audio::Listener;
use engine_client::{CameraKind, CameraOrder, CameraShot, CameraSubject, followed_model};
use engine_client::{
    DrawnEntity, EffectSettings, EffectSource, EffectWorld, ScreenEvent, VIEW_HEIGHT, dead_roll,
    view_bob, view_roll,
};
use engine_core::canvas::Quad;
use engine_core::entity::ModelInstance;
use engine_core::math::{Vec3, view_axes};
use engine_map::{Contents, Rows, WaterPush};
use engine_render::{
    CapsuleDraw, Drawables, EntityDraw, GpuParticles, LightDraw, LiveLight, MovingDraw,
    ParticleDraw, ParticleSim, SpriteDraw, View, height_scale,
};

use super::World;
use super::shading::{StencilMask, capsule_draws, live_lights, model_light};
use crate::models::{DrawLists, Lighting};

/// How bright a probe has to be for what takes light from the level to show its own colour.
const PROBE_FULL: f32 = 128.0;

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
    particle_sims: Vec<ParticleSim>,
    /// What's pushing the water near the view.
    water_pushes: Vec<WaterPush>,
    /// Particles the GPU works out.
    gpu_particles: Vec<GpuParticles>,
    /// The dynamic lights.
    lights: Vec<LightDraw>,
    /// What's drawn over all of that.
    pub(crate) quads: Vec<Quad>,
    /// Where stencils' lights are.
    stencil_lights: Vec<Vec3>,
    /// How open stencils are.
    stencil_masks: Vec<StencilMask>,
    /// How far light gets past each stencil's cells.
    stencil_reaches: Vec<StencilMask>,
    /// Characters shading the light.
    capsules: Vec<CapsuleDraw>,
    /// Lights worked out this frame.
    live_lights: Vec<LiveLight>,
    /// Moving meshes.
    moving: Vec<MovingDraw>,
    /// The joints of those.
    palette: Vec<Rows>,
    /// The morph weights of those.
    morph_weights: Vec<f32>,
}

/// How far the eye sits above a dead player's middle.
pub(super) const DEAD_VIEW_HEIGHT: f32 = -8.0;

/// What an effect that goes with the view goes by, which is out of reach of what scripts get.
const VIEW_EFFECT_HANDLE: u32 = u32::MAX;

/// How long one takes to go, after leaving what it goes with.
const VIEW_EFFECT_FADE: f32 = 0.6;

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
            gpu_particles: &self.gpu_particles,
            particle_sims: &self.particle_sims,
            water_pushes: &self.water_pushes,
            lights: &self.lights,
            quads: &self.quads,
            stencil_lights: &self.stencil_lights,
            stencil_masks: &self.stencil_masks,
            stencil_reaches: &self.stencil_reaches,
            capsules: &self.capsules,
            live_lights: &self.live_lights,
            moving: &self.moving,
            palette: &self.palette,
            morph_weights: &self.morph_weights,
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
                ScreenEvent::CenterPrint(text) => self.game_screen.center_print(text),
                ScreenEvent::Command(text) => self.commands.push(text),
                ScreenEvent::Damage { armor, blood, from } => self.hurt(armor, blood, from),
                ScreenEvent::Camera { camera, blend } => self.change_camera(&camera, blend),
                ScreenEvent::Intermission(kind) => {
                    self.game_screen
                        .set_intermission(Intermission::from_number(kind));
                }
                ScreenEvent::LevelTitle(text) => {
                    self.console_view.print(&text);
                    self.game_screen.set_level_title(text);
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
        self.view_tint.hurt(armor, blood);
        let Some(client) = &self.client else {
            return;
        };
        let count = (f32::from(blood) * 0.5 + f32::from(armor) * 0.5).max(10.0);
        let direction = (from - client.predicted().origin).normalized();
        let [forward, right, _up] = view_axes(self.look.yaw() as f32, self.look.pitch() as f32);
        self.kick
            .hit(count, direction.dot(right), direction.dot(forward));
    }

    /// Changes to the camera called `camera` in text.
    pub(crate) fn change_camera(&mut self, camera: &str, blend: f32) {
        if let Some(kind) = CameraKind::parse(camera) {
            let order = CameraOrder { kind, blend };
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a camera's start is near enough"
            )]
            let time = self.map_time as f32;
            self.cameras.order(&order, self.map.as_deref(), time);
        }
    }

    /// What's at the eye, for the colour of the view.
    pub(super) fn contents_at(&self, eye: Vec3) -> Contents {
        self.map.as_ref().map_or(Contents::Empty, |map| {
            let leaf = map.tree.leaf(map.tree.leaf_at(eye));
            map.liquid_or(eye, leaf.map_or(Contents::Empty, |leaf| leaf.contents))
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
        let seconds = (time - self.effects_time).max(0.0);
        self.effects_time = time;
        self.effects.update(time);
        let client = self.client.as_ref();
        let moved = client.map_or_else(Vec::new, |client| client.draw_instances(1.0));
        let place = |follows: u64| match followed_model(follows) {
            Some(model) => moved
                .iter()
                .find(|instance| instance.model == model)
                .map(|instance| instance.origin),
            None => client.and_then(|client| client.entity_origin(follows)),
        };
        let eye = self.last_eye;
        let authored = self.effects.authored_mut();
        authored.set_eye(eye);
        authored.set_budget(self.effect_budget);
        let hit = |from: Vec3, to: Vec3| client.and_then(|client| client.line_hit(from, to));
        let world = EffectWorld {
            place: &place,
            hit: &hit,
        };
        self.effects.authored_mut().update(seconds, &world);
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

    /// How bright it is at `at`, for what takes light from the level.
    fn light_at(&self, at: Vec3) -> [f32; 3] {
        let probes = self.map.as_ref().and_then(|map| map.probes.as_ref());
        let sample = probes.and_then(|probes| probes.sample(at));
        let level = sample.map_or(1.0, |sample| sample.brightness / PROBE_FULL);
        [level; 3]
    }

    /// The particles to draw, in their colours, far to near, as what covers has to go over what's
    /// behind it, and one blend does for all of them.
    pub(super) fn particle_draws(&self, eye: Vec3) -> Vec<ParticleDraw> {
        let (authored, _) = self.effects.authored().draws(&|at| self.light_at(at));
        let mut out: Vec<ParticleDraw> = authored.iter().map(ParticleDraw::from).collect();
        let away = |particle: &ParticleDraw| (particle.origin - eye).length();
        out.sort_by(|a, b| away(b).total_cmp(&away(a)));
        out
    }

    /// Does what going through water does this frame, and says what's pushing it.
    fn follow_water(
        &mut self,
        drawn: &[DrawnEntity],
        (player, eye): (u64, Vec3),
        seconds: f32,
    ) -> Vec<WaterPush> {
        let impacts = self.effects.take_impacts();
        let Some(map) = self.map.clone() else {
            return Vec::new();
        };
        let mut movers: Vec<(u64, Vec3)> = drawn
            .iter()
            .map(|entity| (entity.state.id, entity.state.origin))
            .collect();
        if !movers.iter().any(|(id, _)| *id == player) {
            movers.push((player, eye));
        }
        let events = self
            .water_play
            .follow(&map, &movers, &impacts, (eye, seconds));
        for (effect, place) in events.effects {
            self.effects
                .authored_mut()
                .start(effect, place, None, false);
        }
        for (sound, place) in events.sounds {
            if let Some(name) = map.water_sounds.get(sound) {
                self.effects.sound(name, place);
            }
        }
        events.pushes
    }

    /// Keeps the effect of what the eye's in going round it, and stops the last one on leaving.
    fn follow_view_effect(&mut self, wanted: u32, origin: Vec3) {
        let authored = self.effects.authored_mut();
        if self.view_effect != wanted {
            authored.stop(VIEW_EFFECT_HANDLE, VIEW_EFFECT_FADE);
            self.view_effect = wanted;
            if wanted != 0 {
                let own = (VIEW_EFFECT_HANDLE, EffectSettings::default());
                authored.start_as(wanted, origin, None, true, own);
            }
        }
        authored.move_effect(VIEW_EFFECT_HANDLE, origin);
    }

    /// Follows the view from zone to zone, and does what going into one says.
    fn follow_zones(&mut self, origin: Vec3, seconds: f32) {
        let Some(map) = self.map.clone() else {
            return;
        };
        let Some(change) = self.zones.update(&map, origin, seconds) else {
            return;
        };
        if let Some(title) = change.title {
            if self.theme.has_fonts() {
                self.zone_title = Some((title, self.map_time));
            } else {
                self.game_screen.center_print(title);
            }
        }
        if let Some(Some(music)) = change.music {
            self.play_named_music(&music);
        }
        if let Some(sound) = change.sound {
            let clip =
                sound.and_then(|name| self.sounds.clip(&name, &self.filesystem, &mut self.audio));
            self.audio.set_zone_sound(clip);
        }
    }

    /// The batches of particles the GPU works out.
    pub(super) fn gpu_particles(&self) -> Vec<GpuParticles> {
        let emitters = self.effects.authored().gpu_emitters();
        emitters.iter().map(GpuParticles::from).collect()
    }

    /// What compute runs of effects this frame.
    pub(super) fn particle_sims(&self) -> Vec<ParticleSim> {
        self.effects
            .authored()
            .sims()
            .iter()
            .map(|(sim, emitter)| {
                // One light for all of it, as there's no asking for each of so many.
                let light = if emitter.lit {
                    self.light_at(sim.origin)
                } else {
                    [1.0; 3]
                };
                ParticleSim::of(sim, emitter, light)
            })
            .collect()
    }

    /// The lights among effects.
    pub(super) fn effect_lights(&self) -> Vec<LiveLight> {
        let (_, lights) = self.effects.authored().draws(&|_| [1.0; 3]);
        lights
            .iter()
            .map(|light| LiveLight {
                origin: light.origin,
                radius: light.reach,
                color: light.color,
                ..LiveLight::default()
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
        let online = self.is_online();
        let mut shot = CameraShot {
            origin: Vec3::ZERO,
            yaw: self.look.yaw() as f32,
            pitch: self.look.pitch() as f32,
            roll: 0.0,
            first_person: true,
        };
        let mut capsules = Vec::new();
        let mut moving = Vec::new();
        let mut water_pushes = Vec::new();
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
            let mut punch = client.punch(fraction);
            let (kick_pitch, kick_roll) = self.kick.update(frame_seconds);
            let right = self.listener.right;
            let mut roll = view_roll(state.velocity.dot(right)) + kick_roll;
            punch.x += kick_pitch;
            if state.dead {
                roll = dead_roll();
            }
            if frozen {
                roll = 0.0;
            }
            let subject = CameraSubject {
                eye: origin,
                yaw: self.look.yaw() as f32 + punch.y,
                pitch: self.look.pitch() as f32 + punch.x,
                roll,
                time,
                seconds: frame_seconds,
            };
            shot = self.cameras.shot(&subject, &|start, way, most| {
                client.room_along(start, way, most)
            });
            client.show_self(!shot.first_person);
            origin = shot.origin;
            let (sin, cos) = f64::from(shot.yaw).to_radians().sin_cos();
            self.listener = Listener {
                origin,
                right: Vec3::new(sin as f32, -cos as f32, 0.0),
            };
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
            let player = EffectSource {
                id: client.player_entity(),
                origin: body,
                angles: Vec3::new(self.look.pitch() as f32, self.look.yaw() as f32, 0.0),
                effects: client.take_player_effects(),
                model_flags: 0,
            };
            lights = self.update_effects(time + leftover, &drawn, player);
            water_pushes = self.follow_water(&drawn, (player.id, origin), frame_seconds);
            moving = self.map.as_ref().map_or_else(Vec::new, |map| {
                live_lights(map, origin, time, self.live_light_budget)
            });
            moving.extend(self.effect_lights());
            if let Some(map) = &self.map {
                let (flames, sparks) = self.flames.update(map, origin, time, frame_seconds);
                lights.extend(flames);
                for spark in sparks {
                    self.effects.spark(spark);
                }
            }
            if let Some(map) = self.map.clone() {
                // Models take moving lights like any other dynamic light.
                let mut lighting_up = lights.clone();
                lighting_up.extend(moving.iter().map(model_light));
                let lighting = Lighting {
                    map: &map,
                    styles: &styles,
                    lights: &lighting_up,
                    time,
                    eye: self.last_eye,
                    height_scale: height_scale(self.fov),
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
                self.effect_meshes.draw(
                    &map,
                    &self.effects.authored().mesh_draws(),
                    (origin, height_scale(self.fov), &|at| {
                        (map.light_at(at, &styles) / 255.0).clamp(0.0, 2.0)
                    }),
                    (&mut lists.palette, &mut lists.moving),
                );
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
                capsules = capsule_draws(&map, body, origin, &lists.shades, &instances);
                if !frozen && shot.first_person {
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
        let look = self
            .map
            .as_ref()
            .and_then(|map| map.look_at(origin, contents));
        self.view_tint
            .set_surroundings(look.map_or([0.0; 4], |look| look.tint));
        self.follow_view_effect(look.map_or(0, |look| look.view_effect), origin);
        self.view_tint.pass(fade);
        self.last_eye = origin;
        let view = View {
            origin,
            yaw: shot.yaw,
            pitch: shot.pitch,
            roll: shot.roll,
            fov: self.fov,
            time: self.map_time as f32,
            styles,
            gamma: self.gamma,
            light_shift: self.zones.light().map(|share| share - 1.0),
        };
        self.follow_zones(origin, frame_seconds);
        // A device that's failed says so when it draws. Without this, far
        // colours stay, which is safe.
        if let (Some(streamer), Some(renderer)) = (&mut self.tile_streamer, &mut self.renderer)
            && streamer.update(view.origin, renderer).is_err()
        {
            self.tile_streamer = None;
        }
        Picture {
            view,
            instances,
            entities: lists.entities,
            sprites: lists.sprites,
            moving: lists.moving,
            palette: lists.palette,
            morph_weights: lists.morph_weights,
            particles: self.particle_draws(view.origin),
            gpu_particles: self.gpu_particles(),
            particle_sims: self.particle_sims(),
            water_pushes,
            lights,
            quads: Vec::new(),
            stencil_lights: self.gates.lights().to_vec(),
            stencil_masks: self.gates.masks().to_vec(),
            stencil_reaches: self.gates.reaches().to_vec(),
            capsules,
            live_lights: moving,
        }
    }
}
