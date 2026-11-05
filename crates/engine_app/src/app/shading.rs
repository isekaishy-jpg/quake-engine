//! What the view asks collision and the bake about light: which of a
//! stencil's boxes are open, and who shades what (proposal 0076).

use engine_client::Client;
use engine_core::entity::ModelInstance;
use engine_core::math::Vec3;
use engine_map::{Map, OccluderSphere, RayStencil, STENCIL_GRID};
use engine_render::{CapsuleDraw, LightDraw, LiveLight, MAX_LIVE_LIGHTS};

/// A stencil's occlusion grid.
pub(super) type StencilMask = [f32; STENCIL_GRID * STENCIL_GRID];

/// How many box tests a frame gets. A window with many boxes takes a few
/// frames to go round, which its fade hides.
const TESTS_PER_FRAME: usize = 24;

/// How many seconds a box takes to fade.
const FADE_SECONDS: f32 = 0.2;

/// How far past a stencil a test starts, so the stencil's own wall isn't hit.
const TEST_START: f32 = 2.0;

/// How far a sun's test looks for what's in the way.
const SUN_REACH: f32 = 4096.0;

/// How wide the player's shade is.
const CAPSULE_RADIUS: f32 = 22.0;

/// How many characters shade the light, nearest first.
const MAX_CAPSULES: usize = 16;

/// Which way light comes from when nothing says otherwise.
const OVERHEAD: Vec3 = Vec3::new(0.0, 0.0, 1.0);

/// How near a model shows its parts' shade, rather than the whole of it.
const PART_DETAIL_REACH: f32 = 512.0;

/// How dark a character's shade is at its strongest.
const CAPSULE_STRENGTH: f32 = 0.7;

/// The moving lights near `eye` at `time`, nearest first.
pub(super) fn live_lights(map: &Map, eye: Vec3, time: f32, most: usize) -> Vec<LiveLight> {
    let mut lights: Vec<LiveLight> = (0_u32..)
        .zip(&map.live_lights)
        .map(|(id, source)| LiveLight {
            id,
            origin: source.position(time),
            radius: source.radius,
            color: source.color,
            spot: source.spot,
            spot_cosine: source.spot_cosine,
            shadows: source.shadows,
        })
        .collect();
    lights.sort_by(|a, b| {
        let reach = |light: &LiveLight| (light.origin - eye).length() - light.radius;
        reach(a).total_cmp(&reach(b))
    });
    lights.truncate(most.min(MAX_LIVE_LIGHTS));
    lights
}

/// A moving light, as what lights models sees it.
pub(super) fn model_light(light: &LiveLight) -> LightDraw {
    let [red, green, blue] = light.color;
    LightDraw {
        origin: light.origin,
        radius: light.radius * (red + green + blue) / 3.0,
        min_light: 0.0,
    }
}

/// How many flames light a frame, nearest first.
const MAX_FLAMES: usize = 8;

/// How far off a flame is seen to spark or light anything.
const FLAME_REACH: f32 = 1200.0;

/// How strong a flame's light is. Baked light does most of the work, so this is
/// only the flicker on top of it.
const FLAME_LIGHT: f32 = 0.35;

/// The sparks owed by each of a map's flames.
#[derive(Debug, Default)]
pub(super) struct Flames {
    /// How many sparks each is owed, so slow rates add up.
    owed: Vec<f32>,
}

impl Flames {
    /// The flickering lights near `eye`, and the sparks they give off in
    /// `seconds`, as places to start them.
    pub(super) fn update(
        &mut self,
        map: &Map,
        eye: Vec3,
        time: f32,
        seconds: f32,
    ) -> (Vec<LightDraw>, Vec<Vec3>) {
        self.owed.resize(map.emitters.len(), 0.0);
        let mut near: Vec<(usize, f32)> = map
            .emitters
            .iter()
            .enumerate()
            .map(|(index, emitter)| (index, (emitter.origin - eye).length()))
            .filter(|(_, distance)| *distance < FLAME_REACH)
            .collect();
        near.sort_by(|a, b| a.1.total_cmp(&b.1));
        near.truncate(MAX_FLAMES);
        let mut lights = Vec::with_capacity(near.len());
        let mut sparks = Vec::new();
        for (index, _) in near {
            let emitter = &map.emitters[index];
            lights.push(LightDraw {
                origin: emitter.origin,
                radius: emitter.light_radius * flicker(time, emitter.origin) * FLAME_LIGHT,
                min_light: 0.0,
            });
            self.owed[index] += emitter.sparks * seconds;
            while self.owed[index] >= 1.0 {
                self.owed[index] -= 1.0;
                sparks.push(emitter.origin);
            }
        }
        (lights, sparks)
    }
}

/// How a flame's light moves at `time`, around 1. Each flame has its own
/// rhythm, from where it stands.
fn flicker(time: f32, place: Vec3) -> f32 {
    let phase = place.x * 0.37 + place.y * 0.11;
    let slow = (time * 7.3 + phase).sin();
    let fast = (time * 17.1 + phase * 1.7).sin();
    1.0 + slow * 0.08 + fast * 0.05
}

/// Which of a map's stencil boxes are open.
#[derive(Debug, Default)]
pub(super) struct StencilGates {
    /// Whether each box should be open, by stencil.
    wanted: Vec<Vec<bool>>,
    /// How open each box is, by stencil.
    open: Vec<Vec<f32>>,
    /// Each stencil's grid.
    masks: Vec<StencilMask>,
    /// Where each stencil's light is.
    lights: Vec<Vec3>,
    /// The next box to test, counting through all of them.
    next: usize,
}

impl StencilGates {
    /// Where stencils' lights are.
    pub(super) fn lights(&self) -> &[Vec3] {
        &self.lights
    }

    /// How open stencils are.
    pub(super) fn masks(&self) -> &[StencilMask] {
        &self.masks
    }

    /// Starts again for `stencils`.
    fn reset(&mut self, stencils: &[RayStencil]) {
        self.wanted = stencils.iter().map(|s| vec![true; s.boxes.len()]).collect();
        self.open = stencils.iter().map(|s| vec![1.0; s.boxes.len()]).collect();
        self.masks = vec![[1.0; STENCIL_GRID * STENCIL_GRID]; stencils.len()];
        self.next = 0;
    }

    /// Tests some boxes against `client`'s collision, and eases all of them.
    pub(super) fn update(&mut self, map: &Map, client: &Client, time: f32, seconds: f32) {
        let stencils = &map.stencils;
        if self.open.len() != stencils.len() {
            self.reset(stencils);
        }
        self.lights = stencils.iter().map(|s| s.light_at(time)).collect();
        let total: usize = stencils.iter().map(|s| s.boxes.len()).sum();
        for _ in 0..TESTS_PER_FRAME.min(total) {
            self.next = (self.next + 1) % total;
            let mut left = self.next;
            for (index, stencil) in stencils.iter().enumerate() {
                if left < stencil.boxes.len() {
                    let area = stencil.boxes[left];
                    self.wanted[index][left] =
                        box_is_clear(stencil, area, self.lights[index], client);
                    break;
                }
                left -= stencil.boxes.len();
            }
        }
        let step = seconds / FADE_SECONDS;
        for (index, stencil) in stencils.iter().enumerate() {
            for (open, wanted) in self.open[index].iter_mut().zip(&self.wanted[index]) {
                let target = if *wanted { 1.0 } else { 0.0 };
                *open += (target - *open).clamp(-step, step);
            }
            stencil.fill_mask(&self.open[index], &mut self.masks[index]);
        }
    }
}

/// Whether light reaches a box of a stencil.
fn box_is_clear(stencil: &RayStencil, area: [f32; 4], light: Vec3, client: &Client) -> bool {
    let point = stencil.point_in(area);
    let (towards, end) = if stencil.sun {
        let towards = light.normalized();
        (towards, point + towards * SUN_REACH)
    } else {
        ((light - point).normalized(), light)
    };
    client.line_is_clear(point + towards * TEST_START, end, stencil.sun)
}

/// Where the spheres of moving models are, as a view from `eye` sees them:
/// the whole of one far off, and its parts up close.
fn model_spheres(map: &Map, eye: Vec3, instances: &[ModelInstance]) -> Vec<(Vec3, f32)> {
    let mut out = Vec::new();
    for instance in instances {
        let Some(found) = map.occluders.iter().find(|o| o.model == instance.model) else {
            continue;
        };
        let place = |sphere: &OccluderSphere| {
            let [x, y, z] = sphere.center;
            (Vec3::new(x, y, z) + instance.origin, sphere.radius)
        };
        let Some(whole) = found.spheres.first() else {
            continue;
        };
        let (middle, _) = place(whole);
        let parts = found.spheres.get(1..).unwrap_or_default();
        if parts.is_empty() || (middle - eye).length() > PART_DETAIL_REACH {
            out.push(place(whole));
        } else {
            out.extend(parts.iter().map(place));
        }
    }
    out
}

/// The bodies shading light for a view from `eye`, with `body` being the
/// player, and light coming the way the bake says.
pub(super) fn capsule_draws(
    map: &Map,
    body: Vec3,
    eye: Vec3,
    solid_boxes: &[(Vec3, Vec3)],
    instances: &[ModelInstance],
) -> Vec<CapsuleDraw> {
    let mut places: Vec<(Vec3, f32)> = solid_boxes
        .iter()
        .map(|(low, high)| ((*low + *high) * 0.5, (high.x - low.x) * 0.7))
        .collect();
    places.extend(model_spheres(map, eye, instances));
    places.sort_by(|a, b| (a.0 - eye).length().total_cmp(&(b.0 - eye).length()));
    places.truncate(MAX_CAPSULES - 1);
    places.push((body, CAPSULE_RADIUS));
    places
        .into_iter()
        .map(|(origin, radius)| {
            let sample = map.probes.as_ref().and_then(|probes| probes.sample(origin));
            let (toward_light, strength) = sample.map_or((OVERHEAD, 0.5), |sample| {
                let brightness = (sample.brightness / 96.0).min(1.0);
                // With no light strong enough to have a direction, it falls on
                // a body from all round, and a soft shade under it grounds it.
                let leans = sample.direction.length() > 0.3;
                let toward = if leans {
                    sample.direction.normalized()
                } else {
                    OVERHEAD
                };
                (toward, CAPSULE_STRENGTH * (0.4 + 0.6 * brightness))
            });
            CapsuleDraw {
                origin,
                radius,
                toward_light,
                strength,
            }
        })
        .collect()
}
