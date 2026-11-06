//! How much light falls on a point, from `R_LightPoint` in `r_light.c`.

use engine_core::math::Vec3;

use crate::map::{Face, Map};
use crate::tile_light::RESIDENT;
use crate::tree::NodeChild;

/// How far down a point looks for the floor it takes its light from.
const LOOK_DOWN: f32 = 2048.0;

impl Map {
    /// The light on the floor under `point`, from 0 to 255 or so, with each
    /// style at the brightness in `styles`.
    #[must_use]
    pub fn light_at(&self, point: Vec3, styles: &[f32]) -> f32 {
        if let Some(sample) = self.probes.as_ref().and_then(|probes| probes.sample(point)) {
            return sample.brightness;
        }
        if self.light.is_empty() {
            return 255.0;
        }
        let end = point - Vec3::new(0.0, 0.0, LOOK_DOWN);
        self.light_below(NodeChild::Node(0), point, end, styles)
            .unwrap_or(0.0)
    }

    /// `RecursiveLightPoint`: the light where the line first meets a face.
    fn light_below(&self, child: NodeChild, start: Vec3, end: Vec3, styles: &[f32]) -> Option<f32> {
        let NodeChild::Node(index) = child else {
            return None;
        };
        let node = self.tree.node(index)?;
        let plane = self.tree.plane(node.plane)?;
        let front = plane.normal.dot(start) - plane.distance;
        let back = plane.normal.dot(end) - plane.distance;
        let side = usize::from(front < 0.0);
        if (back < 0.0) == (front < 0.0) {
            return self.light_below(node.children[side], start, end, styles);
        }
        let mid = start.lerp(end, front / (front - back));
        if let Some(light) = self.light_below(node.children[side], start, mid, styles) {
            return Some(light);
        }
        let usize_of = |value: u32| usize::try_from(value).unwrap_or(usize::MAX);
        let &(first, count) = self.tree.node_faces.get(usize_of(index))?;
        for offset in 0..count {
            let face = self.faces.get(usize_of(first + offset));
            if let Some(light) = face.and_then(|face| self.light_on(face, mid, styles)) {
                return Some(light);
            }
        }
        self.light_below(node.children[1 - side], mid, end, styles)
    }

    /// The light on `face` at `point`, if it's lit and covers that point.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a texel is checked to be in range before it's turned into a sample"
    )]
    fn light_on(&self, face: &Face, point: Vec3, styles: &[f32]) -> Option<f32> {
        let light = face.light.as_ref()?;
        let spacing = light.texels_per_sample.max(1);
        let reach = |axis: usize, samples: u32| {
            let texel = face.light_space()[axis].project(point) - light.texel_origin[axis] as f32;
            let extent = (samples.saturating_sub(1) * spacing) as f32;
            (texel >= 0.0 && texel <= extent).then_some(texel as u32 / spacing)
        };
        let across = reach(0, light.width)?;
        let down = reach(1, light.height)?;
        let held = (light.tile != RESIDENT)
            .then(|| self.light_tiles.as_ref()?.held(light.tile))
            .flatten();
        if light.tile != RESIDENT && held.is_none() {
            let [red, green, blue] = light.average.map(f32::from);
            return Some((red + green + blue) / 3.0);
        }
        let samples = held.as_ref().map_or(&self.light, |tile| &tile.light);
        let layer = light.width * light.height;
        let mut total = 0.0;
        for (number, &style) in (0_u32..).zip(&light.styles) {
            if style == Face::NO_STYLE {
                break;
            }
            let at = light.first_sample + number * layer + down * light.width + across;
            let sample = samples.get(usize::try_from(at).ok()?)?;
            let sum = u16::from(sample[0]) + u16::from(sample[1]) + u16::from(sample[2]);
            let brightness = styles.get(usize::from(style)).copied().unwrap_or(1.0);
            total += f32::from(sum) / 3.0 * brightness;
        }
        Some(total)
    }
}
