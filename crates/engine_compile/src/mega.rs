//! The mega texture: one image holding unique colour for a map's surfaces, so
//! what's painted and layered costs one read when drawn (proposal 0087).

use std::sync::Arc;

use engine_core::math::Vec3;
use engine_map::{
    Face, Map, Material, MegaMaps, MipLevel, PAINT_MASK, SlotImage, TexelAxis, Texture,
    TextureKind, mip_chain,
};

use crate::source::PaintStroke;

/// The side of the image.
pub(crate) const MEGA_SIZE: u32 = 2048;

/// Texels left round each surface, so filtering doesn't reach its neighbours.
const PADDING: i32 = 2;

/// The most specular or emissive strength the images can hold. A material's
/// own strength goes into its texels, as faces of many materials share one.
const MEGA_STRENGTH: f32 = 4.0;

/// The most gloss the image can hold, for the same reason.
const MEGA_GLOSS: f32 = 256.0;

/// A normal that leaves a surface as it is.
const FLAT_NORMAL: [u8; 4] = [128, 128, 255, 255];

/// Where a face went in the image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MegaPlace {
    /// The face.
    pub(crate) face: u32,
    /// How far its texture coordinates moved.
    pub(crate) shift: [i32; 2],
}

/// Fills rows of the image from the top.
struct Shelves {
    /// Where the next surface goes along this row.
    x: i32,
    /// This row's top.
    y: i32,
    /// This row's height so far.
    height: i32,
}

impl Shelves {
    /// Finds room for a surface of `size`, or `None` if there's none left.
    fn place(&mut self, size: [i32; 2]) -> Option<[i32; 2]> {
        let side = i32::try_from(MEGA_SIZE).ok()?;
        if self.x + size[0] > side {
            self.x = 0;
            self.y += self.height;
            self.height = 0;
        }
        if self.y + size[1] > side || size[0] > side {
            return None;
        }
        let at = [self.x, self.y];
        self.x += size[0];
        self.height = self.height.max(size[1]);
        Some(at)
    }
}

/// A texel of `image` at texel coordinates that wrap.
fn texel(image: &MipLevel, s: f32, t: f32) -> [u8; 4] {
    #[expect(clippy::cast_possible_wrap, reason = "image sizes are small")]
    let (width, height) = (image.width as i32, image.height as i32);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "texel coordinates are small"
    )]
    let (x, y) = (
        (s.floor() as i32).rem_euclid(width.max(1)),
        (t.floor() as i32).rem_euclid(height.max(1)),
    );
    #[expect(clippy::cast_sign_loss, reason = "wrapped to the image")]
    let at = (y as usize * image.width as usize + x as usize) * 4;
    let mut out = [0; 4];
    if let Some(found) = image.rgba.get(at..at + 4) {
        out.copy_from_slice(found);
    }
    out
}

/// What a material's slot holds at a share of the base image.
fn slot_at(material: &Material, slot: Option<u8>, u: f32, v: f32) -> Option<[u8; 4]> {
    let number = usize::from(slot?).checked_sub(1)?;
    let image = material.slots.get(number)?.first()?;
    #[expect(clippy::cast_precision_loss, reason = "image sizes are small")]
    let (width, height) = (image.width as f32, image.height as f32);
    Some(texel(image, u * width, v * height))
}

/// `value` scaled by `share`, as a texel.
fn scaled(value: u8, share: f32) -> u8 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to a byte"
    )]
    let out = (f32::from(value) * share).round().clamp(0.0, 255.0) as u8;
    out
}

/// A surface's share of the base image at a point.
fn share_of(texture: &Texture, axes: &[TexelAxis; 2], point: Vec3) -> Option<(f32, f32)> {
    let tile = texture.tile_size()?;
    Some((
        axes[0].project(point) / tile[0],
        axes[1].project(point) / tile[1],
    ))
}

/// Writes what a surface's other maps hold at a point into the texel at `to`
/// of each image there is.
fn write_maps(maps: &mut MegaMaps, to: usize, material: &Material, u: f32, v: f32) {
    let write = |image: &mut Option<Vec<u8>>, color: Option<[u8; 4]>| {
        if let (Some(image), Some(color)) = (image, color) {
            image[to..to + 4].copy_from_slice(&color);
        }
    };
    let strength = material.specular_strength / MEGA_STRENGTH;
    let gloss = material.gloss / MEGA_GLOSS;
    let glow = material.emissive_strength / MEGA_STRENGTH;
    let shine = |[r, g, b, a]: [u8; 4]| {
        [
            scaled(r, strength),
            scaled(g, strength),
            scaled(b, strength),
            scaled(a, gloss),
        ]
    };
    let lit = |[r, g, b, a]: [u8; 4]| [scaled(r, glow), scaled(g, glow), scaled(b, glow), a];
    write(&mut maps.normal, slot_at(material, material.normal, u, v));
    write(
        &mut maps.specular,
        slot_at(material, material.specular, u, v).map(shine),
    );
    write(
        &mut maps.emissive,
        slot_at(material, material.emissive, u, v).map(lit),
    );
}

/// The images a mega texture of these materials needs, empty to start.
fn empty_maps<'a>(materials: impl Iterator<Item = &'a Material> + Clone) -> MegaMaps {
    let texels = MEGA_SIZE as usize * MEGA_SIZE as usize;
    let image = |pick: fn(&Material) -> bool, color: [u8; 4]| {
        materials.clone().any(pick).then(|| color.repeat(texels))
    };
    MegaMaps {
        normal: image(|material| material.normal.is_some(), FLAT_NORMAL),
        specular: image(|material| material.specular.is_some(), [0; 4]),
        emissive: image(|material| material.emissive.is_some(), [0; 4]),
    }
}

/// The colour of a surface at a point, with its layers mixed in.
fn color_at(
    texture: &Texture,
    material: &Material,
    axes: &[TexelAxis; 2],
    point: Vec3,
    paint: &dyn Fn(Vec3) -> [u8; 4],
) -> [u8; 4] {
    let (s, t) = (axes[0].project(point), axes[1].project(point));
    let Some(base) = texture.mips.first() else {
        return [0, 0, 0, 255];
    };
    // Layers repeat at their own size, over the same share of the base.
    let Some(tile) = texture.tile_size() else {
        return [0, 0, 0, 255];
    };
    let (u, v) = (s / tile[0], t / tile[1]);
    #[expect(clippy::cast_precision_loss, reason = "image sizes are small")]
    let mut color = texel(base, u * base.width as f32, v * base.height as f32);
    let image_of = |slot: u8| {
        material
            .slots
            .get(usize::from(slot).wrapping_sub(1))
            .and_then(|image| image.first())
    };
    let sample = |image: &MipLevel| {
        #[expect(clippy::cast_precision_loss, reason = "image sizes are small")]
        let (width, height) = (image.width as f32, image.height as f32);
        texel(image, u * width, v * height)
    };
    for (index, layer) in material.layers.iter().enumerate() {
        let Some(over) = image_of(layer.base).map(sample) else {
            continue;
        };
        let share = if layer.mask == PAINT_MASK {
            paint(point)[index.min(3)]
        } else {
            image_of(layer.mask).map_or(0, |mask| sample(mask)[0])
        };
        let share = u32::from(share);
        for part in 0..3 {
            let mixed =
                (u32::from(color[part]) * (255 - share) + u32::from(over[part]) * share) / 255;
            color[part] = u8::try_from(mixed).unwrap_or(u8::MAX);
        }
    }
    color
}

/// The point on a face's plane at texture coordinates `s` and `t`.
fn point_at(origin: Vec3, across: Vec3, down: Vec3, s: f32, t: f32) -> Vec3 {
    origin + across * s + down * t
}

/// What a face's texel axes step through in the world, as a texel along each,
/// with the point at texel 0.
fn texel_frame(map: &Map, face: &Face) -> Option<(Vec3, Vec3, Vec3)> {
    let points = map.face_points(face);
    let first = *points.first()?;
    let second = *points.get(1)?;
    let third = *points.get(2)?;
    let (a, b) = (second - first, third - first);
    let project = |axis: &TexelAxis, edge: Vec3| axis.axis.dot(edge);
    let (a_s, a_t) = (project(&face.axes[0], a), project(&face.axes[1], a));
    let (b_s, b_t) = (project(&face.axes[0], b), project(&face.axes[1], b));
    let determinant = a_s * b_t - a_t * b_s;
    if determinant.abs() < 1e-6 {
        return None;
    }
    let across = (a * b_t - b * a_t) * (1.0 / determinant);
    let down = (b * a_s - a * b_s) * (1.0 / determinant);
    let (s, t) = (face.axes[0].project(first), face.axes[1].project(first));
    Some((first - across * s - down * t, across, down))
}

/// Gathers the surfaces of materials marked `mega` into one image, with their
/// layers and paint mixed in, and their other maps in images of their own. Each
/// keeps its light. Returns what moved, and the images, or `None` if nothing
/// asks for one.
pub(crate) fn bake_mega(
    map: &mut Map,
    strokes: &[PaintStroke],
    paint_at: &dyn Fn(Vec3, &[PaintStroke]) -> [u8; 4],
) -> Option<(Vec<MegaPlace>, Vec<u8>, MegaMaps)> {
    let wanted = |face: &Face| {
        map.textures
            .get(face.texture as usize)
            .and_then(|texture| texture.material.as_ref())
            .is_some_and(|material| material.mega)
    };
    if !map.faces.iter().any(wanted) {
        return None;
    }
    let side = MEGA_SIZE as usize;
    let mut rgba = vec![0; side * side * 4];
    let marked = map
        .textures
        .iter()
        .filter_map(|texture| texture.material.as_deref())
        .filter(|material| material.mega);
    let mut maps = empty_maps(marked);
    let mut shelves = Shelves {
        x: 0,
        y: 0,
        height: 0,
    };
    let mut places = Vec::new();
    let paint = |point: Vec3| paint_at(point, strokes);
    for (number, face) in (0_u32..).zip(&map.faces) {
        if !wanted(face) {
            continue;
        }
        let Some(texture) = map.textures.get(face.texture as usize) else {
            continue;
        };
        let Some(material) = texture.material.as_deref() else {
            continue;
        };
        let Some((origin, across, down)) = texel_frame(map, face) else {
            continue;
        };
        let (mut low, mut high) = ([f32::MAX; 2], [f32::MIN; 2]);
        for point in map.face_points(face) {
            for axis in 0..2 {
                let value = face.axes[axis].project(*point);
                low[axis] = low[axis].min(value);
                high[axis] = high[axis].max(value);
            }
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "texel coordinates are small"
        )]
        let (low, high) = (
            low.map(|value| value.floor() as i32),
            high.map(|value| value.ceil() as i32),
        );
        let size = [
            high[0] - low[0] + PADDING * 2,
            high[1] - low[1] + PADDING * 2,
        ];
        let Some(at) = shelves.place(size) else {
            continue;
        };
        for y in 0..size[1] {
            for x in 0..size[0] {
                #[expect(clippy::cast_precision_loss, reason = "texel coordinates are small")]
                let (s, t) = (
                    (low[0] + x - PADDING) as f32 + 0.5,
                    (low[1] + y - PADDING) as f32 + 0.5,
                );
                let point = point_at(origin, across, down, s, t);
                let color = color_at(texture, material, &face.axes, point, &paint);
                #[expect(clippy::cast_sign_loss, reason = "inside the image")]
                let to = ((at[1] + y) as usize * side + (at[0] + x) as usize) * 4;
                rgba[to..to + 4].copy_from_slice(&color);
                if let Some((u, v)) = share_of(texture, &face.axes, point) {
                    write_maps(&mut maps, to, material, u, v);
                }
            }
        }
        places.push(MegaPlace {
            face: number,
            shift: [at[0] + PADDING - low[0], at[1] + PADDING - low[1]],
        });
    }
    apply_mega(map, &places, rgba.clone(), &maps);
    // A face's light follows its texture coordinates, and a bake holds it moved,
    // so this is only done here, and not each time a bake is read.
    for place in &places {
        let light = map.faces.get_mut(place.face as usize);
        let shifted = light.filter(|face| face.light_axes.is_none());
        if let Some(light) = shifted.and_then(|face| face.light.as_mut()) {
            light.texel_origin[0] += place.shift[0];
            light.texel_origin[1] += place.shift[1];
        }
    }
    Some((places, rgba, maps))
}

/// The material of a mega texture with `maps`.
fn mega_material(maps: &MegaMaps) -> Material {
    let mut material = Material::default();
    let mut slot = |image: &Option<Vec<u8>>| {
        let rgba = image.as_ref()?;
        let mips = mip_chain(MEGA_SIZE, MEGA_SIZE, rgba.clone());
        material_slots_push(&mut material.slots, mips)
    };
    let (normal, specular, emissive) = (
        slot(&maps.normal),
        slot(&maps.specular),
        slot(&maps.emissive),
    );
    material.normal = normal;
    material.specular = specular;
    material.emissive = emissive;
    if specular.is_some() {
        material.specular_strength = MEGA_STRENGTH;
        material.gloss = MEGA_GLOSS;
    }
    if emissive.is_some() {
        material.emissive_strength = MEGA_STRENGTH;
    }
    material
}

/// Adds an image to `slots`, and returns its slot number.
fn material_slots_push(slots: &mut Vec<SlotImage>, mips: Vec<MipLevel>) -> Option<u8> {
    slots.push(Arc::new(mips));
    u8::try_from(slots.len()).ok()
}

/// Points the faces in `places` at a mega texture of `rgba`.
pub(crate) fn apply_mega(map: &mut Map, places: &[MegaPlace], rgba: Vec<u8>, maps: &MegaMaps) {
    let number = u32::try_from(map.textures.len()).unwrap_or(0);
    map.textures.push(Texture {
        name: "*mega".to_owned(),
        kind: TextureKind::Solid,
        mips: mip_chain(MEGA_SIZE, MEGA_SIZE, rgba),
        animation: None,
        alternate: None,
        material: Some(Arc::new(mega_material(maps))),
    });
    for place in places {
        let Some(face) = map.faces.get_mut(place.face as usize) else {
            continue;
        };
        face.texture = number;
        for (axis, shift) in face.axes.iter_mut().zip(place.shift) {
            #[expect(clippy::cast_precision_loss, reason = "texel coordinates are small")]
            let moved = shift as f32;
            axis.offset += moved;
        }
    }
}
