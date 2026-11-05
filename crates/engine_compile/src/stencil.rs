//! Ray stencils, from entities (proposal 0076).

use std::sync::Arc;

use engine_core::math::Vec3;
use engine_format::Value;
use engine_map::{RayStencil, stencil_boxes};

use crate::build::TextureSource;
use crate::error::CompileError;
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
