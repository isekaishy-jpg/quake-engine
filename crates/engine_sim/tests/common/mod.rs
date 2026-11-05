//! Small maps for tests.

use std::sync::Arc;

use engine_core::math::Vec3;
use engine_map::{ClipChild, ClipNode, Contents, Leaf, Map, Node, NodeChild, Plane};
pub(crate) use engine_test_maps::room;

/// A plane facing along `normal`.
fn plane(normal: Vec3, distance: f32) -> Plane {
    Plane { normal, distance }
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
