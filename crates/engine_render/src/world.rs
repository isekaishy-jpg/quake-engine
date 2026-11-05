//! Turning a [`Map`] into data for the GPU. No GPU is involved here.

use engine_core::math::Vec3;
use engine_map::{AlphaMode, Face, FaceLight, Map, Texture, TextureKind};

use crate::atlas::Atlas;

/// The largest atlas side, which every Vulkan device of interest can sample.
const ATLAS_LIMIT: u32 = 8192;

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
}

impl Vertex {
    /// The size in bytes.
    pub const SIZE: usize = 36;

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
    }
}

/// How a batch is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BatchKind {
    /// Lit, opaque.
    Solid,
    /// Lit, with holes.
    Cutout,
    /// A moving liquid.
    Liquid,
    /// Sky.
    Sky,
    /// An opaque material.
    Material,
    /// A material with holes.
    Masked,
    /// A see-through material, drawn after everything else.
    Blended,
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
}

/// Where a face's light sits in the atlas.
#[derive(Clone, Copy, Debug, Default)]
struct LightPlace {
    /// The atlas position of its first style.
    position: [u32; 2],
    /// Its width in samples.
    width: u32,
}

/// Whether `texture` is drawn through the material pipelines. Upload and
/// batching must agree on this, or a draw has no material bound.
pub(crate) fn uses_material(texture: &Texture) -> bool {
    texture.material.as_ref().is_some_and(|material| {
        matches!(texture.kind, TextureKind::Solid | TextureKind::Cutout)
            || (texture.kind == TextureKind::Liquid && material.alpha == AlphaMode::Blend)
    })
}

/// The batch kind of a texture, or `None` for hidden ones.
fn batch_kind(texture: &Texture) -> Option<BatchKind> {
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
    match texture.kind {
        TextureKind::Solid => Some(BatchKind::Solid),
        TextureKind::Cutout => Some(BatchKind::Cutout),
        TextureKind::Liquid => Some(BatchKind::Liquid),
        TextureKind::Sky => Some(BatchKind::Sky),
        TextureKind::Hidden => None,
    }
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
    /// The pixels, in RGBA bytes.
    bytes: Vec<u8>,
    /// The pixels of the light's directions.
    directions: Vec<u8>,
    /// Each face's place, if it has one.
    places: Vec<Option<LightPlace>>,
    /// A pixel at full brightness, for faces in unlit maps.
    neutral: [u32; 2],
}

/// Packs every face's styles side by side.
fn build_atlas(map: &Map) -> Option<BuiltAtlas> {
    let lights: Vec<Option<FaceLight>> = map.faces.iter().map(|f| lit_face(map, f)).collect();
    let mut sizes: Vec<[u32; 2]> = lights
        .iter()
        .flatten()
        .map(|light| [light.width * light.style_count().max(1), light.height])
        .collect();
    sizes.push([1, 1]);
    let packed = Atlas::pack(&sizes, ATLAS_LIMIT)?;
    let side = packed.size as usize;
    let mut bytes = vec![0; side * side * ATLAS_PIXEL];
    let mut directions = vec![128; side * side * ATLAS_PIXEL];
    let mut positions = packed.positions.iter().copied();
    let mut places = Vec::with_capacity(lights.len());
    for light in &lights {
        let Some(light) = light else {
            places.push(None);
            continue;
        };
        let position = positions.next()?;
        copy_light(map, light, position, side, &mut bytes, &mut directions);
        places.push(Some(LightPlace {
            position,
            width: light.width,
        }));
    }
    let neutral = positions.next()?;
    let at = (neutral[1] as usize * side + neutral[0] as usize) * ATLAS_PIXEL;
    bytes[at..at + ATLAS_PIXEL].copy_from_slice(&[FULL_LIGHT, FULL_LIGHT, FULL_LIGHT, 255]);
    Some(BuiltAtlas {
        size: packed.size,
        bytes,
        directions,
        places,
        neutral,
    })
}

/// Copies a face's samples into the atlas, one style after another.
fn copy_light(
    map: &Map,
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
                let Some(sample) = map.light.get(first + y * width + x) else {
                    continue;
                };
                let at = ((top + y) * side + left + style * width + x) * ATLAS_PIXEL;
                bytes[at..at + 3].copy_from_slice(sample);
                bytes[at + 3] = 255;
                if let Some(direction) = map.light_directions.get(first + y * width + x) {
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
    point: Vec3,
    place: Option<LightPlace>,
    neutral: [u32; 2],
    atlas_size: u32,
) -> Vertex {
    let size = texture_size(map, face);
    let s = face.axes[0].project(point);
    let t = face.axes[1].project(point);
    let mut vertex = Vertex {
        position: [point.x, point.y, point.z],
        uv: [s / size[0], t / size[1]],
        styles: [Face::NO_STYLE; 4],
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
    if let (Some(place), Some(light)) = (place, face.light) {
        let step = light.texels_per_sample as f32;
        let x = (s - light.texel_origin[0] as f32) / step + 0.5;
        let y = (t - light.texel_origin[1] as f32) / step + 0.5;
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
    #[expect(
        clippy::cast_precision_loss,
        reason = "texture sizes are far below 2^24"
    )]
    map.textures
        .get(face.texture as usize)
        .and_then(|texture| texture.mips.first())
        .map_or([1.0, 1.0], |mip| [mip.width as f32, mip.height as f32])
}

/// Builds the GPU data of `map`, or `None` if its light won't fit the atlas.
#[must_use]
pub fn build_world(map: &Map) -> Option<WorldMesh> {
    let BuiltAtlas {
        size: atlas_size,
        bytes: atlas,
        directions: direction_atlas,
        places,
        neutral,
    } = build_atlas(map)?;
    let mut mesh = WorldMesh {
        atlas_size,
        atlas,
        direction_atlas,
        ..WorldMesh::default()
    };
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
    mesh.model_ranges = vec![Vec::new(); map.models.len()];
    for (number, model) in map.models.iter().enumerate() {
        let mut per_batch: Vec<Vec<u32>> = vec![Vec::new(); mesh.batches.len()];
        for index in model.first_face..model.first_face.saturating_add(model.face_count) {
            let at = index as usize;
            let Some(face) = map.faces.get(at) else {
                continue;
            };
            let Some(mut batch) = batch_of_texture
                .get(face.texture as usize)
                .copied()
                .flatten()
            else {
                continue;
            };
            // What's see-through is drawn far to near, so each face stands
            // alone.
            if mesh.batches[batch as usize].kind == BatchKind::Blended {
                let shared = mesh.batches[batch as usize].clone();
                batch = u32::try_from(mesh.batches.len()).ok()?;
                mesh.batches.push(shared);
                mesh.batch_centers.push(face_center(map, face));
                per_batch.push(Vec::new());
            }
            let first = u32::try_from(mesh.vertices.len()).ok()?;
            for &point in map.face_points(face) {
                mesh.vertices.push(make_vertex(
                    map, face, point, places[at], neutral, atlas_size,
                ));
            }
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
    Some(mesh)
}
