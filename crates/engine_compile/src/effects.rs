//! What entities become beyond scripts: ray stencils, flames and moving lights
//! (proposals 0076, 0079 and 0084).

use std::sync::Arc;

use engine_core::math::Vec3;
use engine_map::{Emitter, LiveLightSource, RayStencil, stencil_boxes};

use crate::build::TextureSource;
use crate::error::CompileError;
use crate::geometry::Point;
use crate::mesh::{PlacedMesh, PlacedTriangle, WHITE_CORNERS};
use crate::source::EntitySource;

/// A point as the map stores it.
#[expect(clippy::cast_possible_truncation, reason = "the map stores f32")]
fn stored(point: Point) -> Vec3 {
    Vec3::new(point[0] as f32, point[1] as f32, point[2] as f32)
}

/// A field's number as the map stores it, or `default`.
#[expect(clippy::cast_possible_truncation, reason = "the map stores f32")]
fn number_of(entity: &EntitySource, key: &str, default: f32) -> f32 {
    entity.number(key).map_or(default, |value| value as f32)
}

/// How tall a flame is unless it says.
const FLAME_SIZE: f32 = 24.0;

/// A quad as two triangles, seen from both sides.
fn both_sides(corners: [Point; 4], material: &str) -> [PlacedTriangle; 4] {
    let [a, b, c, d] = corners;
    let (uv_a, uv_b, uv_c, uv_d) = ([0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]);
    let triangle = |points, uvs| PlacedTriangle {
        normals: [[0.0; 3]; 3],
        colors: WHITE_CORNERS,
        points,
        uvs,
        material: material.to_owned(),
    };
    [
        triangle([a, b, c], [uv_a, uv_b, uv_c]),
        triangle([a, c, d], [uv_a, uv_c, uv_d]),
        triangle([c, b, a], [uv_c, uv_b, uv_a]),
        triangle([d, c, a], [uv_d, uv_c, uv_a]),
    ]
}

/// The faces and emitters of `flame` entities: two crossed quads each, with
/// the flame's animation in its material, and no collision.
pub(crate) fn build_flames<'a>(
    flames: impl Iterator<Item = &'a EntitySource>,
) -> (PlacedMesh, Vec<Emitter>) {
    let mut faces = PlacedMesh::default();
    let mut emitters = Vec::new();
    for flame in flames {
        let Some(origin) = flame.point("origin").map(stored) else {
            continue;
        };
        let size = number_of(flame, "size", FLAME_SIZE);
        let material = flame.text("material").unwrap_or("torch_flame");
        let (x, y, z) = (
            f64::from(origin.x),
            f64::from(origin.y),
            f64::from(origin.z),
        );
        let half = f64::from(size) * 0.5;
        let top = z + f64::from(size);
        faces.triangles.extend(both_sides(
            [
                [x - half, y, z],
                [x + half, y, z],
                [x + half, y, top],
                [x - half, y, top],
            ],
            material,
        ));
        faces.triangles.extend(both_sides(
            [
                [x, y - half, z],
                [x, y + half, z],
                [x, y + half, top],
                [x, y - half, top],
            ],
            material,
        ));
        emitters.push(Emitter {
            origin: origin + Vec3::new(0.0, 0.0, size * 0.5),
            light_radius: number_of(flame, "light", size * 6.0),
            sparks: number_of(flame, "sparks", 3.0),
        });
    }
    (faces, emitters)
}

/// The light of a `live_light` entity, if it says where it is.
pub(crate) fn build_live_light(entity: &EntitySource) -> Option<LiveLightSource> {
    let origin = entity.point("origin").map(stored)?;
    let color = entity.point("color").map(stored);
    let color = color.map_or([1.0; 3], |c| [c.x, c.y, c.z]);
    let spot = entity.point("spot_direction").map(stored);
    let spot = spot.map_or(Vec3::ZERO, Vec3::normalized);
    let degrees = number_of(entity, "spot_angle", 45.0);
    Some(LiveLightSource {
        origin,
        radius: number_of(entity, "radius", 300.0),
        color,
        spot,
        spot_cosine: degrees.to_radians().cos(),
        shadows: entity.is_on("shadows"),
        bounce: number_of(entity, "bounce", 1.0).max(0.0),
        path: entity.points("path").into_iter().map(stored).collect(),
        period: number_of(entity, "period", 0.0),
    })
}

/// Builds a `ray_stencil` entity.
///
/// # Errors
///
/// Returns an error if it's missing what it needs.
pub(crate) fn build_stencil(
    entity: &EntitySource,
    source: &mut dyn TextureSource,
) -> Result<RayStencil, CompileError> {
    let missing = |what: &str| CompileError::Bad(format!("a ray_stencil needs {what}"));
    let point = |key: &str| entity.point(key).map(stored);
    let name = entity.text("image").ok_or_else(|| missing("an image"))?;
    let image = source.pixels(name).mips;
    let boxes = entity
        .text("marks")
        .and_then(|name| source.pixels(name).mips.into_iter().next())
        .map_or_else(
            || vec![[0.0, 0.0, 1.0, 1.0]],
            |mip| stencil_boxes(mip.width as usize, mip.height as usize, &mip.rgba),
        );
    Ok(RayStencil {
        corner: point("corner").ok_or_else(|| missing("a corner"))?,
        across: point("across").ok_or_else(|| missing("an across edge"))?,
        down: point("down").ok_or_else(|| missing("a down edge"))?,
        light: point("light").ok_or_else(|| missing("a light"))?,
        sun: entity.is_on("sun"),
        path: entity.points("path").into_iter().map(stored).collect(),
        period: number_of(entity, "period", 0.0),
        strength: number_of(entity, "strength", 1.0),
        reach: number_of(entity, "reach", 1024.0),
        image: Arc::new(image),
        boxes,
    })
}
