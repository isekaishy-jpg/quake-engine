//! Building a [`Map`] from BSP data. Only this file knows `qbsp`.

use std::collections::BTreeMap;
use std::fmt;

use engine_core::math::Vec3;
use engine_map::{
    Animation, ClipChild, ClipNode, Contents, Entity, Face, FaceLight, Leaf, Look, Map, MipLevel,
    Model, Node, NodeChild, Plane, RESIDENT, TexelAxis, Texture, TextureKind, Tree,
};
use qbsp::data::models::BspFace;
use qbsp::data::nodes::{BspLeaf, BspLeafContentFlags, BspNodeRef};
use qbsp::data::texture::{BspMipTexture, BspTexInfo};
use qbsp::data::visdata::VisDataRef;
use qbsp::{BspData, BspParseInput, BspParseSettings};

use crate::texture::{Palette, texture_kind};

/// World units between lightmap samples, from `gl_model.c`.
const TEXELS_PER_SAMPLE: u32 = 16;

/// How far `Mod_LoadSubmodels` spreads a model's bounds. Door travel and door
/// linking both depend on it.
const SPREAD: Vec3 = Vec3::new(1.0, 1.0, 1.0);

/// The side of the checker texture used for missing textures.
const MISSING_TEXTURE_SIZE: u32 = 16;

/// A map that can't be imported.
#[derive(Debug)]
pub struct MapError(String);

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for MapError {}

/// Turns a `usize` from the file format into a `u32`, which every count in a
/// map fits.
fn index(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// Converts a `qbsp` vector.
fn vec3(value: qbsp::glam::Vec3) -> Vec3 {
    Vec3::new(value.x, value.y, value.z)
}

/// A grey checker, for textures the map doesn't carry.
fn missing_texture(name: &str) -> Texture {
    let side = MISSING_TEXTURE_SIZE;
    let mut rgba = Vec::new();
    for y in 0..side {
        for x in 0..side {
            let shade = if (x / 4 + y / 4) % 2 == 0 { 96 } else { 160 };
            rgba.extend_from_slice(&[shade, shade, shade, 255]);
        }
    }
    Texture {
        name: name.to_owned(),
        kind: TextureKind::Solid,
        mips: vec![MipLevel {
            width: side,
            height: side,
            rgba,
            form: engine_map::PixelForm::Rgba,
        }],
        animation: None,
        alternate: None,
        material: None,
    }
}

/// Expands one embedded texture.
fn import_texture(source: &BspMipTexture, palette: &Palette) -> Texture {
    let name = source.header.name.as_str();
    let kind = texture_kind(name);
    let levels = [
        source.data.full.as_ref(),
        source.data.half.as_ref(),
        source.data.quarter.as_ref(),
        source.data.eighth.as_ref(),
    ];
    let mut mips = Vec::new();
    for (level, indices) in levels.into_iter().enumerate() {
        let width = source.header.width >> level;
        let height = source.header.height >> level;
        let Some(indices) = indices else {
            break;
        };
        if width == 0
            || height == 0
            || Some(indices.len()) != width.checked_mul(height).map(|size| size as usize)
        {
            break;
        }
        mips.push(palette.expand(indices, width, height, kind));
    }
    if mips.is_empty() {
        return missing_texture(name);
    }
    Texture {
        name: name.to_owned(),
        kind,
        mips,
        animation: None,
        alternate: None,
        material: None,
    }
}

/// Splits `+0name` into its frame character and base name.
fn animation_key(name: &str) -> Option<(char, String)> {
    let mut characters = name.strip_prefix('+')?.chars();
    let frame = characters.next()?.to_ascii_lowercase();
    frame
        .is_ascii_alphanumeric()
        .then(|| (frame, characters.as_str().to_ascii_lowercase()))
}

/// Makes `frames` a cycle, if there's more than one.
fn link_cycle(textures: &mut [Texture], frames: &[u32]) {
    if frames.len() < 2 {
        return;
    }
    for (position, &texture) in frames.iter().enumerate() {
        if let Some(texture) = textures.get_mut(usize::try_from(texture).unwrap_or(usize::MAX)) {
            texture.animation = Some(Animation {
                frames: frames.to_vec(),
                position: index(position),
            });
        }
    }
}

/// Points every texture of `frames` at `other`.
fn link_alternate(textures: &mut [Texture], frames: &[u32], other: u32) {
    for &texture in frames {
        if let Some(texture) = textures.get_mut(usize::try_from(texture).unwrap_or(usize::MAX)) {
            texture.alternate = Some(other);
        }
    }
}

/// Links the frames of each animation, as `Mod_LoadTextures` does. The numbered
/// `+0` frames are one cycle and the lettered `+a` frames another, and a brush
/// entity swaps between the two by its frame.
fn link_animations(textures: &mut [Texture]) {
    let mut groups: BTreeMap<String, BTreeMap<char, u32>> = BTreeMap::new();
    for (position, texture) in textures.iter().enumerate() {
        if let Some((frame, base)) = animation_key(&texture.name) {
            let _previous = groups
                .entry(base)
                .or_default()
                .insert(frame, index(position));
        }
    }
    for frames in groups.values() {
        let (numbers, letters): (Vec<_>, Vec<_>) =
            frames.iter().partition(|(frame, _)| frame.is_ascii_digit());
        let numbers: Vec<u32> = numbers.into_iter().map(|(_, texture)| *texture).collect();
        let letters: Vec<u32> = letters.into_iter().map(|(_, texture)| *texture).collect();
        link_cycle(textures, &numbers);
        link_cycle(textures, &letters);
        if let (Some(&number), Some(&letter)) = (numbers.first(), letters.first()) {
            link_alternate(textures, &numbers, letter);
            link_alternate(textures, &letters, number);
        }
    }
}

/// The two texture axes of `info`.
fn texture_axes(info: &BspTexInfo) -> [TexelAxis; 2] {
    let projection = &info.projection;
    [
        TexelAxis {
            axis: vec3(projection.u_axis),
            offset: projection.u_offset,
        },
        TexelAxis {
            axis: vec3(projection.v_axis),
            offset: projection.v_offset,
        },
    ]
}

/// Lightmap placement for a face with these points, from
/// `CalcSurfaceExtents`: whole sample steps around the face's texel bounds.
fn light_extents(points: &[Vec3], axes: &[TexelAxis; 2]) -> ([i32; 2], [u32; 2]) {
    let step = f64::from(TEXELS_PER_SAMPLE);
    let mut origin = [0; 2];
    let mut size = [0; 2];
    for (side, axis) in axes.iter().enumerate() {
        let mut low = f64::MAX;
        let mut high = f64::MIN;
        for point in points {
            let value = f64::from(point.x) * f64::from(axis.axis.x)
                + f64::from(point.y) * f64::from(axis.axis.y)
                + f64::from(point.z) * f64::from(axis.axis.z)
                + f64::from(axis.offset);
            low = low.min(value);
            high = high.max(value);
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "map coordinates are far below the range of i32"
        )]
        let (first, last) = ((low / step).floor() as i32, (high / step).ceil() as i32);
        origin[side] = first * 16;
        size[side] = u32::try_from(last - first + 1).unwrap_or(1);
    }
    (origin, size)
}

/// The light of `face`, if it has any.
fn face_light(face: &BspFace, points: &[Vec3], axes: &[TexelAxis; 2]) -> Option<FaceLight> {
    let offset = u32::try_from(face.lightmap_offset.pixels).ok()?;
    let (texel_origin, [width, height]) = light_extents(points, axes);
    let mut styles = [Face::NO_STYLE; 4];
    for (slot, style) in styles.iter_mut().zip(face.lightmap_styles) {
        *slot = style.0;
    }
    Some(FaceLight {
        first_sample: offset,
        width,
        height,
        texel_origin,
        texels_per_sample: TEXELS_PER_SAMPLE,
        styles,
        tile: RESIDENT,
        average: [0; 3],
    })
}

/// `CONTENTS_SKY`, as a file has it.
const CONTENTS_SKY: i32 = -6;

/// The number of the leaves' lump.
const LEAF_LUMP: usize = 10;

/// Which leaves are sky, in order. `qbsp` calls them solid, which they aren't:
/// a rocket flies on into one, and is gone when it gets to the far side.
fn sky_leaves(bsp: &[u8]) -> Vec<bool> {
    let size = match bsp.get(..VERSION_SIZE) {
        Some([29, 0, 0, 0]) => 28,
        Some(b"2PSB") => 32,
        Some(b"BSP2") => 44,
        _ => return Vec::new(),
    };
    let at = VERSION_SIZE + LEAF_LUMP * 8;
    let number = |at: usize| -> Option<usize> {
        let field = bsp.get(at..at + 4)?.try_into().ok()?;
        usize::try_from(u32::from_le_bytes(field)).ok()
    };
    let lump = number(at)
        .zip(number(at + 4))
        .and_then(|(offset, length)| bsp.get(offset..offset.checked_add(length)?));
    lump.map_or_else(Vec::new, |lump| {
        lump.chunks_exact(size)
            .map(|leaf| leaf[..4] == CONTENTS_SKY.to_le_bytes())
            .collect()
    })
}

/// A leaf's contents. The BSP29 values arrive as `qbsp` flags.
fn contents(leaf: &BspLeaf, sky: bool) -> Contents {
    let flags = leaf.contents.0;
    if sky {
        Contents::Sky
    } else if flags.contains(BspLeafContentFlags::SOLID) {
        Contents::Solid
    } else if flags.contains(BspLeafContentFlags::WATER) {
        Contents::Water
    } else if flags.contains(BspLeafContentFlags::SLIME) {
        Contents::Slime
    } else if flags.contains(BspLeafContentFlags::LAVA) {
        Contents::Lava
    } else {
        Contents::Empty
    }
}

/// Converts a `qbsp` child reference.
fn node_child(child: BspNodeRef) -> NodeChild {
    match child {
        BspNodeRef::Node(node) => NodeChild::Node(node),
        BspNodeRef::Leaf(leaf) => NodeChild::Leaf(leaf),
    }
}

/// The leaf number `qbsp` gives `CONTENTS_SOLID` in a clipping hull.
const CLIP_SOLID_LEAF: u32 = 1;

/// Converts a `qbsp` clipping hull reference. Only solid stops a box, so
/// water and the like count as open space, as `SV_RecursiveHullCheck` has it.
fn clip_child(child: BspNodeRef) -> ClipChild {
    match child {
        BspNodeRef::Node(node) => ClipChild::Node(node),
        BspNodeRef::Leaf(CLIP_SOLID_LEAF) => ClipChild::Solid,
        BspNodeRef::Leaf(_) => ClipChild::Empty,
    }
}

/// Builds the tree, with `sky` saying which leaves are sky.
fn import_tree(bsp: &BspData, sky: &[bool]) -> Tree {
    let planes = bsp
        .planes
        .iter()
        .map(|plane| Plane {
            normal: vec3(plane.normal),
            distance: plane.dist,
        })
        .collect();
    let nodes = bsp
        .nodes
        .iter()
        .map(|node| Node {
            plane: node.plane_idx,
            children: [node_child(*node.front), node_child(*node.back)],
        })
        .collect();
    let node_faces = bsp
        .nodes
        .iter()
        .map(|node| (node.face_idx.0, node.face_num.0))
        .collect();
    let leaves = bsp
        .leaves
        .iter()
        .enumerate()
        .map(|(number, leaf)| Leaf {
            contents: contents(leaf, sky.get(number).copied().unwrap_or(false)),
            visibility: match leaf.visdata {
                VisDataRef::Offset(offset) => u32::try_from(offset).ok(),
                VisDataRef::Cluster(_) => None,
            },
            first_face: leaf.face_idx.0,
            face_count: leaf.face_num.0,
            ambient: leaf.ambience.map_or([0; 4], |ambience| {
                [ambience.water, ambience.sky, ambience.slime, ambience.lava]
            }),
        })
        .collect();
    Tree {
        planes,
        nodes,
        node_faces,
        leaves,
        leaf_faces: bsp.mark_surfaces.iter().map(|face| face.0).collect(),
        clip_nodes: bsp
            .clip_nodes
            .iter()
            .map(|node| ClipNode {
                plane: node.plane_idx,
                children: [clip_child(*node.front), clip_child(*node.back)],
            })
            .collect(),
        leaf_boxes: Vec::new(),
        visible_leaf_count: bsp
            .models
            .first()
            .and_then(|model| model.visleafs.0)
            .unwrap_or(0),
        visibility: bsp.visibility.visdata.clone(),
    }
}

/// Reads the entity lump: `{`, quoted pairs and `}`.
///
/// Trailing spaces leave a key, as in `ED_ParseEdict`: `r2m1` has a `"map "`.
#[must_use]
pub fn parse_entities(text: &str) -> Vec<Entity> {
    let mut entities = Vec::new();
    let mut current: Option<Entity> = None;
    let mut key: Option<String> = None;
    let mut rest = text;
    loop {
        rest = rest.trim_start();
        let Some(next) = rest.chars().next() else {
            return entities;
        };
        match next {
            '{' => {
                current = Some(Entity::default());
                rest = &rest[1..];
            }
            '}' => {
                entities.extend(current.take());
                rest = &rest[1..];
            }
            '"' => {
                let body = &rest[1..];
                let end = body.find('"').unwrap_or(body.len());
                let token = body[..end].to_owned();
                rest = body.get(end + 1..).unwrap_or("");
                match key.take() {
                    None => key = Some(token.trim_end_matches(' ').to_owned()),
                    Some(name) => {
                        if let Some(entity) = &mut current {
                            entity.fields.push((name, token));
                        }
                    }
                }
            }
            _ => {
                rest = rest.get(next.len_utf8()..).unwrap_or("");
            }
        }
    }
}

/// Reads a vector field such as `origin`.
fn parse_vector(text: &str) -> Option<Vec3> {
    let mut parts = text.split_whitespace().map(str::parse::<f32>);
    let x = parts.next()?.ok()?;
    let y = parts.next()?.ok()?;
    let z = parts.next()?.ok()?;
    Some(Vec3::new(x, y, z))
}

/// Where a player starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpawnPoint {
    /// The centre of the player's box.
    pub origin: Vec3,
    /// The yaw in degrees.
    pub yaw: f32,
}

/// How far above a spawn point a player starts, from `PutClientInServer`.
const SPAWN_HEIGHT: f32 = 1.0;

/// The first `info_player_start`, or the middle of nowhere without one.
#[must_use]
pub fn spawn_point(map: &Map) -> SpawnPoint {
    let start = map.entity_of_class("info_player_start");
    let origin = start
        .and_then(|entity| entity.get("origin"))
        .and_then(parse_vector)
        .unwrap_or(Vec3::ZERO);
    let yaw = start
        .and_then(|entity| entity.get("angle"))
        .and_then(|angle| angle.parse().ok())
        .unwrap_or(0.0);
    SpawnPoint {
        origin: origin + Vec3::new(0.0, 0.0, SPAWN_HEIGHT),
        yaw,
    }
}

/// The size of the version field at the start of a BSP file.
const VERSION_SIZE: usize = 4;

/// The number of lumps in a BSP header.
const LUMP_COUNT: usize = 15;

/// Removes anything after the last lump that isn't a BSPX section. Some maps
/// carry stray bytes there, such as `r1m1` from Dissolution of Eternity, which
/// `qbsp` would read as a broken BSPX directory.
#[must_use]
pub fn without_stray_tail(bsp: &[u8]) -> &[u8] {
    let Some(table) = bsp.get(VERSION_SIZE..VERSION_SIZE + LUMP_COUNT * 8) else {
        return bsp;
    };
    let (lumps, _remainder) = table.as_chunks::<8>();
    let end = lumps
        .iter()
        .map(|lump| {
            let offset = u32::from_le_bytes([lump[0], lump[1], lump[2], lump[3]]);
            let length = u32::from_le_bytes([lump[4], lump[5], lump[6], lump[7]]);
            usize::try_from(u64::from(offset) + u64::from(length)).unwrap_or(usize::MAX)
        })
        .max()
        .unwrap_or(0);
    let aligned = end.next_multiple_of(4);
    let has_bspx = bsp.get(aligned..aligned + 4) == Some(b"BSPX".as_slice());
    if has_bspx {
        bsp
    } else {
        bsp.get(..end).unwrap_or(bsp)
    }
}

/// Imports a BSP file, with its `.lit` file if it has one.
///
/// # Errors
///
/// Returns [`MapError`] if the file can't be parsed.
pub fn import_map(bsp: &[u8], lit: Option<&[u8]>, palette: &Palette) -> Result<Map, MapError> {
    let sky = sky_leaves(bsp);
    let bsp = BspData::parse(BspParseInput {
        bsp: without_stray_tail(bsp),
        lit,
        settings: BspParseSettings::default(),
    })
    .map_err(|error| MapError(error.to_string()))?;

    let mut textures: Vec<Texture> = bsp
        .textures
        .iter()
        .map(|texture| {
            texture.as_ref().map_or_else(
                || missing_texture("missing"),
                |texture| import_texture(texture, palette),
            )
        })
        .collect();
    link_animations(&mut textures);
    let fallback = index(textures.len());
    textures.push(missing_texture("missing"));

    let mut points = Vec::new();
    let mut faces = Vec::with_capacity(bsp.faces.len());
    for face in &bsp.faces {
        let first_point = index(points.len());
        points.extend(face.vertices(&bsp).map(vec3));
        let face_points = points.get(first_point as usize..).unwrap_or_default();
        let info = bsp
            .tex_info
            .get(face.texture_info_idx.0 as usize)
            .ok_or_else(|| MapError("a face names a missing texinfo".to_owned()))?;
        let axes = texture_axes(info);
        faces.push(Face {
            first_point,
            point_count: index(face_points.len()),
            texture: info.texture_idx.0.unwrap_or(fallback),
            axes,
            light: face_light(face, face_points, &axes),
        });
    }

    let light: Vec<[u8; 3]> = bsp.lighting.as_ref().map_or_else(Vec::new, |lighting| {
        (0..lighting.len())
            .filter_map(|i| lighting.get(i))
            .collect()
    });
    let entities = parse_entities(&String::from_utf8_lossy(&bsp.entities));
    let models: Vec<Model> = bsp
        .models
        .iter()
        .map(|model| Model {
            first_face: model.first_face,
            face_count: model.num_faces,
            mins: vec3(model.bound.min) - SPREAD,
            maxs: vec3(model.bound.max) + SPREAD,
            root: Some(node_child(model.hulls.root)),
            hulls: model.hulls.for_size.map_or([ClipChild::Empty; 2], |hulls| {
                [clip_child(hulls.small), clip_child(hulls.large)]
            }),
        })
        .collect();

    Ok(Map {
        // The original's light, on pictures in gamma space, as it always looked.
        look: Look {
            light_curve: 2.2,
            exposure: 1.0,
        },
        textures,
        points,
        faces,
        models,
        lit: !light.is_empty(),
        light,
        tree: import_tree(&bsp, &sky),
        entities,
        ..Map::default()
    })
}
