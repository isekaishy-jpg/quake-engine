//! Turning a [`Map`] into data for the GPU. No GPU is involved here.

use engine_core::math::Vec3;
use std::collections::HashMap;

use engine_map::{AlphaMode, Face, FaceLight, Map, RESIDENT, Texture, TextureKind, TileLight};

use crate::atlas::{Atlas, AtlasSpace};
use crate::terrain::ChunkLight;

/// The largest atlas side, which every Vulkan device of interest can sample.
const ATLAS_LIMIT: u32 = 8192;

/// The side of the atlas of a map whose light streams.
const STREAMED_ATLAS: u32 = 4096;

/// Bytes per atlas pixel.
const ATLAS_PIXEL: usize = 4;

/// The grey that draws at full brightness once doubled.
const FULL_LIGHT: u8 = 128;

/// One vertex, laid out for the GPU.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
    /// World position.
    pub position: [f32; 3],
    /// Texture coordinates, where 1 is the texture's width.
    pub uv: [f32; 2],
    /// Atlas coordinates of the first style.
    pub light_uv: [f32; 2],
    /// The distance across the atlas to the next style.
    pub light_step: f32,
    /// The style of each layer.
    pub styles: [u8; 4],
    /// Which way it faces, in steps of 1/127, or zeros for a flat face.
    pub normal: [i8; 4],
    /// What its colour multiplies its surface by.
    pub color: [u8; 4],
}

impl Vertex {
    /// The size in bytes.
    pub const SIZE: usize = 44;

    /// Appends this vertex's bytes.
    pub(crate) fn write(&self, out: &mut Vec<u8>) {
        for value in self
            .position
            .iter()
            .chain(&self.uv)
            .chain(&self.light_uv)
            .chain(std::iter::once(&self.light_step))
        {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&self.styles);
        out.extend(self.normal.map(i8::to_ne_bytes).into_iter().flatten());
        out.extend_from_slice(&self.color);
    }
}

/// How a batch is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BatchKind {
    /// Sky.
    Sky,
    /// An opaque surface.
    Material,
    /// A surface with holes that follow its alpha.
    Masked,
    /// A see-through surface, drawn after everything else.
    Blended,
}

/// The style saying that what's drawn brings its own light, which is in the shader too.
pub(crate) const OWN_LIGHT_STYLE: u8 = 254;

/// Where an instance's light is, as a vertex would say.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InstanceLight {
    /// Its place in the atlas.
    pub uv: [f32; 2],
    /// The distance across the atlas to the next style.
    pub step: f32,
    /// The style of each layer.
    pub styles: [u8; 4],
}

/// The faces of one texture, for one kind of drawing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Batch {
    /// The texture, by index in the map.
    pub texture: u32,
    /// How it's drawn.
    pub kind: BatchKind,
}

/// The part of a brush model that uses one batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelRange {
    /// The batch.
    pub batch: u32,
    /// The first index in [`WorldMesh::model_indices`].
    pub first: u32,
    /// How many indices.
    pub count: u32,
}

/// A map's GPU data, in plain form.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorldMesh {
    /// All vertices.
    pub vertices: Vec<Vertex>,
    /// Triangle indices for each face, drawn or not.
    pub face_indices: Vec<Vec<u32>>,
    /// Each face's batch, or `None` if it's never drawn.
    pub face_batches: Vec<Option<u32>>,
    /// The batches.
    pub batches: Vec<Batch>,
    /// Where each batch's faces are, for sorting what's see-through.
    pub batch_centers: Vec<Vec3>,
    /// The bounds of each see-through face, for what's behind them to draw first.
    pub see_through_boxes: Vec<(Vec3, Vec3)>,
    /// The triangles of every brush model, batch after batch.
    pub model_indices: Vec<u32>,
    /// Each model's batches, with their first index and count, by model
    /// number. The world's are empty, because its faces go through the PVS.
    pub model_ranges: Vec<Vec<ModelRange>>,
    /// The most indices the world's visible set can need.
    pub world_index_count: u32,
    /// The atlas side.
    pub atlas_size: u32,
    /// The atlas, in RGBA bytes.
    pub atlas: Vec<u8>,
    /// Where each atlas pixel's light comes from, in RGBA bytes, laid out the
    /// same way.
    pub direction_atlas: Vec<u8>,
    /// How much of each material layer is painted, laid out the same way.
    pub paint_atlas: Vec<u8>,
    /// Each face's first vertex and how many it has.
    pub face_vertices: Vec<(u32, u32)>,
    /// Where each face's light is when its tile's isn't shown.
    far_places: Vec<Option<LightPlace>>,
    /// The side of the corner of the atlas that's always in use.
    pub reserved: u32,
    /// The faces with light in each tile, by tile number.
    pub tile_faces: Vec<Vec<u32>>,
    /// Which instance each face belongs to, if any.
    pub face_instance: Vec<Option<u32>>,
    /// Where each instance's light is.
    pub(crate) instance_lights: Vec<InstanceLight>,
    /// Which chunk of land each face stands in for, if any.
    pub face_chunk: Vec<Option<u32>>,
    /// Where each chunk's light is.
    pub(crate) chunk_lights: Vec<ChunkLight>,
}

/// The light of a tile as a square of the atlas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TileImage {
    /// The side of the square.
    pub size: u32,
    /// The pixels, in RGBA bytes.
    pub bytes: Vec<u8>,
    /// The pixels of the light's directions.
    pub directions: Vec<u8>,
    /// The pixels of the paint.
    pub paint: Vec<u8>,
    /// Each of the tile's faces, with its corner in the square.
    places: Vec<(u32, [u32; 2])>,
}

/// Vertices to write, as the first one's number and their bytes.
pub(crate) type VertexPatch = (u32, Vec<u8>);

/// Which tiles' light is in the atlas, and where.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShownTiles {
    /// What room there is.
    space: AtlasSpace,
    /// Each shown tile's corner and side.
    shown: HashMap<u32, ([u32; 2], u32)>,
}

impl ShownTiles {
    /// Nothing shown, in `mesh`'s atlas.
    #[must_use]
    pub fn new(mesh: &WorldMesh) -> Self {
        Self {
            space: AtlasSpace::new(mesh.atlas_size, mesh.reserved),
            shown: HashMap::new(),
        }
    }

    /// How many tiles are shown.
    #[must_use]
    pub fn count(&self) -> usize {
        self.shown.len()
    }

    /// Finds room for a tile's `size` square. Returns where it is, or `None` if
    /// the atlas is full, or the tile's already shown.
    pub fn place(&mut self, tile: u32, size: u32) -> Option<[u32; 2]> {
        if self.shown.contains_key(&tile) {
            return None;
        }
        let at = self.space.take(size)?;
        self.shown.insert(tile, (at, size));
        Some(at)
    }

    /// Frees a tile's room. Returns whether it had any.
    pub fn free(&mut self, tile: u32) -> bool {
        let Some((at, size)) = self.shown.remove(&tile) else {
            return false;
        };
        self.space.give(at, size);
        true
    }
}

/// Where a face's light sits in the atlas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct LightPlace {
    /// The atlas position of its first style.
    position: [u32; 2],
    /// Its width in samples.
    width: u32,
}

/// Where a batch that's drawn before all other see-through things says it is.
const BEHIND_EVERYTHING: Vec3 = Vec3::new(1.0e9, 1.0e9, 1.0e9);

/// Whether `face` is a decal, which needs no place in the order of see-through things.
fn lies_on_surfaces(map: &Map, face: &Face) -> bool {
    let texture = map.textures.get(face.texture as usize);
    texture
        .and_then(|texture| texture.material.as_ref())
        .is_some_and(|material| material.decal && material.alpha == AlphaMode::Blend)
}

/// Whether `texture` is drawn through the material pipelines. Upload and
/// batching must agree on this, or a draw has no material bound.
pub(crate) fn uses_material(texture: &Texture) -> bool {
    texture.material.as_ref().is_some_and(|material| {
        matches!(texture.kind, TextureKind::Solid | TextureKind::Cutout)
            || (texture.kind == TextureKind::Liquid
                && (material.alpha == AlphaMode::Blend || material.shader.is_some()))
    })
}

/// The batch kind of a texture, or `None` for hidden ones.
pub(crate) fn batch_kind(texture: &Texture) -> Option<BatchKind> {
    if texture.kind == TextureKind::Hidden {
        return None;
    }
    if let Some(material) = texture.material.as_ref().filter(|_| uses_material(texture)) {
        return Some(match material.alpha {
            AlphaMode::Blend => BatchKind::Blended,
            AlphaMode::Mask => BatchKind::Masked,
            AlphaMode::Opaque => BatchKind::Material,
        });
    }
    Some(if texture.kind == TextureKind::Sky {
        BatchKind::Sky
    } else {
        BatchKind::Material
    })
}

/// The middle of a face's corners.
fn face_center(map: &Map, face: &Face) -> Vec3 {
    let points = map.face_points(face);
    #[expect(clippy::cast_precision_loss, reason = "faces have few corners")]
    let share = 1.0 / points.len().max(1) as f32;
    points
        .iter()
        .fold(Vec3::ZERO, |sum, point| sum + *point * share)
}

/// The bounds of `face`, a little wider than it, as a flat box has no inside.
fn face_box(map: &Map, face: &Face) -> (Vec3, Vec3) {
    let points = map.face_points(face);
    let first = points.first().copied().unwrap_or(Vec3::ZERO);
    let (low, high) = points.iter().fold((first, first), |(low, high), point| {
        (
            Vec3::new(low.x.min(point.x), low.y.min(point.y), low.z.min(point.z)),
            Vec3::new(
                high.x.max(point.x),
                high.y.max(point.y),
                high.z.max(point.z),
            ),
        )
    });
    let margin = Vec3::new(0.5, 0.5, 0.5);
    (low - margin, high + margin)
}

/// Whether `face` samples the lightmap.
fn lit_face(map: &Map, face: &Face) -> Option<FaceLight> {
    let texture = map.textures.get(face.texture as usize)?;
    matches!(texture.kind, TextureKind::Solid | TextureKind::Cutout)
        .then_some(face.light)
        .flatten()
}

/// The packed atlas.
struct BuiltAtlas {
    /// The side of the square.
    size: u32,
    /// The side of the corner in use.
    reserved: u32,
    /// The pixels, in RGBA bytes.
    bytes: Vec<u8>,
    /// The pixels of the light's directions.
    directions: Vec<u8>,
    /// The pixels of the paint.
    paint: Vec<u8>,
    /// Each face's place, if it has one.
    places: Vec<Option<LightPlace>>,
    /// A pixel at full brightness, for faces in unlit maps.
    neutral: [u32; 2],
}

/// The light a face's atlas place is copied from.
#[derive(Clone, Copy)]
struct Samples<'a> {
    /// Samples, as RGB.
    light: &'a [[u8; 3]],
    /// Where each sample's light comes from.
    directions: &'a [[u8; 3]],
    /// How far each sample is from a shadow's edge.
    edges: &'a [u8],
    /// How much of each layer is painted.
    paint: &'a [[u8; 4]],
}

impl<'a> Samples<'a> {
    /// What `map` holds.
    fn of_map(map: &'a Map) -> Self {
        Self {
            light: &map.light,
            directions: &map.light_directions,
            edges: &map.light_edges,
            paint: &map.paint,
        }
    }

    /// What a tile holds.
    fn of_tile(tile: &'a TileLight) -> Self {
        Self {
            light: &tile.light,
            directions: &tile.directions,
            edges: &tile.edges,
            paint: &tile.paint,
        }
    }
}

/// How much atlas a face's light takes. With its tile's light elsewhere, it's
/// one texel of its average.
fn atlas_size_of(light: &FaceLight) -> [u32; 2] {
    if light.tile == RESIDENT {
        [light.width * light.style_count().max(1), light.height]
    } else {
        [1, 1]
    }
}

/// Packs every face's styles side by side.
fn build_atlas(map: &Map) -> Option<BuiltAtlas> {
    let lights: Vec<Option<FaceLight>> = map.faces.iter().map(|f| lit_face(map, f)).collect();
    // A chart's faces share their light, so it goes in once.
    let mut first_of: HashMap<(u32, u32), usize> = HashMap::new();
    let mut sizes: Vec<[u32; 2]> = Vec::new();
    let mut slots = Vec::with_capacity(lights.len());
    for (light, face) in lights.iter().zip(&map.faces) {
        let Some(light) = light else {
            slots.push(None);
            continue;
        };
        let shared = face
            .light_axes
            .is_some()
            .then_some((light.tile, light.first_sample));
        let known = shared.and_then(|key| first_of.get(&key).copied());
        slots.push(Some(known.unwrap_or_else(|| {
            sizes.push(atlas_size_of(light));
            if let Some(key) = shared {
                first_of.insert(key, sizes.len() - 1);
            }
            sizes.len() - 1
        })));
    }
    let neutral_slot = sizes.len();
    sizes.push([1, 1]);
    let packed = Atlas::pack(&sizes, ATLAS_LIMIT)?;
    // What streams needs room beyond what's always there.
    let size = if map.light_tiles.is_some() {
        STREAMED_ATLAS.max(packed.size * 2).min(ATLAS_LIMIT)
    } else {
        packed.size
    };
    let reserved = packed.size;
    let side = size as usize;
    let mut bytes = vec![0; side * side * ATLAS_PIXEL];
    let mut directions = vec![128; side * side * ATLAS_PIXEL];
    let mut paint = vec![0; side * side * ATLAS_PIXEL];
    let mut places = Vec::with_capacity(lights.len());
    let mut copied = vec![false; sizes.len()];
    for (light, slot) in lights.iter().zip(&slots) {
        let (Some(light), Some(slot)) = (light, slot) else {
            places.push(None);
            continue;
        };
        let position = *packed.positions.get(*slot)?;
        if std::mem::replace(&mut copied[*slot], true) {
            // It's already there.
        } else if light.tile == RESIDENT {
            let samples = Samples::of_map(map);
            copy_light(samples, light, position, side, &mut bytes, &mut directions);
            copy_paint(samples, light, position, side, &mut paint);
        } else {
            let at = (position[1] as usize * side + position[0] as usize) * ATLAS_PIXEL;
            bytes[at..at + 3].copy_from_slice(&light.average);
            bytes[at + 3] = 255;
        }
        places.push(Some(LightPlace {
            position,
            width: light.width,
        }));
    }
    let neutral = *packed.positions.get(neutral_slot)?;
    let at = (neutral[1] as usize * side + neutral[0] as usize) * ATLAS_PIXEL;
    bytes[at..at + ATLAS_PIXEL].copy_from_slice(&[FULL_LIGHT, FULL_LIGHT, FULL_LIGHT, 255]);
    Some(BuiltAtlas {
        size,
        reserved,
        bytes,
        directions,
        paint,
        places,
        neutral,
    })
}

/// Copies a face's paint into its atlas, in the first style's place.
fn copy_paint(
    samples: Samples<'_>,
    light: &FaceLight,
    position: [u32; 2],
    side: usize,
    paint: &mut [u8],
) {
    let (width, height) = (light.width as usize, light.height as usize);
    let [left, top] = position.map(|value| value as usize);
    for y in 0..height {
        for x in 0..width {
            let sample = samples
                .paint
                .get(light.first_sample as usize + y * width + x);
            if let Some(sample) = sample {
                let at = ((top + y) * side + left + x) * ATLAS_PIXEL;
                paint[at..at + 4].copy_from_slice(sample);
            }
        }
    }
}

/// Copies a face's samples into the atlas, one style after another.
fn copy_light(
    samples: Samples<'_>,
    light: &FaceLight,
    position: [u32; 2],
    side: usize,
    bytes: &mut [u8],
    directions: &mut [u8],
) {
    let (width, height) = (light.width as usize, light.height as usize);
    let [left, top] = position.map(|value| value as usize);
    for style in 0..light.style_count() as usize {
        let first = light.first_sample as usize + style * width * height;
        for y in 0..height {
            for x in 0..width {
                let Some(sample) = samples.light.get(first + y * width + x) else {
                    continue;
                };
                let at = ((top + y) * side + left + style * width + x) * ATLAS_PIXEL;
                bytes[at..at + 3].copy_from_slice(sample);
                let edge = samples.edges.get(first + y * width + x);
                bytes[at + 3] = edge.copied().unwrap_or(255);
                if let Some(direction) = samples.directions.get(first + y * width + x) {
                    directions[at..at + 3].copy_from_slice(direction);
                    directions[at + 3] = 255;
                }
            }
        }
    }
}

/// The vertex of `point`, on `face`.
fn make_vertex(
    map: &Map,
    face: &Face,
    point: (usize, Vec3),
    place: Option<LightPlace>,
    neutral: [u32; 2],
    atlas_size: u32,
    detail: bool,
) -> Vertex {
    let size = texture_size(map, face);
    let (number, point) = point;
    let s = face.axes[0].project(point);
    let t = face.axes[1].project(point);
    let [x, y, z] = map.normals.get(number).copied().unwrap_or([0; 3]);
    let mut vertex = Vertex {
        position: [point.x, point.y, point.z],
        uv: [s / size[0], t / size[1]],
        styles: [Face::NO_STYLE; 4],
        normal: [x, y, z, if face.lit_by_probe() { 127 } else { 0 }],
        color: map
            .point_colors
            .get(number)
            .copied()
            .unwrap_or([u8::MAX; 4]),
        ..Vertex::default()
    };
    #[expect(
        clippy::cast_precision_loss,
        reason = "atlas positions are far below 2^24"
    )]
    let scale = 1.0 / atlas_size as f32;
    #[expect(
        clippy::cast_precision_loss,
        reason = "atlas positions are far below 2^24"
    )]
    if let (Some(place), Some(light)) = (place, face.light)
        && light.tile != RESIDENT
        && !detail
    {
        // One texel of its average, which has every layer in it.
        vertex.light_uv = [
            (place.position[0] as f32 + 0.5) * scale,
            (place.position[1] as f32 + 0.5) * scale,
        ];
        vertex.styles[0] = 0;
    } else if let (Some(place), Some(light)) = (place, face.light) {
        let step = light.texels_per_sample as f32;
        let [across, down] = face.light_space();
        let x = (across.project(point) - light.texel_origin[0] as f32) / step + 0.5;
        let y = (down.project(point) - light.texel_origin[1] as f32) / step + 0.5;
        vertex.light_uv = [
            (place.position[0] as f32 + x) * scale,
            (place.position[1] as f32 + y) * scale,
        ];
        vertex.light_step = place.width as f32 * scale;
        vertex.styles = light.styles;
    } else if !map.lit {
        vertex.light_uv = [
            (neutral[0] as f32 + 0.5) * scale,
            (neutral[1] as f32 + 0.5) * scale,
        ];
        vertex.styles[0] = 0;
    }
    vertex
}

/// A face's texture size in texels, as floats.
fn texture_size(map: &Map, face: &Face) -> [f32; 2] {
    map.textures
        .get(face.texture as usize)
        .and_then(Texture::tile_size)
        .unwrap_or([1.0, 1.0])
}

/// Builds the GPU data of `map`, or `None` if its light won't fit the atlas.
#[must_use]
pub fn build_world(map: &Map) -> Option<WorldMesh> {
    let BuiltAtlas {
        size: atlas_size,
        reserved,
        bytes: atlas,
        directions: direction_atlas,
        paint: paint_atlas,
        places,
        neutral,
    } = build_atlas(map)?;
    let mut mesh = WorldMesh {
        atlas_size,
        atlas,
        direction_atlas,
        paint_atlas,
        reserved,
        ..WorldMesh::default()
    };
    if let Some(tiles) = &map.light_tiles {
        mesh.tile_faces = vec![Vec::new(); tiles.cells().len()];
        for (number, place) in (0_u32..).zip(&places) {
            let light = map.faces.get(number as usize).and_then(|face| face.light);
            let tile = light.filter(|light| light.tile != RESIDENT && place.is_some());
            if let Some(faces) = tile.and_then(|l| mesh.tile_faces.get_mut(l.tile as usize)) {
                faces.push(number);
            }
        }
    }
    let mut batch_of_texture = vec![None; map.textures.len()];
    for (index, texture) in map.textures.iter().enumerate() {
        if let Some(kind) = batch_kind(texture) {
            batch_of_texture[index] = Some(u32::try_from(mesh.batches.len()).ok()?);
            mesh.batches.push(Batch {
                texture: u32::try_from(index).ok()?,
                kind,
            });
            mesh.batch_centers.push(Vec3::ZERO);
        }
    }
    mesh.face_indices = vec![Vec::new(); map.faces.len()];
    mesh.face_batches = vec![None; map.faces.len()];
    mesh.face_vertices = vec![(0, 0); map.faces.len()];
    mesh.model_ranges = vec![Vec::new(); map.models.len()];
    mesh.face_instance = vec![None; map.faces.len()];
    for (number, instance) in (0_u32..).zip(&map.mesh_instances) {
        let first = instance.first_face as usize;
        let last = first + instance.face_count as usize;
        if let Some(faces) = mesh.face_instance.get_mut(first..last) {
            faces.fill(Some(number));
        }
        let vertex = map.faces.get(first).and_then(|face| {
            let point = *map.face_points(face).first()?;
            let number = face.first_point as usize;
            let place = places.get(first).copied().flatten();
            Some(make_vertex(
                map,
                face,
                (number, point),
                place,
                neutral,
                atlas_size,
                false,
            ))
        });
        let vertex = vertex.unwrap_or_default();
        mesh.instance_lights.push(InstanceLight {
            uv: vertex.light_uv,
            step: vertex.light_step,
            styles: vertex.styles,
        });
    }
    mesh.face_chunk = vec![None; map.faces.len()];
    let mut chunk = 0_u32;
    for sheet in &map.terrain {
        for at in 0..sheet.chunk_count() {
            let face_at = (sheet.first_face + at) as usize;
            if let Some(slot) = mesh.face_chunk.get_mut(face_at) {
                *slot = Some(chunk);
            }
            let place = places.get(face_at).copied().flatten();
            mesh.chunk_lights
                .push(chunk_light(map, face_at, place, neutral, atlas_size, false));
            chunk += 1;
        }
    }
    for (number, model) in map.models.iter().enumerate() {
        let mut per_batch: Vec<Vec<u32>> = vec![Vec::new(); mesh.batches.len()];
        for index in model.first_face..model.first_face.saturating_add(model.face_count) {
            let at = index as usize;
            let Some(face) = map.faces.get(at) else {
                continue;
            };
            // An instance draws these from its mesh, and land from its heights.
            if mesh.face_instance[at].is_some() || mesh.face_chunk[at].is_some() {
                continue;
            }
            let Some(mut batch) = batch_of_texture
                .get(face.texture as usize)
                .copied()
                .flatten()
            else {
                continue;
            };
            // What's see-through is drawn far to near, so each face stands
            // alone.
            if lies_on_surfaces(map, face) {
                mesh.batch_centers[batch as usize] = BEHIND_EVERYTHING;
            } else if mesh.batches[batch as usize].kind == BatchKind::Blended {
                let shared = mesh.batches[batch as usize].clone();
                batch = u32::try_from(mesh.batches.len()).ok()?;
                mesh.batches.push(shared);
                mesh.batch_centers.push(face_center(map, face));
                mesh.see_through_boxes.push(face_box(map, face));
                per_batch.push(Vec::new());
            }
            let first = u32::try_from(mesh.vertices.len()).ok()?;
            let numbered = (face.first_point as usize..).zip(map.face_points(face));
            for (number, &point) in numbered {
                mesh.vertices.push(make_vertex(
                    map,
                    face,
                    (number, point),
                    places[at],
                    neutral,
                    atlas_size,
                    false,
                ));
            }
            mesh.face_vertices[at] = (first, face.point_count);
            for corner in 2..face.point_count {
                mesh.face_indices[at].extend([first, first + corner - 1, first + corner]);
            }
            if number == 0 {
                mesh.face_batches[at] = Some(batch);
            } else {
                per_batch[batch as usize].extend_from_slice(&mesh.face_indices[at]);
            }
        }
        for (batch, indices) in per_batch.iter().enumerate() {
            if !indices.is_empty() {
                mesh.model_ranges[number].push(ModelRange {
                    batch: u32::try_from(batch).ok()?,
                    first: u32::try_from(mesh.model_indices.len()).ok()?,
                    count: u32::try_from(indices.len()).ok()?,
                });
                mesh.model_indices.extend_from_slice(indices);
            }
        }
    }
    let world_indices: usize = mesh
        .face_batches
        .iter()
        .zip(&mesh.face_indices)
        .filter(|(batch, _)| batch.is_some())
        .map(|(_, indices)| indices.len())
        .sum();
    mesh.world_index_count = u32::try_from(world_indices).ok()?;
    mesh.far_places = places;
    Some(mesh)
}

/// The opaque faces of one part of the world, for shadow maps.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ShadowGroup {
    /// The triangles.
    pub(crate) indices: Vec<u32>,
    /// The middle and size of what they cover, or `None` for all the world.
    pub(crate) bounds: Option<(Vec3, f32)>,
}

/// The faces `wanted` picks, grouped by tile, so a shadow map can leave out what
/// its light can't reach. A map with no tiles is one group.
pub(crate) fn shadow_groups(
    map: &Map,
    mesh: &WorldMesh,
    wanted: impl Fn(&Option<u32>) -> bool,
) -> Vec<ShadowGroup> {
    let faces = (0..).zip(mesh.face_batches.iter().zip(&mesh.face_indices));
    let faces = faces.filter(|(_, (batch, _))| wanted(batch));
    if map.tile_size <= 0.0 {
        let indices = faces.flat_map(|(_, (_, indices))| indices.iter().copied());
        return vec![ShadowGroup {
            indices: indices.collect(),
            bounds: None,
        }];
    }
    let mut cells: HashMap<[i32; 2], (Vec<u32>, Vec3, Vec3)> = HashMap::new();
    for (number, (_, indices)) in faces {
        let Some(face) = map.faces.get::<usize>(number) else {
            continue;
        };
        let cell = map.tile_of(face_center(map, face)).0;
        let far = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
        let entry = cells
            .entry(cell)
            .or_insert_with(|| (Vec::new(), far, far * -1.0));
        entry.0.extend_from_slice(indices);
        for point in map.face_points(face) {
            entry.1 = Vec3::new(
                entry.1.x.min(point.x),
                entry.1.y.min(point.y),
                entry.1.z.min(point.z),
            );
            entry.2 = Vec3::new(
                entry.2.x.max(point.x),
                entry.2.y.max(point.y),
                entry.2.z.max(point.z),
            );
        }
    }
    cells
        .into_values()
        .map(|(indices, low, high)| ShadowGroup {
            indices,
            bounds: Some(((low + high) * 0.5, (high - low).length() * 0.5)),
        })
        .collect()
}

/// The light of `tile`, as a square for the atlas, or `None` if it has no faces
/// or won't fit.
#[must_use]
pub fn tile_image(mesh: &WorldMesh, map: &Map, tile: u32, light: &TileLight) -> Option<TileImage> {
    let faces = mesh.tile_faces.get(tile as usize)?;
    let lights: Vec<(u32, FaceLight)> = faces
        .iter()
        .filter_map(|face| Some((*face, map.faces.get(*face as usize)?.light?)))
        .collect();
    // A chart's faces share their light, so it goes in once.
    let mut first_of: HashMap<u32, usize> = HashMap::new();
    let mut sizes: Vec<[u32; 2]> = Vec::new();
    let mut slots = Vec::with_capacity(lights.len());
    for (face, light) in &lights {
        let charted = map
            .faces
            .get(*face as usize)
            .is_some_and(|f| f.light_axes.is_some());
        let known = charted
            .then(|| first_of.get(&light.first_sample).copied())
            .flatten();
        slots.push(known.unwrap_or_else(|| {
            sizes.push([light.width * light.style_count().max(1), light.height]);
            if charted {
                first_of.insert(light.first_sample, sizes.len() - 1);
            }
            sizes.len() - 1
        }));
    }
    let packed = Atlas::pack_tile(&sizes, mesh.atlas_size / 2)?;
    let side = packed.size as usize;
    let mut image = TileImage {
        size: packed.size,
        bytes: vec![0; side * side * ATLAS_PIXEL],
        directions: vec![128; side * side * ATLAS_PIXEL],
        paint: vec![0; side * side * ATLAS_PIXEL],
        places: Vec::with_capacity(lights.len()),
    };
    let samples = Samples::of_tile(light);
    let mut copied = vec![false; sizes.len()];
    for ((face, found), slot) in lights.iter().zip(&slots) {
        let position = packed.positions.get(*slot)?;
        if !std::mem::replace(&mut copied[*slot], true) {
            copy_light(
                samples,
                found,
                *position,
                side,
                &mut image.bytes,
                &mut image.directions,
            );
            copy_paint(samples, found, *position, side, &mut image.paint);
        }
        image.places.push((*face, *position));
    }
    Some(image)
}

/// Where the light of the chunk that face `face_at` stands in for is, given its place in the atlas.
fn chunk_light(
    map: &Map,
    face_at: usize,
    place: Option<LightPlace>,
    neutral: [u32; 2],
    atlas_size: u32,
    detail: bool,
) -> ChunkLight {
    let corner = |index: usize| {
        let face = map.faces.get(face_at)?;
        let point = *map.face_points(face).get(index)?;
        let number = face.first_point as usize + index;
        let point = (number, point);
        Some(make_vertex(
            map, face, point, place, neutral, atlas_size, detail,
        ))
    };
    let (low, high) = (corner(0).unwrap_or_default(), corner(2).unwrap_or_default());
    ChunkLight {
        low: low.light_uv,
        high: high.light_uv,
        step: low.light_step,
        styles: low.styles,
    }
}

/// What the light of `image`'s chunks of land becomes with it at `corner`, by chunk.
#[must_use]
pub(crate) fn detail_chunks(
    map: &Map,
    mesh: &WorldMesh,
    image: &TileImage,
    corner: [u32; 2],
) -> Vec<(u32, ChunkLight)> {
    image
        .places
        .iter()
        .filter_map(|(face, position)| {
            let chunk = mesh.face_chunk.get(*face as usize).copied().flatten()?;
            let light = map.faces.get(*face as usize)?.light?;
            let place = LightPlace {
                position: [corner[0] + position[0], corner[1] + position[1]],
                width: light.width,
            };
            let found = chunk_light(
                map,
                *face as usize,
                Some(place),
                [0, 0],
                mesh.atlas_size,
                true,
            );
            Some((chunk, found))
        })
        .collect()
}

/// What the light of `tile`'s chunks of land goes back to with its light gone, by chunk.
#[must_use]
pub(crate) fn far_chunks(map: &Map, mesh: &WorldMesh, tile: u32) -> Vec<(u32, ChunkLight)> {
    let faces = mesh.tile_faces.get(tile as usize);
    faces
        .into_iter()
        .flatten()
        .filter_map(|face| {
            let chunk = mesh.face_chunk.get(*face as usize).copied().flatten()?;
            let place = mesh.far_places.get(*face as usize).copied().flatten();
            let found = chunk_light(map, *face as usize, place, [0, 0], mesh.atlas_size, false);
            Some((chunk, found))
        })
        .collect()
}

/// `face`'s vertices, given its place in the atlas. A chunk of land has none: its light goes with
/// the land, so it's for [`detail_chunks`] and [`far_chunks`].
fn patch_of(
    map: &Map,
    mesh: &WorldMesh,
    face: u32,
    place: Option<LightPlace>,
    detail: bool,
) -> Option<VertexPatch> {
    if mesh
        .face_chunk
        .get(face as usize)
        .copied()
        .flatten()
        .is_some()
    {
        return None;
    }
    let found = map.faces.get(face as usize)?;
    let (first, _) = *mesh.face_vertices.get(face as usize)?;
    let mut bytes = Vec::with_capacity(found.point_count as usize * Vertex::SIZE);
    let numbered = (found.first_point as usize..).zip(map.face_points(found));
    for (number, &point) in numbered {
        let point = (number, point);
        let vertex = make_vertex(map, found, point, place, [0, 0], mesh.atlas_size, detail);
        vertex.write(&mut bytes);
    }
    Some((first, bytes))
}

/// What the vertices of `image`'s faces become with it at `corner`.
#[must_use]
pub(crate) fn detail_patches(
    map: &Map,
    mesh: &WorldMesh,
    image: &TileImage,
    corner: [u32; 2],
) -> Vec<VertexPatch> {
    image
        .places
        .iter()
        .filter_map(|(face, position)| {
            let light = map.faces.get(*face as usize)?.light?;
            let place = LightPlace {
                position: [corner[0] + position[0], corner[1] + position[1]],
                width: light.width,
            };
            patch_of(map, mesh, *face, Some(place), true)
        })
        .collect()
}

/// What the vertices of `tile`'s faces go back to with its light gone.
#[must_use]
pub(crate) fn far_patches(map: &Map, mesh: &WorldMesh, tile: u32) -> Vec<VertexPatch> {
    let faces = mesh.tile_faces.get(tile as usize);
    faces
        .into_iter()
        .flatten()
        .filter_map(|face| {
            let place = mesh.far_places.get(*face as usize).copied().flatten();
            patch_of(map, mesh, *face, place, false)
        })
        .collect()
}
