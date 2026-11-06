//! Building GPU data from a map. No GPU is involved.

mod common;

use common::map;
use engine_core::canvas::Quad;
use engine_core::math::Vec3;
use engine_map::{Face, Map};
use engine_render::{Atlas, BatchKind, VisibleSet, build_world, canvas_vertices, view_projection};

/// A map with one square.
fn square_map() -> Map {
    map(1)
}

/// Whether the atlas has a pixel starting with `rgb`.
fn atlas_has(atlas: &[u8], rgb: [u8; 3]) -> bool {
    atlas
        .as_chunks::<4>()
        .0
        .iter()
        .any(|pixel| pixel[..3] == rgb)
}

#[test]
fn a_quad_becomes_two_triangles() {
    let mesh = build_world(&square_map()).expect("mesh");
    assert_eq!(mesh.vertices.len(), 4);
    assert_eq!(mesh.face_indices[0], [0, 1, 2, 0, 2, 3]);
    assert_eq!(mesh.batches[0].kind, BatchKind::Material);
}

#[test]
fn texture_coordinates_wrap_at_the_texture_size() {
    let mesh = build_world(&square_map()).expect("mesh");
    assert_eq!(
        mesh.vertices[2].uv,
        [2.0, 2.0],
        "128 units on a 64 texel texture"
    );
}

#[test]
fn light_coordinates_land_on_sample_centres() {
    let mesh = build_world(&square_map()).expect("mesh");
    let scale = 1.0 / f32::from(u16::try_from(mesh.atlas_size).expect("small"));
    let first = mesh.vertices[0].light_uv;
    let last = mesh.vertices[2].light_uv;
    assert!(
        ((last[0] - first[0]) - 8.0 * scale).abs() < 1e-6,
        "128 units is 8 samples"
    );
    assert_eq!(
        mesh.vertices[0].styles,
        [0, Face::NO_STYLE, Face::NO_STYLE, Face::NO_STYLE]
    );
}

#[test]
fn the_samples_reach_the_atlas() {
    let mesh = build_world(&square_map()).expect("mesh");
    assert!(atlas_has(&mesh.atlas, [200, 100, 50]));
}

#[test]
fn hidden_faces_are_not_drawn() {
    let mut map = square_map();
    map.textures[0].kind = engine_map::TextureKind::Hidden;
    let mesh = build_world(&map).expect("mesh");
    assert!(mesh.vertices.is_empty());
    assert_eq!(mesh.face_batches, [None]);
}

#[test]
fn an_unlit_map_uses_full_brightness() {
    let mut map = square_map();
    map.light.clear();
    map.lit = false;
    map.faces[0].light = None;
    let mesh = build_world(&map).expect("mesh");
    assert_eq!(mesh.vertices[0].styles[0], 0);
    assert!(atlas_has(&mesh.atlas, [128, 128, 128]));
}

#[test]
fn brush_models_keep_their_own_ranges_outside_the_visible_set() {
    let map = map(2);
    let mesh = build_world(&map).expect("mesh");
    assert!(mesh.model_ranges[0].is_empty());
    assert_eq!(mesh.model_ranges[1].len(), 1);
    assert_eq!(mesh.model_ranges[1][0].count, 6);
    assert_eq!(mesh.model_indices.len(), 6);
    assert_eq!(mesh.world_index_count, 6);
    assert_eq!(
        mesh.vertices[4].position, mesh.vertices[0].position,
        "not moved here"
    );
}

#[test]
fn the_visible_set_groups_faces_and_keeps_its_leaf() {
    let map = square_map();
    let mesh = build_world(&map).expect("mesh");
    let mut visible = VisibleSet::default();
    assert!(visible.update(&map, &mesh, Vec3::new(1.0, 0.0, 0.0)));
    assert_eq!(visible.indices.len(), 6);
    assert_eq!(visible.ranges, [(0, 6)]);
    assert!(
        !visible.update(&map, &mesh, Vec3::new(2.0, 0.0, 0.0)),
        "same leaf"
    );
}

#[test]
fn rectangles_never_overlap_and_the_atlas_grows() {
    let sizes: Vec<[u32; 2]> = (0_u32..500).map(|i| [8 + i % 40, 8 + i % 23]).collect();
    let atlas = Atlas::pack(&sizes, 8192).expect("fits");
    assert!(atlas.size >= 512);
    for (a, (at, size)) in atlas.positions.iter().zip(&sizes).enumerate() {
        assert!(at[0] + size[0] <= atlas.size && at[1] + size[1] <= atlas.size);
        for (other, other_size) in atlas.positions.iter().zip(&sizes).skip(a + 1) {
            let apart = at[0] + size[0] <= other[0]
                || other[0] + other_size[0] <= at[0]
                || at[1] + size[1] <= other[1]
                || other[1] + other_size[1] <= at[1];
            assert!(apart);
        }
    }
}

#[test]
fn rectangles_that_cannot_fit_are_refused() {
    assert_eq!(Atlas::pack(&[[9000, 1]], 8192), None);
}

/// A quad of `picture` over the middle of a screen of 200 by 100.
fn quad(picture: u32) -> Quad {
    Quad {
        picture,
        place: [50.0, 25.0, 150.0, 75.0],
        source: [0.0, 0.0, 1.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
    }
}

#[test]
fn a_quad_lands_in_clip_space_with_y_up() {
    let (vertices, _batches) = canvas_vertices(&[quad(1)], (200, 100));
    let (corners, _rest) = vertices.as_chunks::<8>();
    assert_eq!(corners.len(), 6);
    assert_eq!(corners[0][..4], [-0.5, 0.5, 0.0, 0.0]);
    assert_eq!(corners[2][..4], [0.5, -0.5, 1.0, 1.0]);
}

#[test]
fn quads_of_one_picture_share_a_batch_until_it_changes() {
    let (_vertices, batches) = canvas_vertices(&[quad(1), quad(1), quad(2), quad(1)], (200, 100));
    let runs: Vec<(u32, u32, u32)> = batches
        .iter()
        .map(|batch| (batch.picture, batch.first, batch.count))
        .collect();
    assert_eq!(runs, [(1, 0, 12), (2, 12, 6), (1, 18, 6)]);
}

#[test]
fn an_empty_canvas_draws_nothing() {
    let (vertices, batches) = canvas_vertices(&[], (200, 100));
    assert!(vertices.is_empty());
    assert!(batches.is_empty());
}

/// Applies a column-major matrix to a point, and divides by w.
fn project(matrix: &[f32; 16], point: Vec3) -> [f32; 3] {
    let row = |r: usize| {
        matrix[r] * point.x + matrix[4 + r] * point.y + matrix[8 + r] * point.z + matrix[12 + r]
    };
    let w = row(3);
    [row(0) / w, row(1) / w, row(2) / w]
}

#[test]
fn the_camera_looks_along_yaw_with_up_at_the_screen_top() {
    let matrix = view_projection(Vec3::ZERO, [0.0, 0.0, 0.0], 90.0, 4.0 / 3.0);
    let ahead = project(&matrix, Vec3::new(100.0, 0.0, 0.0));
    assert!(ahead[0].abs() < 1e-5 && ahead[1].abs() < 1e-5);
    assert!((0.0..1.0).contains(&ahead[2]));
    let right = project(&matrix, Vec3::new(100.0, -100.0, 0.0));
    assert!(
        (right[0] - 1.0).abs() < 1e-4,
        "90 degrees at 4:3 reaches the edge"
    );
    let up = project(&matrix, Vec3::new(100.0, 0.0, 10.0));
    assert!(
        up[1] > 0.0,
        "WGSL's y runs up the screen, and naga flips it for Vulkan"
    );
}

#[test]
fn positive_pitch_looks_down_and_yaw_turns_left() {
    let down = view_projection(Vec3::ZERO, [0.0, 90.0, 0.0], 90.0, 1.0);
    let floor = project(&down, Vec3::new(0.0, 0.0, -100.0));
    assert!(floor[0].abs() < 1e-4 && floor[1].abs() < 1e-4);
    let left = view_projection(Vec3::ZERO, [90.0, 0.0, 0.0], 90.0, 1.0);
    let target = project(&left, Vec3::new(0.0, 100.0, 0.0));
    assert!(target[0].abs() < 1e-4);
}

#[test]
fn a_vertex_says_which_way_a_mesh_faces() {
    let mut built = map(1);
    built.normals = vec![[0, 0, 127]; built.points.len()];
    let mesh = build_world(&built).expect("builds");
    assert!(
        mesh.vertices
            .iter()
            .all(|vertex| vertex.normal == [0, 0, 127, 0])
    );
}

#[test]
fn a_vertex_on_a_flat_face_says_nothing() {
    let mesh = build_world(&map(1)).expect("builds");
    assert!(mesh.vertices.iter().all(|vertex| vertex.normal == [0; 4]));
}
