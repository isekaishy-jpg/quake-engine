//! Land, as it's kept in a file and as a level names it (proposal 0144).

use engine_core::math::Vec3;
use engine_format::{Block, Document, Value, from_binary, to_binary};
use engine_map::{Face, TerrainData, TerrainSheet, TexelAxis};

use crate::error::CompileError;

/// What a level says of a sheet of land.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TerrainSource {
    /// The file its heights are in, under `terrain/`.
    pub source: String,
    /// Where its low corner goes.
    pub origin: [f64; 3],
    /// Whether it faces down.
    pub facing_down: bool,
    /// How thick it is, or 0 for as far as it goes.
    pub thickness: f64,
    /// Its material.
    pub material: String,
    /// Its heights, once they've been read.
    pub data: TerrainData,
}

/// Reads a `terrain` block.
pub(crate) fn read_terrain(block: &Block) -> Result<TerrainSource, CompileError> {
    let text = |name: &str| block.get(name).and_then(Value::text);
    let source = text("source").ok_or_else(|| CompileError::Bad("land needs a source".into()))?;
    let origin = block.get("origin").and_then(Value::numbers);
    let origin = match origin.as_deref() {
        Some(&[x, y, z]) => [x, y, z],
        _ => [0.0; 3],
    };
    Ok(TerrainSource {
        source: source.to_owned(),
        origin,
        facing_down: text("facing") == Some("down"),
        thickness: block
            .get("thickness")
            .and_then(Value::number)
            .unwrap_or(0.0),
        material: text("material").unwrap_or("ground").to_owned(),
        data: TerrainData::default(),
    })
}

/// `data` as a file.
#[must_use]
pub fn write_terrain(data: &TerrainData) -> Vec<u8> {
    let mut document = Document::new("terrain");
    let mut block = Block::new("heights");
    block.set("cell", Value::Number(f64::from(data.cell)));
    block.set("across", Value::Number(f64::from(data.cells[0])));
    block.set("down", Value::Number(f64::from(data.cells[1])));
    let bytes = data.heights.iter().flat_map(|height| height.to_le_bytes());
    block.set("heights", Value::Bytes(bytes.collect()));
    block.set("holes", Value::Bytes(data.holes.clone()));
    document.blocks.push(block);
    to_binary(&document)
}

/// The land in a file.
///
/// # Errors
///
/// Returns an error if it isn't land, or its heights don't fit its size.
pub fn read_terrain_data(bytes: &[u8]) -> Result<TerrainData, CompileError> {
    let document = from_binary(bytes)?;
    let bad = |what: &str| CompileError::Bad(format!("land {what}"));
    let block = document
        .blocks_of("heights")
        .next()
        .ok_or_else(|| bad("has no heights"))?;
    let number = |name: &str| block.get(name).and_then(Value::number);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a count of cells"
    )]
    let cells = [
        number("across").ok_or_else(|| bad("has no size"))? as u32,
        number("down").ok_or_else(|| bad("has no size"))? as u32,
    ];
    let raw = block.get("heights").and_then(Value::bytes).unwrap_or(&[]);
    let heights: Vec<f32> = raw
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| f32::from_le_bytes(*bytes))
        .collect();
    if heights.len() != (cells[0] as usize + 1) * (cells[1] as usize + 1) {
        return Err(bad("has the wrong number of heights"));
    }
    #[expect(clippy::cast_possible_truncation, reason = "land is in f32")]
    let cell = number("cell").unwrap_or(32.0) as f32;
    let holes = block.get("holes").and_then(Value::bytes).unwrap_or(&[]);
    Ok(TerrainData {
        cell,
        cells,
        heights,
        holes: holes.to_vec(),
    })
}

/// What stands in for the chunk at `x`, `y` of `sheet`: a flat face over it, which is never drawn.
/// It holds the chunk's light, and is what finds it in leaves.
pub(crate) fn chunk_points(sheet: &TerrainSheet, x: u32, y: u32) -> [Vec3; 4] {
    let cells = engine_map::CHUNK_CELLS;
    let [low, high] = sheet.chunk_heights(x, y);
    let middle = (low + high) * 0.5;
    let corner = |column: u32, row: u32| {
        let place = sheet.sample(
            column.min(sheet.data.cells[0]),
            row.min(sheet.data.cells[1]),
        );
        Vec3::new(place.x, place.y, middle)
    };
    let (left, near) = (x * cells, y * cells);
    [
        corner(left, near),
        corner(left + cells, near),
        corner(left + cells, near + cells),
        corner(left, near + cells),
    ]
}

/// A chunk's face, given where its points are.
pub(crate) fn chunk_face(first_point: u32, texture: u32) -> Face {
    let axis = |axis: Vec3| TexelAxis { axis, offset: 0.0 };
    Face {
        first_point,
        point_count: 4,
        texture,
        axes: [
            axis(Vec3::new(1.0, 0.0, 0.0)),
            axis(Vec3::new(0.0, 1.0, 0.0)),
        ],
        light: None,
        light_axes: None,
    }
}
