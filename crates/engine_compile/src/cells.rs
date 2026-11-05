//! Cutting the world into cells of space, so that visibility has leaves to work
//! with (proposal 0088).
//!
//! The tree from brushes knows what's solid but has one leaf for each kind of
//! contents. Cells are a grid laid over it: each keeps a copy of the part of the
//! tree that touches it, with leaves of its own.

use std::collections::HashMap;

use engine_core::math::Vec3;
use engine_map::{Contents, Leaf, Map, Node, NodeChild, Tree};

use crate::geometry::{Plane3, Point};
use crate::tree::PlaneSet;

/// How wide a cell is, unless the map says otherwise.
pub(crate) const DEFAULT_CELL: f64 = 256.0;

/// A box.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cell {
    /// Its low corner.
    low: Point,
    /// Its high corner.
    high: Point,
}

/// Builds the new top of the tree.
struct Cutter<'a> {
    /// The old tree.
    tree: &'a Tree,
    /// Its planes, which the grid adds to.
    planes: &'a mut PlaneSet,
    /// How wide a cell is.
    size: f64,
    /// The new nodes, which go first.
    nodes: Vec<Node>,
    /// The leaves, starting with the old ones.
    leaves: Vec<Leaf>,
    /// The box of each leaf that has one.
    boxes: Vec<(Vec3, Vec3)>,
    /// Each cell's leaf for each contents.
    made: HashMap<(usize, Contents), u32>,
    /// How many cells so far.
    cells: usize,
    /// Each cell's box and leaves.
    found: Vec<(Cell, Vec<u32>)>,
}

/// Where a box is, against a plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    /// All in front.
    Front,
    /// All behind.
    Back,
    /// Across it.
    Both,
}

/// Which side of `plane` `cell` is on.
fn side_of(plane: &Plane3, cell: &Cell) -> Side {
    let mut nearest = -plane.distance;
    let mut furthest = -plane.distance;
    for axis in 0..3 {
        let normal = plane.normal[axis];
        let (low, high) = (normal * cell.low[axis], normal * cell.high[axis]);
        nearest += low.min(high);
        furthest += low.max(high);
    }
    if nearest >= 0.0 {
        Side::Front
    } else if furthest < 0.0 {
        Side::Back
    } else {
        Side::Both
    }
}

impl Cutter<'_> {
    /// The leaf of `contents` in the cell being made.
    fn leaf_of(&mut self, contents: Contents, cell: &Cell) -> NodeChild {
        if contents == Contents::Solid {
            return NodeChild::Leaf(0);
        }
        let number = self.cells;
        if let Some(leaf) = self.made.get(&(number, contents)) {
            return NodeChild::Leaf(*leaf);
        }
        let leaf = u32::try_from(self.leaves.len()).unwrap_or(0);
        self.leaves.push(Leaf {
            contents,
            ..Leaf::default()
        });
        #[expect(clippy::cast_possible_truncation, reason = "the map stores f32")]
        let corner = |point: Point| Vec3::new(point[0] as f32, point[1] as f32, point[2] as f32);
        self.boxes.push((corner(cell.low), corner(cell.high)));
        self.made.insert((number, contents), leaf);
        NodeChild::Leaf(leaf)
    }

    /// The part of the old tree under `child` that touches `cell`.
    fn copy(&mut self, child: NodeChild, cell: &Cell) -> NodeChild {
        let index = match child {
            NodeChild::Leaf(leaf) => {
                let contents = self
                    .tree
                    .leaf(leaf)
                    .map_or(Contents::Solid, |leaf| leaf.contents);
                return self.leaf_of(contents, cell);
            }
            NodeChild::Node(index) => index,
        };
        let Some(node) = self.tree.nodes.get(index as usize).copied() else {
            return NodeChild::Leaf(0);
        };
        let Some(plane) = self.planes.planes.get(node.plane as usize).copied() else {
            return NodeChild::Leaf(0);
        };
        match side_of(&plane, cell) {
            Side::Front => self.copy(node.children[0], cell),
            Side::Back => self.copy(node.children[1], cell),
            Side::Both => {
                let front = self.copy(node.children[0], cell);
                let back = self.copy(node.children[1], cell);
                if front == back {
                    return front;
                }
                self.push(node.plane, front, back)
            }
        }
    }

    /// Adds a node.
    fn push(&mut self, plane: u32, front: NodeChild, back: NodeChild) -> NodeChild {
        let at = u32::try_from(self.nodes.len()).unwrap_or(0);
        self.nodes.push(Node {
            plane,
            children: [front, back],
        });
        NodeChild::Node(at)
    }

    /// Splits `area` along its longest side until it's a cell, then copies the
    /// tree in. `root` is the old root.
    fn split(&mut self, root: NodeChild, area: Cell, first: bool) -> NodeChild {
        let sizes = [0, 1, 2].map(|axis| area.high[axis] - area.low[axis]);
        let axis = (0..3)
            .max_by(|a, b| sizes[*a].total_cmp(&sizes[*b]))
            .unwrap_or(0);
        // The first cut always happens, as the tree starts at node 0.
        if sizes[axis] <= self.size * 1.001 && !first {
            let before = self.made.len();
            let child = self.copy(root, &area);
            let leaves: Vec<u32> = self
                .made
                .iter()
                .filter(|((cell, _), _)| *cell == self.cells)
                .map(|(_, leaf)| *leaf)
                .collect();
            if self.made.len() > before {
                self.found.push((area, leaves));
            }
            self.cells += 1;
            return child;
        }
        // On a multiple of the cell size, so maps line up however they grow.
        let middle = (area.low[axis] + area.high[axis]) * 0.5;
        let cut = (middle / self.size).round() * self.size;
        let cut = if cut <= area.low[axis] || cut >= area.high[axis] {
            middle
        } else {
            cut
        };
        let mut normal = [0.0; 3];
        normal[axis] = 1.0;
        let plane = self.planes.index_of(Plane3 {
            normal,
            distance: cut,
        });
        let mut high = area;
        high.low[axis] = cut;
        let mut low = area;
        low.high[axis] = cut;
        let reserved = self.push(plane, NodeChild::Leaf(0), NodeChild::Leaf(0));
        let front = self.split(root, high, false);
        let back = self.split(root, low, false);
        if let NodeChild::Node(index) = reserved {
            self.nodes[index as usize].children = [front, back];
        }
        reserved
    }
}

/// Cuts the world's space into cells of `size`, with `low` and `high` round it.
/// Each cell's leaves list the world's faces that touch it.
pub(crate) fn cut_cells(map: &mut Map, planes: &mut PlaneSet, size: f64) {
    let Some(world) = map.models.first() else {
        return;
    };
    let Some(root) = world.root else {
        return;
    };
    let wide = |value: Vec3| [f64::from(value.x), f64::from(value.y), f64::from(value.z)];
    let (low, high) = (wide(world.mins), wide(world.maxs));
    if low == high {
        return;
    }
    let (first_face, face_count) = (world.first_face, world.face_count);
    let mut cutter = Cutter {
        tree: &map.tree,
        planes,
        size,
        nodes: Vec::new(),
        leaves: map.tree.leaves.clone(),
        boxes: vec![(Vec3::ZERO, Vec3::ZERO); map.tree.leaves.len()],
        made: HashMap::new(),
        cells: 0,
        found: Vec::new(),
    };
    let new_root = cutter.split(root, Cell { low, high }, true);
    let Cutter {
        mut nodes,
        leaves,
        boxes,
        found,
        ..
    } = cutter;
    // The tree starts at node 0, so the new nodes go first and the old ones,
    // which moving models still use, move along.
    let moved = u32::try_from(nodes.len()).unwrap_or(0);
    let shift = |child: &mut NodeChild| {
        if let NodeChild::Node(index) = child {
            *index += moved;
        }
    };
    for node in &mut map.tree.nodes {
        node.children.iter_mut().for_each(shift);
    }
    for model in &mut map.models {
        if let Some(root) = &mut model.root {
            shift(root);
        }
    }
    if let Some(world) = map.models.first_mut() {
        world.root = Some(new_root);
    }
    let count = nodes.len();
    nodes.append(&mut map.tree.nodes);
    map.tree.nodes = nodes;
    let mut node_faces = vec![(0, 0); count];
    node_faces.append(&mut map.tree.node_faces);
    map.tree.node_faces = node_faces;
    map.tree.leaves = leaves;
    map.tree.leaf_boxes = boxes;
    list_faces(map, first_face, face_count, &found);
}

/// Lists each leaf's faces, by which cells their bounds touch.
fn list_faces(map: &mut Map, first_face: u32, face_count: u32, cells: &[(Cell, Vec<u32>)]) {
    let mut faces: Vec<Vec<u32>> = vec![Vec::new(); map.tree.leaves.len()];
    for number in first_face..first_face + face_count {
        let Some(face) = map.faces.get(number as usize) else {
            continue;
        };
        let (mut low, mut high) = ([f64::MAX; 3], [f64::MIN; 3]);
        for point in map.face_points(face) {
            let parts = [point.x, point.y, point.z];
            for axis in 0..3 {
                low[axis] = low[axis].min(f64::from(parts[axis]) - 1.0);
                high[axis] = high[axis].max(f64::from(parts[axis]) + 1.0);
            }
        }
        for (cell, leaves) in cells {
            let apart =
                (0..3).any(|axis| high[axis] < cell.low[axis] || low[axis] > cell.high[axis]);
            if !apart {
                for leaf in leaves {
                    faces[*leaf as usize].push(number);
                }
            }
        }
    }
    let mut list = Vec::new();
    for (leaf, faces) in map.tree.leaves.iter_mut().zip(faces) {
        leaf.first_face = u32::try_from(list.len()).unwrap_or(0);
        leaf.face_count = u32::try_from(faces.len()).unwrap_or(0);
        list.extend(faces);
    }
    map.tree.leaf_faces = list;
}
