//! A small map to test on.

use std::sync::Arc;

use engine_core::math::Vec3;
use engine_map::{
    ClipChild, ClipNode, Contents, Entity, Leaf, Map, Model, Node, NodeChild, Plane, Tree,
};

/// Adds a plane and returns its number.
fn add_plane(tree: &mut Tree, normal: Vec3, distance: f32) -> u32 {
    tree.planes.push(Plane { normal, distance });
    u32::try_from(tree.planes.len() - 1).expect("small")
}

/// Adds a hull that's solid inside a box, and returns its root.
fn add_box_hull(tree: &mut Tree, mins: Vec3, maxs: Vec3) -> ClipChild {
    let x = Vec3::new(1.0, 0.0, 0.0);
    let y = Vec3::new(0.0, 1.0, 0.0);
    let z = Vec3::new(0.0, 0.0, 1.0);
    let sides = [
        (x, maxs.x, false),
        (x, mins.x, true),
        (y, maxs.y, false),
        (y, mins.y, true),
        (z, maxs.z, false),
        (z, mins.z, true),
    ];
    let first = u32::try_from(tree.clip_nodes.len()).expect("small");
    for (index, (normal, distance, inside_is_front)) in (0_u32..).zip(sides) {
        let plane = add_plane(tree, normal, distance);
        let inner = if index == 5 {
            ClipChild::Solid
        } else {
            ClipChild::Node(first + index + 1)
        };
        let children = if inside_is_front {
            [inner, ClipChild::Empty]
        } else {
            [ClipChild::Empty, inner]
        };
        tree.clip_nodes.push(ClipNode { plane, children });
    }
    ClipChild::Node(first)
}

/// Adds a drawing tree that's solid inside a box, and returns its root. Leaf 0
/// is solid and leaf 1 open.
fn add_box_nodes(tree: &mut Tree, mins: Vec3, maxs: Vec3) -> NodeChild {
    let x = Vec3::new(1.0, 0.0, 0.0);
    let y = Vec3::new(0.0, 1.0, 0.0);
    let z = Vec3::new(0.0, 0.0, 1.0);
    let sides = [
        (x, maxs.x, false),
        (x, mins.x, true),
        (y, maxs.y, false),
        (y, mins.y, true),
        (z, maxs.z, false),
        (z, mins.z, true),
    ];
    let first = u32::try_from(tree.nodes.len()).expect("small");
    for (index, (normal, distance, inside_is_front)) in (0_u32..).zip(sides) {
        let plane = add_plane(tree, normal, distance);
        let inner = if index == 5 {
            NodeChild::Leaf(0)
        } else {
            NodeChild::Node(first + index + 1)
        };
        let children = if inside_is_front {
            [inner, NodeChild::Leaf(1)]
        } else {
            [NodeChild::Leaf(1), inner]
        };
        tree.nodes.push(Node { plane, children });
    }
    NodeChild::Node(first)
}

/// An open room from -256 to 256, with brush models for `boxes`, and `entities`.
pub(crate) fn map(boxes: &[(Vec3, Vec3)], entities: &[&[(&str, &str)]]) -> Arc<Map> {
    let mut tree = Tree::default();
    let x = Vec3::new(1.0, 0.0, 0.0);
    let y = Vec3::new(0.0, 1.0, 0.0);
    let z = Vec3::new(0.0, 0.0, 1.0);
    let walls = [
        (x, 256.0, true),
        (x, -256.0, false),
        (y, 256.0, true),
        (y, -256.0, false),
        (z, 256.0, true),
        (z, -256.0, false),
    ];
    for (index, (normal, distance, solid_in_front)) in (0_u32..).zip(walls) {
        let plane = add_plane(&mut tree, normal, distance);
        let next = if index == 5 {
            ClipChild::Empty
        } else {
            ClipChild::Node(index + 1)
        };
        let children = if solid_in_front {
            [ClipChild::Solid, next]
        } else {
            [next, ClipChild::Solid]
        };
        tree.clip_nodes.push(ClipNode { plane, children });
        let open = if index == 5 {
            NodeChild::Leaf(1)
        } else {
            NodeChild::Node(index + 1)
        };
        let children = if solid_in_front {
            [NodeChild::Leaf(0), open]
        } else {
            [open, NodeChild::Leaf(0)]
        };
        tree.nodes.push(Node { plane, children });
    }
    tree.leaves = vec![
        Leaf {
            contents: Contents::Solid,
            ..Leaf::default()
        },
        Leaf::default(),
    ];
    let mut models = vec![Model {
        mins: Vec3::new(-256.0, -256.0, -256.0),
        maxs: Vec3::new(256.0, 256.0, 256.0),
        hulls: [ClipChild::Node(0), ClipChild::Empty],
        root: Some(NodeChild::Node(0)),
        ..Model::default()
    }];
    for &(mins, maxs) in boxes {
        let hull = add_box_hull(&mut tree, mins, maxs);
        let root = add_box_nodes(&mut tree, mins, maxs);
        models.push(Model {
            mins,
            maxs,
            hulls: [hull, ClipChild::Empty],
            root: Some(root),
            ..Model::default()
        });
    }
    let entities = entities
        .iter()
        .map(|fields| Entity {
            fields: fields
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        })
        .collect();
    Arc::new(Map {
        tree,
        models,
        entities,
        ..Map::default()
    })
}
