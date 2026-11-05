//! Putting a map together.

use std::collections::HashMap;
use std::sync::Arc;

use engine_core::math::Vec3;
use engine_format::{Value, from_text};
use engine_map::{
    ClipChild, ClipNode, Contents, Entity, Face, Leaf, Map, Model, Node, NodeChild, Plane,
    TexelAxis, Texture, TextureKind, Tree,
};

use crate::error::CompileError;
use crate::geometry::{
    Plane3, Point, Polygon, Split, clip_behind, plane_polygon, solid_corners, split,
};
use crate::source::{Brush, BrushFace, WorkingMap, read_map};
use crate::stencil::build_stencil;
use crate::tree::{BuildNode, Child, PlaneSet, TreeBuilder};

/// Supplies textures for materials.
pub trait TextureSource {
    /// The texture of `material`.
    fn texture(&mut self, material: &str) -> Texture;
}

impl<F: FnMut(&str) -> Texture> TextureSource for F {
    fn texture(&mut self, material: &str) -> Texture {
        self(material)
    }
}

/// A player's box in the first clip tree, as `Model::hull_for` expects.
const PLAYER_BOX: (Point, Point) = ([-16.0, -16.0, -24.0], [16.0, 16.0, 32.0]);

/// A large monster's box in the second.
const LARGE_BOX: (Point, Point) = ([-32.0, -32.0, -24.0], [32.0, 32.0, 64.0]);

/// Leaf numbers, by contents. Leaf 0 is solid, as `Tree` expects.
const LEAF_CONTENTS: [Contents; 6] = [
    Contents::Solid,
    Contents::Empty,
    Contents::Water,
    Contents::Slime,
    Contents::Lava,
    Contents::Sky,
];

/// The leaf all faces hang from.
const FACE_LEAF: usize = 1;

/// Converts to the engine's narrower type.
#[expect(
    clippy::cast_possible_truncation,
    reason = "map coordinates are stored as f32"
)]
fn vec3(point: Point) -> Vec3 {
    Vec3::new(point[0] as f32, point[1] as f32, point[2] as f32)
}

/// A brush's planes grown so that a box's origin stops where the box would
/// touch it, with axial bevels so sharp corners don't reach too far.
fn grown(brush: &Brush, size: (Point, Point)) -> Vec<Plane3> {
    let support = |plane: &Plane3| {
        let (low, high) = size;
        (0..3)
            .map(|axis| {
                let part = plane.normal[axis];
                (part * low[axis]).min(part * high[axis])
            })
            .sum::<f64>()
    };
    let mut planes: Vec<Plane3> = brush.planes();
    let corners = solid_corners(&planes);
    for axis in 0..3 {
        let mut normal = [0.0; 3];
        normal[axis] = 1.0;
        let high = corners
            .iter()
            .map(|corner| corner[axis])
            .fold(f64::NEG_INFINITY, f64::max);
        let low = corners
            .iter()
            .map(|corner| corner[axis])
            .fold(f64::INFINITY, f64::min);
        let plane = Plane3 {
            normal,
            distance: high,
        };
        let opposite = Plane3 {
            normal,
            distance: low,
        }
        .flipped();
        for bevel in [plane, opposite] {
            let known = planes.iter().any(|known| {
                (0..3).all(|part| (known.normal[part] - bevel.normal[part]).abs() < 1e-9)
            });
            if !known {
                planes.push(bevel);
            }
        }
    }
    planes
        .iter()
        .map(|plane| Plane3 {
            normal: plane.normal,
            distance: plane.distance - support(plane),
        })
        .collect()
}

/// The texture axes of a face, facing along `normal`.
fn base_axes(normal: Point) -> (Point, Point) {
    let (x, y, z) = (normal[0].abs(), normal[1].abs(), normal[2].abs());
    if z >= x && z >= y {
        ([1.0, 0.0, 0.0], [0.0, -1.0, 0.0])
    } else if x >= y {
        ([0.0, 1.0, 0.0], [0.0, 0.0, -1.0])
    } else {
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0])
    }
}

/// The engine's texture axes for `face`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "texture axes are stored as f32"
)]
fn texel_axes(face: &BrushFace) -> [TexelAxis; 2] {
    let (u, v) = base_axes(face.plane.normal);
    let (sin, cos) = face.rotation.to_radians().sin_cos();
    let turned = |a: Point, b: Point, a_share: f64, b_share: f64| {
        [
            a[0] * a_share + b[0] * b_share,
            a[1] * a_share + b[1] * b_share,
            a[2] * a_share + b[2] * b_share,
        ]
    };
    let across = turned(u, v, cos, sin);
    let down = turned(u, v, -sin, cos);
    let axis = |direction: Point, scale: f64, offset: f64| TexelAxis {
        axis: vec3([
            direction[0] * scale,
            direction[1] * scale,
            direction[2] * scale,
        ]),
        offset: offset as f32,
    };
    [
        axis(across, face.scale[0], face.offset[0]),
        axis(down, face.scale[1], face.offset[1]),
    ]
}

/// What's left of `polygon`, on `brush`, after removing what `other` hides.
/// `after` says `other` comes after the polygon's own brush, so that of two
/// coplanar faces the first stays.
fn outside(polygon: Polygon, face_plane: &Plane3, other: &Brush, after: bool) -> Vec<Polygon> {
    let mut kept = Vec::new();
    let mut rest = polygon;
    for plane in other.planes() {
        match split(&rest, &plane) {
            Split::Front => {
                kept.push(rest);
                return kept;
            }
            Split::Back => {}
            Split::On => {
                let same = plane.normal.iter().zip(face_plane.normal).all(|(a, b)| {
                    let (a, b): (f64, f64) = (*a, b);
                    (a - b).abs() < 1e-9
                });
                if same && after {
                    kept.push(rest);
                    return kept;
                }
            }
            Split::Both(front, back) => {
                kept.push(front);
                rest = back;
            }
        }
    }
    kept
}

/// The visible polygons of `brushes`, each with the face it came from.
fn visible_faces(brushes: &[Brush]) -> Vec<(Polygon, &BrushFace, Contents)> {
    let mut out = Vec::new();
    for (index, brush) in brushes.iter().enumerate() {
        let planes = brush.planes();
        for (at, face) in brush.faces.iter().enumerate() {
            let Some(polygon) = clip_behind(plane_polygon(&face.plane), &planes, Some(at)) else {
                continue;
            };
            let mut pieces = vec![polygon];
            for (other_index, other) in brushes.iter().enumerate() {
                if other_index == index || other.contents != Contents::Solid {
                    continue;
                }
                let after = other_index > index;
                pieces = pieces
                    .into_iter()
                    .flat_map(|piece| outside(piece, &face.plane, other, after))
                    .collect();
            }
            for piece in pieces.into_iter().filter(|piece| piece.len() >= 3) {
                out.push((piece, face, brush.contents));
            }
        }
    }
    out
}

/// Inserts `brushes` into a tree, each grown by `size` if it has one.
pub(crate) fn build_tree(
    brushes: &[Brush],
    size: Option<(Point, Point)>,
    set: &mut PlaneSet,
) -> (TreeBuilder, Child) {
    let mut tree = TreeBuilder::default();
    let mut root = Child::Leaf(Contents::Empty);
    for brush in brushes {
        if size.is_some() && brush.contents != Contents::Solid {
            continue;
        }
        let planes = size.map_or_else(|| brush.planes(), |size| grown(brush, size));
        let corners = solid_corners(&planes);
        let indices: Vec<u32> = planes.iter().map(|plane| set.index_of(*plane)).collect();
        root = tree.insert(root, set, &indices, &corners, brush.contents);
    }
    (tree, root)
}

/// Adds a built tree to the point nodes, and returns its root.
fn add_point_tree(tree: &mut Tree, built: &TreeBuilder, root: Child) -> Option<NodeChild> {
    let offset = u32::try_from(tree.nodes.len()).ok()?;
    let child = |child: Child| match child {
        Child::Node(index) => NodeChild::Node(index + offset),
        Child::Leaf(contents) => {
            let leaf = LEAF_CONTENTS.iter().position(|known| *known == contents);
            NodeChild::Leaf(u32::try_from(leaf.unwrap_or(0)).unwrap_or(0))
        }
    };
    for BuildNode { plane, front, back } in &built.nodes {
        tree.nodes.push(Node {
            plane: *plane,
            children: [child(*front), child(*back)],
        });
        tree.node_faces.push((0, 0));
    }
    Some(child(root))
}

/// Adds a built tree to the clip nodes, and returns its root.
fn add_clip_tree(tree: &mut Tree, built: &TreeBuilder, root: Child) -> Option<ClipChild> {
    let offset = u32::try_from(tree.clip_nodes.len()).ok()?;
    let child = |child: Child| match child {
        Child::Node(index) => ClipChild::Node(index + offset),
        Child::Leaf(Contents::Solid) => ClipChild::Solid,
        Child::Leaf(_) => ClipChild::Empty,
    };
    for BuildNode { plane, front, back } in &built.nodes {
        tree.clip_nodes.push(ClipNode {
            plane: *plane,
            children: [child(*front), child(*back)],
        });
    }
    Some(child(root))
}

/// Builds everything from a working map.
struct Builder<'a> {
    /// Supplies textures.
    source: &'a mut dyn TextureSource,
    /// The map so far.
    map: Map,
    /// Texture numbers, by material.
    materials: HashMap<String, u32>,
    /// The planes of every tree.
    planes: PlaneSet,
}

impl Builder<'_> {
    /// The number of `material`'s texture.
    fn texture_of(&mut self, material: &str) -> u32 {
        if let Some(number) = self.materials.get(material) {
            return *number;
        }
        let texture = self.source.texture(material);
        self.map.textures.push(texture);
        let number = u32::try_from(self.map.textures.len() - 1).unwrap_or(0);
        self.materials.insert(material.to_owned(), number);
        number
    }

    /// Adds one polygon as a face, and returns its bounds.
    fn add_face(&mut self, polygon: &[Point], face: &BrushFace) -> Option<()> {
        let first_point = u32::try_from(self.map.points.len()).ok()?;
        self.map.points.extend(polygon.iter().map(|p| vec3(*p)));
        let texture = self.texture_of(&face.material);
        self.map.faces.push(Face {
            first_point,
            point_count: u32::try_from(polygon.len()).ok()?,
            texture,
            axes: texel_axes(face),
            light: None,
        });
        Some(())
    }

    /// Adds a model for `brushes`, and returns its number.
    fn add_model(&mut self, brushes: &[Brush]) -> Result<u32, CompileError> {
        let too_big = || CompileError::Bad("the map is too big".to_owned());
        let first_face = u32::try_from(self.map.faces.len()).map_err(|_| too_big())?;
        let (mut low, mut high) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for (polygon, face, contents) in visible_faces(brushes) {
            for point in &polygon {
                for axis in 0..3 {
                    low[axis] = low[axis].min(point[axis]);
                    high[axis] = high[axis].max(point[axis]);
                }
            }
            self.add_face(&polygon, face).ok_or_else(too_big)?;
            if contents != Contents::Solid {
                // Liquids are seen from both sides.
                let flipped: Vec<Point> = polygon.iter().rev().copied().collect();
                self.add_face(&flipped, face).ok_or_else(too_big)?;
            }
        }
        if low[0] > high[0] {
            (low, high) = ([0.0; 3], [0.0; 3]);
        }
        let face_count = u32::try_from(self.map.faces.len())
            .map_err(|_| too_big())?
            .saturating_sub(first_face);
        let (point, root) = build_tree(brushes, None, &mut self.planes);
        let (player, player_root) = build_tree(brushes, Some(PLAYER_BOX), &mut self.planes);
        let (large, large_root) = build_tree(brushes, Some(LARGE_BOX), &mut self.planes);
        let tree = &mut self.map.tree;
        let root = add_point_tree(tree, &point, root).ok_or_else(too_big)?;
        let player = add_clip_tree(tree, &player, player_root).ok_or_else(too_big)?;
        let large = add_clip_tree(tree, &large, large_root).ok_or_else(too_big)?;
        self.map.models.push(Model {
            first_face,
            face_count,
            mins: vec3(low),
            maxs: vec3(high),
            root: Some(root),
            hulls: [player, large],
        });
        u32::try_from(self.map.models.len() - 1).map_err(|_| too_big())
    }

    /// Finishes the tree's shared parts.
    fn finish(&mut self) {
        let count = u32::try_from(self.map.faces.len()).unwrap_or(0);
        let tree = &mut self.map.tree;
        tree.planes = self
            .planes
            .planes
            .iter()
            .map(|plane| {
                #[expect(clippy::cast_possible_truncation, reason = "stored as f32")]
                let distance = plane.distance as f32;
                Plane {
                    normal: vec3(plane.normal),
                    distance,
                }
            })
            .collect();
        tree.leaves = LEAF_CONTENTS
            .iter()
            .map(|contents| Leaf {
                contents: *contents,
                ..Leaf::default()
            })
            .collect();
        if let Some(leaf) = tree.leaves.get_mut(FACE_LEAF) {
            leaf.face_count = count;
        }
        tree.leaf_faces = (0..count).collect();
    }
}

/// A field's value as the text scripts read.
fn field_text(value: &Value) -> String {
    match value {
        Value::Number(number) => number.to_string(),
        Value::Text(text) => text.clone(),
        Value::Flag(flag) => u8::from(*flag).to_string(),
        Value::Bytes(_) => String::new(),
        Value::List(values) => values.iter().map(field_text).collect::<Vec<_>>().join(" "),
    }
}

/// Compiles a working map.
///
/// # Errors
///
/// Returns an error if it can't be built.
pub fn compile(working: &WorkingMap, source: &mut dyn TextureSource) -> Result<Map, CompileError> {
    let mut builder = Builder {
        source,
        map: Map::default(),
        materials: HashMap::new(),
        planes: PlaneSet::default(),
    };
    builder.add_model(&working.brushes)?;
    let mut entities = Vec::with_capacity(working.entities.len() + 1);
    let is_world = |kind: &str| kind == "worldspawn";
    if !working.entities.iter().any(|entity| is_world(&entity.kind)) {
        entities.push(Entity {
            fields: vec![("classname".to_owned(), "worldspawn".to_owned())],
        });
    }
    for entity in &working.entities {
        let mut fields = vec![("classname".to_owned(), entity.kind.clone())];
        if let Some(name) = &entity.name {
            fields.push(("targetname".to_owned(), name.clone()));
        }
        fields.extend(
            entity
                .fields
                .iter()
                .map(|(key, value)| (key.clone(), field_text(value))),
        );
        if !entity.brushes.is_empty() {
            let number = builder.add_model(&entity.brushes)?;
            fields.push(("model".to_owned(), format!("*{number}")));
        }
        let shown = Entity { fields };
        if is_world(&entity.kind) {
            entities.insert(0, shown);
        } else {
            entities.push(shown);
        }
    }
    builder.map.entities = entities;
    for entity in working.entities.iter().filter(|e| e.kind == "ray_stencil") {
        let stencil = build_stencil(entity, builder.source)?;
        builder.map.stencils.push(stencil);
    }
    builder.map.fog = working.fog;
    if let Some(sky) = &working.sky {
        let mut built = sky.sky.clone();
        if let (Some(clouds), Some(image)) = (&mut built.clouds, &sky.cloud_image) {
            let texture = builder.source.texture(image);
            clouds.image = Some(Arc::new(texture.mips));
        }
        builder.map.sky = Some(built);
    }
    builder.finish();
    let mut map = builder.map;
    for texture in &mut map.textures {
        if texture.mips.is_empty() {
            texture.kind = TextureKind::Hidden;
        }
    }
    Ok(map)
}

/// Compiles a map from its text.
///
/// # Errors
///
/// Returns an error if the text can't be read or the map can't be built.
pub fn compile_text(text: &str, source: &mut dyn TextureSource) -> Result<Map, CompileError> {
    let document = from_text(text)?;
    compile(&read_map(&document)?, source)
}
