//! Compiled output: a map's baked data, in a file beside it (proposal 0072).
//!
//! Geometry is quick to rebuild, so only what's slow to make is stored. A bake
//! records the text it was made from, so one that's out of date is ignored and
//! the map falls back on playing unlit.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use engine_core::math::Vec3;
use engine_format::{Block, Document, Value, from_binary, to_binary, to_text};
use engine_map::{
    CHUNK_COLUMNS, CRISP_STYLE, ChunkedNav, Face, FaceLight, LightTiles, Map, MegaMaps, NavGrid,
    NavPlace, NavShape, ProbeGrid, RESIDENT, SideFiles, TexelAxis, TileLight, chunks_of,
};

use crate::error::CompileError;
use crate::mega::{MegaPlace, apply_mega};

/// Changes when bakes from older compilers can't be trusted.
const BAKE_VERSION: u64 = 21;

/// A hash of a map's text, with the compiler's own version mixed in.
#[must_use]
pub fn source_hash(text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64 ^ BAKE_VERSION;
    for byte in text.bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// The hash of a map, in its canonical text, with what else it took to build:
/// the same for a working map and the binary made from it.
#[must_use]
pub fn map_hash(document: &Document, used: &str) -> u64 {
    source_hash(&format!("{}{used}", to_text(document)))
}

/// A number as text, so a 64 bit hash survives exactly.
fn hash_text(hash: u64) -> String {
    format!("{hash:016x}")
}

/// Where a shadow's edge is, on the scale of [`Map::light_edges`].
const CRISP_EDGE: u8 = 128;

/// The start of the name of hull `hull`'s chunks.
fn nav_prefix(hull: usize) -> String {
    format!("nav{hull}")
}

/// A grid's shape as bytes.
fn write_shape(nav: &NavGrid) -> Vec<u8> {
    let mut out = Vec::new();
    for value in [nav.origin.x, nav.origin.y, nav.origin.z, nav.spacing] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    for value in [nav.size[0], nav.size[1], CHUNK_COLUMNS] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// A grid's shape from bytes, if they make one.
fn read_shape(bytes: &[u8]) -> Option<NavShape> {
    let (words, _) = bytes.as_chunks::<4>();
    let mut words = words.iter().copied();
    let mut float = || words.next().map(f32::from_le_bytes);
    let origin = Vec3::new(float()?, float()?, float()?);
    let spacing = float()?;
    let mut number = || words.next().map(u32::from_le_bytes);
    let size = [number()?, number()?];
    let chunk = number()?;
    (chunk > 0).then_some(NavShape {
        origin,
        spacing,
        size,
        chunk,
    })
}

/// A tiled map's light, sorted out by who holds it.
struct SplitLight {
    /// Each face's light, pointing where its samples went.
    faces: Vec<Option<FaceLight>>,
    /// What's always held.
    resident: TileLight,
    /// Each tile's place on the grid.
    cells: Vec<[i32; 2]>,
    /// Each tile's light.
    tiles: Vec<TileLight>,
}

/// The middle of a face's corners.
fn face_middle(map: &Map, face: &Face) -> Vec3 {
    let points = map.face_points(face);
    #[expect(clippy::cast_precision_loss, reason = "faces have few corners")]
    let share = 1.0 / points.len().max(1) as f32;
    points
        .iter()
        .fold(Vec3::ZERO, |sum, point| sum + *point * share)
}

/// A face's colour at a distance: each sample's layers added together, as they
/// show with styles at their usual brightness, then averaged.
fn average_of(map: &Map, light: &FaceLight) -> [u8; 3] {
    let layer = (light.width * light.height) as usize;
    let first = light.first_sample as usize;
    let mut total = [0_u64; 3];
    for sample in 0..layer {
        let mut color = [0_u32; 3];
        for (number, style) in light.styles.iter().enumerate() {
            if *style == Face::NO_STYLE {
                break;
            }
            let at = first + number * layer + sample;
            let shadowed = *style == CRISP_STYLE
                && map
                    .light_edges
                    .get(at)
                    .is_some_and(|edge| *edge < CRISP_EDGE);
            if let (false, Some(found)) = (shadowed, map.light.get(at)) {
                for (sum, part) in color.iter_mut().zip(found) {
                    *sum += u32::from(*part);
                }
            }
        }
        for (sum, part) in total.iter_mut().zip(color) {
            *sum += u64::from(part.min(255));
        }
    }
    let count = u64::try_from(layer.max(1)).unwrap_or(1);
    total.map(|sum| u8::try_from(sum / count).unwrap_or(u8::MAX))
}

/// Copies the samples of `light` to the end of `into`, and returns where they
/// start.
fn copy_samples(map: &Map, light: &FaceLight, into: &mut TileLight) -> u32 {
    let count = (light.width * light.height * light.style_count().max(1)) as usize;
    let first = light.first_sample as usize;
    let range = first..first + count;
    let at = u32::try_from(into.light.len()).unwrap_or(u32::MAX);
    into.light
        .extend_from_slice(map.light.get(range.clone()).unwrap_or_default());
    into.directions
        .extend_from_slice(map.light_directions.get(range.clone()).unwrap_or_default());
    into.edges
        .extend_from_slice(map.light_edges.get(range.clone()).unwrap_or_default());
    into.paint
        .extend_from_slice(map.paint.get(range).unwrap_or_default());
    at
}

/// Sorts `map`'s light by tile. What a world face has goes with the tile its
/// middle is in, and a moving model's stays held.
fn split_light(map: &Map) -> SplitLight {
    let world = map.models.first().map_or(0..0, |model| {
        model.first_face as usize..(model.first_face + model.face_count) as usize
    });
    let mut numbers: BTreeMap<[i32; 2], usize> = BTreeMap::new();
    let cell_of = |face: &Face| map.tile_of(face_middle(map, face)).0;
    for (at, face) in map.faces.iter().enumerate() {
        if world.contains(&at) && face.light.is_some() {
            numbers.entry(cell_of(face)).or_default();
        }
    }
    for (number, slot) in numbers.values_mut().enumerate() {
        *slot = number;
    }
    let mut split = SplitLight {
        faces: Vec::with_capacity(map.faces.len()),
        resident: TileLight::default(),
        cells: numbers.keys().copied().collect(),
        tiles: vec![TileLight::default(); numbers.len()],
    };
    // A chart's faces share their samples, so they go once, with the first.
    let mut charts: HashMap<u32, FaceLight> = HashMap::new();
    for (at, face) in map.faces.iter().enumerate() {
        let Some(mut light) = face.light else {
            split.faces.push(None);
            continue;
        };
        if let Some(done) = charts
            .get(&light.first_sample)
            .filter(|_| face.light_axes.is_some())
        {
            split.faces.push(Some(*done));
            continue;
        }
        let held_at = light.first_sample;
        light.average = average_of(map, &light);
        if world.contains(&at) {
            let number = numbers[&cell_of(face)];
            light.first_sample = copy_samples(map, &light, &mut split.tiles[number]);
            light.tile = u32::try_from(number).unwrap_or(RESIDENT);
        } else {
            let source = face.light.unwrap_or(light);
            light.first_sample = copy_samples(map, &source, &mut split.resident);
            light.tile = RESIDENT;
        }
        if face.light_axes.is_some() {
            charts.insert(held_at, light);
        }
        split.faces.push(Some(light));
    }
    split
}

/// What a tiled map keeps beside its bake, as names and bytes, for the folder
/// [`bake_folder`] names. Other maps keep nothing there.
#[must_use]
pub fn side_files(map: &Map) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    if map.tile_size <= 0.0 {
        return out;
    }
    let split = split_light(map);
    for (cell, tile) in split.cells.iter().zip(&split.tiles) {
        out.push((LightTiles::tile_file(*cell), tile.to_bytes()));
    }
    for (hull, nav) in [&map.nav, &map.nav_large].into_iter().enumerate() {
        let Some(nav) = nav else {
            continue;
        };
        for (at, chunk) in chunks_of(nav, CHUNK_COLUMNS) {
            let name = ChunkedNav::chunk_file(&nav_prefix(hull), at);
            out.push((name, chunk.to_bytes()));
        }
    }
    out
}

/// The folder of what's beside map `name`'s bake, from the content root.
#[must_use]
pub fn bake_folder(name: &str) -> String {
    format!("maps/{name}/bake/")
}

/// Says where `map` can read what's beside its bake.
pub fn attach_side_files(map: &mut Map, name: &str, files: &SideFiles) {
    let folder = bake_folder(name);
    for nav in map.nav_chunks.iter_mut().flatten() {
        nav.attach(&folder, Arc::clone(files));
    }
    if let Some(tiles) = &mut map.light_tiles {
        tiles.attach(&folder, Arc::clone(files));
    }
}

/// A walkable grid as bytes.
fn write_nav(nav: &NavGrid) -> Vec<u8> {
    let mut out = Vec::new();
    for value in [nav.origin.x, nav.origin.y, nav.origin.z, nav.spacing] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    let places = u32::try_from(nav.places.len()).unwrap_or(0);
    for value in [nav.size[0], nav.size[1], places] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    for column in &nav.columns {
        out.extend_from_slice(&column.to_le_bytes());
    }
    for place in &nav.places {
        for value in [place.point.x, place.point.y, place.point.z] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        for link in place.links {
            out.extend_from_slice(&link.to_le_bytes());
        }
    }
    out
}

/// A walkable grid from bytes, if they make one.
fn read_nav(bytes: &[u8]) -> Option<NavGrid> {
    let (words, _) = bytes.as_chunks::<4>();
    let mut words = words.iter().copied();
    let mut float = || words.next().map(f32::from_le_bytes);
    let origin = Vec3::new(float()?, float()?, float()?);
    let spacing = float()?;
    let mut number = || words.next().map(u32::from_le_bytes);
    let size = [number()?, number()?];
    let count = number()? as usize;
    let columns: Option<Vec<u32>> = (0..=size[0] * size[1]).map(|_| number()).collect();
    let mut places = Vec::with_capacity(count);
    for _ in 0..count {
        let mut word = || words.next();
        let point = Vec3::new(
            f32::from_le_bytes(word()?),
            f32::from_le_bytes(word()?),
            f32::from_le_bytes(word()?),
        );
        let mut place = NavPlace::at(point);
        for link in &mut place.links {
            *link = u32::from_le_bytes(word()?);
        }
        places.push(place);
    }
    Some(NavGrid {
        origin,
        spacing,
        size,
        columns: columns?,
        places,
    })
}

/// Writes `map`'s baked data as a compiled document.
#[must_use]
pub fn write_bake(map: &Map, hash: u64) -> Vec<u8> {
    let mut document = Document::new("bake");
    let mut info = Block::new("source");
    info.set("hash", Value::Text(hash_text(hash)));
    document.blocks.push(info);
    let tiled = map.tile_size > 0.0;
    let split = tiled.then(|| split_light(map));
    let face_lights: Vec<Option<FaceLight>> = split.as_ref().map_or_else(
        || map.faces.iter().map(|face| face.light).collect(),
        |split| split.faces.clone(),
    );
    let mut lights = Vec::new();
    for (face, face_light) in map.faces.iter().zip(&face_lights) {
        match *face_light {
            None => lights.push(0),
            Some(light) => {
                lights.push(if face.light_axes.is_some() { 2 } else { 1 });
                lights.extend_from_slice(&light.first_sample.to_le_bytes());
                lights.extend_from_slice(&light.width.to_le_bytes());
                lights.extend_from_slice(&light.height.to_le_bytes());
                lights.extend_from_slice(&light.texel_origin[0].to_le_bytes());
                lights.extend_from_slice(&light.texel_origin[1].to_le_bytes());
                lights.extend_from_slice(&light.texels_per_sample.to_le_bytes());
                lights.extend_from_slice(&light.styles);
                lights.extend_from_slice(&light.tile.to_le_bytes());
                lights.extend_from_slice(&light.average);
                for axis in face.light_axes.iter().flatten() {
                    for number in [axis.axis.x, axis.axis.y, axis.axis.z, axis.offset] {
                        lights.extend_from_slice(&number.to_le_bytes());
                    }
                }
            }
        }
    }
    let mut block = Block::new("light");
    block.set("faces", Value::Bytes(lights));
    let (samples, directions, edges, paint) = split.as_ref().map_or(
        (
            &map.light,
            &map.light_directions,
            &map.light_edges,
            &map.paint,
        ),
        |split| {
            let held = &split.resident;
            (&held.light, &held.directions, &held.edges, &held.paint)
        },
    );
    block.set("samples", Value::Bytes(samples.as_flattened().to_vec()));
    block.set(
        "directions",
        Value::Bytes(directions.as_flattened().to_vec()),
    );
    block.set("edges", Value::Bytes(edges.clone()));
    block.set("paint", Value::Bytes(paint.as_flattened().to_vec()));
    if let Some(split) = &split {
        let mut cells = Vec::with_capacity(split.cells.len() * 8);
        for part in split.cells.as_flattened() {
            cells.extend_from_slice(&part.to_le_bytes());
        }
        block.set("light_cells", Value::Bytes(cells));
    }
    block.set("visibility", Value::Bytes(map.tree.visibility.clone()));
    // A tiled map's grids go beside the bake, and only their shapes are here.
    if let Some(nav) = &map.nav {
        if tiled {
            block.set("nav_shape", Value::Bytes(write_shape(nav)));
        } else {
            block.set("nav", Value::Bytes(write_nav(nav)));
        }
    }
    if let Some(nav) = &map.nav_large {
        if tiled {
            block.set("nav_large_shape", Value::Bytes(write_shape(nav)));
        } else {
            block.set("nav_large", Value::Bytes(write_nav(nav)));
        }
    }
    let mut starts = Vec::with_capacity(map.tree.leaves.len() * 4);
    for leaf in &map.tree.leaves {
        starts.extend_from_slice(&leaf.visibility.unwrap_or(u32::MAX).to_le_bytes());
    }
    block.set("visible_from", Value::Bytes(starts));
    if let Some(mega) = &map.mega {
        let mut places = Vec::with_capacity(mega.faces.len() * 4);
        for number in &mega.faces {
            places.extend_from_slice(&number.to_le_bytes());
        }
        block.set("mega_faces", Value::Bytes(places));
        block.set("mega", Value::Bytes(mega.rgba.clone()));
        let maps = [
            ("mega_normal", &mega.maps.normal),
            ("mega_specular", &mega.maps.specular),
            ("mega_emissive", &mega.maps.emissive),
        ];
        for (name, image) in maps {
            if let Some(image) = image {
                block.set(name, Value::Bytes(image.clone()));
            }
        }
    }
    document.blocks.push(block);
    if let Some(probes) = &map.probes {
        let mut block = Block::new("probes");
        block.set(
            "origin",
            Value::List(
                [probes.origin.x, probes.origin.y, probes.origin.z]
                    .iter()
                    .map(|part| Value::Number(f64::from(*part)))
                    .collect(),
            ),
        );
        block.set("spacing", Value::Number(f64::from(probes.spacing)));
        block.set(
            "size",
            Value::List(
                probes
                    .size
                    .iter()
                    .map(|part| Value::Number(f64::from(*part)))
                    .collect(),
            ),
        );
        block.set(
            "colors",
            Value::Bytes(probes.colors.as_flattened().as_flattened().to_vec()),
        );
        block.set(
            "directions",
            Value::Bytes(probes.directions.as_flattened().to_vec()),
        );
        document.blocks.push(block);
    }
    to_binary(&document)
}

/// A face's light, and what places it, if that isn't its texture.
type ReadLight = (Option<FaceLight>, Option<[TexelAxis; 2]>);

/// Reads the faces' light records. `None` if they don't match `faces`.
fn read_face_lights(bytes: &[u8], faces: &[Face]) -> Option<Vec<ReadLight>> {
    let mut at = 0;
    let mut out = Vec::with_capacity(faces.len());
    for _ in faces {
        let flag = *bytes.get(at)?;
        at += 1;
        if flag == 0 {
            out.push((None, None));
            continue;
        }
        let word = |offset: usize| {
            let chunk: [u8; 4] = bytes.get(at + offset..at + offset + 4)?.try_into().ok()?;
            Some(chunk)
        };
        let light = FaceLight {
            first_sample: u32::from_le_bytes(word(0)?),
            width: u32::from_le_bytes(word(4)?),
            height: u32::from_le_bytes(word(8)?),
            texel_origin: [i32::from_le_bytes(word(12)?), i32::from_le_bytes(word(16)?)],
            texels_per_sample: u32::from_le_bytes(word(20)?),
            styles: word(24)?,
            tile: u32::from_le_bytes(word(28)?),
            average: bytes.get(at + 32..at + 35)?.try_into().ok()?,
        };
        let axes = if flag == 2 {
            let number = |offset: usize| word(offset).map(f32::from_le_bytes);
            let axis = |first: usize| -> Option<TexelAxis> {
                Some(TexelAxis {
                    axis: Vec3::new(number(first)?, number(first + 4)?, number(first + 8)?),
                    offset: number(first + 12)?,
                })
            };
            Some([axis(35)?, axis(51)?])
        } else {
            None
        };
        at += if axes.is_some() { 67 } else { 35 };
        out.push((Some(light), axes));
    }
    (at == bytes.len()).then_some(out)
}

/// Bytes as colours.
fn triples(bytes: &[u8]) -> Vec<[u8; 3]> {
    bytes.as_chunks::<3>().0.to_vec()
}

/// A block's bytes.
fn bytes_of<'a>(block: &'a Block, key: &str) -> Option<&'a [u8]> {
    block.get(key).and_then(Value::bytes)
}

/// Reads the probes.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "sizes come from our own writer and are checked against the data"
)]
fn read_probes(block: &Block) -> Option<ProbeGrid> {
    let origin = block.get("origin")?.numbers()?;
    let size = block.get("size")?.numbers()?;
    let [x, y, z] = origin.as_slice() else {
        return None;
    };
    let [sx, sy, sz] = size.as_slice() else {
        return None;
    };
    let (x, y, z, sx, sy, sz) = (*x, *y, *z, *sx, *sy, *sz);
    let colors = bytes_of(block, "colors")?.as_chunks::<18>().0;
    let count = (sx as usize)
        .checked_mul(sy as usize)?
        .checked_mul(sz as usize)?;
    if colors.len() != count {
        return None;
    }
    Some(ProbeGrid {
        origin: Vec3::new(x as f32, y as f32, z as f32),
        spacing: block.get("spacing")?.number()? as f32,
        size: [sx as u32, sy as u32, sz as u32],
        colors: colors
            .iter()
            .map(|chunk| {
                let mut sides = [[0; 3]; 6];
                for (side, part) in sides.iter_mut().zip(chunk.as_chunks::<3>().0) {
                    *side = *part;
                }
                sides
            })
            .collect(),
        directions: triples(bytes_of(block, "directions")?),
    })
}

/// Puts a bake's data on `map`, if it was made from text with this `hash` and
/// fits. Returns whether it did. A bake that doesn't is ignored, because a map
/// should never show light from an older version of itself.
///
/// # Errors
///
/// Returns an error if the file isn't a bake at all.
pub fn apply_bake(map: &mut Map, bytes: &[u8], hash: u64) -> Result<bool, CompileError> {
    let document = from_binary(bytes)?;
    if document.kind != "bake" {
        return Err(CompileError::Bad("not a bake".to_owned()));
    }
    let matches = document
        .blocks_of("source")
        .next()
        .and_then(|block| block.get("hash"))
        .and_then(Value::text)
        .is_some_and(|known| known == hash_text(hash));
    if !matches {
        return Ok(false);
    }
    let Some(light) = document.blocks_of("light").next() else {
        return Ok(false);
    };
    let Some(faces) = bytes_of(light, "faces").and_then(|b| read_face_lights(b, &map.faces)) else {
        return Ok(false);
    };
    let (Some(samples), Some(directions)) =
        (bytes_of(light, "samples"), bytes_of(light, "directions"))
    else {
        return Ok(false);
    };
    map.light = triples(samples);
    map.light_directions = triples(directions);
    map.light_edges = bytes_of(light, "edges").unwrap_or_default().to_vec();
    if let (Some(data), Some(starts)) = (
        bytes_of(light, "visibility"),
        bytes_of(light, "visible_from"),
    ) {
        let (starts, _) = starts.as_chunks::<4>();
        if starts.len() == map.tree.leaves.len() {
            for (leaf, start) in map.tree.leaves.iter_mut().zip(starts) {
                let start = u32::from_le_bytes(*start);
                leaf.visibility = (start != u32::MAX).then_some(start);
            }
            map.tree.visibility = data.to_vec();
            map.tree.visible_leaf_count =
                u32::try_from(map.tree.leaves.len().saturating_sub(1)).unwrap_or(0);
        }
    }
    map.nav = bytes_of(light, "nav").and_then(read_nav);
    map.nav_large = bytes_of(light, "nav_large").and_then(read_nav);
    map.light_tiles = bytes_of(light, "light_cells").map(|bytes| {
        let (words, _) = bytes.as_chunks::<4>();
        let numbers: Vec<i32> = words.iter().map(|word| i32::from_le_bytes(*word)).collect();
        LightTiles::new(map.tile_size, numbers.as_chunks::<2>().0.to_vec())
    });
    for (hull, name) in ["nav_shape", "nav_large_shape"].into_iter().enumerate() {
        map.nav_chunks[hull] = bytes_of(light, name)
            .and_then(read_shape)
            .map(|shape| ChunkedNav::new(shape, nav_prefix(hull)));
    }
    let paint = bytes_of(light, "paint").unwrap_or_default();
    map.paint = paint.as_chunks::<4>().0.to_vec();
    for (face, (light, axes)) in map.faces.iter_mut().zip(faces) {
        face.light = light;
        face.light_axes = axes;
    }
    if let (Some(rgba), Some(faces)) = (bytes_of(light, "mega"), bytes_of(light, "mega_faces")) {
        let numbers: Vec<i32> = faces
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| i32::from_le_bytes(*bytes))
            .collect();
        let places: Vec<MegaPlace> = numbers
            .as_chunks::<3>()
            .0
            .iter()
            .map(|[face, x, y]| MegaPlace {
                face: face.cast_unsigned(),
                shift: [*x, *y],
            })
            .collect();
        let image = |name| bytes_of(light, name).map(<[u8]>::to_vec);
        let maps = MegaMaps {
            normal: image("mega_normal"),
            specular: image("mega_specular"),
            emissive: image("mega_emissive"),
        };
        apply_mega(map, &places, rgba.to_vec(), &maps);
    }
    map.probes = document.blocks_of("probes").next().and_then(read_probes);
    map.lit = true;
    Ok(true)
}
