//! Poses, from clips.

use engine_core::math::Vec3;
use engine_map::{
    Channel, ChannelPath, Clip, Interpolation, Joint, JointPose, NO_MOVE, Skeleton, SkeletonPlace,
    after, blend_poses, moved, turned,
};

/// Close enough.
fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.001
}

/// Whether two points are the same.
fn same(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 0.001
}

/// A turn of `degrees` about Z, as a quaternion.
fn about_z(degrees: f32) -> [f32; 4] {
    let (sine, cosine) = (degrees.to_radians() * 0.5).sin_cos();
    [0.0, 0.0, sine, cosine]
}

/// Two joints in a line along X: one at the origin and one a unit on.
fn two_joints() -> Skeleton {
    let second = JointPose {
        translation: [1.0, 0.0, 0.0],
        ..JointPose::default()
    };
    let mut inverse_bind = NO_MOVE;
    inverse_bind[0][3] = -1.0;
    Skeleton {
        joints: vec![
            Joint {
                name: "first".to_owned(),
                inverse_bind: NO_MOVE,
                ..Joint::default()
            },
            Joint {
                name: "second".to_owned(),
                parent: Some(0),
                rest: second,
                inverse_bind,
            },
        ],
        ..Skeleton::default()
    }
}

/// A channel of `values`, on joint `joint`.
fn channel(joint: u16, path: ChannelPath, times: &[f32], values: &[f32]) -> Channel {
    Channel {
        joint,
        path,
        interpolation: Interpolation::Linear,
        times: times.to_vec(),
        values: values.to_vec(),
    }
}

/// A clip of two seconds, of `channels`.
fn clip(channels: Vec<Channel>) -> Clip {
    Clip {
        name: "clip".to_owned(),
        length: 2.0,
        channels,
    }
}

#[test]
fn matrices_go_in_the_right_order() {
    let mut move_along = NO_MOVE;
    move_along[0][3] = 5.0;
    let turn = JointPose {
        rotation: about_z(90.0),
        ..JointPose::default()
    }
    .rows();
    // Moved first and then turned, it's gone round to Y.
    let point = moved(&after(&turn, &move_along), Vec3::ZERO);
    assert!(same(point, Vec3::new(0.0, 5.0, 0.0)), "{point:?}");
    // Turned first and then moved, it's stayed on X.
    let point = moved(&after(&move_along, &turn), Vec3::ZERO);
    assert!(same(point, Vec3::new(5.0, 0.0, 0.0)), "{point:?}");
}

#[test]
fn a_pose_stretches_then_turns_then_moves() {
    let pose = JointPose {
        translation: [10.0, 0.0, 0.0],
        rotation: about_z(90.0),
        scale: [2.0, 1.0, 1.0],
    };
    let point = moved(&pose.rows(), Vec3::new(1.0, 0.0, 0.0));
    assert!(same(point, Vec3::new(10.0, 2.0, 0.0)), "{point:?}");
}

#[test]
fn at_rest_a_palette_moves_nothing() {
    let skeleton = two_joints();
    let placed = skeleton.placed(&skeleton.pose(None, 0.0));
    for rows in skeleton.palette(&placed) {
        let point = Vec3::new(3.0, 4.0, 5.0);
        assert!(same(moved(&rows, point), point), "{rows:?}");
    }
}

/// A joint carries what hangs from it, and a corner goes with its joint.
#[test]
fn turning_a_joint_carries_what_hangs_from_it() {
    let mut skeleton = two_joints();
    let turn = about_z(90.0);
    let keys = [turn, turn].concat();
    skeleton.clips.push(clip(vec![channel(
        0,
        ChannelPath::Rotation,
        &[0.0, 2.0],
        &keys,
    )]));
    let placed = skeleton.placed(&skeleton.pose(Some(0), 1.0));
    let second = moved(&placed[1], Vec3::ZERO);
    assert!(same(second, Vec3::new(0.0, 1.0, 0.0)), "{second:?}");
    // A corner on the second joint, at x = 2 at rest, goes round with it.
    let palette = skeleton.palette(&placed);
    let corner = moved(&palette[1], Vec3::new(2.0, 0.0, 0.0));
    assert!(same(corner, Vec3::new(0.0, 2.0, 0.0)), "{corner:?}");
}

#[test]
fn a_linear_channel_goes_evenly() {
    let mut skeleton = two_joints();
    let keys = [0.0, 0.0, 0.0, 10.0, 0.0, 0.0];
    skeleton.clips.push(clip(vec![channel(
        0,
        ChannelPath::Translation,
        &[0.0, 2.0],
        &keys,
    )]));
    assert!(near(skeleton.pose(Some(0), 0.5)[0].translation[0], 2.5));
}

#[test]
fn a_step_channel_holds_until_the_next_key() {
    let mut skeleton = two_joints();
    let mut held = channel(
        0,
        ChannelPath::Translation,
        &[0.0, 1.0],
        &[1.0, 0.0, 0.0, 9.0, 0.0, 0.0],
    );
    held.interpolation = Interpolation::Step;
    skeleton.clips.push(clip(vec![held]));
    assert!(near(skeleton.pose(Some(0), 0.99)[0].translation[0], 1.0));
    assert!(near(skeleton.pose(Some(0), 1.0)[0].translation[0], 9.0));
}

/// From glTF's own definition: with no slope at the ends it's a smooth start and stop.
#[test]
fn a_curve_goes_through_its_keys_and_follows_its_tangents() {
    let mut skeleton = two_joints();
    let flat = [0.0; 3];
    let values = [flat, [0.0, 0.0, 0.0], flat, flat, [10.0, 0.0, 0.0], flat].concat();
    let mut curve = channel(0, ChannelPath::Translation, &[0.0, 2.0], &values);
    curve.interpolation = Interpolation::Cubic;
    skeleton.clips.push(clip(vec![curve]));
    let at = |time: f32| skeleton.pose(Some(0), time)[0].translation[0];
    assert!(near(at(0.0), 0.0));
    assert!(near(at(1.0), 5.0), "halfway: {}", at(1.0));
    assert!(at(0.5) < 2.5, "it starts slowly: {}", at(0.5));
    assert!(at(1.5) > 7.5, "and ends slowly: {}", at(1.5));
}

/// A quaternion and its opposite are the same turn, and only one way is the short way.
#[test]
fn a_turn_goes_the_short_way() {
    let mut skeleton = two_joints();
    let start = about_z(10.0);
    let end = about_z(350.0).map(|part| -part);
    let opposite = about_z(350.0);
    for keys in [[start, end].concat(), [start, opposite].concat()] {
        skeleton.clips = vec![clip(vec![channel(
            0,
            ChannelPath::Rotation,
            &[0.0, 2.0],
            &keys,
        )])];
        let pose = skeleton.pose(Some(0), 1.0);
        let facing = turned(&pose[0].rows(), Vec3::new(1.0, 0.0, 0.0));
        assert!(
            same(facing, Vec3::new(1.0, 0.0, 0.0)),
            "halfway is 0 degrees: {facing:?}"
        );
    }
}

#[test]
fn a_turn_stays_a_pure_turn() {
    let mut skeleton = two_joints();
    let keys = [about_z(0.0), about_z(120.0)].concat();
    skeleton.clips.push(clip(vec![channel(
        0,
        ChannelPath::Rotation,
        &[0.0, 2.0],
        &keys,
    )]));
    for step in 0..20_u8 {
        let pose = skeleton.pose(Some(0), f32::from(step) * 0.1);
        let length: f32 = pose[0].rotation.iter().map(|part| part * part).sum();
        assert!(near(length, 1.0), "{length}");
    }
}

#[test]
fn a_clip_goes_round() {
    let mut skeleton = two_joints();
    let keys = [0.0, 0.0, 0.0, 10.0, 0.0, 0.0];
    skeleton.clips.push(clip(vec![channel(
        0,
        ChannelPath::Translation,
        &[0.0, 2.0],
        &keys,
    )]));
    let once = skeleton.pose(Some(0), 0.5)[0].translation[0];
    assert!(near(skeleton.pose(Some(0), 2.5)[0].translation[0], once));
    assert!(near(skeleton.pose(Some(0), 40.5)[0].translation[0], once));
}

/// Before the first key and after the last, a channel holds, as a clip can start late.
#[test]
fn a_channel_holds_outside_its_keys() {
    let mut skeleton = two_joints();
    let keys = [3.0, 0.0, 0.0, 7.0, 0.0, 0.0];
    skeleton.clips.push(clip(vec![channel(
        0,
        ChannelPath::Translation,
        &[0.5, 1.5],
        &keys,
    )]));
    assert!(near(skeleton.pose(Some(0), 0.1)[0].translation[0], 3.0));
    assert!(near(skeleton.pose(Some(0), 1.9)[0].translation[0], 7.0));
}

/// A clip that doesn't move something leaves it at rest.
#[test]
fn what_a_clip_does_not_move_stays_at_rest() {
    let mut skeleton = two_joints();
    let keys = [about_z(90.0), about_z(90.0)].concat();
    skeleton.clips.push(clip(vec![channel(
        0,
        ChannelPath::Rotation,
        &[0.0, 2.0],
        &keys,
    )]));
    let pose = skeleton.pose(Some(0), 1.0);
    assert!(near(pose[1].translation[0], 1.0));
}

/// Nonsense from a file does nothing, and doesn't fall over.
#[test]
fn a_broken_clip_does_no_harm() {
    let mut skeleton = two_joints();
    skeleton.clips.push(clip(vec![
        channel(9, ChannelPath::Rotation, &[0.0], &[0.0, 0.0, 0.0, 1.0]),
        channel(0, ChannelPath::Translation, &[], &[]),
        channel(1, ChannelPath::Translation, &[0.0, 1.0], &[1.0]),
    ]));
    let pose = skeleton.pose(Some(0), 0.5);
    assert_eq!(pose.len(), 2);
    assert!(
        skeleton.pose(Some(5), 0.0).len() == 2,
        "no such clip is at rest"
    );
    let empty = Skeleton::default();
    assert!(empty.pose(None, 0.0).is_empty());
    assert_eq!(empty.bounds(&[]).1, 0.0);
}

#[test]
fn a_place_goes_with_its_joint() {
    let mut skeleton = two_joints();
    let mut offset = NO_MOVE;
    offset[0][3] = 0.5;
    skeleton.places = vec![
        SkeletonPlace {
            name: "tip".to_owned(),
            joint: Some(1),
            offset,
        },
        SkeletonPlace {
            name: "base".to_owned(),
            joint: None,
            offset,
        },
    ];
    let keys = [about_z(90.0), about_z(90.0)].concat();
    skeleton.clips.push(clip(vec![channel(
        0,
        ChannelPath::Rotation,
        &[0.0, 2.0],
        &keys,
    )]));
    let placed = skeleton.placed(&skeleton.pose(Some(0), 1.0));
    let tip = moved(&skeleton.place("tip", &placed).expect("there"), Vec3::ZERO);
    assert!(same(tip, Vec3::new(0.0, 1.5, 0.0)), "{tip:?}");
    let base = moved(&skeleton.place("base", &placed).expect("there"), Vec3::ZERO);
    assert!(same(base, Vec3::new(0.5, 0.0, 0.0)), "it stays: {base:?}");
    assert!(skeleton.place("nothing", &placed).is_none());
}

/// Bounds go by the pose, or a mesh that's reached a long way pops out of view.
#[test]
fn bounds_follow_the_pose() {
    let mut skeleton = two_joints();
    skeleton.reach = 0.25;
    let keys = [0.0, 0.0, 0.0, 100.0, 0.0, 0.0];
    skeleton.clips.push(clip(vec![channel(
        1,
        ChannelPath::Translation,
        &[0.0, 2.0],
        &keys,
    )]));
    let placed = skeleton.placed(&skeleton.pose(Some(0), 1.99));
    let (center, radius) = skeleton.bounds(&placed);
    assert!(near(center.x, 49.75), "{center:?}");
    assert!(radius > 49.0, "{radius}");
}

/// Halfway through a blend, a joint is halfway, and at the ends it's what it was going between.
#[test]
fn a_blend_goes_from_one_pose_to_the_other() {
    let from = [JointPose {
        translation: [0.0, 0.0, 0.0],
        rotation: about_z(0.0),
        ..JointPose::default()
    }];
    let to = [JointPose {
        translation: [10.0, 0.0, 0.0],
        rotation: about_z(90.0),
        ..JointPose::default()
    }];
    let halfway = blend_poses(&from, &to, 0.5);
    assert!(near(halfway[0].translation[0], 5.0));
    let facing = turned(&halfway[0].rows(), Vec3::new(1.0, 0.0, 0.0));
    let wanted = about_z(45.0);
    let want = turned(
        &JointPose {
            rotation: wanted,
            ..JointPose::default()
        }
        .rows(),
        Vec3::new(1.0, 0.0, 0.0),
    );
    assert!(same(facing, want), "{facing:?}");
    assert_eq!(
        blend_poses(&from, &to, 0.0)[0].translation,
        from[0].translation
    );
    assert!(near(blend_poses(&from, &to, 1.0)[0].translation[0], 10.0));
}

/// A skeleton with two morph targets, the first resting at a half.
fn morphing() -> Skeleton {
    Skeleton {
        morph_names: vec!["smile".to_owned(), "frown".to_owned()],
        rest_weights: vec![0.5],
        ..Skeleton::default()
    }
}

#[test]
fn morph_weights_rest_where_the_model_says() {
    assert_eq!(morphing().weights(None, 0.0), [0.5, 0.0]);
}

#[test]
fn a_clip_moves_morph_weights() {
    let mut skeleton = morphing();
    let keys = [0.0, 0.0, 1.0, 0.5];
    skeleton.clips.push(clip(vec![channel(
        0,
        ChannelPath::Weights,
        &[0.0, 2.0],
        &keys,
    )]));
    let weights = skeleton.weights(Some(0), 1.0);
    assert!(
        near(weights[0], 0.5) && near(weights[1], 0.25),
        "{weights:?}"
    );
}

/// Weights aren't a joint's, whatever joint they happen to name.
#[test]
fn morph_weights_leave_joints_alone() {
    let mut skeleton = two_joints();
    let keys = [1.0, 1.0];
    skeleton.clips.push(clip(vec![channel(
        0,
        ChannelPath::Weights,
        &[0.0, 2.0],
        &keys,
    )]));
    assert_eq!(skeleton.pose(Some(0), 1.0)[0], skeleton.joints[0].rest);
}
