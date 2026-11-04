//! Alias models, on files built here.

use engine_core::math::Vec3;
use fixture_quake::{Palette, parse_mdl};

/// A palette where index `i` is grey `i`.
fn palette() -> Palette {
    let bytes: Vec<u8> = (0..=255_u8).flat_map(|grey| [grey, grey, grey]).collect();
    Palette::parse(&bytes).expect("768 bytes")
}

/// Appends numbers.
fn put(out: &mut Vec<u8>, numbers: &[i32]) {
    for number in numbers {
        out.extend_from_slice(&number.to_le_bytes());
    }
}

/// Appends floats.
fn put_floats(out: &mut Vec<u8>, numbers: &[f32]) {
    for number in numbers {
        out.extend_from_slice(&number.to_le_bytes());
    }
}

/// One pose of 3 vertices at height `z`.
fn pose(out: &mut Vec<u8>, z: u8) {
    out.extend_from_slice(&[0; 8]);
    out.extend_from_slice(&[0; 16]);
    for x in [1, 2, 3] {
        out.extend_from_slice(&[x, 0, z, 0]);
    }
}

/// A model with a 4 by 2 skin, 3 vertices, one back-facing triangle, and two
/// frames: one pose, then a run of two.
fn file() -> Vec<u8> {
    let mut out = b"IDPO".to_vec();
    put(&mut out, &[6]);
    put_floats(
        &mut out,
        &[2.0, 2.0, 2.0, -1.0, -1.0, -1.0, 30.0, 0.0, 0.0, 22.0],
    );
    put(&mut out, &[1, 4, 2, 3, 1, 2, 1, 8, 0]);
    put(&mut out, &[0]);
    out.extend_from_slice(&[10, 20, 30, 40, 224, 225, 226, 227]);
    // The first vertex is on the seam.
    put(&mut out, &[32, 0, 0, 0, 1, 1, 0, 3, 1]);
    put(&mut out, &[0, 0, 1, 2]);
    put(&mut out, &[0]);
    pose(&mut out, 5);
    put(&mut out, &[1, 2]);
    out.extend_from_slice(&[0; 8]);
    put_floats(&mut out, &[0.1, 0.3]);
    pose(&mut out, 6);
    pose(&mut out, 7);
    out
}

#[test]
fn a_model_keeps_its_scale_and_flags() {
    let model = parse_mdl(&file(), &palette()).expect("it parses");
    assert_eq!(model.scale, Vec3::new(2.0, 2.0, 2.0));
    assert_eq!(model.offset, Vec3::new(-1.0, -1.0, -1.0));
    assert_eq!(model.flags, 8);
    assert!(model.spins);
}

#[test]
fn skins_are_rgba_and_fullbright_has_no_alpha() {
    let model = parse_mdl(&file(), &palette()).expect("it parses");
    assert_eq!((model.skin_width, model.skin_height), (4, 2));
    assert_eq!(model.skins.len(), 1);
    assert_eq!(model.skins[0][..4], [10, 10, 10, 255]);
    assert_eq!(model.skins[0][16..20], [224, 224, 224, 0]);
}

#[test]
fn a_seam_on_a_back_facing_triangle_moves_half_a_skin() {
    let model = parse_mdl(&file(), &palette()).expect("it parses");
    assert_eq!(model.corners.len(), 3);
    assert!(
        (model.corners[0].uv[0] - 2.5 / 4.0).abs() < 1e-6,
        "on the seam"
    );
    assert!((model.corners[1].uv[0] - 1.5 / 4.0).abs() < 1e-6);
    assert!((model.corners[2].uv[1] - 1.5 / 2.0).abs() < 1e-6);
}

#[test]
fn frames_run_over_poses_and_a_run_cycles_by_time() {
    let model = parse_mdl(&file(), &palette()).expect("it parses");
    assert_eq!(model.poses.len(), 3);
    assert_eq!(model.poses[0][1].position, [2, 0, 5]);
    assert_eq!(model.frames.len(), 2);
    assert_eq!(model.frames[0].pose_at(5.0), 0);
    let run = &model.frames[1];
    assert_eq!(run.pose_at(0.05), 1);
    assert_eq!(run.pose_at(0.2), 2);
    assert_eq!(run.pose_at(0.35), 1, "it goes round");
}

#[test]
fn normals_come_from_the_table() {
    let model = parse_mdl(&file(), &palette()).expect("it parses");
    assert_eq!(model.poses[0][0].normal, [-67, 0, 108]);
}

#[test]
fn bad_files_are_refused() {
    let good = file();
    assert!(parse_mdl(b"", &palette()).is_none());
    assert!(parse_mdl(b"IDPO", &palette()).is_none());
    let mut version = good.clone();
    version[4] = 3;
    assert!(parse_mdl(&version, &palette()).is_none());
    let mut vertex = good.clone();
    let triangle_at = 84 + 4 + 8 + 36 + 4;
    vertex[triangle_at] = 9;
    assert!(
        parse_mdl(&vertex, &palette()).is_none(),
        "a vertex out of range"
    );
    let mut huge = good.clone();
    huge[52..56].copy_from_slice(&i32::MAX.to_le_bytes());
    assert!(parse_mdl(&huge, &palette()).is_none(), "a hostile count");
    for cut in [10, 90, 150, good.len() - 1] {
        assert!(
            parse_mdl(&good[..cut], &palette()).is_none(),
            "cut at {cut}"
        );
    }
}
