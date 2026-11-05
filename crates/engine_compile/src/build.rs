//! Putting a map together.

use std::collections::HashMap;
use std::sync::Arc;

use engine_core::math::Vec3;
use engine_format::{Value, from_text};
use engine_map::{
    ClipChild, ClipNode, Contents, Entity, Face, Leaf, Map, Model, ModelOccluder, Node, NodeChild,
    OccluderSphere, Plane, Pool, PoolLook, TexelAxis, Texture, TextureKind, Tree,
};

use crate::error::CompileError;
use crate::geometry::{
    Plane3, Point, Polygon, Split, add_scaled, clip_behind, cross, dot, normalized, plane_polygon,
    solid_corners, split,
};
use crate::mesh::{PlacedMesh, PlacedTriangle};
use crate::source::{Brush, BrushFace, DecalSource, WorkingMap, read_map};
use crate::stencil::{build_flames, build_stencil};
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

/// The texture axes that fit a face to the best of its material's hotspots: the
/// area nearest its shape, so trim and panels fall right with no aligning.
fn hotspot_axes(polygon: &[Point], face: &BrushFace, texture: &Texture) -> Option<[TexelAxis; 2]> {
    let hotspots = &texture.material.as_ref()?.hotspots;
    let mip = texture.mips.first()?;
    let (u, v) = base_axes(face.plane.normal);
    let extent = |axis: Point| {
        polygon
            .iter()
            .fold((f64::MAX, f64::MIN), |(low, high), point| {
                let value = dot(*point, axis);
                (low.min(value), high.max(value))
            })
    };
    let (low_u, high_u) = extent(u);
    let (low_v, high_v) = extent(v);
    let (width, height) = (high_u - low_u, high_v - low_v);
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let misfit = |area: &[f32; 4]| {
        let shape = f64::from(area[2] - area[0]) / f64::from((area[3] - area[1]).max(0.001));
        ((width / height) / shape.max(0.001)).ln().abs()
    };
    let best = hotspots
        .iter()
        .min_by(|a, b| misfit(a).total_cmp(&misfit(b)))?;
    let fit = |axis: Point, low: f64, size: f64, start: f32, end: f32, texels: u32| {
        let from = f64::from(start) * f64::from(texels);
        let span = f64::from(end - start) * f64::from(texels);
        let scale = span / size;
        #[expect(clippy::cast_possible_truncation, reason = "faces store f32")]
        TexelAxis {
            axis: vec3(axis.map(|part| part * scale)),
            offset: (from - low * scale) as f32,
        }
    };
    Some([
        fit(u, low_u, width, best[0], best[2], mip.width),
        fit(v, low_v, height, best[1], best[3], mip.height),
    ])
}

/// A piece of a brush's face that can be seen.
struct VisibleFace<'a> {
    /// The piece.
    polygon: Polygon,
    /// The whole of the face it's from, which brushes in front of it have cut.
    whole: Arc<Polygon>,
    /// The face.
    face: &'a BrushFace,
    /// What's behind it.
    contents: Contents,
}

/// How far a decal sits off its surface.
const DECAL_LIFT: f64 = 0.2;

/// The triangles of a decal, clipped to the world's faces under it.
fn decal_triangles(decal: &DecalSource, faces: &[VisibleFace<'_>]) -> Vec<PlacedTriangle> {
    let mut out = Vec::new();
    let Some(normal) = normalized(decal.normal) else {
        return out;
    };
    let reference = if normal[2].abs() > 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    let Some(side) = normalized(cross(reference, normal)) else {
        return out;
    };
    let up = cross(normal, side);
    let (sin, cos) = decal.angle.to_radians().sin_cos();
    let across = add_scaled(side.map(|part| part * cos), up, sin);
    let upward = add_scaled(up.map(|part| part * cos), side, -sin);
    let [width, height] = decal.size;
    let wall = |direction: Point, reach: f64| Plane3 {
        normal: direction,
        distance: dot(direction, decal.origin) + reach,
    };
    let flipped = |direction: Point| direction.map(|part| -part);
    let bounds = [
        wall(across, width * 0.5),
        wall(flipped(across), width * 0.5),
        wall(upward, height * 0.5),
        wall(flipped(upward), height * 0.5),
        wall(normal, decal.depth),
        wall(flipped(normal), decal.depth),
    ];
    for visible in faces {
        let face = visible.face;
        if visible.contents != Contents::Solid || dot(face.plane.normal, normal) < 0.25 {
            continue;
        }
        let Some(kept) = clip_behind(visible.polygon.clone(), &bounds, None) else {
            continue;
        };
        let lifted: Vec<Point> = kept
            .iter()
            .map(|point| add_scaled(*point, face.plane.normal, DECAL_LIFT))
            .collect();
        let uv = |point: Point| {
            let offset = add_scaled(point, decal.origin, -1.0);
            [
                dot(offset, across) / width + 0.5,
                0.5 - dot(offset, upward) / height,
            ]
        };
        for pair in lifted[1..].windows(2) {
            let points = [lifted[0], pair[0], pair[1]];
            out.push(PlacedTriangle {
                points,
                uvs: points.map(uv),
                material: decal.material.clone(),
            });
        }
    }
    out
}

/// The texture axes that put `across` and `down` texels at a triangle's
/// corners, or `None` for a triangle with no area.
fn uv_axes(points: &[Point; 3], across: [f64; 3], down: [f64; 3]) -> Option<[TexelAxis; 2]> {
    let first = add_scaled(points[1], points[0], -1.0);
    let second = add_scaled(points[2], points[0], -1.0);
    let (aa, ab, bb) = (dot(first, first), dot(first, second), dot(second, second));
    let determinant = aa * bb - ab * ab;
    if determinant.abs() < 1e-9 {
        return None;
    }
    let axis = |values: [f64; 3]| {
        let (du, dv) = (values[1] - values[0], values[2] - values[0]);
        let share_first = (du * bb - dv * ab) / determinant;
        let share_second = (dv * aa - du * ab) / determinant;
        let direction = add_scaled(
            [
                first[0] * share_first,
                first[1] * share_first,
                first[2] * share_first,
            ],
            second,
            share_second,
        );
        #[expect(clippy::cast_possible_truncation, reason = "faces store f32")]
        TexelAxis {
            axis: vec3(direction),
            offset: (values[0] - dot(direction, points[0])) as f32,
        }
    };
    Some([axis(across), axis(down)])
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
fn visible_faces(brushes: &[Brush]) -> Vec<VisibleFace<'_>> {
    let mut out = Vec::new();
    for (index, brush) in brushes.iter().enumerate() {
        let planes = brush.planes();
        for (at, face) in brush.faces.iter().enumerate() {
            let Some(polygon) = clip_behind(plane_polygon(&face.plane), &planes, Some(at)) else {
                continue;
            };
            let whole = Arc::new(polygon.clone());
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
                out.push(VisibleFace {
                    polygon: piece,
                    whole: Arc::clone(&whole),
                    face,
                    contents: brush.contents,
                });
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
    fn add_face(&mut self, polygon: &[Point], whole: &[Point], face: &BrushFace) -> Option<()> {
        let first_point = u32::try_from(self.map.points.len()).ok()?;
        self.map.points.extend(polygon.iter().map(|p| vec3(*p)));
        let texture = self.texture_of(&face.material);
        let found = self.map.textures.get(texture as usize)?;
        let axes = hotspot_axes(whole, face, found).unwrap_or_else(|| texel_axes(face));
        self.map.faces.push(Face {
            first_point,
            point_count: u32::try_from(polygon.len()).ok()?,
            texture,
            axes,
            light: None,
        });
        Some(())
    }

    /// Adds a triangle of a mesh.
    fn add_triangle(&mut self, triangle: &PlacedTriangle) -> Option<()> {
        let texture = self.texture_of(&triangle.material);
        let image = self.map.textures.get(texture as usize)?.mips.first();
        let size = image.map_or((64.0, 64.0), |mip| {
            (f64::from(mip.width), f64::from(mip.height))
        });
        let across = triangle.uvs.map(|uv| uv[0] * size.0);
        let down = triangle.uvs.map(|uv| uv[1] * size.1);
        let Some(axes) = uv_axes(&triangle.points, across, down) else {
            return Some(());
        };
        let first_point = u32::try_from(self.map.points.len()).ok()?;
        self.map
            .points
            .extend(triangle.points.iter().map(|p| vec3(*p)));
        self.map.faces.push(Face {
            first_point,
            point_count: 3,
            texture,
            axes,
            light: None,
        });
        Some(())
    }

    /// Adds a model for `brushes` and `meshes`, and returns its number.
    fn add_model(
        &mut self,
        brushes: &[Brush],
        meshes: &[&PlacedMesh],
    ) -> Result<u32, CompileError> {
        let too_big = || CompileError::Bad("the map is too big".to_owned());
        let first_face = u32::try_from(self.map.faces.len()).map_err(|_| too_big())?;
        let (mut low, mut high) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for visible in visible_faces(brushes) {
            let VisibleFace {
                polygon,
                whole,
                face,
                contents,
            } = &visible;
            for point in polygon {
                for axis in 0..3 {
                    low[axis] = low[axis].min(point[axis]);
                    high[axis] = high[axis].max(point[axis]);
                }
            }
            self.add_face(polygon, whole, face).ok_or_else(too_big)?;
            if *contents != Contents::Solid {
                // Liquids are seen from both sides.
                let flipped: Vec<Point> = polygon.iter().rev().copied().collect();
                self.add_face(&flipped, whole, face).ok_or_else(too_big)?;
            }
        }
        for triangle in meshes.iter().flat_map(|mesh| &mesh.triangles) {
            for point in &triangle.points {
                for axis in 0..3 {
                    low[axis] = low[axis].min(point[axis]);
                    high[axis] = high[axis].max(point[axis]);
                }
            }
            self.add_triangle(triangle).ok_or_else(too_big)?;
        }
        if low[0] > high[0] {
            (low, high) = ([0.0; 3], [0.0; 3]);
        }
        // Hulls are in the trees, but they cut no faces.
        let mut solids = brushes.to_vec();
        solids.extend(meshes.iter().flat_map(|mesh| mesh.hulls.iter().cloned()));
        let brushes = solids.as_slice();
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

/// The bodies of liquid among a map's brushes.
#[expect(clippy::cast_possible_truncation, reason = "pools store f32")]
fn pools_of(working: &WorkingMap) -> Vec<Pool> {
    let mut pools = Vec::new();
    for brush in &working.brushes {
        let Some(mut look) = PoolLook::of(brush.contents) else {
            continue;
        };
        if let Some(water) = &working.water {
            look.color = water.color.unwrap_or(look.color);
            look.absorption = water.absorption.unwrap_or(look.absorption);
            look.caustics = water.caustics.unwrap_or(look.caustics);
        }
        let corners = solid_corners(&brush.planes());
        let Some(first) = corners.first() else {
            continue;
        };
        let (mut low, mut high) = (*first, *first);
        for corner in &corners {
            for axis in 0..3 {
                low[axis] = low[axis].min(corner[axis]);
                high[axis] = high[axis].max(corner[axis]);
            }
        }
        let point = |at: Point| Vec3::new(at[0] as f32, at[1] as f32, at[2] as f32);
        pools.push(Pool {
            low: point(low),
            high: point(high),
            look,
        });
    }
    pools
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
    let flames = working.entities.iter().filter(|e| e.kind == "flame");
    let (flame_faces, emitters) = build_flames(flames);
    let surfaces = visible_faces(&working.brushes);
    let decals = PlacedMesh {
        triangles: working
            .decals
            .iter()
            .flat_map(|decal| decal_triangles(decal, &surfaces))
            .collect(),
        ..PlacedMesh::default()
    };
    let mut world_meshes: Vec<&PlacedMesh> = working.placed.iter().collect();
    world_meshes.push(&flame_faces);
    world_meshes.push(&decals);
    builder.add_model(&working.brushes, &world_meshes)?;
    builder.map.emitters = emitters;
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
        let meshes: Vec<&PlacedMesh> = entity.placed.iter().collect();
        if !entity.brushes.is_empty() || !meshes.is_empty() {
            let number = builder.add_model(&entity.brushes, &meshes)?;
            fields.push(("model".to_owned(), format!("*{number}")));
            let spheres: Vec<OccluderSphere> = meshes
                .iter()
                .flat_map(|mesh| mesh.occluders.clone())
                .collect();
            if !spheres.is_empty() {
                builder.map.occluders.push(ModelOccluder {
                    model: number,
                    spheres,
                });
            }
        }
        let shown = Entity { fields };
        if is_world(&entity.kind) {
            entities.insert(0, shown);
        } else {
            entities.push(shown);
        }
    }
    builder.map.entities = entities;
    builder.map.pools = pools_of(working);
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

/// Reads a map from its text.
///
/// # Errors
///
/// Returns an error if the text can't be read.
pub fn read_map_text(text: &str) -> Result<WorkingMap, CompileError> {
    read_map(&from_text(text)?)
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
