//! The light on a point, from the floor below it.

use engine_core::math::Vec3;
use engine_map::{
    Face, FaceLight, Leaf, LightTrack, Map, Node, NodeChild, Plane, RESIDENT, TexelAxis, Tree,
};

/// A floor at z = 0 with a lit face over x and y from 0 to 16, with 2 by 2
/// samples, in style 3.
fn map() -> Map {
    let axis = |axis: Vec3| TexelAxis { axis, offset: 0.0 };
    Map {
        faces: vec![Face {
            first_point: 0,
            point_count: 0,
            texture: 0,
            axes: [
                axis(Vec3::new(1.0, 0.0, 0.0)),
                axis(Vec3::new(0.0, 1.0, 0.0)),
            ],
            light: Some(FaceLight {
                first_sample: 0,
                width: 2,
                height: 2,
                texel_origin: [0, 0],
                texels_per_sample: 16,
                styles: [3, Face::NO_STYLE, Face::NO_STYLE, Face::NO_STYLE],
                tile: RESIDENT,
                average: [0; 3],
            }),
            light_axes: None,
        }],
        light: vec![[30; 3], [60; 3], [90; 3], [120; 3]],
        tree: Tree {
            planes: vec![Plane {
                normal: Vec3::new(0.0, 0.0, 1.0),
                distance: 0.0,
            }],
            nodes: vec![Node {
                plane: 0,
                children: [NodeChild::Leaf(1), NodeChild::Leaf(0)],
            }],
            node_faces: vec![(0, 1)],
            leaves: vec![Leaf::default(), Leaf::default()],
            ..Tree::default()
        },
        ..Map::default()
    }
}

#[test]
fn a_point_takes_the_sample_under_it() {
    let map = map();
    let styles = [1.0; 64];
    assert!((map.light_at(Vec3::new(1.0, 1.0, 40.0), &styles) - 30.0).abs() < 1e-3);
    assert!((map.light_at(Vec3::new(16.0, 1.0, 40.0), &styles) - 60.0).abs() < 1e-3);
    assert!((map.light_at(Vec3::new(1.0, 16.0, 40.0), &styles) - 90.0).abs() < 1e-3);
}

#[test]
fn a_style_scales_its_samples() {
    let mut styles = [1.0; 64];
    styles[3] = 0.5;
    let light = map().light_at(Vec3::new(1.0, 1.0, 40.0), &styles);
    assert!((light - 15.0).abs() < 1e-3);
}

#[test]
fn a_point_over_no_face_is_dark() {
    let map = map();
    assert!(map.light_at(Vec3::new(100.0, 1.0, 40.0), &[1.0; 64]).abs() < 1e-3);
}

#[test]
fn a_map_with_no_light_is_fully_bright() {
    let map = Map::default();
    assert!((map.light_at(Vec3::ZERO, &[1.0; 64]) - 255.0).abs() < 1e-3);
}

/// A track brightens each keyframe in turn, and shares light between two as
/// it moves, coming back round to the first.
#[test]
fn a_track_blends_between_its_keyframes() {
    let track = LightTrack {
        first_style: 2,
        count: 3,
        period: 3.0,
    };
    let at = |time: f32| {
        let mut styles = [9.0; 6];
        track.blend(&mut styles, time);
        styles
    };
    assert_eq!(at(0.0), [9.0, 9.0, 1.0, 0.0, 0.0, 9.0]);
    assert_eq!(at(1.0), [9.0, 9.0, 0.0, 1.0, 0.0, 9.0]);
    assert_eq!(at(1.5), [9.0, 9.0, 0.0, 0.5, 0.5, 9.0]);
    assert_eq!(at(2.5), [9.0, 9.0, 0.5, 0.0, 0.5, 9.0]);
}

/// A track with no period is a script's to drive.
#[test]
fn a_track_with_no_period_is_left_alone() {
    let track = LightTrack {
        first_style: 0,
        count: 2,
        period: 0.0,
    };
    let mut styles = [0.25; 2];
    track.blend(&mut styles, 1.0);
    assert_eq!(styles, [0.25; 2]);
}
