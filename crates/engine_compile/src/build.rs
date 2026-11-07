//! Putting a map together.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use engine_core::math::Vec3;
use engine_format::{Value, from_text};
use engine_map::{
    AttachPoint, ClipChild, ClipNode, Contents, Entity, Face, LandPicture, Leaf, Look, Map,
    MeshGroup, MeshInstance, Model, ModelOccluder, MovingMesh, Node, NodeChild, OccluderSphere,
    Plane, Pool, PoolLook, TerrainSheet, TexelAxis, Texture, TextureKind, Tree, Zone, ZonePlane,
};

use crate::cells::{DEFAULT_CELL, cut_cells};
use crate::compiled::Geometry;
use crate::effects::{build_flames, build_live_light, build_stencil};
use crate::error::CompileError;
use crate::field::FieldSource;
use crate::geometry::{
    Plane3, Point, Polygon, Split, add_scaled, clip_behind, cross, dot, normalized, plane_polygon,
    solid_corners, split,
};
use crate::land_pictures::plain_pictures;
use crate::light::light_tracks;
use crate::mesh::{
    PlacedMesh, PlacedTriangle, WHITE_CORNERS, at_rest, coarsest_corners_first, placement_rows,
    shared_mesh, with_hit_capsules,
};
use crate::source::{
    Brush, BrushFace, DecalSource, EntitySource, StampSource, WorkingMap, read_map,
};
use crate::stamp::stamp_piece;
use crate::terrain::{chunk_face, chunk_points};
use crate::tree::{BuildNode, Child, Item, PlaneSet, TreeBuilder};

/// A direction, as a map keeps it.
#[expect(clippy::cast_possible_truncation, reason = "held to a byte")]
pub(crate) fn packed_normal(normal: Point) -> [i8; 3] {
    normal.map(|part| (part * 127.0).round().clamp(-127.0, 127.0) as i8)
}

/// The zone an entity describes. Its brushes are the shape of it, and aren't
/// there to be seen or touched.
#[expect(clippy::cast_possible_truncation, reason = "maps store f32")]
fn read_zone(entity: &EntitySource) -> Zone {
    let shapes: Vec<Vec<ZonePlane>> = entity
        .brushes
        .iter()
        .map(|brush| {
            brush
                .planes()
                .iter()
                .map(|plane| ZonePlane {
                    normal: vec3(plane.normal),
                    distance: plane.distance as f32,
                })
                .collect()
        })
        .collect();
    let mut size = 0.0;
    for brush in &entity.brushes {
        let corners = solid_corners(&brush.planes());
        let (mut low, mut high) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for corner in &corners {
            for axis in 0..3 {
                low[axis] = low[axis].min(corner[axis]);
                high[axis] = high[axis].max(corner[axis]);
            }
        }
        if !corners.is_empty() {
            size += (high[0] - low[0]) * (high[1] - low[1]) * (high[2] - low[2]);
        }
    }
    let text = |key: &str| entity.text(key).map(str::to_owned);
    let number_text = entity.field("music").and_then(Value::number);
    let music = text("music").or_else(|| number_text.map(|track| format!("{track}")));
    let light = entity
        .point("light")
        .map(|[r, g, b]| [r as f32, g as f32, b as f32]);
    let brightness = entity.number("light").map(|all| [all as f32; 3]);
    Zone {
        name: entity.name.clone().unwrap_or_default(),
        title: text("title"),
        parent: text("parent"),
        priority: entity.number("priority").map_or(0, |number| number as i32),
        shapes,
        size: size as f32,
        music,
        sound: text("sound"),
        light: light.or(brightness),
    }
}

/// The most cells a map with land is cut into, unless it says otherwise.
const MOST_LAND_CELLS: f64 = 4096.0;

/// Supplies textures for materials.
pub trait TextureSource {
    /// The texture of `material`.
    fn texture(&mut self, material: &str) -> Texture;

    /// The texture of `material`, in plain pixels, for what's read here and not
    /// by the GPU. A game's textures can be compiled (proposal 0106).
    fn pixels(&mut self, material: &str) -> Texture {
        self.texture(material)
    }

    /// What `materials` are to land.
    fn land_pictures(&mut self, materials: &[String]) -> Vec<LandPicture> {
        let textures: Vec<Texture> = materials.iter().map(|name| self.pixels(name)).collect();
        plain_pictures(materials, &textures)
    }
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
    let tile = texture.tile_size()?;
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
    let fit = |axis: Point, low: f64, size: f64, start: f32, end: f32, texels: f32| {
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
        fit(u, low_u, width, best[0], best[2], tile[0]),
        fit(v, low_v, height, best[1], best[3], tile[1]),
    ])
}

/// A piece of a brush's face that can be seen.
struct VisibleFace<'a> {
    /// The piece.
    polygon: Polygon,
    /// The whole of the face it's from, which brushes in front of it have cut.
    whole: Arc<Polygon>,
    /// The face.
    face: Cow<'a, BrushFace>,
    /// What's behind it.
    contents: Contents,
    /// How far its light samples are lifted, if a stamp made it.
    lift: Option<Point>,
}

/// How far a decal sits off its surface.
const DECAL_LIFT: f64 = 0.2;

/// Every name something in `working` aims at. A name on its own is only a label: a door
/// or lift with a `targetname` waits to be fired, and with nothing to fire it, it would
/// wait for ever.
fn targeted_names(working: &WorkingMap) -> HashSet<&str> {
    working
        .entities
        .iter()
        .flat_map(|entity| &entity.fields)
        .filter(|(key, _)| key == "target" || key == "killtarget")
        .filter_map(|(_, value)| value.text())
        .collect()
}

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
        let face = &visible.face;
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
                normals: [[0.0; 3]; 3],
                colors: WHITE_CORNERS,
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
    let mut rest = polygon.clone();
    for plane in other.planes() {
        match split(&rest, &plane) {
            // Nothing's inside `other`, so its planes have no business cutting
            // this: they'd shatter faces right across a map.
            Split::Front => return vec![polygon],
            Split::Back => {}
            Split::On => {
                let same = plane.normal.iter().zip(face_plane.normal).all(|(a, b)| {
                    let (a, b): (f64, f64) = (*a, b);
                    (a - b).abs() < 1e-9
                });
                if same && after {
                    return vec![polygon];
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

/// How wide a square of [`BrushGrid`] is.
const BRUSH_GRID: f64 = 512.0;

/// Which brushes are in each square of a flat grid.
struct BrushGrid {
    /// The brushes reaching into each square, in order.
    squares: HashMap<[i64; 2], Vec<usize>>,
}

impl BrushGrid {
    /// The squares a box reaches.
    #[expect(clippy::cast_possible_truncation, reason = "maps are small")]
    fn squares_of(area: (Point, Point)) -> impl Iterator<Item = [i64; 2]> {
        let square = |value: f64| (value / BRUSH_GRID).floor() as i64;
        let (x0, x1) = (square(area.0[0]), square(area.1[0]));
        let (y0, y1) = (square(area.0[1]), square(area.1[1]));
        (x0..=x1).flat_map(move |x| (y0..=y1).map(move |y| [x, y]))
    }

    /// The grid of `brushes`.
    fn new(brushes: &[Brush], bounds: &[Option<(Point, Point)>]) -> Self {
        let mut squares: HashMap<[i64; 2], Vec<usize>> = HashMap::new();
        for (index, area) in bounds.iter().enumerate() {
            if let Some(area) = area.filter(|_| brushes[index].contents == Contents::Solid) {
                for square in Self::squares_of(area) {
                    squares.entry(square).or_default().push(index);
                }
            }
        }
        Self { squares }
    }

    /// The brushes that might touch `area`, each once, in order.
    fn near(&self, area: (Point, Point)) -> Vec<usize> {
        let mut found: Vec<usize> = Self::squares_of(area)
            .filter_map(|square| self.squares.get(&square))
            .flatten()
            .copied()
            .collect();
        found.sort_unstable();
        found.dedup();
        found
    }
}

/// The box round a brush, a little grown, or `None` if it has no corners.
fn brush_bounds(brush: &Brush) -> Option<(Point, Point)> {
    let corners = solid_corners(&brush.planes());
    let (first, rest) = corners.split_first()?;
    let (mut low, mut high) = (*first, *first);
    for corner in rest {
        for axis in 0..3 {
            low[axis] = low[axis].min(corner[axis]);
            high[axis] = high[axis].max(corner[axis]);
        }
    }
    Some((low.map(|part| part - 1.0), high.map(|part| part + 1.0)))
}

/// Whether two boxes touch.
fn boxes_touch(a: (Point, Point), b: (Point, Point)) -> bool {
    (0..3).all(|axis| a.0[axis] <= b.1[axis] && b.0[axis] <= a.1[axis])
}

/// The visible polygons of `brushes`, each with the face it came from.
fn visible_faces(brushes: &[Brush]) -> Vec<VisibleFace<'_>> {
    let mut out = Vec::new();
    let bounds: Vec<Option<(Point, Point)>> = brushes.iter().map(brush_bounds).collect();
    // Solid brushes by where they are, so a face only meets its neighbours.
    let grid = BrushGrid::new(brushes, &bounds);
    for (index, brush) in brushes.iter().enumerate() {
        let planes = brush.planes();
        for (at, face) in brush.faces.iter().enumerate() {
            let Some(polygon) = clip_behind(plane_polygon(&face.plane), &planes, Some(at)) else {
                continue;
            };
            let whole = Arc::new(polygon.clone());
            let mut pieces = vec![polygon];
            let Some(own) = bounds[index] else {
                continue;
            };
            for other_index in grid.near(own) {
                let other = &brushes[other_index];
                if other_index == index
                    || other.contents != Contents::Solid
                    || !bounds[other_index].is_some_and(|box_of| boxes_touch(own, box_of))
                {
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
                    face: Cow::Borrowed(face),
                    contents: brush.contents,
                    lift: None,
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
    let mut items = Vec::with_capacity(brushes.len());
    for brush in brushes {
        if size.is_some() && brush.contents != Contents::Solid {
            continue;
        }
        let planes = size.map_or_else(|| brush.planes(), |size| grown(brush, size));
        let corners = solid_corners(&planes);
        let planes: Vec<u32> = planes.iter().map(|plane| set.index_of(*plane)).collect();
        items.push(Item {
            planes,
            corners,
            contents: brush.contents,
        });
    }
    let mut tree = TreeBuilder::default();
    let root = tree.build(set, &items);
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
    /// How many models are kept, and how many have been asked for, if geometry
    /// was kept from an earlier compile.
    known_models: Option<(u32, u32)>,
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
            light_axes: None,
        });
        Some(())
    }

    /// Adds a triangle of a mesh.
    /// What `name`'s texture is, and what its places on it are multiplied by.
    fn mesh_material(&mut self, name: &str) -> (u32, f32) {
        let texture = self.texture_of(name);
        let tile = self.map.textures.get(texture as usize);
        // A face's places go by its texture's size, or this with none.
        let across = if tile.is_some_and(|tile| tile.tile_size().is_some()) {
            1.0
        } else {
            64.0
        };
        (texture, across)
    }

    /// Keeps the meshes that things in the level can show.
    fn keep_moving_meshes(&mut self, working: &WorkingMap) {
        for (name, mesh) in &working.moving {
            let mut shared = shared_mesh(mesh, &mut |material| self.mesh_material(material));
            coarsest_corners_first(&mut shared);
            self.map.moving_meshes.push(MovingMesh {
                name: name.clone(),
                mesh: shared,
                skeleton: with_hit_capsules(mesh),
            });
        }
    }

    /// Adds each sheet of land, and a face for each of its chunks to the world, or finds them
    /// where `known` says they are.
    fn add_terrain(&mut self, working: &WorkingMap, known: Option<&[u32]>) {
        let mut painted: Vec<String> = Vec::new();
        for layer in working.terrain.iter().flat_map(|sheet| &sheet.data.layers) {
            if !painted.contains(&layer.material) {
                painted.push(layer.material.clone());
            }
        }
        if !painted.is_empty() {
            self.map.land_pictures = self.source.land_pictures(&painted);
        }
        for (at, source) in working.terrain.iter().enumerate() {
            let texture = self.texture_of(&source.material);
            #[expect(clippy::cast_possible_truncation, reason = "land is in f32")]
            let [x, y, z] = source.origin.map(|part| part as f32);
            let mut sheet = TerrainSheet {
                data: source.data.clone(),
                origin: Vec3::new(x, y, z),
                facing_down: source.facing_down,
                #[expect(clippy::cast_possible_truncation, reason = "land is in f32")]
                thickness: source.thickness as f32,
                texture,
                first_face: u32::try_from(self.map.faces.len()).unwrap_or(0),
                pictures: source
                    .data
                    .layers
                    .iter()
                    .filter_map(|layer| painted.iter().position(|name| *name == layer.material))
                    .filter_map(|at| u32::try_from(at).ok())
                    .collect(),
            };
            if let Some(known) = known {
                sheet.first_face = known.get(at).copied().unwrap_or(0);
            } else {
                self.add_chunk_faces(&sheet);
            }
            self.map.terrain.push(sheet);
        }
    }

    /// Adds a face for each chunk of `sheet` to the world.
    fn add_chunk_faces(&mut self, sheet: &TerrainSheet) {
        let [across, down] = sheet.data.chunks();
        for y in 0..down {
            for x in 0..across {
                let points = chunk_points(sheet, x, y);
                let first_point = u32::try_from(self.map.points.len()).unwrap_or(0);
                self.map.points.extend(points);
                self.map.faces.push(chunk_face(first_point, sheet.texture));
                if let Some(world) = self.map.models.first_mut() {
                    world.face_count += 1;
                    for point in points {
                        world.mins.x = world.mins.x.min(point.x);
                        world.mins.y = world.mins.y.min(point.y);
                        world.maxs.x = world.maxs.x.max(point.x);
                        world.maxs.y = world.maxs.y.max(point.y);
                    }
                    let [low, high] = sheet.chunk_heights(x, y);
                    world.mins.z = world.mins.z.min(low);
                    world.maxs.z = world.maxs.z.max(high);
                }
            }
        }
    }

    /// Makes an instance of each placement that shares its mesh, keeping each mesh once. A
    /// placement's faces stay, for its light and for finding it by leaves.
    fn share_meshes(&mut self, working: &WorkingMap) {
        let mut numbers: HashMap<&str, u32> = HashMap::new();
        for group in self.map.mesh_groups.clone() {
            let placement = group
                .placement
                .and_then(|at| working.meshes.get(at as usize));
            let Some(placement) = placement else {
                continue;
            };
            let Some(mesh) = working.shared.get(&placement.source) else {
                continue;
            };
            let number = if let Some(&number) = numbers.get(placement.source.as_str()) {
                number
            } else {
                let number = u32::try_from(self.map.shared_meshes.len()).unwrap_or(u32::MAX);
                let shared = shared_mesh(&at_rest(mesh), &mut |name| self.mesh_material(name));
                self.map.shared_meshes.push(shared);
                numbers.insert(placement.source.as_str(), number);
                number
            };
            self.map.mesh_instances.push(MeshInstance {
                mesh: number,
                rows: placement_rows(placement),
                first_face: group.first_face,
                face_count: group.face_count,
            });
        }
    }

    fn add_triangle(&mut self, triangle: &PlacedTriangle) -> Option<()> {
        let texture = self.texture_of(&triangle.material);
        let tile = self.map.textures.get(texture as usize)?.tile_size();
        let size = tile.map_or((64.0, 64.0), |tile| {
            (f64::from(tile[0]), f64::from(tile[1]))
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
        self.map.normals.resize(first_point as usize, [0; 3]);
        self.map
            .normals
            .extend(triangle.normals.iter().map(|normal| packed_normal(*normal)));
        if triangle.colors != WHITE_CORNERS {
            self.map
                .point_colors
                .resize(first_point as usize, [u8::MAX; 4]);
            self.map.point_colors.extend(triangle.colors);
        }
        self.map.faces.push(Face {
            first_point,
            point_count: 3,
            texture,
            axes,
            light: None,
            light_axes: None,
        });
        Some(())
    }

    /// `faces`, with `stamps` pressed into them.
    fn stamped<'a>(
        &mut self,
        faces: Vec<VisibleFace<'a>>,
        stamps: &[StampSource],
    ) -> Vec<VisibleFace<'a>> {
        let mut faces = faces;
        for stamp in stamps {
            let texture = self.source.pixels(&stamp.image);
            let Some(image) = texture.mips.first() else {
                continue;
            };
            let mut out = Vec::with_capacity(faces.len());
            for visible in faces {
                let touched = (visible.contents == Contents::Solid && visible.lift.is_none())
                    .then(|| stamp_piece(stamp, image, &visible.polygon, &visible.face))
                    .flatten();
                let Some((outside, new_faces)) = touched else {
                    out.push(visible);
                    continue;
                };
                for piece in outside.into_iter().filter(|piece| piece.len() >= 3) {
                    out.push(VisibleFace {
                        polygon: piece,
                        whole: Arc::clone(&visible.whole),
                        face: visible.face.clone(),
                        contents: visible.contents,
                        lift: None,
                    });
                }
                for made in new_faces {
                    out.push(VisibleFace {
                        whole: Arc::new(made.polygon.clone()),
                        polygon: made.polygon,
                        face: Cow::Owned(made.face),
                        contents: Contents::Solid,
                        lift: Some(made.lift),
                    });
                }
            }
            faces = out;
        }
        faces
    }

    /// Adds a model for `brushes` and `meshes`, with `stamps` pressed into its
    /// faces, and returns its number.
    fn add_model(
        &mut self,
        brushes: &[Brush],
        meshes: &[&PlacedMesh],
        stamps: &[StampSource],
    ) -> Result<u32, CompileError> {
        let too_big = || CompileError::Bad("the map is too big".to_owned());
        if let Some((count, next)) = &mut self.known_models {
            if *next >= *count {
                return Err(CompileError::Bad(
                    "kept geometry has too few models".to_owned(),
                ));
            }
            *next += 1;
            return Ok(*next - 1);
        }
        let first_face = u32::try_from(self.map.faces.len()).map_err(|_| too_big())?;
        let (mut low, mut high) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for visible in self.stamped(visible_faces(brushes), stamps) {
            let VisibleFace {
                polygon,
                whole,
                face,
                contents,
                ..
            } = &visible;
            for point in polygon {
                for axis in 0..3 {
                    low[axis] = low[axis].min(point[axis]);
                    high[axis] = high[axis].max(point[axis]);
                }
            }
            if let Some(lift) = visible.lift {
                let face = u32::try_from(self.map.faces.len()).map_err(|_| too_big())?;
                self.map.light_lifts.push((face, vec3(lift)));
            }
            self.add_face(polygon, whole, face).ok_or_else(too_big)?;
            if *contents != Contents::Solid {
                // Liquids are seen from both sides.
                let flipped: Vec<Point> = polygon.iter().rev().copied().collect();
                self.add_face(&flipped, whole, face).ok_or_else(too_big)?;
            }
        }
        for mesh in meshes {
            let first_face = u32::try_from(self.map.faces.len()).map_err(|_| too_big())?;
            for triangle in &mesh.triangles {
                for point in &triangle.points {
                    for axis in 0..3 {
                        low[axis] = low[axis].min(point[axis]);
                        high[axis] = high[axis].max(point[axis]);
                    }
                }
                self.add_triangle(triangle).ok_or_else(too_big)?;
            }
            let last_face = u32::try_from(self.map.faces.len()).map_err(|_| too_big())?;
            if last_face > first_face {
                self.map.mesh_groups.push(MeshGroup {
                    first_face,
                    face_count: last_face - first_face,
                    by_probe: mesh.by_probe,
                    placement: mesh.placement,
                });
            }
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
        self.planes_to_map();
        let tree = &mut self.map.tree;
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

impl Builder<'_> {
    /// Gives the map the planes found so far.
    fn planes_to_map(&mut self) {
        self.map.tree.planes = self
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
            let [red, green, blue, strength] = look.tint;
            // Water that says its colour is tinted by it, unless it says otherwise.
            let [red, green, blue] = water.tint.or(water.color).unwrap_or([red, green, blue]);
            look.tint = [red, green, blue, water.tint_strength.unwrap_or(strength)];
            look.reflect = water.reflect.unwrap_or(look.reflect);
            look.waves = water.waves.unwrap_or(look.waves);
            look.refraction = water.refraction.unwrap_or(look.refraction);
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

/// The named places of a level: its `point` blocks, and those of placements with names.
fn level_places(working: &WorkingMap) -> Vec<AttachPoint> {
    #[expect(clippy::cast_possible_truncation, reason = "maps store f32")]
    let of = |point: Point| Vec3::new(point[0] as f32, point[1] as f32, point[2] as f32);
    let mut places: Vec<AttachPoint> = working
        .points
        .iter()
        .map(|point| AttachPoint {
            name: point.name.clone(),
            origin: of(point.origin),
            angles: of(point.angles),
        })
        .collect();
    for (placement, placed) in working.meshes.iter().zip(&working.placed) {
        if let Some(name) = &placement.name {
            places.extend(placed.attach.iter().map(|point| AttachPoint {
                name: format!("{name}.{}", point.name),
                ..point.clone()
            }));
        }
    }
    places
}

/// Compiles a working map.
///
/// # Errors
///
/// Returns an error if it can't be built.
pub fn compile(working: &WorkingMap, source: &mut dyn TextureSource) -> Result<Map, CompileError> {
    compile_with(working, source, None)
}

/// Compiles a working map whose geometry is already known, as an earlier compile
/// left it, which is most of the work of one.
///
/// # Errors
///
/// Returns an error if it can't be built.
pub fn compile_known(
    working: &WorkingMap,
    source: &mut dyn TextureSource,
    geometry: Geometry,
) -> Result<Map, CompileError> {
    compile_with(working, source, Some(geometry))
}

/// Compiles a working map, with its geometry if that's known.
fn compile_with(
    working: &WorkingMap,
    source: &mut dyn TextureSource,
    known: Option<Geometry>,
) -> Result<Map, CompileError> {
    let mut builder = Builder {
        source,
        map: Map::default(),
        materials: HashMap::new(),
        planes: PlaneSet::default(),
        known_models: None,
    };
    let flames = working.entities.iter().filter(|e| e.kind == "flame");
    let (flame_faces, emitters) = build_flames(flames);
    let mut terrain_faces = None;
    let known = known.map(|geometry| {
        // Faces go by a texture's number, so they come in the same order.
        for name in &geometry.textures {
            builder.texture_of(name);
        }
        let count = u32::try_from(geometry.models.len()).unwrap_or(u32::MAX);
        builder.known_models = Some((count, 0));
        builder.map.points = geometry.points;
        builder.map.normals = geometry.normals;
        builder.map.point_colors = geometry.point_colors;
        terrain_faces = Some(geometry.terrain_faces);
        builder.map.mesh_groups = geometry.mesh_groups;
        builder.map.faces = geometry.faces;
        builder.map.models = geometry.models;
        builder.map.light_lifts = geometry.light_lifts;
        geometry.tree
    });
    let surfaces = if known.is_some() {
        Vec::new()
    } else {
        visible_faces(&working.brushes)
    };
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
    builder.add_model(&working.brushes, &world_meshes, &working.stamps)?;
    builder.add_terrain(working, terrain_faces.as_deref());
    builder.map.fields = working.fields.iter().map(FieldSource::def).collect();
    builder.share_meshes(working);
    builder.keep_moving_meshes(working);
    builder.map.places = level_places(working);
    builder.map.emitters = emitters;
    let mut entities = Vec::with_capacity(working.entities.len() + 1);
    let is_world = |kind: &str| kind == "worldspawn";
    if !working.entities.iter().any(|entity| is_world(&entity.kind)) {
        entities.push(Entity {
            fields: vec![("classname".to_owned(), "worldspawn".to_owned())],
        });
    }
    let tracks = light_tracks(working);
    let mut next_track = tracks.iter();
    let aimed_at = targeted_names(working);
    for entity in &working.entities {
        let mut fields = vec![("classname".to_owned(), entity.kind.clone())];
        // Scripts drive a track by its styles, from here.
        let keyed = entity.kind.starts_with("light")
            && entity.fields.iter().any(|(key, _)| key == "keyframes");
        if keyed && let Some(track) = next_track.next() {
            fields.push(("keyframe_style".to_owned(), track.first_style.to_string()));
        }
        if let Some(name) = &entity.name {
            fields.push(("name".to_owned(), name.clone()));
            let has_its_own = entity.fields.iter().any(|(key, _)| key == "targetname");
            if aimed_at.contains(name.as_str()) && !has_its_own {
                fields.push(("targetname".to_owned(), name.clone()));
            }
        }
        fields.extend(
            entity
                .fields
                .iter()
                .map(|(key, value)| (key.clone(), field_text(value))),
        );
        if entity.kind == "zone" {
            builder.map.zones.push(read_zone(entity));
            continue;
        }
        let meshes: Vec<&PlacedMesh> = entity.placed.iter().collect();
        if !entity.brushes.is_empty() || !meshes.is_empty() {
            let number = builder.add_model(&entity.brushes, &meshes, &[])?;
            fields.push(("model".to_owned(), format!("*{number}")));
            let points = meshes.iter().flat_map(|mesh| mesh.attach.iter().cloned());
            builder
                .map
                .model_points
                .extend(points.map(|point| (number, point)));
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
    builder.map.light_tracks = tracks;
    builder.map.splines.clone_from(&working.splines);
    #[expect(clippy::cast_possible_truncation, reason = "the map stores f32")]
    let size = working.tile_size as f32;
    builder.map.tile_size = size;
    let reach = working
        .entities
        .iter()
        .find(|entity| entity.kind == "worldspawn")
        .and_then(|world| world.fields.iter().find(|(key, _)| key == "wake_reach"))
        .and_then(|(_, value)| value.number());
    #[expect(clippy::cast_possible_truncation, reason = "the map stores f32")]
    let reach = reach.map(|reach| reach as f32);
    builder.map.wake_reach = reach.unwrap_or(size * 1.5);
    builder.map.look = look_of(working);
    builder.map.live_lights = working
        .entities
        .iter()
        .filter(|entity| entity.kind == "live_light")
        .filter_map(build_live_light)
        .collect();
    builder.map.pools = pools_of(working);
    for entity in working.entities.iter().filter(|e| e.kind == "ray_stencil") {
        let stencil = build_stencil(entity, builder.source)?;
        builder.map.stencils.push(stencil);
    }
    builder.map.fog = working.fog;
    builder.map.fog_volumes.clone_from(&working.fog_volumes);
    if let Some(sky) = &working.sky {
        let mut built = sky.sky.clone();
        if let (Some(clouds), Some(image)) = (&mut built.clouds, &sky.cloud_image) {
            let texture = builder.source.pixels(image);
            clouds.image = Some(Arc::new(texture.mips));
        }
        builder.map.sky = Some(built);
    }
    if let Some(tree) = known {
        builder.map.tree = tree;
        return Ok(finished(builder.map));
    }
    builder.finish();
    let size = working
        .entities
        .iter()
        .find(|entity| entity.kind == "worldspawn")
        .and_then(|world| world.fields.iter().find(|(key, _)| key == "cell_size"))
        .and_then(|(_, value)| value.number())
        .unwrap_or_else(|| usual_cell(&builder.map))
        .max(32.0);
    cut_cells(&mut builder.map, &mut builder.planes, size);
    builder.planes_to_map();
    Ok(finished(builder.map))
}

/// How wide `map`'s cells are, if it doesn't say. Every cell looks at every other, and land's wide
/// and mostly open air, so over land they're as wide as it takes to keep their number down. Land
/// draws by what's near, so it doesn't lose much by that.
fn usual_cell(map: &Map) -> f64 {
    let Some(world) = map.models.first().filter(|_| !map.terrain.is_empty()) else {
        return DEFAULT_CELL;
    };
    let across = world.maxs - world.mins;
    let mut size = DEFAULT_CELL;
    let count = |size: f64| {
        [across.x, across.y, across.z]
            .map(|side| (f64::from(side) / size).ceil().max(1.0))
            .iter()
            .product::<f64>()
    };
    while count(size) > MOST_LAND_CELLS {
        size *= 2.0;
    }
    size
}

/// How a working map says its light comes out.
#[expect(clippy::cast_possible_truncation, reason = "the map stores f32")]
fn look_of(working: &WorkingMap) -> Look {
    let world = working.entities.iter().find(|e| e.kind == "worldspawn");
    let field = |key: &str| {
        let fields = world.into_iter().flat_map(|world| &world.fields);
        let found = fields
            .filter(|(name, _)| name == key)
            .find_map(|(_, value)| value.number());
        found.map(|value| value as f32)
    };
    let usual = Look::default();
    Look {
        light_curve: field("light_curve").unwrap_or(usual.light_curve),
        exposure: field("exposure").unwrap_or(usual.exposure),
    }
}

/// A built map, with what has no picture hidden.
fn finished(mut map: Map) -> Map {
    for texture in &mut map.textures {
        if texture.mips.is_empty() {
            texture.kind = TextureKind::Hidden;
        }
    }
    map
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
