//! Bringing models in from glTF 2.0 (proposal 0109). What an artist hands over
//! and what the engine keeps are two formats, as they are for textures.

use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;

use engine_format::{Block, Document, Value, to_binary};
use engine_map::{ChannelPath, Interpolation, Skeleton, SkeletonPlace};
use gltf::image::{Data, Format};

use crate::convex::fit_convex;
use crate::derive::encode_png;
use crate::error::CompileError;
use crate::geometry::{Point, dot, normalized};
use crate::gltf_skin::{ImportedSkeleton, reach_of, read_skeleton};
use crate::mesh::{Mesh, MeshAttach, MeshPart};

/// What a model comes to, in the engine's own files.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportedModel {
    /// The mesh.
    pub mesh: Mesh,
    /// Each material's name and text.
    pub materials: Vec<(String, String)>,
    /// Each picture's path under the content, and its PNG.
    pub pictures: Vec<(String, Vec<u8>)>,
}

/// A 4 by 4 matrix, by columns.
type Matrix = [[f64; 4]; 4];

/// `a` times `b`.
fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    let mut out = [[0.0; 4]; 4];
    for (column, target) in out.iter_mut().enumerate() {
        for (row, value) in target.iter_mut().enumerate() {
            *value = (0..4).map(|at| a[at][row] * b[column][at]).sum();
        }
    }
    out
}

/// `point` through `matrix`.
fn moved(matrix: &Matrix, point: Point) -> Point {
    let mut out = [matrix[3][0], matrix[3][1], matrix[3][2]];
    for (axis, part) in point.iter().enumerate() {
        for (row, value) in out.iter_mut().enumerate() {
            *value += matrix[axis][row] * part;
        }
    }
    out
}

/// `normal` through `matrix`. A surface's direction goes by the inverse of its
/// transpose, or a stretched model's normals would lean the wrong way.
fn turned(matrix: &Matrix, normal: Point) -> Point {
    let column = |at: usize| [matrix[at][0], matrix[at][1], matrix[at][2]];
    let (a, b, c) = (column(0), column(1), column(2));
    let across = |p: Point, q: Point| {
        [
            p[1] * q[2] - p[2] * q[1],
            p[2] * q[0] - p[0] * q[2],
            p[0] * q[1] - p[1] * q[0],
        ]
    };
    let rows = [across(b, c), across(c, a), across(a, b)];
    let mut out = [0.0; 3];
    for (row, part) in rows.iter().zip(normal) {
        for (value, entry) in out.iter_mut().zip(row) {
            *value += entry * part;
        }
    }
    if dot(a, across(b, c)) < 0.0 {
        out = out.map(|part| -part);
    }
    normalized(out).unwrap_or([0.0, 0.0, 1.0])
}

/// A point from glTF's axes, where Y is up, to ours, where Z is.
const fn to_engine(point: Point) -> Point {
    [point[0], -point[2], point[1]]
}

/// A name that's safe in a file's.
fn slug(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    clean.trim_matches('_').to_owned()
}

/// `image` as RGBA bytes.
fn rgba_of(image: &Data) -> Option<Vec<u8>> {
    let pixels = &image.pixels;
    Some(match image.format {
        Format::R8G8B8A8 => pixels.clone(),
        Format::R8G8B8 => pixels
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|[r, g, b]| [*r, *g, *b, 255])
            .collect(),
        Format::R8G8 => pixels
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|[r, g]| [*r, *g, 0, 255])
            .collect(),
        Format::R8 => pixels.iter().flat_map(|r| [*r, *r, *r, 255]).collect(),
        _ => return None,
    })
}

/// What's being brought in.
struct Importer<'a> {
    /// The name the model's files start with.
    name: &'a str,
    /// The model's pictures.
    images: &'a [Data],
    /// What's made so far.
    model: ImportedModel,
    /// The materials done, by their number in the file.
    done: HashMap<Option<usize>, String>,
    /// Pictures written so far, by what they hold.
    written: HashMap<(usize, &'static str), String>,
    /// Its skeleton, if it has skins.
    skeleton: Option<ImportedSkeleton>,
    /// What a skinned mesh is placed by: only the scale, as a skinned mesh's own node is ignored
    /// and its joints say where it is.
    unmoved: Matrix,
}

/// How a picture is changed on its way in.
#[derive(Clone, Copy)]
enum Change {
    /// It's as it was.
    None,
    /// Multiplied by a colour.
    Tinted([f32; 4]),
    /// A normal map: glTF's green goes up the picture, and ours down.
    Normals,
}

impl Importer<'_> {
    /// Writes the picture numbered `source`, and returns its path.
    fn picture(
        &mut self,
        source: usize,
        role: &'static str,
        change: Change,
    ) -> Result<String, CompileError> {
        if let Some(path) = self.written.get(&(source, role)) {
            return Ok(path.clone());
        }
        let bad = |what: &str| CompileError::Bad(format!("{}: {what}", self.name));
        let image = self
            .images
            .get(source)
            .ok_or_else(|| bad("a picture isn't there"))?;
        let mut rgba = rgba_of(image).ok_or_else(|| bad("a picture has too many bits"))?;
        match change {
            Change::None => {}
            Change::Tinted(tint) => {
                for pixel in rgba.as_chunks_mut::<4>().0 {
                    for (part, scale) in pixel.iter_mut().zip(tint) {
                        let value = f32::from(*part) * scale;
                        #[expect(
                            clippy::cast_possible_truncation,
                            clippy::cast_sign_loss,
                            reason = "clamped to a byte"
                        )]
                        let value = value.clamp(0.0, 255.0) as u8;
                        *part = value;
                    }
                }
            }
            Change::Normals => {
                for pixel in rgba.as_chunks_mut::<4>().0 {
                    pixel[1] = 255 - pixel[1];
                }
            }
        }
        let png = encode_png(image.width, image.height, &rgba)?;
        let path = format!("textures/{}_{source}_{role}.png", self.name);
        self.model.pictures.push((path.clone(), png));
        self.written.insert((source, role), path.clone());
        Ok(path)
    }

    /// A small picture of one colour, and its path.
    fn plain_picture(&mut self, tint: [f32; 4]) -> Result<String, CompileError> {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to a byte"
        )]
        let color = tint.map(|part| (part.clamp(0.0, 1.0) * 255.0).round() as u8);
        let png = encode_png(4, 4, &color.repeat(16))?;
        let [r, g, b, a] = color;
        let path = format!("textures/{}_plain_{r}_{g}_{b}_{a}.png", self.name);
        if self.model.pictures.iter().all(|(known, _)| *known != path) {
            self.model.pictures.push((path.clone(), png));
        }
        Ok(path)
    }

    /// The name of `material`, which is written if it's new.
    fn material(&mut self, material: &gltf::Material<'_>) -> Result<String, CompileError> {
        if let Some(name) = self.done.get(&material.index()) {
            return Ok(name.clone());
        }
        let own = material.name().map(slug).filter(|name| !name.is_empty());
        let number = material.index().map_or(0, |index| index + 1);
        let own = own.unwrap_or_else(|| format!("material_{number}"));
        let name = format!("{}_{own}", self.name);
        let shine = material.pbr_metallic_roughness();
        let tint = shine.base_color_factor();
        let base = match shine.base_color_texture() {
            Some(info) => {
                let source = info.texture().source().index();
                let change = if tint == [1.0; 4] {
                    Change::None
                } else {
                    Change::Tinted(tint)
                };
                self.picture(source, "base", change)?
            }
            None => self.plain_picture(tint)?,
        };
        let mut text = format!("format 1 material\n\nsurface {{\n    base = {base}\n");
        let mut say = |line: String| {
            let _written = writeln!(text, "    {line}");
        };
        if let Some(normal) = material.normal_texture() {
            let source = normal.texture().source().index();
            let path = self.picture(source, "normal", Change::Normals)?;
            say(format!("normal = {path}"));
        }
        if let Some(info) = shine.metallic_roughness_texture() {
            let source = info.texture().source().index();
            let path = self.picture(source, "metal_rough", Change::None)?;
            say(format!("metal_rough = {path}"));
        }
        say(format!("metallic = {}", shine.metallic_factor()));
        say(format!("roughness = {}", shine.roughness_factor()));
        if let Some(info) = material.emissive_texture() {
            let source = info.texture().source().index();
            let path = self.picture(source, "glow", Change::None)?;
            say(format!("emissive = {path}"));
            let [r, g, b] = material.emissive_factor();
            say(format!("emissive_strength = {}", r.max(g).max(b)));
        }
        match material.alpha_mode() {
            gltf::material::AlphaMode::Opaque => {}
            gltf::material::AlphaMode::Mask => {
                say("alpha = mask".to_owned());
                say(format!(
                    "cutoff = {}",
                    material.alpha_cutoff().unwrap_or(0.5)
                ));
            }
            gltf::material::AlphaMode::Blend => say("alpha = blend".to_owned()),
        }
        text.push_str("}\n");
        self.model.materials.push((name.clone(), text));
        self.done.insert(material.index(), name.clone());
        Ok(name)
    }

    /// Adds a primitive, placed by `matrix`.
    fn primitive(
        &mut self,
        primitive: &gltf::Primitive<'_>,
        buffers: &[gltf::buffer::Data],
        matrix: &Matrix,
        part_name: &str,
        skin: Option<usize>,
    ) -> Result<(), CompileError> {
        if primitive.mode() != gltf::mesh::Mode::Triangles {
            return Ok(());
        }
        let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|data| &data.0[..]));
        let Some(positions) = reader.read_positions() else {
            return Ok(());
        };
        let points: Vec<Point> = positions
            .map(|point| to_engine(moved(matrix, point.map(f64::from))))
            .collect();
        let normals: Vec<Point> = reader.read_normals().map_or_else(Vec::new, |normals| {
            normals
                .map(|normal| to_engine(turned(matrix, normal.map(f64::from))))
                .collect()
        });
        let uvs: Vec<[f64; 2]> = reader.read_tex_coords(0).map_or_else(
            || vec![[0.0; 2]; points.len()],
            |uvs| uvs.into_f32().map(|uv| uv.map(f64::from)).collect(),
        );
        let corners: Vec<u32> = reader.read_indices().map_or_else(
            || (0..u32::try_from(points.len()).unwrap_or(0)).collect(),
            |indices| indices.into_u32().collect(),
        );
        let triangles: Vec<[u32; 3]> = corners.as_chunks::<3>().0.to_vec();
        let material = self.material(&primitive.material())?;
        let (mut joints, mut weights) = (Vec::new(), Vec::new());
        if let (Some(skin), Some(skeleton)) = (skin, &self.skeleton) {
            // A corner can have more than one set of four. They're read together, and only
            // the strongest four are kept.
            let mut moves: Vec<Vec<(usize, f32)>> = vec![Vec::new(); points.len()];
            for set in 0.. {
                let (Some(carried), Some(shares)) =
                    (reader.read_joints(set), reader.read_weights(set))
                else {
                    break;
                };
                for ((corner, carried), shares) in moves
                    .iter_mut()
                    .zip(carried.into_u16())
                    .zip(shares.into_f32())
                {
                    corner.extend(carried.map(usize::from).into_iter().zip(shares));
                }
            }
            for corner in &moves {
                let (corner_joints, corner_weights) = skeleton.weights_of(skin, corner);
                joints.push(corner_joints);
                weights.push(corner_weights);
            }
        }
        self.model.mesh.parts.push(MeshPart {
            name: part_name.to_owned(),
            material,
            normals,
            points,
            uvs,
            triangles,
            levels: Vec::new(),
            joints,
            weights,
        });
        Ok(())
    }

    /// Adds `node` and what's under it.
    fn node(
        &mut self,
        node: &gltf::Node<'_>,
        buffers: &[gltf::buffer::Data],
        parent: &Matrix,
    ) -> Result<(), CompileError> {
        let local = node
            .transform()
            .matrix()
            .map(|column| column.map(f64::from));
        let matrix = multiply(parent, &local);
        // An empty named for it is a place on the model, facing along its own +X, as a model
        // does once it's in, with +Y up.
        if let Some(name) = node
            .name()
            .map(slug)
            .and_then(|name| name.strip_prefix("attach_").map(str::to_owned))
        {
            self.model.mesh.attach.push(MeshAttach {
                name,
                origin: to_engine(moved(&matrix, [0.0; 3])),
                forward: normal_or(
                    to_engine(turned_plain(&matrix, [1.0, 0.0, 0.0])),
                    [1.0, 0.0, 0.0],
                ),
                left: normal_or(
                    to_engine(turned_plain(&matrix, [0.0, 0.0, -1.0])),
                    [0.0, 1.0, 0.0],
                ),
            });
        }
        if let Some(mesh) = node.mesh() {
            let name = node.name().or(mesh.name()).map_or_else(String::new, slug);
            let skin = node.skin().map(|skin| skin.index());
            let unmoved = self.unmoved;
            let placed = if skin.is_some() { &unmoved } else { &matrix };
            for primitive in mesh.primitives() {
                self.primitive(&primitive, buffers, placed, &name, skin)?;
            }
        }
        for child in node.children() {
            self.node(&child, buffers, &matrix)?;
        }
        Ok(())
    }
}

/// `direction` under `matrix`, ignoring where it's moved to.
fn turned_plain(matrix: &Matrix, direction: Point) -> Point {
    let origin = moved(matrix, [0.0; 3]);
    let end = moved(matrix, direction);
    [end[0] - origin[0], end[1] - origin[1], end[2] - origin[2]]
}

/// `direction` at a length of one, or `fallback` if it has no length.
fn normal_or(direction: Point, fallback: Point) -> Point {
    normalized(direction).unwrap_or(fallback)
}

/// What a part named as a level of detail is a level of, and which level, from `<name>_lod<n>`.
fn level_of_name(name: &str) -> Option<(&str, usize)> {
    let (base, level) = name.rsplit_once("_lod")?;
    let level = level.parse().ok().filter(|level| *level > 0)?;
    Some((base, level))
}

/// Moves parts named as levels into the parts they're levels of, as an author's own levels
/// are better than made ones. Corners are added to that part, so a level costs nothing at
/// finer levels.
fn fold_levels(mesh: &mut Mesh) {
    let (levels, mut finest): (Vec<MeshPart>, Vec<MeshPart>) = std::mem::take(&mut mesh.parts)
        .into_iter()
        .partition(|part| level_of_name(&part.name).is_some());
    let mut levels = levels;
    levels.sort_by_key(|part| level_of_name(&part.name).map_or(0, |(_, level)| level));
    for mut part in levels {
        let Some((base, level)) = level_of_name(&part.name) else {
            continue;
        };
        let material = part.material.clone();
        let named = finest
            .iter_mut()
            .find(|other| other.name == base && other.material == material);
        let Some(target) = named else {
            continue;
        };
        // Normals are for every corner of a part or none of them.
        if target.normals.is_empty() != part.normals.is_empty() {
            target.give_normals();
            part.give_normals();
        }
        let first = u32::try_from(target.points.len()).unwrap_or(u32::MAX);
        target.points.append(&mut part.points);
        target.uvs.append(&mut part.uvs);
        target.normals.append(&mut part.normals);
        target.joints.append(&mut part.joints);
        target.weights.append(&mut part.weights);
        // A level skipped in the file is the one before it.
        while target.levels.len() + 1 < level {
            let last = target.levels.last().unwrap_or(&target.triangles).clone();
            target.levels.push(last);
        }
        let moved = part
            .triangles
            .iter()
            .map(|triangle| triangle.map(|corner| corner + first));
        target.levels.push(moved.collect());
    }
    mesh.parts = finest;
}

/// Reads the glTF file at `path`, naming what's made after `name`. `scale` is how
/// many world units a metre is.
///
/// # Errors
///
/// Returns an error if the file can't be read, or there's nothing to draw in it.
pub fn import_gltf(path: &Path, name: &str, scale: f64) -> Result<ImportedModel, CompileError> {
    let bad = |error: gltf::Error| CompileError::Bad(format!("{}: {error}", path.display()));
    let (document, buffers, images) = gltf::import(path).map_err(bad)?;
    let root = [
        [scale, 0.0, 0.0, 0.0],
        [0.0, scale, 0.0, 0.0],
        [0.0, 0.0, scale, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let mut importer = Importer {
        name,
        images: &images,
        model: ImportedModel::default(),
        done: HashMap::new(),
        written: HashMap::new(),
        skeleton: read_skeleton(&document, &buffers, scale)?,
        unmoved: root,
    };
    let scene = document.default_scene().or(document.scenes().next());
    let Some(scene) = scene else {
        return Err(CompileError::Bad(format!("{name}: it has no scene")));
    };
    for node in scene.nodes() {
        importer.node(&node, &buffers, &root)?;
    }
    if importer.model.mesh.parts.is_empty() {
        return Err(CompileError::Bad(format!("{name}: nothing in it draws")));
    }
    fold_levels(&mut importer.model.mesh);
    if let Some(imported) = importer.skeleton.take() {
        let mesh = &mut importer.model.mesh;
        let mut skeleton = imported.skeleton;
        skeleton.reach = reach_of(mesh, &skeleton);
        // A place on no joint goes with the whole.
        for point in &mesh.attach {
            if !skeleton.places.iter().any(|place| place.name == point.name) {
                skeleton.places.push(place_of(point));
            }
        }
        mesh.skeleton = Some(skeleton);
    }
    importer.model.mesh.convex = fit_convex(&importer.model.mesh);
    Ok(importer.model)
}

/// A place on a model as a skeleton keeps it, on no joint.
fn place_of(point: &MeshAttach) -> SkeletonPlace {
    let left = point.left;
    let forward = point.forward;
    let up = [
        forward[1] * left[2] - forward[2] * left[1],
        forward[2] * left[0] - forward[0] * left[2],
        forward[0] * left[1] - forward[1] * left[0],
    ];
    #[expect(clippy::cast_possible_truncation, reason = "joints store f32")]
    let row = |at: usize| [forward[at], left[at], up[at], point.origin[at]].map(|part| part as f32);
    SkeletonPlace {
        name: point.name.clone(),
        joint: None,
        offset: [row(0), row(1), row(2)],
    }
}

/// Numbers as a value.
fn numbers(values: impl Iterator<Item = f32>) -> Value {
    Value::List(
        values
            .map(|value| Value::Number(f64::from(value)))
            .collect(),
    )
}

/// A joint's number as a file keeps it, where -1 is none.
fn joint_number(joint: Option<u16>) -> Value {
    Value::Number(joint.map_or(-1.0, f64::from))
}

/// Adds `skeleton` to `document`.
fn write_skeleton(document: &mut Document, skeleton: &Skeleton) {
    let mut settings = Block::new("skeleton");
    settings.set("reach", Value::Number(f64::from(skeleton.reach)));
    document.blocks.push(settings);
    for joint in &skeleton.joints {
        let mut block = Block::new("joint");
        block.name = Some(joint.name.clone());
        block.set("parent", joint_number(joint.parent));
        block.set(
            "translation",
            numbers(joint.rest.translation.iter().copied()),
        );
        block.set("rotation", numbers(joint.rest.rotation.iter().copied()));
        block.set("scale", numbers(joint.rest.scale.iter().copied()));
        block.set(
            "inverse_bind",
            numbers(joint.inverse_bind.iter().flatten().copied()),
        );
        document.blocks.push(block);
    }
    for clip in &skeleton.clips {
        let mut block = Block::new("clip");
        block.name = Some(clip.name.clone());
        block.set("length", Value::Number(f64::from(clip.length)));
        for channel in &clip.channels {
            let mut child = Block::new("channel");
            child.set("joint", Value::Number(f64::from(channel.joint)));
            let path = match channel.path {
                ChannelPath::Translation => "translation",
                ChannelPath::Rotation => "rotation",
                ChannelPath::Scale => "scale",
            };
            child.set("path", Value::Text(path.to_owned()));
            let between = match channel.interpolation {
                Interpolation::Step => "step",
                Interpolation::Linear => "linear",
                Interpolation::Cubic => "cubic",
            };
            child.set("between", Value::Text(between.to_owned()));
            child.set("times", numbers(channel.times.iter().copied()));
            child.set("values", numbers(channel.values.iter().copied()));
            block.children.push(child);
        }
        document.blocks.push(block);
    }
    for place in &skeleton.places {
        let mut block = Block::new("place");
        block.name = Some(place.name.clone());
        block.set("joint", joint_number(place.joint));
        block.set("offset", numbers(place.offset.iter().flatten().copied()));
        document.blocks.push(block);
    }
}

/// `mesh` as a binary document.
#[must_use]
pub fn write_mesh(mesh: &Mesh) -> Vec<u8> {
    let flat =
        |numbers: &mut dyn Iterator<Item = f64>| Value::List(numbers.map(Value::Number).collect());
    let mut document = Document::new("mesh");
    for part in &mesh.parts {
        let mut block = Block::new("part");
        block.name = Some(part.name.clone());
        block.set("material", Value::Text(part.material.clone()));
        block.set("points", flat(&mut part.points.iter().flatten().copied()));
        block.set("uvs", flat(&mut part.uvs.iter().flatten().copied()));
        if !part.normals.is_empty() {
            block.set("normals", flat(&mut part.normals.iter().flatten().copied()));
        }
        let corners = part.triangles.iter().flatten().map(|c| f64::from(*c));
        block.set("triangles", flat(&mut corners.into_iter()));
        if !part.joints.is_empty() {
            let bytes = |groups: &[[u8; 4]]| {
                Value::List(
                    groups
                        .iter()
                        .flatten()
                        .map(|byte| Value::Number(f64::from(*byte)))
                        .collect(),
                )
            };
            block.set("joints", bytes(&part.joints));
            block.set("weights", bytes(&part.weights));
        }
        for (level, triangles) in (1_usize..).zip(&part.levels) {
            let corners = triangles.iter().flatten().map(|c| f64::from(*c));
            block.set(&format!("level_{level}"), flat(&mut corners.into_iter()));
        }
        document.blocks.push(block);
    }
    if let Some(skeleton) = &mesh.skeleton {
        write_skeleton(&mut document, skeleton);
    }
    for point in &mesh.attach {
        let mut block = Block::new("attach");
        block.name = Some(point.name.clone());
        block.set("origin", flat(&mut point.origin.iter().copied()));
        block.set("forward", flat(&mut point.forward.iter().copied()));
        block.set("left", flat(&mut point.left.iter().copied()));
        document.blocks.push(block);
    }
    if !mesh.level_sizes.is_empty() {
        let mut block = Block::new("levels");
        block.set(
            "sizes",
            flat(&mut mesh.level_sizes.iter().map(|size| f64::from(*size))),
        );
        document.blocks.push(block);
    }
    for piece in &mesh.convex {
        let mut block = Block::new("convex");
        let numbers = piece
            .iter()
            .flat_map(|(normal, far)| [normal[0], normal[1], normal[2], *far]);
        block.set("planes", flat(&mut numbers.into_iter()));
        document.blocks.push(block);
    }
    for hull in &mesh.hulls {
        let mut block = Block::new("hull");
        block.set("cuboid", flat(&mut hull.iter().copied()));
        document.blocks.push(block);
    }
    to_binary(&document)
}
