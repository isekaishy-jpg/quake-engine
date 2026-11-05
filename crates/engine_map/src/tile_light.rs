//! A tiled map's light, kept beside its bake and held only for tiles near the
//! view (proposal 0095).

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, PoisonError, RwLock};

use engine_core::math::Vec3;

use crate::nav_chunks::SideFiles;

/// The tile of light that's always held, which is what a moving brush model has,
/// as it can go anywhere.
pub const RESIDENT: u32 = u32::MAX;

/// The light of one tile. A face's samples are found by its
/// [`crate::FaceLight::first_sample`], as they are in a whole map.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TileLight {
    /// Samples, as RGB.
    pub light: Vec<[u8; 3]>,
    /// Where each sample's light comes from.
    pub directions: Vec<[u8; 3]>,
    /// How far each sample is from a shadow's edge, or nothing if nothing is
    /// crisp.
    pub edges: Vec<u8>,
    /// How much of each layer is painted, or nothing if nothing is.
    pub paint: Vec<[u8; 4]>,
}

impl TileLight {
    /// The tile as bytes: how long each part is, then the parts.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let lengths = [
            self.light.len(),
            self.directions.len(),
            self.edges.len(),
            self.paint.len(),
        ];
        let mut out = Vec::new();
        for length in lengths {
            let length = u32::try_from(length).unwrap_or(u32::MAX);
            out.extend_from_slice(&length.to_le_bytes());
        }
        out.extend_from_slice(self.light.as_flattened());
        out.extend_from_slice(self.directions.as_flattened());
        out.extend_from_slice(&self.edges);
        out.extend_from_slice(self.paint.as_flattened());
        out
    }

    /// A tile from bytes, if they make one.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let (header, rest) = bytes.split_first_chunk::<16>()?;
        let (words, _) = header.as_chunks::<4>();
        let mut lengths = words.iter().map(|word| u32::from_le_bytes(*word) as usize);
        let (light, directions) = (lengths.next()?, lengths.next()?);
        let (edges, paint) = (lengths.next()?, lengths.next()?);
        let (light_bytes, rest) = rest.split_at_checked(light.checked_mul(3)?)?;
        let (direction_bytes, rest) = rest.split_at_checked(directions.checked_mul(3)?)?;
        let (edge_bytes, rest) = rest.split_at_checked(edges)?;
        let (paint_bytes, rest) = rest.split_at_checked(paint.checked_mul(4)?)?;
        rest.is_empty().then(|| Self {
            light: light_bytes.as_chunks::<3>().0.to_vec(),
            directions: direction_bytes.as_chunks::<3>().0.to_vec(),
            edges: edge_bytes.to_vec(),
            paint: paint_bytes.as_chunks::<4>().0.to_vec(),
        })
    }
}

/// What a tiled map knows of its light without holding it.
pub struct LightTiles {
    /// How wide a tile is.
    tile_size: f32,
    /// Each tile's place on the grid, by tile number.
    cells: Vec<[i32; 2]>,
    /// Tile numbers by place.
    numbers: HashMap<[i32; 2], u32>,
    /// What's in front of a tile's file name.
    folder: String,
    /// Where its files are.
    files: Option<SideFiles>,
    /// The tiles being held, by tile number.
    held: Vec<RwLock<Option<Arc<TileLight>>>>,
}

impl LightTiles {
    /// The light of tiles at `cells`, `tile_size` wide.
    #[must_use]
    pub fn new(tile_size: f32, cells: Vec<[i32; 2]>) -> Self {
        let numbers = (0..).zip(&cells).map(|(number, cell)| (*cell, number));
        let numbers = numbers.collect();
        let held = cells.iter().map(|_| RwLock::new(None)).collect();
        Self {
            tile_size,
            cells,
            numbers,
            folder: String::new(),
            files: None,
            held,
        }
    }

    /// What a tile's file is called.
    #[must_use]
    pub fn tile_file(cell: [i32; 2]) -> String {
        format!("light_{}_{}.ebake", cell[0], cell[1])
    }

    /// Says where to read tiles from, with `folder` in front of their names.
    pub fn attach(&mut self, folder: &str, files: SideFiles) {
        folder.clone_into(&mut self.folder);
        self.files = Some(files);
    }

    /// Each tile's place on the grid, by tile number.
    #[must_use]
    pub fn cells(&self) -> &[[i32; 2]] {
        &self.cells
    }

    /// The tile numbers within `reach` tiles of `point`, nearest first.
    #[must_use]
    pub fn tiles_near(&self, point: Vec3, reach: i32) -> Vec<u32> {
        let size = self.tile_size.max(1.0);
        #[expect(clippy::cast_possible_truncation, reason = "grid places are small")]
        let (x, y) = (
            (point.x / size).floor() as i32,
            (point.y / size).floor() as i32,
        );
        let mut near = Vec::new();
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                if let Some(number) = self.numbers.get(&[x + dx, y + dy]) {
                    near.push((dx * dx + dy * dy, *number));
                }
            }
        }
        near.sort_unstable();
        near.into_iter().map(|(_, number)| number).collect()
    }

    /// Reads a tile's light from its file, on any thread, without holding it.
    #[must_use]
    pub fn read(&self, tile: u32) -> Option<TileLight> {
        let cell = self.cells.get(tile as usize)?;
        let path = format!("{}{}", self.folder, Self::tile_file(*cell));
        let bytes = self.files.as_ref()?(&path)?;
        TileLight::from_bytes(&bytes)
    }

    /// Holds a tile's light, or lets it go, for what asks about light at a
    /// point.
    pub fn hold(&self, tile: u32, light: Option<Arc<TileLight>>) {
        if let Some(slot) = self.held.get(tile as usize) {
            *slot.write().unwrap_or_else(PoisonError::into_inner) = light;
        }
    }

    /// A tile's light, if it's held.
    #[must_use]
    pub fn held(&self, tile: u32) -> Option<Arc<TileLight>> {
        let slot = self.held.get(tile as usize)?;
        slot.read()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(Arc::clone)
    }
}

impl Clone for LightTiles {
    fn clone(&self) -> Self {
        let mut copy = Self::new(self.tile_size, self.cells.clone());
        copy.folder.clone_from(&self.folder);
        copy.files.clone_from(&self.files);
        copy
    }
}

impl PartialEq for LightTiles {
    fn eq(&self, other: &Self) -> bool {
        self.tile_size.to_bits() == other.tile_size.to_bits() && self.cells == other.cells
    }
}

impl fmt::Debug for LightTiles {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LightTiles")
            .field("tile_size", &self.tile_size)
            .field("tiles", &self.cells.len())
            .finish_non_exhaustive()
    }
}
