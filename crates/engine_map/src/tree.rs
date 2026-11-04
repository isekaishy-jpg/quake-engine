//! The spatial tree and potentially visible sets.

use engine_core::math::Vec3;

/// What fills a leaf.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Contents {
    /// Open air.
    #[default]
    Empty,
    /// Solid.
    Solid,
    /// Water.
    Water,
    /// Slime.
    Slime,
    /// Lava.
    Lava,
    /// Sky.
    Sky,
}

/// A plane: points where `dot(normal, point) == distance`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Plane {
    /// The normal.
    pub normal: Vec3,
    /// The distance from the origin.
    pub distance: f32,
}

/// What a side of a node leads to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeChild {
    /// Another node.
    Node(u32),
    /// A leaf.
    Leaf(u32),
}

/// A node, dividing space by a plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node {
    /// The dividing plane, by index.
    pub plane: u32,
    /// The front and back sides.
    pub children: [NodeChild; 2],
}

/// A convex region of space.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Leaf {
    /// What fills it.
    pub contents: Contents,
    /// Where its set starts in [`Tree::visibility`], if it has one.
    pub visibility: Option<u32>,
    /// The first entry in [`Tree::leaf_faces`].
    pub first_face: u32,
    /// How many faces touch it.
    pub face_count: u32,
}

/// The tree of the world model, with its visibility data.
///
/// Leaf 0 is the solid outside. Visibility is a bit per leaf, starting at leaf
/// 1, with runs of zero bytes stored as a zero and a count.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tree {
    /// The planes.
    pub planes: Vec<Plane>,
    /// The nodes. The first is the root.
    pub nodes: Vec<Node>,
    /// The leaves.
    pub leaves: Vec<Leaf>,
    /// Faces by leaf, as indices into [`crate::Map::faces`].
    pub leaf_faces: Vec<u32>,
    /// How many leaves visibility covers, not counting leaf 0.
    pub visible_leaf_count: u32,
    /// Run-length encoded visible sets.
    pub visibility: Vec<u8>,
}

impl Tree {
    /// The leaf holding `point`.
    #[must_use]
    pub fn leaf_at(&self, point: Vec3) -> u32 {
        let mut child = if self.nodes.is_empty() {
            NodeChild::Leaf(0)
        } else {
            NodeChild::Node(0)
        };
        loop {
            match child {
                NodeChild::Leaf(leaf) => return leaf,
                NodeChild::Node(index) => {
                    let Some(node) = self.node(index) else {
                        return 0;
                    };
                    let Some(plane) = self.plane(node.plane) else {
                        return 0;
                    };
                    let side = plane.normal.dot(point) - plane.distance;
                    child = node.children[usize::from(side < 0.0)];
                }
            }
        }
    }

    /// The node at `index`.
    fn node(&self, index: u32) -> Option<&Node> {
        self.nodes.get(usize::try_from(index).ok()?)
    }

    /// The plane at `index`.
    fn plane(&self, index: u32) -> Option<&Plane> {
        self.planes.get(usize::try_from(index).ok()?)
    }

    /// The leaf at `index`.
    #[must_use]
    pub fn leaf(&self, index: u32) -> Option<&Leaf> {
        self.leaves.get(usize::try_from(index).ok()?)
    }

    /// Adds every leaf visible from `leaf` to `visible`, a flag per leaf.
    /// Everything is visible from a leaf with no set, such as the outside.
    pub fn mark_visible(&self, leaf: u32, visible: &mut [bool]) {
        let start = self
            .leaf(leaf)
            .filter(|_| leaf != 0)
            .and_then(|leaf| leaf.visibility)
            .and_then(|offset| usize::try_from(offset).ok());
        let Some(start) = start else {
            visible.iter_mut().for_each(|flag| *flag = true);
            return;
        };
        let count = usize::try_from(self.visible_leaf_count).unwrap_or(0);
        let mut bytes = self.visibility.iter().skip(start).copied();
        let mut index = 0;
        while index < count {
            let Some(byte) = bytes.next() else {
                return;
            };
            if byte == 0 {
                index += usize::from(bytes.next().unwrap_or(0)) * 8;
                continue;
            }
            for bit in 0..8 {
                if byte & (1 << bit) != 0
                    && index + bit < count
                    && let Some(flag) = visible.get_mut(index + bit + 1)
                {
                    *flag = true;
                }
            }
            index += 8;
        }
    }
}
