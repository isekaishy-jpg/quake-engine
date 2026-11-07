//! Land: which parts of it are drawn, and how finely (proposal 0144).

use engine_core::math::Vec3;
use engine_map::{Map, TerrainData, TerrainSheet};
use engine_render::{GRIDS, TerrainBatch, grid_indices, terrain_nodes};

/// Land of `chunks` by `chunks`, flat.
fn map_of(chunks: u32) -> Map {
    Map {
        terrain: vec![TerrainSheet {
            data: TerrainData::flat([chunks, chunks], 32.0),
            ..TerrainSheet::default()
        }],
        ..Map::default()
    }
}

/// Planes that see everything.
const EVERYWHERE: [[f32; 4]; 6] = [[0.0, 0.0, 0.0, 1.0e9]; 6];

/// A part of the land: its first sample, its stride and its size in cells.
#[derive(Clone, Copy, Debug)]
struct Part {
    first: [u32; 2],
    stride: u32,
    cells: u32,
}

/// What's drawn of `map` from `eye`.
fn parts(map: &Map, eye: Vec3) -> (Vec<Part>, Vec<TerrainBatch>) {
    let count: u32 = map.terrain.iter().map(TerrainSheet::chunk_count).sum();
    let chunks: Vec<u32> = (0..count).collect();
    let mut out = vec![0; 1 << 20];
    let batches = terrain_nodes(map, (&chunks, &[]), (eye, &EVERYWHERE), &mut out);
    let total: u32 = batches.iter().map(|batch| batch.count).sum();
    let word = |at: usize| u32::from_le_bytes(out[at..at + 4].try_into().expect("four"));
    let parts = (0..total as usize)
        .map(|at| {
            let base = at * 64;
            let (stride, grid) = (word(base + 8), word(base + 12));
            Part {
                first: [word(base), word(base + 4)],
                stride,
                cells: stride * grid,
            }
        })
        .collect();
    (parts, batches)
}

/// Whether two parts share an edge.
fn touching(a: &Part, b: &Part) -> bool {
    let overlap =
        |low_a: u32, high_a: u32, low_b: u32, high_b: u32| low_a < high_b && low_b < high_a;
    let [ax, ay] = a.first;
    let [bx, by] = b.first;
    let (ax2, ay2, bx2, by2) = (ax + a.cells, ay + a.cells, bx + b.cells, by + b.cells);
    (ax2 == bx || bx2 == ax) && overlap(ay, ay2, by, by2)
        || (ay2 == by || by2 == ay) && overlap(ax, ax2, bx, bx2)
}

/// Sliding only meets up if what's side by side is at most a level apart.
#[test]
fn neighbours_are_never_more_than_a_level_apart() {
    let map = map_of(16);
    for eye in [
        Vec3::new(100.0, 100.0, 40.0),
        Vec3::new(16000.0, 16000.0, 200.0),
        Vec3::new(9000.0, 3000.0, 1500.0),
    ] {
        let (found, _) = parts(&map, eye);
        for (at, one) in found.iter().enumerate() {
            for other in &found[at + 1..] {
                if touching(one, other) {
                    let (fine, coarse) =
                        (one.stride.min(other.stride), one.stride.max(other.stride));
                    assert!(coarse <= fine * 2, "{one:?} beside {other:?}");
                }
            }
        }
    }
}

/// All of the land is covered, once.
#[test]
fn parts_cover_the_land_with_no_gaps_or_overlaps() {
    let map = map_of(8);
    let (found, _) = parts(&map, Vec3::new(3000.0, 5000.0, 300.0));
    let area: u32 = found.iter().map(|part| part.cells * part.cells).sum();
    assert_eq!(area, 512 * 512);
    let mut seen = vec![false; 512 * 512];
    for part in &found {
        for y in part.first[1]..part.first[1] + part.cells {
            for x in part.first[0]..part.first[0] + part.cells {
                let at = (y * 512 + x) as usize;
                assert!(!seen[at], "{x}, {y} twice");
                seen[at] = true;
            }
        }
    }
}

/// What's far off costs little, which is what this is for.
#[test]
fn far_land_is_coarse() {
    let map = map_of(32);
    let (_, batches) = parts(&map, Vec3::new(100.0, 100.0, 100.0));
    let triangles: u64 = batches
        .iter()
        .map(|batch| {
            let grid = u64::from(GRIDS[batch.grid as usize]);
            u64::from(batch.count) * grid * grid * 2
        })
        .sum();
    let whole = 2048_u64 * 2048 * 2;
    assert!(triangles * 50 < whole, "{triangles} of {whole}");
}

/// A grid splits its cells as collision splits them, from the low corner to the high one.
#[test]
fn a_grid_splits_a_cell_as_collision_does() {
    let indices = grid_indices(2);
    assert_eq!(indices.len(), 2 * 2 * 6);
    assert_eq!(indices[..6], [0, 1, 4, 0, 4, 3]);
}
