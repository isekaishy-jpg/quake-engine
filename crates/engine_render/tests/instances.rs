//! Which copies of shared meshes are drawn, and how they're grouped.

use engine_core::math::Vec3;
use engine_map::{Map, MeshInstance, SharedMesh};
use engine_render::{
    INSTANCE_BYTES, InstanceBatch, InstanceLight, LevelView, ball_in_view, choose_level,
    height_share, instance_batches, write_instance,
};

/// A map of `mesh_of` for each instance, spread along x.
fn scattered(mesh_of: &[u32]) -> Map {
    let mut map = Map::default();
    let shared = SharedMesh {
        radius: 1.0,
        ..SharedMesh::default()
    };
    map.shared_meshes = vec![shared.clone(), shared];
    for (at, &mesh) in (0_u16..).zip(mesh_of) {
        let mut instance = MeshInstance {
            mesh,
            rows: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
            ],
            ..MeshInstance::default()
        };
        instance.rows[0][3] = f32::from(at) * 10.0;
        map.mesh_instances.push(instance);
    }
    map
}

/// Everything's in view.
const EVERYWHERE: [[f32; 4]; 6] = [[0.0, 0.0, 0.0, 1.0]; 6];

/// A view from the middle of things.
const VIEW: LevelView = LevelView {
    eye: Vec3::ZERO,
    height_scale: 1.0,
};

/// What's drawn of `map`, seeing every copy, with `levels` as where each copy is.
fn batches_of(map: &Map, planes: &[[f32; 4]; 6], levels: &mut [usize]) -> Vec<InstanceBatch> {
    let count = map.mesh_instances.len();
    let lights = vec![InstanceLight::default(); count];
    let visible: Vec<u32> = (0..u32::try_from(count).expect("few")).collect();
    let mut out = vec![0; count * INSTANCE_BYTES];
    instance_batches(map, &lights, &visible, planes, (&VIEW, levels), &mut out)
}

#[test]
fn copies_of_a_mesh_are_one_batch_however_they_arrive() {
    let map = scattered(&[0, 1, 0, 1, 0]);
    let batches = batches_of(&map, &EVERYWHERE, &mut [0; 5]);
    let want = [
        InstanceBatch {
            mesh: 0,
            level: 0,
            first: 0,
            count: 3,
        },
        InstanceBatch {
            mesh: 1,
            level: 0,
            first: 3,
            count: 2,
        },
    ];
    assert_eq!(batches, want);
}

#[test]
fn what_is_out_of_view_is_left_out() {
    let map = scattered(&[0, 0, 0]);
    // Only what's short of x = 5.
    let mut planes = EVERYWHERE;
    planes[0] = [-1.0, 0.0, 0.0, 5.0];
    let batches = batches_of(&map, &planes, &mut [0; 3]);
    assert_eq!(
        batches,
        [InstanceBatch {
            mesh: 0,
            level: 0,
            first: 0,
            count: 1
        }]
    );
}

#[test]
fn a_mesh_is_kept_in_view_until_all_of_it_has_gone() {
    let planes = [[1.0, 0.0, 0.0, 0.0]; 6];
    assert!(ball_in_view(&planes, Vec3::new(-0.5, 0.0, 0.0), 1.0));
    assert!(!ball_in_view(&planes, Vec3::new(-2.0, 0.0, 0.0), 1.0));
}

#[test]
fn an_instance_writes_its_place_light_and_styles() {
    let light = InstanceLight {
        uv: [0.25, 0.5],
        step: 0.125,
        styles: [1, 2, 3, 4],
    };
    let rows = [[2.0, 0.0, 0.0, 7.0], [0.0; 4], [0.0; 4]];
    let mut out = [0; INSTANCE_BYTES];
    write_instance(&rows, &light, &mut out);
    assert_eq!(out[..4], 2.0_f32.to_le_bytes());
    assert_eq!(out[12..16], 7.0_f32.to_le_bytes());
    assert_eq!(out[48..52], 0.25_f32.to_le_bytes());
    assert_eq!(out[56..60], 0.125_f32.to_le_bytes());
    assert_eq!(out[60..], [1, 2, 3, 4]);
}

/// The usual sizes.
const SIZES: [f32; 3] = [0.25, 0.1, 0.03];

#[test]
fn smaller_on_screen_is_coarser() {
    assert_eq!(choose_level(0, 0.5, &SIZES, 4), 0);
    assert_eq!(choose_level(0, 0.2, &SIZES, 4), 1);
    assert_eq!(choose_level(0, 0.05, &SIZES, 4), 2);
    assert_eq!(choose_level(0, 0.01, &SIZES, 4), 3);
}

#[test]
fn a_mesh_never_asks_for_a_level_it_does_not_have() {
    assert_eq!(choose_level(0, 0.001, &SIZES, 2), 1);
    assert_eq!(choose_level(0, 0.001, &SIZES, 1), 0);
    assert_eq!(choose_level(0, 0.001, &SIZES, 0), 0);
}

/// Sitting on a boundary doesn't flicker between two levels: coming back takes a margin.
#[test]
fn a_boundary_does_not_flicker() {
    let just_over = 0.25 * 1.05;
    assert_eq!(
        choose_level(1, just_over, &SIZES, 4),
        1,
        "held at the coarser"
    );
    assert_eq!(choose_level(0, just_over, &SIZES, 4), 0, "and at the finer");
    assert_eq!(
        choose_level(1, 0.25 * 1.2, &SIZES, 4),
        0,
        "past the margin it comes back"
    );
}

#[test]
fn share_of_the_view_goes_with_size_and_distance() {
    let near = height_share(10.0, 100.0, 1.0);
    assert!((near - 0.1).abs() < 0.0001);
    assert!((height_share(10.0, 200.0, 1.0) - near / 2.0).abs() < 0.0001);
    assert!((height_share(20.0, 100.0, 1.0) - near * 2.0).abs() < 0.0001);
    // Inside it, it fills the view rather than going to infinity.
    assert!(height_share(10.0, 0.0, 1.0).is_finite());
}

/// A map of one mesh with `levels`, one copy, at `distance`.
fn one_copy(levels: usize, distance: f32) -> Map {
    let mut map = scattered(&[0]);
    map.shared_meshes[0].levels = vec![Vec::new(); levels];
    map.shared_meshes[0].level_sizes = SIZES.to_vec();
    map.mesh_instances[0].rows[0][3] = distance;
    map
}

/// A copy's level is kept, and goes with how far off it is.
#[test]
fn a_copy_changes_level_as_it_gets_further_off() {
    let mut levels = [0];
    let near = batches_of(&one_copy(4, 1.0), &EVERYWHERE, &mut levels);
    assert_eq!(near[0].level, 0);
    let far = batches_of(&one_copy(4, 1000.0), &EVERYWHERE, &mut levels);
    assert_eq!(far[0].level, 3);
    assert_eq!(levels, [3]);
}

/// A copy first seen far off goes straight to the level it wants.
#[test]
fn a_copy_first_seen_far_off_starts_coarse() {
    let far = batches_of(&one_copy(4, 1000.0), &EVERYWHERE, &mut [0]);
    assert_eq!(far.len(), 1);
    assert_eq!(far[0].level, 3);
}
