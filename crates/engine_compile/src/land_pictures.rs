//! What materials are to land: two pictures each, of one size, so that a map's can go in an array
//! and a pixel reads two pictures a layer, not four (proposal 0144).

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use engine_assets::{DirectoryArchive, FileSystem};
use engine_map::{LandPicture, MipLevel, SlotImage, Texture, mip_chain};

use crate::error::CompileError;
use crate::material::MaterialLibrary;
use crate::terrain::read_terrain_data;
use crate::texture_pack::{Role, pack_picture, read_packed};

/// The widest a picture of land gets, as the texture standard has terrain sets.
const WIDEST: u32 = 1024;

/// Where the compiled form of a material's colour for land is kept.
pub(crate) fn packed_color(material: &str) -> String {
    format!("compiled/land/{material}_colour.etex")
}

/// Where the compiled form of a material's surface for land is kept.
pub(crate) fn packed_surface(material: &str) -> String {
    format!("compiled/land/{material}_surface.etex")
}

/// How wide a picture for `texture` wants to be, as a power of two.
fn side_of(texture: &Texture) -> u32 {
    let width = texture
        .mips
        .first()
        .map_or(1, |top| top.width.max(top.height));
    width.next_power_of_two().min(WIDEST)
}

/// `level` at `side` by `side`, wrapped at its edges, as it repeats.
fn scaled(level: &MipLevel, side: u32) -> Vec<u8> {
    scaled_to(level, side, side)
}

/// `level` at `across` by `down`, wrapped at its edges, as it repeats.
fn scaled_to(level: &MipLevel, across: u32, down: u32) -> Vec<u8> {
    if level.width == across && level.height == down {
        return level.rgba.clone();
    }
    let (width, height) = (level.width.max(1) as usize, level.height.max(1) as usize);
    let (across, down) = (across as usize, down as usize);
    let mut out = Vec::with_capacity(across * down * 4);
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "picture sizes are small"
    )]
    let place = |at: usize, side: usize, of: usize| {
        let spot = (at as f32 + 0.5) / side as f32 * of as f32 - 0.5;
        let low = spot.floor();
        let first = (low + of as f32) as usize % of;
        (first, (first + 1) % of, spot - low)
    };
    for y in 0..down {
        let (top, bottom, lower_share) = place(y, down, height);
        for x in 0..across {
            let (left, right, right_share) = place(x, across, width);
            for channel in 0..4 {
                let read = |x: usize, y: usize| {
                    f32::from(
                        level
                            .rgba
                            .get((y * width + x) * 4 + channel)
                            .copied()
                            .unwrap_or(0),
                    )
                };
                let upper = read(left, top) + (read(right, top) - read(left, top)) * right_share;
                let lower =
                    read(left, bottom) + (read(right, bottom) - read(left, bottom)) * right_share;
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "between two bytes"
                )]
                out.push((upper + (lower - upper) * lower_share).round() as u8);
            }
        }
    }
    out
}

/// The first level of slot `number` of `texture`, if it has that slot.
fn slot_top(texture: &Texture, number: Option<u8>) -> Option<&MipLevel> {
    let material = texture.material.as_ref()?;
    let slot = usize::from(number?).checked_sub(1)?;
    material.slots.get(slot)?.first()
}

/// How bright the colour in `pixel` is, from 0 to 1.
fn brightness(pixel: &[u8]) -> f32 {
    let [red, green, blue] = [pixel[0], pixel[1], pixel[2]].map(|part| f32::from(part) / 255.0);
    red * 0.3 + green * 0.6 + blue * 0.1
}

/// A byte from a number from 0 to 1.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to a byte"
)]
fn byte(value: f32) -> u8 {
    (value * 255.0).round().clamp(0.0, 255.0) as u8
}

/// What a surface picture's made from, each in plain pixels.
#[derive(Clone, Copy, Default)]
pub(crate) struct SurfaceMaps<'a> {
    /// A normal map.
    pub normal: Option<&'a MipLevel>,
    /// A height map.
    pub height: Option<&'a MipLevel>,
    /// A specular map.
    pub specular: Option<&'a MipLevel>,
    /// What shine's multiplied by.
    pub strength: f32,
}

/// A surface picture, the size of `color`: bumps across and down, how high it stands, and how much
/// it shines. With no height, its own light and dark stand in, as they do on a material with
/// layers.
pub(crate) fn surface_pixels(color: &MipLevel, maps: SurfaceMaps<'_>) -> Vec<u8> {
    let (width, height) = (color.width, color.height);
    let at_size = |map: Option<&MipLevel>| map.map(|map| scaled_to(map, width, height));
    let (normal, high, shine) = (
        at_size(maps.normal),
        at_size(maps.height),
        at_size(maps.specular),
    );
    let mut out = Vec::with_capacity(color.rgba.len());
    for (at, pixel) in color.rgba.as_chunks::<4>().0.iter().enumerate() {
        let of = |picture: &Option<Vec<u8>>| {
            picture
                .as_ref()
                .and_then(|bytes| bytes.get(at * 4..at * 4 + 4))
                .map(<[u8]>::to_vec)
        };
        let bump = of(&normal).map_or([128, 128], |pixel| [pixel[0], pixel[1]]);
        let stands = of(&high).map_or_else(|| byte(brightness(pixel).sqrt()), |pixel| pixel[0]);
        let bright = of(&shine).map_or(0, |pixel| byte(brightness(&pixel) * maps.strength));
        out.extend([bump[0], bump[1], stands, bright]);
    }
    out
}

/// The surface of `texture`, whose colour at `side` is `color`. A material has its normal and
/// specular in plain pixels here.
fn surface_of(texture: &Texture, color: &[u8], side: u32) -> Vec<u8> {
    let material = texture.material.as_ref();
    let maps = SurfaceMaps {
        normal: slot_top(texture, material.and_then(|m| m.normal)),
        height: slot_top(texture, material.and_then(|m| m.height)),
        specular: slot_top(texture, material.and_then(|m| m.specular)),
        strength: material.map_or(1.0, |m| m.specular_strength),
    };
    let top = MipLevel {
        width: side,
        height: side,
        rgba: color.to_vec(),
        form: engine_map::PixelForm::Rgba,
    };
    surface_pixels(&top, maps)
}

/// `texture`, which is in plain pixels, as land has it at `side`.
fn plain_picture(material: &str, texture: &Texture, side: u32) -> LandPicture {
    let color = texture.mips.first().map_or_else(
        || vec![255; (side * side * 4) as usize],
        |top| scaled(top, side),
    );
    let surface = surface_of(texture, &color, side);
    LandPicture {
        material: material.to_owned(),
        color: Arc::new(mip_chain(side, side, color)),
        surface: Arc::new(mip_chain(side, side, surface)),
        gloss: gloss_of(texture),
    }
}

/// How tight `texture`'s shine is.
fn gloss_of(texture: &Texture) -> f32 {
    texture
        .material
        .as_ref()
        .map_or(32.0, |material| material.gloss)
}

/// `textures`, in plain pixels, as land has them, all at the size of the largest.
pub(crate) fn plain_pictures(materials: &[String], textures: &[Texture]) -> Vec<LandPicture> {
    let side = textures.iter().map(side_of).max().unwrap_or(1);
    materials
        .iter()
        .zip(textures)
        .map(|(material, texture)| plain_picture(material, texture, side))
        .collect()
}

/// The pictures compiled for `material`, if they're there and not behind `hash`.
fn packed_pair(
    filesystem: &FileSystem,
    material: &str,
    hash: u64,
) -> Option<(SlotImage, SlotImage)> {
    let read = |path: String| {
        let (known, mips) = read_packed(&filesystem.read(&path).ok()?)?;
        (known == hash).then(|| Arc::new(mips))
    };
    Some((
        read(packed_color(material))?,
        read(packed_surface(material))?,
    ))
}

/// The pictures compiled for `materials`, if each is there and current and they all match, as an
/// array needs them to. `hash` says what each one's made from, and `gloss` how tight its shine is.
pub(crate) fn packed_pictures(
    filesystem: &FileSystem,
    materials: &[String],
    hash: &mut dyn FnMut(&str) -> Option<u64>,
    gloss: &mut dyn FnMut(&str) -> f32,
) -> Option<Vec<LandPicture>> {
    let mut out = Vec::with_capacity(materials.len());
    for material in materials {
        let (color, surface) = packed_pair(filesystem, material, hash(material)?)?;
        out.push(LandPicture {
            material: material.clone(),
            color,
            surface,
            gloss: gloss(material),
        });
    }
    let size = |picture: &LandPicture| picture.color.first().map(|top| top.width);
    let first = out.first().and_then(size);
    out.iter()
        .all(|picture| size(picture) == first)
        .then_some(out)
}

/// Every material the land under `root` is painted with.
fn painted_materials(root: &Path) -> Result<BTreeSet<String>, CompileError> {
    let bad = |error: std::io::Error| CompileError::Bad(format!("terrain: {error}"));
    let mut names = BTreeSet::new();
    let Ok(entries) = std::fs::read_dir(root.join("terrain")) else {
        return Ok(names);
    };
    for entry in entries {
        let path = entry.map_err(bad)?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "eterrain")
        {
            let data = read_terrain_data(&std::fs::read(&path).map_err(bad)?)?;
            names.extend(data.layers.into_iter().map(|layer| layer.material));
        }
    }
    Ok(names)
}

/// Compiles what land under `root` is painted with. Returns how many materials it compiled, and
/// how many there are.
///
/// # Errors
///
/// Returns an error if a material or a picture can't be read, or a file can't be written.
pub fn pack_land(root: &Path) -> Result<(usize, usize), CompileError> {
    let materials = painted_materials(root)?;
    let mut filesystem = FileSystem::new();
    filesystem.mount(Box::new(DirectoryArchive::new(root)));
    let mut library = MaterialLibrary::new(&filesystem);
    let mut compiled = 0;
    for material in &materials {
        let hash = library.land_hash(material).unwrap_or(0);
        if packed_pair(&filesystem, material, hash).is_some() {
            continue;
        }
        let texture = library.load_pixels(material)?;
        let side = side_of(&texture);
        let picture = plain_picture(material, &texture, side);
        let bad = |error: std::io::Error| CompileError::Bad(format!("{material}: {error}"));
        for (path, image, role) in [
            (packed_color(material), &picture.color, Role::Colour),
            (packed_surface(material), &picture.surface, Role::Data),
        ] {
            let top = image.first().map_or(&[][..], |top| &top.rgba[..]);
            let out = root.join(path);
            if let Some(folder) = out.parent() {
                std::fs::create_dir_all(folder).map_err(bad)?;
            }
            std::fs::write(&out, pack_picture(side, side, top, role, hash)?).map_err(bad)?;
        }
        compiled += 1;
    }
    Ok((compiled, materials.len()))
}
