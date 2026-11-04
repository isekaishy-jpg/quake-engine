//! Building GPU data from a map. No GPU is involved.

mod common;

use common::map;
use engine_core::math::Vec3;
use engine_map::{Face, Map};
use engine_render::{Atlas, BatchKind, VisibleSet, build_world, overlay_vertices, view_projection};

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
    assert_eq!(mesh.batches[0].kind, BatchKind::Solid);
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
fn brush_models_move_to_their_origin_and_always_draw() {
    let mut map = map(2);
    map.models[1].origin = Vec3::new(10.0, 0.0, 0.0);
    let mesh = build_world(&map).expect("mesh");
    assert_eq!(
        mesh.vertices[4].position[0],
        mesh.vertices[0].position[0] + 10.0
    );
    assert_eq!(mesh.always_visible, [1]);
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

/// Floats in a quad: six vertices of four.
const QUAD_FLOATS: usize = 24;

#[test]
fn overlay_text_has_a_backing_and_a_quad_per_glyph() {
    assert_eq!(
        overlay_vertices("60 FPS", (1280, 720)).len(),
        6 * QUAD_FLOATS
    );
}

#[test]
fn an_empty_overlay_draws_nothing() {
    assert!(overlay_vertices("", (1280, 720)).is_empty());
    assert!(overlay_vertices("60", (0, 0)).is_empty());
}

#[test]
fn the_overlay_sits_in_the_top_right_corner() {
    let vertices = overlay_vertices("60", (1280, 720));
    let (quads, _rest) = vertices.as_chunks::<4>();
    assert!(
        quads
            .iter()
            .all(|vertex| vertex[0] > 0.5 && vertex[0] <= 1.0)
    );
    assert!(
        quads
            .iter()
            .all(|vertex| vertex[1] > 0.5 && vertex[1] <= 1.0)
    );
}

#[test]
fn the_overlay_grows_with_the_window() {
    let width = |height| {
        let vertices = overlay_vertices("60", (height * 2, height));
        let (quads, _rest) = vertices.as_chunks::<4>();
        let low = quads.iter().map(|v| v[0]).fold(f32::MAX, f32::min);
        let high = quads.iter().map(|v| v[0]).fold(f32::MIN, f32::max);
        (high - low) * f32::from(u16::try_from(height).expect("small"))
    };
    assert!(width(1440) > width(720) * 1.5);
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
    let matrix = view_projection(Vec3::ZERO, 0.0, 0.0, 90.0, 4.0 / 3.0);
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
    let down = view_projection(Vec3::ZERO, 0.0, 90.0, 90.0, 1.0);
    let floor = project(&down, Vec3::new(0.0, 0.0, -100.0));
    assert!(floor[0].abs() < 1e-4 && floor[1].abs() < 1e-4);
    let left = view_projection(Vec3::ZERO, 90.0, 0.0, 90.0, 1.0);
    let target = project(&left, Vec3::new(0.0, 100.0, 0.0));
    assert!(target[0].abs() < 1e-4);
}
