//! Small maps for tests.

use std::sync::Arc;

use engine_core::math::Vec3;
use engine_map::{ClipChild, ClipNode, Leaf, Map, Model, Node, NodeChild, Plane, Tree};

/// A plane facing along `normal`.
fn plane(normal: Vec3, distance: f32) -> Plane {
    Plane { normal, distance }
}

/// An open box from -256 to 256 on each axis, solid everywhere else.
pub(crate) fn room() -> Arc<Map> {
    let x = Vec3::new(1.0, 0.0, 0.0);
    let y = Vec3::new(0.0, 1.0, 0.0);
    let z = Vec3::new(0.0, 0.0, 1.0);
    let planes = vec![
        plane(x, 256.0),
        plane(x, -256.0),
        plane(y, 256.0),
        plane(y, -256.0),
        plane(z, 256.0),
        plane(z, -256.0),
    ];
    // Each node's front is solid for a positive wall, and its back for a
    // negative one, leaving the inside open at the end of the chain.
    let chain = |plane: u32, next: ClipChild| {
        if plane.is_multiple_of(2) {
            ClipNode {
                plane,
                children: [ClipChild::Solid, next],
            }
        } else {
            ClipNode {
                plane,
                children: [next, ClipChild::Solid],
            }
        }
    };
    let clip_nodes = vec![
        chain(0, ClipChild::Node(1)),
        chain(1, ClipChild::Node(2)),
        chain(2, ClipChild::Node(3)),
        chain(3, ClipChild::Node(4)),
        chain(4, ClipChild::Node(5)),
        chain(5, ClipChild::Empty),
    ];
    let tree = Tree {
        planes,
        nodes: vec![Node {
            plane: 5,
            children: [NodeChild::Leaf(1), NodeChild::Leaf(0)],
        }],
        leaves: vec![Leaf::default(), Leaf::default()],
        clip_nodes,
        ..Tree::default()
    };
    Arc::new(Map {
        tree,
        models: vec![Model {
            visible: true,
            mins: Vec3::new(-256.0, -256.0, -256.0),
            maxs: Vec3::new(256.0, 256.0, 256.0),
            hulls: [ClipChild::Node(0), ClipChild::Empty],
            ..Model::default()
        }],
        ..Map::default()
    })
}
