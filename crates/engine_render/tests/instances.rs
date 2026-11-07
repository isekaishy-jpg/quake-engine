//! Which copies of shared meshes are drawn, and how they're grouped.

use engine_core::math::Vec3;
use engine_map::{Map, MeshInstance, SharedMesh};
use engine_render::{
    INSTANCE_BYTES, InstanceBatch, InstanceLight, ball_in_view, instance_batches, write_instance,
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

#[test]
fn copies_of_a_mesh_are_one_batch_however_they_arrive() {
    let map = scattered(&[0, 1, 0, 1, 0]);
    let lights = vec![InstanceLight::default(); 5];
    let mut out = vec![0; 5 * INSTANCE_BYTES];
    let batches = instance_batches(&map, &lights, &[0, 1, 2, 3, 4], &EVERYWHERE, &mut out);
    let want = [
        InstanceBatch {
            mesh: 0,
            first: 0,
            count: 3,
        },
        InstanceBatch {
            mesh: 1,
            first: 3,
            count: 2,
        },
    ];
    assert_eq!(batches, want);
}

#[test]
fn what_is_out_of_view_is_left_out() {
    let map = scattered(&[0, 0, 0]);
    let lights = vec![InstanceLight::default(); 3];
    let mut out = vec![0; 3 * INSTANCE_BYTES];
    // Only what's short of x = 5.
    let mut planes = EVERYWHERE;
    planes[0] = [-1.0, 0.0, 0.0, 5.0];
    let batches = instance_batches(&map, &lights, &[0, 1, 2], &planes, &mut out);
    assert_eq!(
        batches,
        [InstanceBatch {
            mesh: 0,
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
