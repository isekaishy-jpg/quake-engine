//! Tiles: parts of a map in files of their own, in local coordinates, merged
//! into place (proposal 0089).

use engine_format::Value;

use crate::error::CompileError;
use crate::geometry::{Point, add_scaled, dot};
use crate::source::{Brush, EntitySource, WorkingMap};

/// How many tiles a grid can reach from the origin, each way, which keeps
/// positions in `f32` well clear of collision's smallest steps.
pub(crate) const MAX_TILE_REACH: i32 = 32;

/// Fields of an entity that are lists of places, to move with it.
const PLACE_FIELDS: [&str; 4] = ["origin", "corner", "path", "keyframes"];

/// `values` moved by `offset`, three numbers at a time.
fn moved_numbers(values: &[Value], offset: Point) -> Vec<Value> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| match value {
            Value::Number(number) => Value::Number(number + offset[index % 3]),
            other => other.clone(),
        })
        .collect()
}

/// `entity` moved by `offset`.
fn move_entity(entity: &mut EntitySource, offset: Point) {
    let sun = entity.fields.iter().any(|(key, value)| {
        key == "sun" && (value.flag() == Some(true) || value.number().is_some_and(|n| n != 0.0))
    });
    for (key, value) in &mut entity.fields {
        // A stencil's light is a place, unless it's the sun's direction.
        let moves = PLACE_FIELDS.contains(&key.as_str()) || (key == "light" && !sun);
        if !moves {
            continue;
        }
        if let Value::List(values) = value {
            *values = moved_numbers(values, offset);
        }
    }
    for brush in &mut entity.brushes {
        move_brush(brush, offset);
    }
    if let Some(mesh) = &mut entity.mesh {
        mesh.origin = add_scaled(mesh.origin, offset, 1.0);
    }
}

/// `brush` moved by `offset`.
fn move_brush(brush: &mut Brush, offset: Point) {
    for face in &mut brush.faces {
        face.plane.distance += dot(face.plane.normal, offset);
    }
}

/// Moves what `tile` holds by `offset`, and adds it to `map`.
pub(crate) fn merge_tile(map: &mut WorkingMap, mut tile: WorkingMap, offset: Point) {
    for brush in &mut tile.brushes {
        move_brush(brush, offset);
    }
    for entity in &mut tile.entities {
        move_entity(entity, offset);
    }
    for mesh in &mut tile.meshes {
        mesh.origin = add_scaled(mesh.origin, offset, 1.0);
    }
    for decal in &mut tile.decals {
        decal.origin = add_scaled(decal.origin, offset, 1.0);
    }
    for stamp in &mut tile.stamps {
        stamp.origin = add_scaled(stamp.origin, offset, 1.0);
    }
    for stroke in &mut tile.paint {
        for point in &mut stroke.points {
            *point = add_scaled(*point, offset, 1.0);
        }
    }
    map.brushes.append(&mut tile.brushes);
    map.entities.append(&mut tile.entities);
    map.meshes.append(&mut tile.meshes);
    map.decals.append(&mut tile.decals);
    map.stamps.append(&mut tile.stamps);
    map.paint.append(&mut tile.paint);
}

/// Checks a map's tiles are on a grid it can hold.
///
/// # Errors
///
/// Returns an error if a tile is too far out, or there are tiles with no grid.
pub(crate) fn check_tiles(map: &WorkingMap) -> Result<(), CompileError> {
    if !map.tiles.is_empty() && map.tile_size <= 0.0 {
        return Err(CompileError::Bad("tiles need a tile_grid size".to_owned()));
    }
    for tile in &map.tiles {
        if tile.x.abs() > MAX_TILE_REACH || tile.y.abs() > MAX_TILE_REACH {
            return Err(CompileError::Bad(format!(
                "tile {} {} is past {MAX_TILE_REACH} tiles from the origin",
                tile.x, tile.y
            )));
        }
    }
    Ok(())
}
