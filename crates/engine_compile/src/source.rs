//! A map document, read into typed brushes and entities.

use engine_core::math::Vec3;
use engine_format::{Block, Document, Value};
use engine_map::{Clouds, Contents, Fog, Sky};

use crate::error::CompileError;
use crate::geometry::{Plane3, Point, normalized};
use crate::mesh::PlacedMesh;

/// One side of a brush.
#[derive(Clone, Debug, PartialEq)]
pub struct BrushFace {
    /// Its plane, facing out of the brush.
    pub plane: Plane3,
    /// Its material.
    pub material: String,
    /// Texels per world unit along each texture axis.
    pub scale: [f64; 2],
    /// Texels added along each texture axis.
    pub offset: [f64; 2],
    /// Degrees the texture turns on the face.
    pub rotation: f64,
}

/// A convex solid: everything behind all of its faces.
#[derive(Clone, Debug, PartialEq)]
pub struct Brush {
    /// Its sides.
    pub faces: Vec<BrushFace>,
    /// What fills it.
    pub contents: Contents,
}

impl Brush {
    /// The planes of its sides.
    #[must_use]
    pub fn planes(&self) -> Vec<Plane3> {
        self.faces.iter().map(|face| face.plane).collect()
    }
}

/// An entity, with what the format holds for it.
#[derive(Clone, Debug, PartialEq)]
pub struct EntitySource {
    /// Its name, which may repeat.
    pub name: Option<String>,
    /// Its type.
    pub kind: String,
    /// Its other fields.
    pub fields: Vec<(String, Value)>,
    /// Its own geometry, if it has any.
    pub brushes: Vec<Brush>,
    /// The mesh that's its model, if it has one.
    pub mesh: Option<MeshPlacement>,
    /// That mesh, once placed.
    pub placed: Option<PlacedMesh>,
}

/// Where a mesh goes.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshPlacement {
    /// Its name, under `meshes/`.
    pub source: String,
    /// Where its origin goes.
    pub origin: Point,
    /// Its pitch, yaw and roll, in degrees.
    pub angles: Point,
    /// How much it's stretched along each of its axes.
    pub scale: Point,
}

/// A map as it's edited.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkingMap {
    /// The world's brushes.
    pub brushes: Vec<Brush>,
    /// The entities.
    pub entities: Vec<EntitySource>,
    /// The sky, if it has one.
    pub sky: Option<SkySource>,
    /// Images that turn parts of faces into relief or carving.
    pub stamps: Vec<StampSource>,
    /// Strokes of paint.
    pub paint: Vec<PaintStroke>,
    /// Images laid over the world's surfaces.
    pub decals: Vec<DecalSource>,
    /// Meshes in the world.
    pub meshes: Vec<MeshPlacement>,
    /// Those meshes, once placed.
    pub placed: Vec<PlacedMesh>,
    /// What's changed about how liquids look.
    pub water: Option<WaterSource>,
    /// Fog, if it has any.
    pub fog: Option<Fog>,
}

/// A map's sky, as written.
#[derive(Clone, Debug, PartialEq)]
pub struct SkySource {
    /// The sky itself.
    pub sky: Sky,
    /// An image for the clouds, if it isn't procedural.
    pub cloud_image: Option<String>,
}

/// A field's three numbers.
fn point_of(value: &Value) -> Option<Point> {
    let numbers = value.numbers()?;
    (numbers.len() == 3).then(|| [numbers[0], numbers[1], numbers[2]])
}

/// An image pressed into surfaces, as a box against them. Its brightness is
/// height, where mid grey leaves a surface alone.
#[derive(Clone, Debug, PartialEq)]
pub struct StampSource {
    /// The material whose image it is.
    pub image: String,
    /// The material of what's cut or raised, if not the surface's own.
    pub inner: Option<String>,
    /// Its middle.
    pub origin: Point,
    /// The way out of the surface it's pressed against.
    pub normal: Point,
    /// Its width and height.
    pub size: [f64; 2],
    /// How far it turns on the surface, in degrees.
    pub angle: f64,
    /// How far white stands out, and black cuts in.
    pub depth: f64,
    /// How wide its cells are.
    pub detail: f64,
}

/// A stroke of a material layer across surfaces.
#[derive(Clone, Debug, PartialEq)]
pub struct PaintStroke {
    /// Which layer, from 0 to 3.
    pub layer: usize,
    /// How far either side of its line it reaches.
    pub radius: f64,
    /// How strong it is at its middle, from 0 to 1.
    pub strength: f64,
    /// The points it passes through.
    pub points: Vec<Point>,
}

/// An image laid over surfaces, as a box pressed against them.
#[derive(Clone, Debug, PartialEq)]
pub struct DecalSource {
    /// Its material.
    pub material: String,
    /// Its middle.
    pub origin: Point,
    /// The way out of the surface it's pressed against.
    pub normal: Point,
    /// Its width and height.
    pub size: [f64; 2],
    /// How far it turns on the surface, in degrees.
    pub angle: f64,
    /// How far in front of and behind its middle it reaches.
    pub depth: f64,
}

/// A map's own look for its liquids.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WaterSource {
    /// The colour depth takes things towards.
    pub color: Option<[f32; 3]>,
    /// How fast depth tints, per unit.
    pub absorption: Option<f32>,
    /// How strong caustics and shafts are.
    pub caustics: Option<f32>,
}

/// What `name` is as contents.
fn contents_named(name: &str) -> Result<Contents, CompileError> {
    match name {
        "solid" => Ok(Contents::Solid),
        "water" => Ok(Contents::Water),
        "slime" => Ok(Contents::Slime),
        "lava" => Ok(Contents::Lava),
        "sky" => Ok(Contents::Sky),
        _ => Err(CompileError::Bad(format!("unknown contents {name}"))),
    }
}

/// A brush face's texture settings, from `block`, falling back on `material`.
fn face_with(block: &Block, plane: Plane3, material: &str) -> BrushFace {
    let pair = |key: &str, default: f64| {
        block
            .get(key)
            .and_then(Value::numbers)
            .filter(|numbers| numbers.len() == 2)
            .map_or([default, default], |numbers| [numbers[0], numbers[1]])
    };
    BrushFace {
        plane,
        material: block
            .get("material")
            .and_then(Value::text)
            .unwrap_or(material)
            .to_owned(),
        scale: pair("scale", 1.0),
        offset: pair("offset", 0.0),
        rotation: block.get("rotation").and_then(Value::number).unwrap_or(0.0),
    }
}

/// The six planes of a box between two corners.
fn cuboid_planes(low: Point, high: Point) -> [Plane3; 6] {
    let plane = |normal: Point, distance: f64| Plane3 { normal, distance };
    [
        plane([1.0, 0.0, 0.0], high[0]),
        plane([-1.0, 0.0, 0.0], -low[0]),
        plane([0.0, 1.0, 0.0], high[1]),
        plane([0.0, -1.0, 0.0], -low[1]),
        plane([0.0, 0.0, 1.0], high[2]),
        plane([0.0, 0.0, -1.0], -low[2]),
    ]
}

/// Reads a `brush` block.
fn read_brush(block: &Block) -> Result<Brush, CompileError> {
    let contents = match block.get("contents").and_then(Value::text) {
        Some(name) => contents_named(name)?,
        None => Contents::Solid,
    };
    let material = block
        .get("material")
        .and_then(Value::text)
        .unwrap_or("default");
    let mut faces = Vec::new();
    if let Some(value) = block.get("cuboid") {
        let numbers = value
            .numbers()
            .filter(|numbers| numbers.len() == 6)
            .ok_or_else(|| CompileError::Bad("a cuboid needs six numbers".to_owned()))?;
        let low = [
            numbers[0].min(numbers[3]),
            numbers[1].min(numbers[4]),
            numbers[2].min(numbers[5]),
        ];
        let high = [
            numbers[0].max(numbers[3]),
            numbers[1].max(numbers[4]),
            numbers[2].max(numbers[5]),
        ];
        for plane in cuboid_planes(low, high) {
            faces.push(face_with(block, plane, material));
        }
    }
    for child in block.children_of("face") {
        let normal = child
            .get("normal")
            .and_then(point_of)
            .and_then(normalized)
            .ok_or_else(|| CompileError::Bad("a face needs a normal".to_owned()))?;
        let distance = child
            .get("distance")
            .and_then(Value::number)
            .ok_or_else(|| CompileError::Bad("a face needs a distance".to_owned()))?;
        faces.push(face_with(child, Plane3 { normal, distance }, material));
    }
    if faces.len() < 4 {
        return Err(CompileError::Bad(
            "a brush needs at least four faces".to_owned(),
        ));
    }
    Ok(Brush { faces, contents })
}

/// Reads an `entity` block.
fn read_entity(block: &Block) -> Result<EntitySource, CompileError> {
    let kind = block
        .get("type")
        .and_then(Value::text)
        .ok_or_else(|| CompileError::Bad("an entity needs a type".to_owned()))?
        .to_owned();
    let mesh = block
        .get("mesh")
        .is_some()
        .then(|| read_placement(block, "mesh"));
    let mesh = mesh.transpose()?;
    // A mesh entity's place is its model's, as a brush entity's is.
    let placing = |key: &str| mesh.is_some() && matches!(key, "origin" | "angles" | "scale");
    let fields = block
        .fields
        .iter()
        .filter(|(key, _)| !matches!(key.as_str(), "type" | "id" | "prefab" | "mesh"))
        .filter(|(key, _)| !placing(key))
        .cloned()
        .collect();
    let brushes = block
        .children_of("brush")
        .map(read_brush)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EntitySource {
        name: block.name.clone(),
        kind,
        fields,
        brushes,
        mesh,
        placed: None,
    })
}

/// A stamp, from its block.
fn read_stamp(block: &Block) -> Result<StampSource, CompileError> {
    let missing = |what: &str| CompileError::Bad(format!("a stamp needs {what}"));
    let number =
        |key: &str, default: f64| block.get(key).and_then(Value::number).unwrap_or(default);
    let size = block
        .get("size")
        .and_then(Value::numbers)
        .unwrap_or_default();
    let size = match size.as_slice() {
        [width, height] => [*width, *height],
        _ => [number("size", 64.0); 2],
    };
    Ok(StampSource {
        image: block
            .get("image")
            .and_then(Value::text)
            .ok_or_else(|| missing("an image"))?
            .to_owned(),
        inner: block.get("inner").and_then(Value::text).map(str::to_owned),
        origin: block
            .get("origin")
            .and_then(point_of)
            .ok_or_else(|| missing("an origin"))?,
        normal: block
            .get("normal")
            .and_then(point_of)
            .ok_or_else(|| missing("a normal"))?,
        size,
        angle: number("angle", 0.0),
        depth: number("depth", 8.0),
        detail: number("detail", 4.0).max(1.0),
    })
}

/// A stroke of paint, from its block.
fn read_paint(block: &Block) -> PaintStroke {
    let number =
        |key: &str, default: f64| block.get(key).and_then(Value::number).unwrap_or(default);
    let numbers = block
        .get("points")
        .and_then(Value::numbers)
        .unwrap_or_default();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to the layers"
    )]
    let layer = number("layer", 0.0).clamp(0.0, 3.0) as usize;
    PaintStroke {
        layer,
        radius: number("radius", 32.0),
        strength: number("strength", 1.0),
        points: numbers.as_chunks::<3>().0.to_vec(),
    }
}

/// A decal, from its block.
fn read_decal(block: &Block) -> Result<DecalSource, CompileError> {
    let missing = |what: &str| CompileError::Bad(format!("a decal needs {what}"));
    let number =
        |key: &str, default: f64| block.get(key).and_then(Value::number).unwrap_or(default);
    let size = block
        .get("size")
        .and_then(Value::numbers)
        .unwrap_or_default();
    let size = match size.as_slice() {
        [width, height] => [*width, *height],
        [both] => [*both, *both],
        _ => [number("size", 64.0); 2],
    };
    Ok(DecalSource {
        material: block
            .get("material")
            .and_then(Value::text)
            .ok_or_else(|| missing("a material"))?
            .to_owned(),
        origin: block
            .get("origin")
            .and_then(point_of)
            .ok_or_else(|| missing("an origin"))?,
        normal: block
            .get("normal")
            .and_then(point_of)
            .ok_or_else(|| missing("a normal"))?,
        size,
        angle: number("angle", 0.0),
        depth: number("depth", 8.0),
    })
}

/// Where `block` puts the mesh its `key` names.
fn read_placement(block: &Block, key: &str) -> Result<MeshPlacement, CompileError> {
    let source = block
        .get(key)
        .and_then(Value::text)
        .ok_or_else(|| CompileError::Bad("a mesh needs a name".to_owned()))?
        .to_owned();
    let triple = |key: &str, default: Point| block.get(key).and_then(point_of).unwrap_or(default);
    let scale = block
        .get("scale")
        .and_then(Value::number)
        .map_or_else(|| triple("scale", [1.0; 3]), |scale| [scale; 3]);
    Ok(MeshPlacement {
        source,
        origin: triple("origin", [0.0; 3]),
        angles: triple("angles", [0.0; 3]),
        scale,
    })
}

/// A field's three numbers as `f32`, or `default`.
fn color_of(block: &Block, key: &str, default: [f32; 3]) -> [f32; 3] {
    #[expect(clippy::cast_possible_truncation, reason = "colours store f32")]
    block
        .get(key)
        .and_then(point_of)
        .map_or(default, |value| value.map(|part| part as f32))
}

/// Reads a `sky` block. The cloud layer is optional, and is an image or
/// procedural, as the author chooses.
fn read_sky(block: &Block) -> SkySource {
    #[expect(clippy::cast_possible_truncation, reason = "skies store f32")]
    let number = |block: &Block, key: &str, default: f32| {
        block
            .get(key)
            .and_then(Value::number)
            .map_or(default, |value| value as f32)
    };
    let defaults = Sky::default();
    let clouds_block = block.children_of("clouds").next();
    let clouds = clouds_block.map(|clouds| Clouds {
        color: color_of(clouds, "color", [1.0, 1.0, 1.0]),
        coverage: number(clouds, "coverage", 0.5),
        speed: {
            let [x, y, _] = color_of(clouds, "speed", [0.01, 0.003, 0.0]);
            [x, y]
        },
        scale: number(clouds, "scale", 1.0),
        image: None,
    });
    let [x, y, z] = color_of(block, "sun_direction", [0.4, 0.3, 0.8]);
    SkySource {
        sky: Sky {
            zenith: color_of(block, "zenith", defaults.zenith),
            horizon: color_of(block, "horizon", defaults.horizon),
            ground: color_of(block, "ground", defaults.ground),
            horizon_height: number(block, "horizon_height", defaults.horizon_height),
            sun_direction: Vec3::new(x, y, z).normalized(),
            sun_color: color_of(block, "sun_color", defaults.sun_color),
            sun_size: number(block, "sun_size", defaults.sun_size),
            clouds,
        },
        cloud_image: clouds_block
            .and_then(|clouds| clouds.get("image"))
            .and_then(Value::text)
            .map(str::to_owned),
    }
}

/// Reads a `fog` block.
#[expect(clippy::cast_possible_truncation, reason = "the look stores f32")]
fn read_water(block: &Block) -> WaterSource {
    let number = |key: &str| block.get(key).and_then(Value::number).map(|v| v as f32);
    WaterSource {
        color: block
            .get("color")
            .is_some()
            .then(|| color_of(block, "color", [0.0; 3])),
        absorption: number("absorption"),
        caustics: number("caustics"),
    }
}

/// Fog, from its block.
fn read_fog(block: &Block) -> Fog {
    #[expect(clippy::cast_possible_truncation, reason = "fog stores f32")]
    let number = |key: &str, default: f32| {
        block
            .get(key)
            .and_then(Value::number)
            .map_or(default, |value| value as f32)
    };
    Fog {
        color: color_of(block, "color", [0.6, 0.65, 0.7]),
        density: number("density", 0.001),
        height: number("height", 0.0),
        falloff: number("falloff", 0.004),
    }
}

/// Reads a map document.
///
/// # Errors
///
/// Returns an error if it isn't a map or a block makes no sense.
pub fn read_map(document: &Document) -> Result<WorkingMap, CompileError> {
    if document.kind != "map" {
        return Err(CompileError::NotAMap(document.kind.clone()));
    }
    Ok(WorkingMap {
        brushes: document
            .blocks_of("brush")
            .map(read_brush)
            .collect::<Result<_, _>>()?,
        entities: document
            .blocks_of("entity")
            .map(read_entity)
            .collect::<Result<_, _>>()?,
        sky: document.blocks_of("sky").next().map(read_sky),
        fog: document.blocks_of("fog").next().map(read_fog),
        stamps: document
            .blocks_of("stamp")
            .map(read_stamp)
            .collect::<Result<Vec<_>, _>>()?,
        paint: document.blocks_of("paint").map(read_paint).collect(),
        decals: document
            .blocks_of("decal")
            .map(read_decal)
            .collect::<Result<Vec<_>, _>>()?,
        meshes: document
            .blocks_of("mesh")
            .map(|block| read_placement(block, "source"))
            .collect::<Result<Vec<_>, _>>()?,
        placed: Vec::new(),
        water: document.blocks_of("water").next().map(read_water),
    })
}
