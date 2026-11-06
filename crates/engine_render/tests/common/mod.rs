//! Small maps for tests.

use engine_core::math::Vec3;
use engine_map::{
    Face, FaceLight, Leaf, Look, Map, MipLevel, Model, RESIDENT, TexelAxis, Texture, TextureKind,
    Tree,
};

/// A map of `models` models, each one 128 unit square at z = 0, facing up.
pub(crate) fn map(models: u32) -> Map {
    let side = 128.0;
    let mut map = Map {
        textures: vec![Texture {
            name: "test".to_owned(),
            kind: TextureKind::Solid,
            mips: vec![MipLevel {
                width: 64,
                height: 64,
                rgba: [255, 255, 255, 255].repeat(64 * 64),
                form: engine_map::PixelForm::Rgba,
            }],
            animation: None,
            alternate: None,
            material: None,
        }],
        light: vec![[200, 100, 50]; 81],
        lit: true,
        tree: Tree {
            leaves: vec![
                Leaf::default(),
                Leaf {
                    first_face: 0,
                    face_count: 1,
                    ..Leaf::default()
                },
            ],
            leaf_faces: vec![0],
            ..Tree::default()
        },
        // Light as it was on a picture in gamma space, which is easy to reason about.
        look: Look {
            light_curve: 2.2,
            exposure: 1.0,
        },
        ..Map::default()
    };
    // With no nodes every point is in leaf 0, from which everything shows.
    map.tree.visible_leaf_count = 1;
    for number in 0..models {
        map.points.extend([
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(side, 0.0, 0.0),
            Vec3::new(side, side, 0.0),
            Vec3::new(0.0, side, 0.0),
        ]);
        map.faces.push(Face {
            first_point: number * 4,
            point_count: 4,
            texture: 0,
            axes: [
                TexelAxis {
                    axis: Vec3::new(1.0, 0.0, 0.0),
                    offset: 0.0,
                },
                TexelAxis {
                    axis: Vec3::new(0.0, 1.0, 0.0),
                    offset: 0.0,
                },
            ],
            light: Some(FaceLight {
                first_sample: 0,
                width: 9,
                height: 9,
                texel_origin: [0, 0],
                texels_per_sample: 16,
                styles: [0, Face::NO_STYLE, Face::NO_STYLE, Face::NO_STYLE],
                tile: RESIDENT,
                average: [0; 3],
            }),
        });
        map.models.push(Model {
            first_face: number,
            face_count: 1,
            ..Model::default()
        });
    }
    map
}
