//! Compiled output: a map's baked data, in a file beside it (proposal 0072).
//!
//! Geometry is quick to rebuild, so only what's slow to make is stored. A bake
//! records the text it was made from, so one that's out of date is ignored and
//! the map falls back on playing unlit.

use engine_core::math::Vec3;
use engine_format::{Block, Document, Value, from_binary, to_binary, to_text};
use engine_map::{Face, FaceLight, Map, NavGrid, NavPlace, ProbeGrid};

use crate::error::CompileError;
use crate::mega::{MegaPlace, apply_mega};

/// Changes when bakes from older compilers can't be trusted.
const BAKE_VERSION: u64 = 7;

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
    let mut lights = Vec::new();
    for face in &map.faces {
        match face.light {
            None => lights.push(0),
            Some(light) => {
                lights.push(1);
                lights.extend_from_slice(&light.first_sample.to_le_bytes());
                lights.extend_from_slice(&light.width.to_le_bytes());
                lights.extend_from_slice(&light.height.to_le_bytes());
                lights.extend_from_slice(&light.texel_origin[0].to_le_bytes());
                lights.extend_from_slice(&light.texel_origin[1].to_le_bytes());
                lights.extend_from_slice(&light.texels_per_sample.to_le_bytes());
                lights.extend_from_slice(&light.styles);
            }
        }
    }
    let mut block = Block::new("light");
    block.set("faces", Value::Bytes(lights));
    block.set("samples", Value::Bytes(map.light.as_flattened().to_vec()));
    block.set(
        "directions",
        Value::Bytes(map.light_directions.as_flattened().to_vec()),
    );
    block.set("edges", Value::Bytes(map.light_edges.clone()));
    block.set("paint", Value::Bytes(map.paint.as_flattened().to_vec()));
    block.set("visibility", Value::Bytes(map.tree.visibility.clone()));
    if let Some(nav) = &map.nav {
        block.set("nav", Value::Bytes(write_nav(nav)));
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

/// Reads the faces' light records. `None` if they don't match `faces`.
fn read_face_lights(bytes: &[u8], faces: &[Face]) -> Option<Vec<Option<FaceLight>>> {
    let mut at = 0;
    let mut out = Vec::with_capacity(faces.len());
    for _ in faces {
        let flag = *bytes.get(at)?;
        at += 1;
        if flag == 0 {
            out.push(None);
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
        };
        at += 28;
        out.push(Some(light));
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
    let paint = bytes_of(light, "paint").unwrap_or_default();
    map.paint = paint.as_chunks::<4>().0.to_vec();
    for (face, light) in map.faces.iter_mut().zip(faces) {
        face.light = light;
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
        apply_mega(map, &places, rgba.to_vec());
    }
    map.probes = document.blocks_of("probes").next().and_then(read_probes);
    map.lit = true;
    Ok(true)
}
