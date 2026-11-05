//! The shade of an alias model's poses.

use engine_core::math::Vec3;
use engine_map::{AliasFrame, AliasModel, AliasVertex, pose_occlusion, pose_spheres};

/// A model in two poses: a tall thin post, and the same post lying down.
fn body() -> AliasModel {
    let vertex = |x, y, z| AliasVertex {
        position: [x, y, z],
        normal: [0, 0, 127],
    };
    let standing = vec![
        vertex(90, 90, 0),
        vertex(110, 110, 0),
        vertex(100, 100, 200),
    ];
    let lying = vec![vertex(0, 90, 0), vertex(200, 110, 0), vertex(100, 100, 20)];
    AliasModel {
        scale: Vec3::new(0.5, 0.5, 0.5),
        offset: Vec3::new(-50.0, -50.0, -24.0),
        skin_width: 1,
        skin_height: 1,
        skins: Vec::new(),
        corners: Vec::new(),
        poses: vec![standing, lying],
        frames: vec![AliasFrame {
            first_pose: 0,
            pose_count: 2,
            pose_ends: Vec::new(),
        }],
        spins: false,
        flags: 0,
    }
}

/// A body lying down is low, where one standing has its middle at its waist.
#[test]
fn a_pose_moves_its_shade() {
    let spheres = pose_occlusion(&body());
    assert_eq!(spheres.len(), 2, "one set for each pose");
    let (standing, lying) = (spheres[0][0], spheres[1][0]);
    assert!(lying.center[2] < standing.center[2] - 30.0, "{lying:?}");
    assert!(standing.center[0].abs() < 1.0, "{standing:?}");
}

/// Parts run along a pose's longest side, however it's lying.
#[test]
fn parts_run_along_a_pose() {
    let model = body();
    let standing = pose_spheres(&model, 0);
    assert_eq!(standing.len(), 4, "the whole and three parts");
    let heights: Vec<f32> = standing[1..].iter().map(|part| part.center[2]).collect();
    assert!(
        heights.windows(2).all(|pair| pair[0] <= pair[1]),
        "{heights:?}"
    );
    let lying = pose_spheres(&model, 1);
    let places: Vec<f32> = lying[1..].iter().map(|part| part.center[0]).collect();
    assert!(
        places.windows(2).all(|pair| pair[0] <= pair[1]),
        "{places:?}"
    );
}

/// A pose that isn't there has no shade.
#[test]
fn a_missing_pose_has_no_shade() {
    assert!(pose_spheres(&body(), 9).is_empty());
}
