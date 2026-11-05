//! Walkable grids read a chunk at a time.

use std::collections::HashMap;
use std::sync::Arc;

use engine_core::math::Vec3;
use engine_map::{ChunkedNav, NAV_WAYS, NavChunk, NavGrid, NavPlace, NavShape, chunks_of};

/// How many columns a chunk has here, so a small grid has several.
const CHUNK: u32 = 8;

/// An open grid of `width` by `depth` columns 16 apart, with a wall of no places
/// along `x = wall` but for a gap at `y = gap`.
fn grid(width: u32, depth: u32, wall: u32, gap: u32) -> NavGrid {
    let open = |x: u32, y: u32| x != wall || y == gap;
    let mut columns = Vec::new();
    let mut places = Vec::new();
    let mut numbers = HashMap::new();
    for y in 0..depth {
        for x in 0..width {
            columns.push(u32::try_from(places.len()).expect("small"));
            if open(x, y) {
                numbers.insert((x, y), u32::try_from(places.len()).expect("small"));
                #[expect(clippy::cast_precision_loss, reason = "small")]
                let point = Vec3::new(x as f32 * 16.0, y as f32 * 16.0, 24.0);
                places.push(NavPlace::at(point));
            }
        }
    }
    columns.push(u32::try_from(places.len()).expect("small"));
    for ((x, y), number) in &numbers {
        for (way, (dx, dy)) in NAV_WAYS.iter().enumerate() {
            let there = (x.wrapping_add_signed(*dx), y.wrapping_add_signed(*dy));
            let side_a = numbers.contains_key(&(x.wrapping_add_signed(*dx), *y));
            let side_b = numbers.contains_key(&(*x, y.wrapping_add_signed(*dy)));
            if let Some(other) = numbers.get(&there).filter(|_| side_a && side_b) {
                places[*number as usize].links[way] = *other;
            }
        }
    }
    NavGrid {
        origin: Vec3::ZERO,
        spacing: 16.0,
        size: [width, depth],
        columns,
        places,
    }
}

/// `grid` as chunks in files in memory.
fn chunked(grid: &NavGrid) -> ChunkedNav {
    let mut files = HashMap::new();
    for (at, chunk) in chunks_of(grid, CHUNK) {
        files.insert(ChunkedNav::chunk_file("nav", at), chunk.to_bytes());
    }
    let shape = NavShape {
        origin: grid.origin,
        spacing: grid.spacing,
        size: grid.size,
        chunk: CHUNK,
    };
    let mut nav = ChunkedNav::new(shape, "nav".to_owned());
    nav.attach("", Arc::new(move |path| files.get(path).cloned()));
    nav
}

/// A chunk comes back from its bytes as it was.
#[test]
fn a_chunk_survives_its_bytes() {
    let whole = grid(20, 20, 9, 4);
    let columns = (CHUNK * CHUNK) as usize;
    for (_, chunk) in chunks_of(&whole, CHUNK) {
        let read = NavChunk::from_bytes(&chunk.to_bytes(), columns).expect("reads");
        assert_eq!(read, chunk);
    }
}

/// Bytes that are cut short aren't a chunk.
#[test]
fn a_short_chunk_is_refused() {
    let whole = grid(8, 8, 99, 0);
    let (_, chunk) = chunks_of(&whole, CHUNK).remove(0);
    let bytes = chunk.to_bytes();
    let columns = (CHUNK * CHUNK) as usize;
    assert!(NavChunk::from_bytes(&bytes[..bytes.len() - 5], columns).is_none());
    assert!(NavChunk::from_bytes(&bytes[..10], columns).is_none());
}

/// A path over chunks is the path over the whole grid, across chunk edges, and
/// through a gap that's in a chunk of its own.
#[test]
fn a_path_over_chunks_matches_the_whole_grid() {
    let whole = grid(40, 30, 19, 25);
    let nav = chunked(&whole);
    let (from, to) = (Vec3::new(16.0, 16.0, 24.0), Vec3::new(600.0, 32.0, 24.0));
    let expected = whole.find_path(from, to).expect("a way");
    assert!(expected.len() > 2, "it bends: {expected:?}");
    assert_eq!(nav.find_path(from, to).expect("a way"), expected);
}

/// With no gap there's no way, whichever way it's stored.
#[test]
fn a_wall_with_no_gap_has_no_way_over_chunks() {
    let whole = grid(40, 30, 19, 999);
    let nav = chunked(&whole);
    let (from, to) = (Vec3::new(16.0, 16.0, 24.0), Vec3::new(600.0, 32.0, 24.0));
    assert!(whole.find_path(from, to).is_none());
    assert!(nav.find_path(from, to).is_none());
}

/// Only chunks a search reaches are read, and what's held stays bounded however
/// far a path goes.
#[test]
fn chunks_are_read_as_they_are_reached() {
    let whole = grid(200, 200, 999, 0);
    let nav = chunked(&whole);
    let near = nav.find_path(Vec3::new(16.0, 16.0, 24.0), Vec3::new(48.0, 16.0, 24.0));
    assert!(near.is_some());
    assert!(nav.reads() <= 4, "{} reads", nav.reads());
    let corner = Vec3::new(3180.0, 3180.0, 24.0);
    assert!(nav.find_path(Vec3::new(16.0, 16.0, 24.0), corner).is_some());
    assert!(nav.held() <= 48, "{} held", nav.held());
}

/// With no files to read there's nowhere to walk, and nothing goes wrong.
#[test]
fn a_grid_with_no_files_has_no_paths() {
    let shape = NavShape {
        origin: Vec3::ZERO,
        spacing: 16.0,
        size: [64, 64],
        chunk: CHUNK,
    };
    let nav = ChunkedNav::new(shape, "nav".to_owned());
    let (from, to) = (Vec3::ZERO, Vec3::new(64.0, 0.0, 0.0));
    assert!(nav.find_path(from, to).is_none());
}
