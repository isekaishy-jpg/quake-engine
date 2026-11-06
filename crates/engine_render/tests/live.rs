//! Which clusters of the view a moving light reaches.

use engine_core::math::Vec3;
use engine_render::{
    CLUSTERS_X, CLUSTERS_Y, CLUSTERS_Z, CapsuleDraw, LiveLight, MAX_LIVE_LIGHTS, View,
    cluster_masks, cube_face, slice_of,
};

/// A view from the origin along x, at a 16 by 9 shape.
fn view() -> View {
    View {
        origin: Vec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        roll: 0.0,
        fov: 90.0,
        time: 0.0,
        styles: [1.0; 64],
        gamma: 1.0,
        light_shift: [0.0; 3],
    }
}

/// A light at `origin`.
fn light(origin: Vec3, radius: f32) -> LiveLight {
    LiveLight {
        origin,
        radius,
        ..LiveLight::default()
    }
}

/// The columns, rows and slices of the clusters whose mask has bit 0.
fn reached(masks: &[u64]) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    for slice in 0..CLUSTERS_Z {
        for row in 0..CLUSTERS_Y {
            for column in 0..CLUSTERS_X {
                let at = (slice * CLUSTERS_Y + row) * CLUSTERS_X + column;
                if masks[at] & 1 != 0 {
                    out.push((column, row, slice));
                }
            }
        }
    }
    out
}

/// A capsule ahead takes a bit after the lights', and only where its shade may
/// fall, so that most pixels never look at it.
#[test]
fn a_capsule_only_reaches_the_clusters_it_shades() {
    let capsule = CapsuleDraw {
        origin: Vec3::new(1500.0, 0.0, 0.0),
        radius: 16.0,
        toward_light: Vec3::new(0.0, 0.0, 1.0),
        strength: 0.5,
    };
    let masks = cluster_masks(&view(), 16.0 / 9.0, &[], &[capsule]);
    let bit = 1_u64 << MAX_LIVE_LIGHTS;
    let reached = masks.iter().filter(|mask| **mask & bit != 0).count();
    assert!(reached > 0);
    assert!(reached < masks.len() / 20, "{reached} of {}", masks.len());
    assert!(masks.iter().all(|mask| *mask & (bit - 1) == 0));
}

/// A small light ahead reaches the middle of the view, at its own distance.
#[test]
fn a_light_ahead_reaches_the_middle() {
    let masks = cluster_masks(
        &view(),
        16.0 / 9.0,
        &[light(Vec3::new(500.0, 0.0, 0.0), 40.0)],
        &[],
    );
    let found = reached(&masks);
    assert!(!found.is_empty());
    for (column, row, slice) in &found {
        assert!((6..=9).contains(column), "{column}");
        assert!((3..=5).contains(row), "{row}");
        assert!(*slice >= slice_of(460.0) && *slice <= slice_of(540.0));
    }
}

/// A light behind the view reaches nothing.
#[test]
fn a_light_behind_reaches_nothing() {
    let masks = cluster_masks(
        &view(),
        16.0 / 9.0,
        &[light(Vec3::new(-500.0, 0.0, 0.0), 40.0)],
        &[],
    );
    assert!(reached(&masks).is_empty());
}

/// Left of the view is the low columns, and up is the low rows.
#[test]
fn a_light_lands_on_its_side_of_the_view() {
    let aspect = 16.0 / 9.0;
    let left = cluster_masks(
        &view(),
        aspect,
        &[light(Vec3::new(500.0, 400.0, 0.0), 30.0)],
        &[],
    );
    assert!(reached(&left).iter().all(|(column, ..)| *column < 8));
    let up = cluster_masks(
        &view(),
        aspect,
        &[light(Vec3::new(500.0, 0.0, 200.0), 30.0)],
        &[],
    );
    assert!(reached(&up).iter().all(|(_, row, _)| *row < 4));
}

/// A light round the eye reaches the whole view.
#[test]
fn a_light_round_the_eye_reaches_everywhere() {
    let masks = cluster_masks(
        &view(),
        16.0 / 9.0,
        &[light(Vec3::new(10.0, 0.0, 0.0), 100.0)],
        &[],
    );
    let found = reached(&masks);
    let first = found.iter().filter(|(.., slice)| *slice == 0).count();
    assert_eq!(first, CLUSTERS_X * CLUSTERS_Y);
}

/// Each light has its own bit.
#[test]
fn lights_have_their_own_bits() {
    let lights = [
        light(Vec3::new(500.0, 400.0, 0.0), 30.0),
        light(Vec3::new(500.0, -400.0, 0.0), 30.0),
    ];
    let masks = cluster_masks(&view(), 16.0 / 9.0, &lights, &[]);
    assert!(masks.contains(&1));
    assert!(masks.contains(&2));
    assert!(!masks.contains(&3), "they are far apart");
}

/// Further is a later slice, and it stays in range.
#[test]
fn slices_follow_distance() {
    assert_eq!(slice_of(0.0), 0);
    assert!(slice_of(100.0) < slice_of(1000.0));
    assert_eq!(slice_of(1.0e9), CLUSTERS_Z - 1);
}

/// Each cube face looks along its own axis, and has what's ahead in the middle.
#[test]
fn cube_faces_look_along_their_axes() {
    let origin = Vec3::new(10.0, 20.0, 30.0);
    let ahead = [
        Vec3::new(50.0, 0.0, 0.0),
        Vec3::new(-50.0, 0.0, 0.0),
        Vec3::new(0.0, 50.0, 0.0),
        Vec3::new(0.0, -50.0, 0.0),
        Vec3::new(0.0, 0.0, 50.0),
        Vec3::new(0.0, 0.0, -50.0),
    ];
    for (face, offset) in ahead.iter().enumerate() {
        let m = cube_face(origin, face, 200.0);
        let p = origin + *offset;
        let clip = |row: usize| m[row] * p.x + m[4 + row] * p.y + m[8 + row] * p.z + m[12 + row];
        let w = clip(3);
        assert!((w - 50.0).abs() < 0.01, "face {face}: {w}");
        assert!(clip(0).abs() < 0.01 && clip(1).abs() < 0.01, "face {face}");
        assert!((0.0..1.0).contains(&(clip(2) / w)), "face {face}");
    }
}
