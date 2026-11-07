//! A map's geometry as compiling left it, kept so a load needn't work it out
//! again (proposal 0103). What's cheap, or needs a level's pictures, stays in
//! [`crate::compile`].

use engine_core::math::Vec3;
use engine_format::{Block, Document, Value, from_binary, to_binary};
use engine_map::{
    ClipChild, ClipNode, Contents, Face, Leaf, Map, MeshGroup, Model, Node, NodeChild, Plane,
    TexelAxis, Tree,
};

/// Changes when what's kept changes, so that a file from before it's kept differently reads as
/// out of date, and isn't misread.
const GEOMETRY_FORMAT: u64 = 2;

/// What takes a compile its time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Geometry {
    /// The material of each texture, in order, as faces go by number.
    pub(crate) textures: Vec<String>,
    /// Every face's corners.
    pub(crate) points: Vec<Vec3>,
    /// Which way corners face.
    pub(crate) normals: Vec<[i8; 3]>,
    /// Each placed mesh's faces.
    pub(crate) mesh_groups: Vec<MeshGroup>,
    /// The faces, without their light.
    pub(crate) faces: Vec<Face>,
    /// The models.
    pub(crate) models: Vec<Model>,
    /// The tree.
    pub(crate) tree: Tree,
    /// How far faces' light samples are lifted.
    pub(crate) light_lifts: Vec<(u32, Vec3)>,
}

/// Bytes being written.
#[derive(Default)]
struct Writer(Vec<u8>);

impl Writer {
    /// Adds a number.
    fn number(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }

    /// Adds a float.
    fn float(&mut self, value: f32) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }

    /// Adds a point.
    fn point(&mut self, value: Vec3) {
        self.float(value.x);
        self.float(value.y);
        self.float(value.z);
    }

    /// Adds how many of something follow.
    fn count(&mut self, value: usize) {
        self.number(u32::try_from(value).unwrap_or(u32::MAX));
    }

    /// Adds a child of a node: which kind, and which one.
    fn node_child(&mut self, child: NodeChild) {
        let (kind, number) = match child {
            NodeChild::Node(number) => (0, number),
            NodeChild::Leaf(number) => (1, number),
        };
        self.number(kind);
        self.number(number);
    }

    /// Adds a child of a clipping node.
    fn clip_child(&mut self, child: ClipChild) {
        let (kind, number) = match child {
            ClipChild::Node(number) => (0, number),
            ClipChild::Empty => (1, 0),
            ClipChild::Solid => (2, 0),
        };
        self.number(kind);
        self.number(number);
    }
}

/// Bytes being read. Anything that runs out says so with `None`, and a file that
/// won't read is compiled again.
struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    /// The next four bytes.
    fn word(&mut self) -> Option<[u8; 4]> {
        let (word, rest) = self.0.split_first_chunk::<4>()?;
        self.0 = rest;
        Some(*word)
    }

    /// The next number.
    fn number(&mut self) -> Option<u32> {
        self.word().map(u32::from_le_bytes)
    }

    /// The next float.
    fn float(&mut self) -> Option<f32> {
        self.word().map(f32::from_le_bytes)
    }

    /// The next point.
    fn point(&mut self) -> Option<Vec3> {
        Some(Vec3::new(self.float()?, self.float()?, self.float()?))
    }

    /// The next direction, as a map keeps it.
    fn normal(&mut self) -> Option<[i8; 3]> {
        let (three, rest) = self.0.split_first_chunk::<3>()?;
        self.0 = rest;
        Some(three.map(|part| i8::from_ne_bytes([part])))
    }

    /// How many of something follow, so long as there are that many bytes left
    /// for them.
    fn count(&mut self, each: usize) -> Option<usize> {
        let count = usize::try_from(self.number()?).ok()?;
        (count.checked_mul(each)? <= self.0.len()).then_some(count)
    }

    /// The next child of a node.
    fn node_child(&mut self) -> Option<NodeChild> {
        let (kind, number) = (self.number()?, self.number()?);
        match kind {
            0 => Some(NodeChild::Node(number)),
            1 => Some(NodeChild::Leaf(number)),
            _ => None,
        }
    }

    /// The next child of a clipping node.
    fn clip_child(&mut self) -> Option<ClipChild> {
        let (kind, number) = (self.number()?, self.number()?);
        match kind {
            0 => Some(ClipChild::Node(number)),
            1 => Some(ClipChild::Empty),
            2 => Some(ClipChild::Solid),
            _ => None,
        }
    }
}

/// What a leaf holds, as a number.
const fn contents_number(contents: Contents) -> u32 {
    match contents {
        Contents::Empty => 0,
        Contents::Solid => 1,
        Contents::Water => 2,
        Contents::Slime => 3,
        Contents::Lava => 4,
        Contents::Sky => 5,
    }
}

/// What a leaf holds, from its number.
const fn contents_of(number: u32) -> Option<Contents> {
    match number {
        0 => Some(Contents::Empty),
        1 => Some(Contents::Solid),
        2 => Some(Contents::Water),
        3 => Some(Contents::Slime),
        4 => Some(Contents::Lava),
        5 => Some(Contents::Sky),
        _ => None,
    }
}

/// Adds a tree, leaving out which leaves can see which, as a bake holds that.
fn write_tree(out: &mut Writer, tree: &Tree) {
    out.count(tree.planes.len());
    for plane in &tree.planes {
        out.point(plane.normal);
        out.float(plane.distance);
    }
    out.count(tree.nodes.len());
    for node in &tree.nodes {
        out.number(node.plane);
        out.node_child(node.children[0]);
        out.node_child(node.children[1]);
    }
    out.count(tree.node_faces.len());
    for (first, count) in &tree.node_faces {
        out.number(*first);
        out.number(*count);
    }
    out.count(tree.leaves.len());
    for leaf in &tree.leaves {
        out.number(contents_number(leaf.contents));
        out.number(leaf.first_face);
        out.number(leaf.face_count);
    }
    out.count(tree.leaf_faces.len());
    for face in &tree.leaf_faces {
        out.number(*face);
    }
    out.count(tree.clip_nodes.len());
    for node in &tree.clip_nodes {
        out.number(node.plane);
        out.clip_child(node.children[0]);
        out.clip_child(node.children[1]);
    }
    out.count(tree.leaf_boxes.len());
    for (low, high) in &tree.leaf_boxes {
        out.point(*low);
        out.point(*high);
    }
    out.number(tree.visible_leaf_count);
}

/// Reads what [`write_tree`] wrote.
fn read_tree(input: &mut Reader<'_>) -> Option<Tree> {
    let mut tree = Tree::default();
    for _ in 0..input.count(16)? {
        tree.planes.push(Plane {
            normal: input.point()?,
            distance: input.float()?,
        });
    }
    for _ in 0..input.count(20)? {
        tree.nodes.push(Node {
            plane: input.number()?,
            children: [input.node_child()?, input.node_child()?],
        });
    }
    for _ in 0..input.count(8)? {
        tree.node_faces.push((input.number()?, input.number()?));
    }
    for _ in 0..input.count(12)? {
        tree.leaves.push(Leaf {
            contents: contents_of(input.number()?)?,
            first_face: input.number()?,
            face_count: input.number()?,
            ..Leaf::default()
        });
    }
    for _ in 0..input.count(4)? {
        tree.leaf_faces.push(input.number()?);
    }
    for _ in 0..input.count(20)? {
        tree.clip_nodes.push(ClipNode {
            plane: input.number()?,
            children: [input.clip_child()?, input.clip_child()?],
        });
    }
    for _ in 0..input.count(24)? {
        tree.leaf_boxes.push((input.point()?, input.point()?));
    }
    tree.visible_leaf_count = input.number()?;
    Some(tree)
}

/// Adds a model.
fn write_model(out: &mut Writer, model: &Model) {
    out.number(model.first_face);
    out.number(model.face_count);
    out.point(model.mins);
    out.point(model.maxs);
    out.number(u32::from(model.root.is_some()));
    out.node_child(model.root.unwrap_or(NodeChild::Leaf(0)));
    out.clip_child(model.hulls[0]);
    out.clip_child(model.hulls[1]);
}

/// Reads what [`write_model`] wrote.
fn read_model(input: &mut Reader<'_>) -> Option<Model> {
    let first_face = input.number()?;
    let face_count = input.number()?;
    let (mins, maxs) = (input.point()?, input.point()?);
    let has_root = input.number()? != 0;
    let root = input.node_child()?;
    Some(Model {
        first_face,
        face_count,
        mins,
        maxs,
        root: has_root.then_some(root),
        hulls: [input.clip_child()?, input.clip_child()?],
    })
}

/// Adds a face's place in the world, leaving out its light.
fn write_face(out: &mut Writer, face: &Face) {
    out.number(face.first_point);
    out.number(face.point_count);
    out.number(face.texture);
    for axis in &face.axes {
        out.point(axis.axis);
        out.float(axis.offset);
    }
}

/// Reads what [`write_face`] wrote.
fn read_face(input: &mut Reader<'_>) -> Option<Face> {
    let (first_point, point_count) = (input.number()?, input.number()?);
    let texture = input.number()?;
    let mut axis = || {
        Some(TexelAxis {
            axis: input.point()?,
            offset: input.float()?,
        })
    };
    Some(Face {
        first_point,
        point_count,
        texture,
        axes: [axis()?, axis()?],
        light: None,
        light_axes: None,
    })
}

/// `map`'s geometry as a file, which only reads back for the same `hash`.
#[must_use]
pub fn write_geometry(map: &Map, hash: u64) -> Vec<u8> {
    let mut out = Writer::default();
    out.count(map.points.len());
    for point in &map.points {
        out.point(*point);
    }
    out.count(map.faces.len());
    for face in &map.faces {
        write_face(&mut out, face);
    }
    out.count(map.models.len());
    for model in &map.models {
        write_model(&mut out, model);
    }
    out.count(map.light_lifts.len());
    for (face, lift) in &map.light_lifts {
        out.number(*face);
        out.point(*lift);
    }
    write_tree(&mut out, &map.tree);
    out.count(map.normals.len());
    for normal in &map.normals {
        out.0
            .extend(normal.map(i8::to_ne_bytes).into_iter().flatten());
    }
    out.count(map.mesh_groups.len());
    for group in &map.mesh_groups {
        out.number(group.first_face);
        out.number(group.face_count);
        out.number(u32::from(group.by_probe));
        out.number(group.placement.map_or(0, |placement| placement + 1));
    }
    let names: Vec<&str> = map.textures.iter().map(|t| t.name.as_str()).collect();
    let mut document = Document::new("geometry");
    let mut block = Block::new("source");
    block.set(
        "hash",
        Value::Text(format!("{:016x}", hash ^ GEOMETRY_FORMAT)),
    );
    block.set("textures", Value::Text(names.join("\n")));
    block.set("data", Value::Bytes(out.0));
    document.blocks.push(block);
    to_binary(&document)
}

/// The geometry in `bytes`, if they're for `hash` and they read.
#[must_use]
pub fn read_geometry(bytes: &[u8], hash: u64) -> Option<Geometry> {
    let document = from_binary(bytes).ok()?;
    if document.kind != "geometry" {
        return None;
    }
    let block = document.blocks_of("source").next()?;
    if block.get("hash")?.text()? != format!("{:016x}", hash ^ GEOMETRY_FORMAT) {
        return None;
    }
    let names = block.get("textures")?.text()?;
    let mut input = Reader(block.get("data")?.bytes()?);
    let mut geometry = Geometry {
        textures: names
            .split('\n')
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect(),
        ..Geometry::default()
    };
    for _ in 0..input.count(12)? {
        geometry.points.push(input.point()?);
    }
    for _ in 0..input.count(44)? {
        geometry.faces.push(read_face(&mut input)?);
    }
    for _ in 0..input.count(60)? {
        geometry.models.push(read_model(&mut input)?);
    }
    for _ in 0..input.count(16)? {
        geometry.light_lifts.push((input.number()?, input.point()?));
    }
    geometry.tree = read_tree(&mut input)?;
    for _ in 0..input.count(3)? {
        geometry.normals.push(input.normal()?);
    }
    for _ in 0..input.count(12)? {
        geometry.mesh_groups.push(MeshGroup {
            first_face: input.number()?,
            face_count: input.number()?,
            by_probe: input.number()? != 0,
            placement: input.number()?.checked_sub(1),
        });
    }
    Some(geometry)
}
