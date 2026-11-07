//! The fixture's models, as what the mesh path draws.

use engine_map::{AliasCorner, AliasModel};
use engine_render::weld_corners;

/// A corner of `vertex`, at `u` across the skin.
const fn corner(vertex: u32, u: f32) -> AliasCorner {
    AliasCorner {
        vertex,
        uv: [u, 0.5],
    }
}

/// Two triangles sharing an edge, as a model keeps them: three corners each.
fn two_triangles() -> AliasModel {
    AliasModel {
        corners: vec![
            corner(0, 0.0),
            corner(1, 0.1),
            corner(2, 0.2),
            corner(2, 0.2),
            corner(1, 0.1),
            corner(3, 0.3),
        ],
        ..AliasModel::default()
    }
}

#[test]
fn triangles_share_the_corners_they_have_in_common() {
    let welded = weld_corners(&two_triangles());
    assert_eq!(welded.corners.len(), 4);
    assert_eq!(welded.indices.len(), 6);
}

/// Each index still leads to the corner it was, so nothing has moved.
#[test]
fn a_welded_triangle_has_the_same_corners() {
    let model = two_triangles();
    let welded = weld_corners(&model);
    for (triangle, indices) in model
        .corners
        .as_chunks::<3>()
        .0
        .iter()
        .zip(welded.indices.as_chunks::<3>().0)
    {
        let found = indices.map(|index| welded.corners[index as usize]);
        assert_eq!(found, [triangle[0], triangle[2], triangle[1]]);
    }
}

/// Quake's triangles wind the other way from a mesh's, so a corner swaps places.
#[test]
fn a_triangle_winds_as_a_meshs_does() {
    let model = AliasModel {
        corners: vec![corner(0, 0.0), corner(1, 0.1), corner(2, 0.2)],
        ..AliasModel::default()
    };
    let welded = weld_corners(&model);
    let vertices = welded
        .indices
        .iter()
        .map(|index| welded.corners[*index as usize].vertex)
        .collect::<Vec<_>>();
    assert_eq!(vertices, [0, 2, 1]);
}

/// A vertex on a skin's seam is in two places on it, and those have to stay apart, or one side
/// of the model gets the other's skin.
#[test]
fn a_vertex_on_a_seam_stays_two_corners() {
    let model = AliasModel {
        corners: vec![
            corner(0, 0.0),
            corner(1, 0.1),
            corner(2, 0.2),
            corner(0, 0.5),
            corner(2, 0.2),
            corner(1, 0.1),
        ],
        ..AliasModel::default()
    };
    assert_eq!(weld_corners(&model).corners.len(), 4);
}
