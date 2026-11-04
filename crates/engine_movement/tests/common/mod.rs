//! Small maps for tests.

use std::sync::Arc;

use engine_core::math::Vec3;
use engine_map::{ClipChild, ClipNode, Contents, Leaf, Map, Model, Node, NodeChild, Plane, Tree};

/// A plane facing along `normal`.
fn plane(normal: Vec3, distance: f32) -> Plane {
    Plane { normal, distance }
}

/// An open box from -256 to 256 on each axis, solid everywhere else. The walls
/// are where a player's centre stops, as in a clipping hull.
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

/// The room with the floor raised by `height` where x is positive.
pub(crate) fn room_with_step(height: f32) -> Arc<Map> {
    let mut map = Map::clone(&room());
    let tree = &mut map.tree;
    let first_plane = u32::try_from(tree.planes.len()).expect("small");
    let first_node = u32::try_from(tree.clip_nodes.len()).expect("small");
    tree.planes.push(plane(Vec3::new(1.0, 0.0, 0.0), 0.0));
    tree.planes
        .push(plane(Vec3::new(0.0, 0.0, 1.0), -256.0 + height));
    tree.clip_nodes[5].children[0] = ClipChild::Node(first_node);
    tree.clip_nodes.push(ClipNode {
        plane: first_plane,
        children: [ClipChild::Node(first_node + 1), ClipChild::Empty],
    });
    tree.clip_nodes.push(ClipNode {
        plane: first_plane + 1,
        children: [ClipChild::Empty, ClipChild::Solid],
    });
    Arc::new(map)
}

/// The room with water below z = -128.
pub(crate) fn room_with_water() -> Arc<Map> {
    let mut map = Map::clone(&room());
    let tree = &mut map.tree;
    let surface = u32::try_from(tree.planes.len()).expect("small");
    tree.planes.push(plane(Vec3::new(0.0, 0.0, 1.0), -128.0));
    tree.nodes = vec![Node {
        plane: surface,
        children: [NodeChild::Leaf(1), NodeChild::Leaf(2)],
    }];
    tree.leaves.push(Leaf {
        contents: Contents::Water,
        ..Leaf::default()
    });
    Arc::new(map)
}
