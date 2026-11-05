//! Ray stencils and flames, from entities (proposals 0076 and 0079).

use std::sync::Arc;

use engine_core::math::Vec3;
use engine_format::Value;
use engine_map::{Emitter, LiveLightSource, RayStencil, stencil_boxes};

use crate::build::TextureSource;
use crate::error::CompileError;
use crate::geometry::Point;
use crate::mesh::{PlacedMesh, PlacedTriangle};
use crate::source::EntitySource;

/// The value of a field.
fn field<'a>(entity: &'a EntitySource, key: &str) -> Option<&'a Value> {
    entity
        .fields
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value)
}

/// A field's points, three numbers each.
#[expect(clippy::cast_possible_truncation, reason = "stencils store f32")]
fn points_of(entity: &EntitySource, key: &str) -> Vec<Vec3> {
    field(entity, key)
        .and_then(Value::numbers)
        .map_or_else(Vec::new, |numbers| {
            let (points, _) = numbers.as_chunks::<3>();
            points
                .iter()
                .map(|[x, y, z]| Vec3::new(*x as f32, *y as f32, *z as f32))
                .collect()
        })
}

/// A field's number.
#[expect(clippy::cast_possible_truncation, reason = "stencils store f32")]
fn number_of(entity: &EntitySource, key: &str, default: f32) -> f32 {
    field(entity, key)
        .and_then(Value::number)
        .map_or(default, |value| value as f32)
}

/// How tall a flame is unless it says.
const FLAME_SIZE: f32 = 24.0;

/// A quad as two triangles, seen from both sides.
fn both_sides(corners: [Point; 4], material: &str) -> [PlacedTriangle; 4] {
    let [a, b, c, d] = corners;
    let (uv_a, uv_b, uv_c, uv_d) = ([0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]);
    let triangle = |points, uvs| PlacedTriangle {
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
        let Some(origin) = points_of(flame, "origin").first().copied() else {
            continue;
        };
        let size = number_of(flame, "size", FLAME_SIZE);
        let material = field(flame, "material")
            .and_then(Value::text)
            .unwrap_or("torch_flame");
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
    let origin = points_of(entity, "origin").first().copied()?;
    let color = points_of(entity, "color").first().copied();
    let color = color.map_or([1.0; 3], |c| [c.x, c.y, c.z]);
    let spot = points_of(entity, "spot_direction").first().copied();
    let spot = spot.map_or(Vec3::ZERO, Vec3::normalized);
    let degrees = number_of(entity, "spot_angle", 45.0);
    let shadows = field(entity, "shadows").is_some_and(|value| {
        value.flag().unwrap_or(false) || value.number().is_some_and(|number| number != 0.0)
    });
    Some(LiveLightSource {
        origin,
        radius: number_of(entity, "radius", 300.0),
        color,
        spot,
        spot_cosine: degrees.to_radians().cos(),
        shadows,
        path: points_of(entity, "path"),
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
    let point = |key: &str| points_of(entity, key).first().copied();
    let name = field(entity, "image")
        .and_then(Value::text)
        .ok_or_else(|| missing("an image"))?;
    let image = source.texture(name).mips;
    let sun = field(entity, "sun").is_some_and(|value| {
        value.flag().unwrap_or(false) || value.number().is_some_and(|number| number != 0.0)
    });
    let marks = field(entity, "marks").and_then(Value::text);
    let boxes = marks
        .and_then(|name| source.texture(name).mips.into_iter().next())
        .map_or_else(
            || vec![[0.0, 0.0, 1.0, 1.0]],
            |mip| stencil_boxes(mip.width as usize, mip.height as usize, &mip.rgba),
        );
    Ok(RayStencil {
        corner: point("corner").ok_or_else(|| missing("a corner"))?,
        across: point("across").ok_or_else(|| missing("an across edge"))?,
        down: point("down").ok_or_else(|| missing("a down edge"))?,
        light: point("light").ok_or_else(|| missing("a light"))?,
        sun,
        path: points_of(entity, "path"),
        period: number_of(entity, "period", 0.0),
        strength: number_of(entity, "strength", 1.0),
        reach: number_of(entity, "reach", 1024.0),
        image: Arc::new(image),
        boxes,
    })
}
