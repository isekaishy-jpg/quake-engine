//! Walkable grids in chunks, read as a search reaches them, for maps too big to
//! hold all of theirs (proposal 0095).

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use engine_core::math::Vec3;

use crate::nav::{LINKS, NAV_WAYS, NONE, NavGrid, SEARCH_REACH, Walk, find_path_on};

/// How many columns a chunk has each way, in a real map.
pub const CHUNK_COLUMNS: u32 = 128;

/// A link to nothing.
const NO_LINK: u8 = u8::MAX;

/// How many chunks a grid keeps. A search across a whole world goes on working,
/// and reads again what's fallen out.
const CHUNKS_KEPT: usize = 48;

/// Reads a file beside a map, from any thread.
pub type SideFiles = Arc<dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync>;

/// A square of a grid's columns. A link is a level of the next column that way,
/// so it says the same thing whichever chunk that column is in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NavChunk {
    /// The first place of each column, with one more at the end.
    first: Vec<u32>,
    /// Each place's height.
    heights: Vec<f32>,
    /// Each place's links.
    links: Vec<[u8; LINKS]>,
}

impl NavChunk {
    /// The chunk as bytes: for each column how many places it has, then each
    /// place's height and links.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.first.len() + self.heights.len() * 12);
        for pair in self.first.windows(2) {
            out.push(u8::try_from(pair[1] - pair[0]).unwrap_or(NO_LINK));
        }
        for (height, links) in self.heights.iter().zip(&self.links) {
            out.extend_from_slice(&height.to_le_bytes());
            out.extend_from_slice(links);
        }
        out
    }

    /// A chunk of `columns` columns from bytes, if they make one.
    #[must_use]
    pub fn from_bytes(bytes: &[u8], columns: usize) -> Option<Self> {
        let counts = bytes.get(..columns)?;
        let mut first = Vec::with_capacity(columns + 1);
        let mut total = 0;
        first.push(0);
        for count in counts {
            total += u32::from(*count);
            first.push(total);
        }
        let (places, _) = bytes.get(columns..)?.as_chunks::<12>();
        if places.len() != total as usize {
            return None;
        }
        let mut heights = Vec::with_capacity(places.len());
        let mut links = Vec::with_capacity(places.len());
        for place in places {
            let (height, rest) = place.split_first_chunk::<4>()?;
            heights.push(f32::from_le_bytes(*height));
            links.push(*rest.first_chunk::<LINKS>()?);
        }
        Some(Self {
            first,
            heights,
            links,
        })
    }
}

/// A grid's shape, without its places.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NavShape {
    /// The low corner.
    pub origin: Vec3,
    /// How far apart places are.
    pub spacing: f32,
    /// How many columns across and along.
    pub size: [u32; 2],
    /// How many columns a chunk has each way.
    pub chunk: u32,
}

impl NavShape {
    /// How many chunks across and along.
    #[must_use]
    pub const fn chunks(&self) -> [u32; 2] {
        [
            self.size[0].div_ceil(self.chunk),
            self.size[1].div_ceil(self.chunk),
        ]
    }

    /// Where a column is: its chunk, and its place in it.
    const fn locate(&self, x: u32, y: u32) -> ([u32; 2], usize) {
        let chunk = [x / self.chunk, y / self.chunk];
        let inside = (y % self.chunk) * self.chunk + x % self.chunk;
        (chunk, inside as usize)
    }
}

/// `grid`'s columns as chunks, with where each is. Chunks with nothing to stand
/// on aren't there.
#[must_use]
pub fn chunks_of(grid: &NavGrid, chunk: u32) -> Vec<([u32; 2], NavChunk)> {
    let shape = NavShape {
        origin: grid.origin,
        spacing: grid.spacing,
        size: grid.size,
        chunk,
    };
    let level_of = levels(grid);
    let [across, along] = shape.chunks();
    let mut out = Vec::new();
    for cy in 0..along {
        for cx in 0..across {
            let built = build_chunk(grid, &shape, &level_of, [cx, cy]);
            if !built.heights.is_empty() {
                out.push(([cx, cy], built));
            }
        }
    }
    out
}

/// Each place's level in its column.
fn levels(grid: &NavGrid) -> Vec<u8> {
    let mut out = vec![0; grid.places.len()];
    for pair in grid.columns.windows(2) {
        for (level, place) in (pair[0]..pair[1]).enumerate() {
            out[place as usize] = u8::try_from(level).unwrap_or(NO_LINK - 1);
        }
    }
    out
}

/// One chunk of `grid`.
fn build_chunk(grid: &NavGrid, shape: &NavShape, level_of: &[u8], at: [u32; 2]) -> NavChunk {
    let side = shape.chunk;
    let mut built = NavChunk {
        first: vec![0],
        ..NavChunk::default()
    };
    for y in 0..side {
        for x in 0..side {
            let (column_x, column_y) = (at[0] * side + x, at[1] * side + y);
            if column_x < shape.size[0] && column_y < shape.size[1] {
                let column = (column_y * shape.size[0] + column_x) as usize;
                for place in grid.places_in(column) {
                    let found = &grid.places[place as usize];
                    built.heights.push(found.point.z);
                    built.links.push(found.links.map(|link| {
                        if link == NONE {
                            NO_LINK
                        } else {
                            level_of[link as usize]
                        }
                    }));
                }
            }
            let count = u32::try_from(built.heights.len()).unwrap_or(u32::MAX);
            built.first.push(count);
        }
    }
    built
}

/// The most recent chunks, oldest first.
#[derive(Default)]
struct Kept {
    /// What's been read, or found to be empty.
    chunks: VecDeque<([u32; 2], Option<Arc<NavChunk>>)>,
    /// How many times a chunk has been read, for tests and for the console.
    reads: usize,
}

/// A place on a chunked grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ChunkPlace {
    /// Its column across.
    x: u32,
    /// Its column along.
    y: u32,
    /// Which floor of the column.
    level: u8,
}

/// A walkable grid read a chunk at a time.
pub struct ChunkedNav {
    /// Its shape.
    shape: NavShape,
    /// What a chunk's file is called, up to its coordinates.
    prefix: String,
    /// Where its files are.
    files: Option<SideFiles>,
    /// The most recent chunks.
    kept: Mutex<Kept>,
}

impl ChunkedNav {
    /// A grid of `shape`, with chunks in files starting `prefix`.
    #[must_use]
    pub fn new(shape: NavShape, prefix: String) -> Self {
        Self {
            shape,
            prefix,
            files: None,
            kept: Mutex::default(),
        }
    }

    /// Its shape.
    #[must_use]
    pub const fn shape(&self) -> NavShape {
        self.shape
    }

    /// Says where to read chunks from, with `folder` in front of their names.
    pub fn attach(&mut self, folder: &str, files: SideFiles) {
        self.prefix = format!("{folder}{}", self.prefix);
        self.files = Some(files);
    }

    /// What a chunk's file is called, from what's before its coordinates.
    #[must_use]
    pub fn chunk_file(prefix: &str, at: [u32; 2]) -> String {
        format!("{prefix}_{}_{}.ebake", at[0], at[1])
    }

    /// How many chunks are held.
    #[must_use]
    pub fn held(&self) -> usize {
        self.lock().chunks.len()
    }

    /// How many times a chunk has been read.
    #[must_use]
    pub fn reads(&self) -> usize {
        self.lock().reads
    }

    /// The cache, whatever a panicking reader left it as. It only holds copies.
    fn lock(&self) -> std::sync::MutexGuard<'_, Kept> {
        self.kept.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A chunk, read if it has to be.
    fn chunk(&self, at: [u32; 2]) -> Option<Arc<NavChunk>> {
        let mut kept = self.lock();
        if let Some(index) = kept.chunks.iter().position(|(place, _)| *place == at) {
            let entry = kept.chunks.remove(index)?;
            let found = entry.1.clone();
            kept.chunks.push_back(entry);
            return found;
        }
        let side = self.shape.chunk as usize;
        let read = self
            .files
            .as_ref()
            .and_then(|files| files(&Self::chunk_file(&self.prefix, at)))
            .and_then(|bytes| NavChunk::from_bytes(&bytes, side * side))
            .map(Arc::new);
        kept.reads += 1;
        kept.chunks.push_back((at, read.clone()));
        if kept.chunks.len() > CHUNKS_KEPT {
            kept.chunks.pop_front();
        }
        read
    }

    /// The heights of a column, with its chunk, to look further into.
    fn column(&self, x: u32, y: u32) -> Option<(Arc<NavChunk>, std::ops::Range<usize>)> {
        if x >= self.shape.size[0] || y >= self.shape.size[1] {
            return None;
        }
        let (at, inside) = self.shape.locate(x, y);
        let chunk = self.chunk(at)?;
        let first = *chunk.first.get(inside)? as usize;
        let last = *chunk.first.get(inside + 1)? as usize;
        Some((chunk, first..last))
    }

    /// The way from `from` to `to`, as [`NavGrid::find_path`] finds it.
    #[must_use]
    pub fn find_path(&self, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        find_path_on(self, from, to)
    }

    /// Where a column is.
    #[expect(clippy::cast_precision_loss, reason = "grids are far below 2^24")]
    fn column_point(&self, x: u32, y: u32) -> (f32, f32) {
        let shape = &self.shape;
        (
            shape.origin.x + x as f32 * shape.spacing,
            shape.origin.y + y as f32 * shape.spacing,
        )
    }
}

impl Walk for ChunkedNav {
    type Place = ChunkPlace;

    fn spacing(&self) -> f32 {
        self.shape.spacing
    }

    fn point(&self, place: ChunkPlace) -> Vec3 {
        let (x, y) = self.column_point(place.x, place.y);
        let height = self.column(place.x, place.y).and_then(|(chunk, range)| {
            let at = range.start + usize::from(place.level);
            chunk.heights.get(at).copied()
        });
        Vec3::new(x, y, height.unwrap_or_default())
    }

    fn links(&self, place: ChunkPlace) -> [Option<ChunkPlace>; LINKS] {
        let mut out = [None; LINKS];
        let Some((chunk, range)) = self.column(place.x, place.y) else {
            return out;
        };
        let Some(links) = chunk.links.get(range.start + usize::from(place.level)) else {
            return out;
        };
        for ((slot, level), (dx, dy)) in out.iter_mut().zip(links).zip(NAV_WAYS) {
            if *level != NO_LINK {
                *slot = Some(ChunkPlace {
                    x: place.x.wrapping_add_signed(dx),
                    y: place.y.wrapping_add_signed(dy),
                    level: *level,
                });
            }
        }
        out
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "grids are far below i32's range"
    )]
    fn nearest_place(&self, point: Vec3) -> Option<ChunkPlace> {
        let spacing = self.shape.spacing.max(1.0);
        let local = (point - self.shape.origin) * (1.0 / spacing);
        let (x, y) = (local.x.round() as i32, local.y.round() as i32);
        let mut best = None;
        let mut closest = f32::MAX;
        for ring in 0..=SEARCH_REACH {
            for dy in -ring..=ring {
                for dx in -ring..=ring {
                    let (Ok(column_x), Ok(column_y)) =
                        (u32::try_from(x + dx), u32::try_from(y + dy))
                    else {
                        continue;
                    };
                    let Some((_, range)) = self.column(column_x, column_y) else {
                        continue;
                    };
                    for level in 0..range.len() {
                        let place = ChunkPlace {
                            x: column_x,
                            y: column_y,
                            level: u8::try_from(level).unwrap_or(NO_LINK - 1),
                        };
                        let distance = (self.point(place) - point).length();
                        if distance < closest {
                            closest = distance;
                            best = Some(place);
                        }
                    }
                }
            }
            if best.is_some() {
                break;
            }
        }
        best
    }
}

impl Clone for ChunkedNav {
    fn clone(&self) -> Self {
        Self {
            shape: self.shape,
            prefix: self.prefix.clone(),
            files: self.files.clone(),
            kept: Mutex::default(),
        }
    }
}

impl PartialEq for ChunkedNav {
    fn eq(&self, other: &Self) -> bool {
        self.shape == other.shape && self.prefix == other.prefix
    }
}

impl fmt::Debug for ChunkedNav {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChunkedNav")
            .field("shape", &self.shape)
            .field("prefix", &self.prefix)
            .finish_non_exhaustive()
    }
}
