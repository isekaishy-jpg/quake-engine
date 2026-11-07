//! The spatial tree and potentially visible sets.

use engine_core::math::Vec3;

/// What fills a leaf.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
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

/// What a side of a clipping node leads to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClipChild {
    /// Another clipping node.
    Node(u32),
    /// Open space.
    #[default]
    Empty,
    /// Solid.
    Solid,
}

/// A node of a clipping hull, whose solids are already grown by the size of
/// the box that uses it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipNode {
    /// The dividing plane, by index.
    pub plane: u32,
    /// The front and back sides.
    pub children: [ClipChild; 2],
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
    /// How much water, sky, slime and lava can be heard, out of 255.
    pub ambient: [u8; 4],
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
    /// Each node's first face and how many it has, in [`crate::Map::faces`].
    pub node_faces: Vec<(u32, u32)>,
    /// The leaves.
    pub leaves: Vec<Leaf>,
    /// Faces by leaf, as indices into [`crate::Map::faces`].
    pub leaf_faces: Vec<u32>,
    /// The clipping hulls' nodes.
    pub clip_nodes: Vec<ClipNode>,
    /// Each leaf's box of space, for leaves that have one.
    pub leaf_boxes: Vec<(Vec3, Vec3)>,
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
    pub(crate) fn node(&self, index: u32) -> Option<&Node> {
        self.nodes.get(usize::try_from(index).ok()?)
    }

    /// The plane at `index`.
    pub(crate) fn plane(&self, index: u32) -> Option<&Plane> {
        self.planes.get(usize::try_from(index).ok()?)
    }

    /// Whether `point` is solid in the hull starting at `root`.
    #[must_use]
    pub fn clip_solid(&self, root: ClipChild, point: Vec3) -> bool {
        let mut child = root;
        loop {
            match child {
                ClipChild::Empty => return false,
                ClipChild::Solid => return true,
                ClipChild::Node(index) => {
                    let node = self
                        .clip_nodes
                        .get(usize::try_from(index).unwrap_or(usize::MAX));
                    let Some(node) = node else {
                        return false;
                    };
                    let Some(plane) = self.plane(node.plane) else {
                        return false;
                    };
                    let side = plane.normal.dot(point) - plane.distance;
                    child = node.children[usize::from(side < 0.0)];
                }
            }
        }
    }

    /// Adds to `out` the heights between `top` and `bottom` where the column at `x`, `y` could go
    /// in or out of what's solid below `child`, in no order. It's every plane the column crosses
    /// on the way, so it takes one walk down the tree where looking at each height would take a
    /// walk for each.
    pub fn clip_crossings(
        &self,
        child: ClipChild,
        (x, y): (f32, f32),
        (top, bottom): (f32, f32),
        out: &mut Vec<f32>,
    ) {
        let ClipChild::Node(index) = child else {
            return;
        };
        let Some(node) = self
            .clip_nodes
            .get(usize::try_from(index).unwrap_or(usize::MAX))
        else {
            return;
        };
        let Some(plane) = self.plane(node.plane) else {
            return;
        };
        let side = |z: f32| plane.normal.dot(Vec3::new(x, y, z)) - plane.distance;
        let (above, below) = (side(top), side(bottom));
        let child_of = |side: f32| node.children[usize::from(side < 0.0)];
        if (above < 0.0) == (below < 0.0) {
            self.clip_crossings(child_of(above), (x, y), (top, bottom), out);
            return;
        }
        let z = top + (bottom - top) * above / (above - below);
        out.push(z);
        self.clip_crossings(child_of(above), (x, y), (top, z), out);
        self.clip_crossings(child_of(below), (x, y), (z, bottom), out);
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
