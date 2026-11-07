//! A map document, read into typed brushes and entities.

use std::collections::HashMap;
use std::sync::Arc;

use engine_core::math::Vec3;
use engine_format::{Block, Document, Value};
use engine_map::{Clouds, Contents, Fog, FogVolume, Sky, Spline};

use crate::error::CompileError;
use crate::field::{FieldSource, read_field};
use crate::geometry::{Plane3, Point, normalized};
use crate::mesh::{Mesh, PlacedMesh};
use crate::terrain::{TerrainSource, read_terrain};

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
    /// Which of a level's waters it is, if it's liquid and says.
    pub water: Option<String>,
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
    /// Which copy of a prefab it came from, if any.
    pub copy: Option<u64>,
}

/// A named place in a map.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointSource {
    /// What it's called.
    pub name: String,
    /// Where it is.
    pub origin: Point,
    /// Its pitch, yaw and roll, in degrees.
    pub angles: Point,
    /// Which copy of a prefab it came from, if any.
    pub copy: Option<u64>,
}

impl WorkingMap {
    /// The meshes the level says things can show, which it can't work out, as scripts choose
    /// them. It's Quake's own rule, that models are known when a level loads.
    #[must_use]
    pub fn moving_names(&self) -> Vec<String> {
        let world = self
            .entities
            .iter()
            .find(|entity| entity.kind == "worldspawn");
        let names = world
            .and_then(|world| world.text("meshes"))
            .unwrap_or_default();
        names.split_whitespace().map(str::to_owned).collect()
    }
}

impl EntitySource {
    /// Sets a field to three numbers.
    pub(crate) fn set_point(&mut self, key: &str, point: Point) {
        self.fields.retain(|(name, _)| name != key);
        let numbers = point.iter().copied().map(Value::Number).collect();
        self.fields.push((key.to_owned(), Value::List(numbers)));
    }

    /// The value of a field.
    #[must_use]
    pub fn field(&self, key: &str) -> Option<&Value> {
        self.fields
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    /// A field's number.
    #[must_use]
    pub fn number(&self, key: &str) -> Option<f64> {
        self.field(key).and_then(Value::number)
    }

    /// A field's text.
    #[must_use]
    pub fn text(&self, key: &str) -> Option<&str> {
        self.field(key).and_then(Value::text)
    }

    /// A field's three numbers.
    #[must_use]
    pub fn point(&self, key: &str) -> Option<Point> {
        self.field(key).and_then(point_of)
    }

    /// A field's points, three numbers each.
    #[must_use]
    pub fn points(&self, key: &str) -> Vec<Point> {
        self.field(key)
            .and_then(Value::numbers)
            .map_or_else(Vec::new, |numbers| numbers.as_chunks::<3>().0.to_vec())
    }

    /// Whether a field is on, as a flag or a number that isn't zero.
    #[must_use]
    pub fn is_on(&self, key: &str) -> bool {
        self.field(key).is_some_and(|value| {
            value.flag().unwrap_or(false) || value.number().is_some_and(|number| number != 0.0)
        })
    }
}

/// Where a mesh goes.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshPlacement {
    /// Its name, under `meshes/`.
    pub source: String,
    /// What this placement is called, so that its places can be named.
    pub name: Option<String>,
    /// Where its origin goes.
    pub origin: Point,
    /// Its pitch, yaw and roll, in degrees.
    pub angles: Point,
    /// How much it's stretched along each of its axes.
    pub scale: Point,
    /// Whether it's only there to be seen: it can't be touched, and light goes
    /// through it. Falling water is like this.
    pub only_seen: bool,
    /// Whether it's lit from one place, as a small prop is, and not by a light
    /// map of its own.
    pub by_probe: bool,
    /// Which of a level's waters it's the surface of. It isn't drawn as a mesh, then.
    pub water: Option<String>,
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
    /// Lines for movement set out by hand.
    pub splines: Vec<Spline>,
    /// How wide a tile is, or 0 for a map with no tiles.
    pub tile_size: f64,
    /// The parts of the map in files of their own.
    pub tiles: Vec<TileSource>,
    /// Images that turn parts of faces into relief or carving.
    pub stamps: Vec<StampSource>,
    /// Strokes of paint.
    pub paint: Vec<PaintStroke>,
    /// Images laid over the world's surfaces.
    pub decals: Vec<DecalSource>,
    /// Land.
    pub terrain: Vec<TerrainSource>,
    /// Pictures worked out a step at a time.
    pub fields: Vec<FieldSource>,
    /// Meshes in the world.
    pub meshes: Vec<MeshPlacement>,
    /// Those meshes, once placed.
    pub placed: Vec<PlacedMesh>,
    /// Named places.
    pub points: Vec<PointSource>,
    /// The meshes that placements share, by name.
    pub shared: HashMap<String, Arc<Mesh>>,
    /// The meshes that move, in the order the level names them.
    pub moving: Vec<(String, Arc<Mesh>)>,
    /// What's changed about how liquids look.
    pub water: Vec<WaterSource>,
    /// Fog, if it has any.
    pub fog: Option<Fog>,
    /// Fog in places.
    pub fog_volumes: Vec<FogVolume>,
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

/// A part of a map in a file of its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TileSource {
    /// Its place on the grid, across.
    pub x: i32,
    /// Its place on the grid, along.
    pub y: i32,
    /// Its file, under `maps/`, with no extension.
    pub file: String,
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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WaterSource {
    /// What brushes call it, or nothing for those that don't say.
    pub name: Option<String>,
    /// The colour depth takes things towards.
    pub color: Option<[f32; 3]>,
    /// How fast depth tints, per unit.
    pub absorption: Option<f32>,
    /// How strong caustics and shafts are.
    pub caustics: Option<f32>,
    /// How much its surface mirrors.
    pub reflect: Option<f32>,
    /// How tall its waves look.
    pub waves: Option<f32>,
    /// How far its surface bends what's seen through it.
    pub refraction: Option<f32>,
    /// The colour over the view with the eye in it.
    pub tint: Option<[f32; 3]>,
    /// How strong that is, from 0 to 1.
    pub tint_strength: Option<f32>,
    /// How much thicker it gets with each unit of depth.
    pub deep: Option<f32>,
    /// How far down a body with a shape goes.
    pub depth: Option<f32>,
    /// How far apart the heights of a body with a shape are.
    pub cell: Option<f32>,
    /// What a body with a shape is, by name.
    pub contents: Option<String>,
    /// What's over its surface: a picture, how wide it is, how it flows, and how much shows.
    pub overlays: Vec<(String, f32, [f32; 2], f32)>,
    /// How far from the shore foam reaches, by depth.
    pub foam: Option<f32>,
    /// What foam looks like, and how wide that is.
    pub foam_image: Option<(String, f32)>,
    /// How deep it has to get before it stops fading into what's under it.
    pub shore: Option<f32>,
    /// The effect that goes with the view while the eye's in it.
    pub view_effect: Option<String>,
}

/// What `name` is as contents.
pub(crate) fn contents_named(name: &str) -> Result<Contents, CompileError> {
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
    let water = block.get("water").and_then(Value::text).map(str::to_owned);
    Ok(Brush {
        faces,
        contents,
        water,
    })
}

/// Which copy of a prefab `block` came from, if any.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a block's id"
)]
fn copy_of(block: &Block) -> Option<u64> {
    block
        .get("copy")
        .and_then(Value::number)
        .map(|copy| copy as u64)
}

/// Reads a `point` block.
fn read_point(block: &Block) -> PointSource {
    let triple = |key: &str| block.get(key).and_then(point_of).unwrap_or([0.0; 3]);
    PointSource {
        name: block
            .get("name")
            .and_then(Value::text)
            .unwrap_or_default()
            .to_owned(),
        origin: triple("origin"),
        angles: triple("angles"),
        copy: copy_of(block),
    }
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
        .filter(|(key, _)| !matches!(key.as_str(), "type" | "id" | "prefab" | "copy" | "mesh"))
        .filter(|(key, _)| !placing(key))
        .cloned()
        .collect();
    let brushes = block
        .children_of("brush")
        .map(read_brush)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EntitySource {
        copy: copy_of(block),
        name: block.name.clone(),
        kind,
        fields,
        brushes,
        mesh,
        placed: None,
    })
}

/// A spline, from its block.
#[expect(clippy::cast_possible_truncation, reason = "the map stores f32")]
fn read_spline(block: &Block) -> Spline {
    let numbers = block
        .get("points")
        .and_then(Value::numbers)
        .unwrap_or_default();
    Spline {
        name: block.name.clone().unwrap_or_default(),
        points: numbers
            .as_chunks::<3>()
            .0
            .iter()
            .map(|[x, y, z]| Vec3::new(*x as f32, *y as f32, *z as f32))
            .collect(),
        closed: block.get("closed").and_then(Value::flag) == Some(true),
    }
}

/// A tile, from its block.
#[expect(clippy::cast_possible_truncation, reason = "grid places are small")]
fn read_tile(block: &Block) -> Result<TileSource, CompileError> {
    let place = |key: &str| block.get(key).and_then(Value::number).map(|n| n as i32);
    let missing = || CompileError::Bad("a tile needs an x, a y and a file".to_owned());
    Ok(TileSource {
        x: place("x").ok_or_else(missing)?,
        y: place("y").ok_or_else(missing)?,
        file: block
            .get("file")
            .and_then(Value::text)
            .ok_or_else(missing)?
            .to_owned(),
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
        name: block.get("name").and_then(Value::text).map(str::to_owned),
        origin: triple("origin", [0.0; 3]),
        angles: triple("angles", [0.0; 3]),
        scale,
        only_seen: block
            .get("only_seen")
            .and_then(Value::number)
            .is_some_and(|flag| flag != 0.0),
        by_probe: block.get("light").and_then(Value::text) == Some("probe"),
        water: block.get("water").and_then(Value::text).map(str::to_owned),
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

/// A picture over a surface, from its block.
fn read_overlay(block: &Block) -> Option<(String, f32, [f32; 2], f32)> {
    let image = block.get("image").and_then(Value::text)?.to_owned();
    let [x, y, _] = three_of(block, "flow", [0.0; 3]);
    let size = number_of(block, "size", 256.0).max(1.0);
    let strength = number_of(block, "strength", 1.0).clamp(0.0, 1.0);
    Some((image, size, [x, y], strength))
}

/// Reads a `water` block.
#[expect(clippy::cast_possible_truncation, reason = "the look stores f32")]
fn read_water(block: &Block) -> WaterSource {
    let number = |key: &str| block.get(key).and_then(Value::number).map(|v| v as f32);
    WaterSource {
        name: block.name.clone(),
        color: block
            .get("color")
            .is_some()
            .then(|| color_of(block, "color", [0.0; 3])),
        absorption: number("absorption"),
        caustics: number("caustics"),
        reflect: number("reflect"),
        waves: number("waves"),
        refraction: number("refraction"),
        tint: block
            .get("tint")
            .is_some()
            .then(|| color_of(block, "tint", [0.0; 3])),
        tint_strength: number("tint_strength"),
        deep: number("deep"),
        depth: number("depth"),
        cell: number("cell"),
        contents: block
            .get("contents")
            .and_then(Value::text)
            .map(str::to_owned),
        overlays: block
            .children_of("overlay")
            .filter_map(read_overlay)
            .collect(),
        foam: number("foam"),
        foam_image: block
            .get("foam_image")
            .and_then(Value::text)
            .map(|image| (image.to_owned(), number("foam_size").unwrap_or(128.0))),
        shore: number("shore"),
        view_effect: block
            .get("view_effect")
            .and_then(Value::text)
            .map(str::to_owned),
    }
}

/// A number from `block`, or `usual`.
#[expect(clippy::cast_possible_truncation, reason = "these store f32")]
fn number_of(block: &Block, key: &str, usual: f32) -> f32 {
    block
        .get(key)
        .and_then(Value::number)
        .map_or(usual, |number| number as f32)
}

/// Three numbers from `block`, or `usual`. Two leave the third as it was.
#[expect(clippy::cast_possible_truncation, reason = "these store f32")]
fn three_of(block: &Block, key: &str, usual: [f32; 3]) -> [f32; 3] {
    match block.get(key).and_then(Value::numbers).as_deref() {
        Some([x, y, z, ..]) => [*x as f32, *y as f32, *z as f32],
        Some([x, y]) => [*x as f32, *y as f32, usual[2]],
        _ => usual,
    }
}

/// Fog in a place, from its block.
fn read_fog_volume(block: &Block) -> Result<FogVolume, CompileError> {
    let corner = |key: &str| {
        let wanted = block.get(key).and_then(Value::numbers);
        wanted.filter(|numbers| numbers.len() >= 3).ok_or_else(|| {
            CompileError::Bad(format!("fog_volume has to say its {key}, as three numbers"))
        })
    };
    let (one, two) = (corner("low")?, corner("high")?);
    #[expect(clippy::cast_possible_truncation, reason = "these store f32")]
    let least = |axis: usize| one[axis].min(two[axis]) as f32;
    #[expect(clippy::cast_possible_truncation, reason = "these store f32")]
    let most = |axis: usize| one[axis].max(two[axis]) as f32;
    let [x, y, z] = three_of(block, "drift", [0.0; 3]);
    Ok(FogVolume {
        low: Vec3::new(least(0), least(1), least(2)),
        high: Vec3::new(most(0), most(1), most(2)),
        color: three_of(block, "color", [0.6; 3]),
        density: number_of(block, "density", 0.01).max(0.0),
        edge: number_of(block, "edge", 32.0).max(0.0),
        wisps: number_of(block, "wisps", 0.0).clamp(0.0, 1.0),
        wisp_size: number_of(block, "wisp_size", 96.0).max(1.0),
        drift: Vec3::new(x, y, z),
        lit: block.get("lit").and_then(Value::flag) == Some(true),
    })
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
    if document.kind != "map" && document.kind != "tile" {
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
        fog_volumes: document
            .blocks_of("fog_volume")
            .map(read_fog_volume)
            .collect::<Result<_, _>>()?,
        splines: document.blocks_of("spline").map(read_spline).collect(),
        tile_size: document
            .blocks_of("tile_grid")
            .next()
            .and_then(|block| block.get("size"))
            .and_then(Value::number)
            .unwrap_or(0.0),
        tiles: document
            .blocks_of("tile")
            .map(read_tile)
            .collect::<Result<Vec<_>, _>>()?,
        stamps: document
            .blocks_of("stamp")
            .map(read_stamp)
            .collect::<Result<Vec<_>, _>>()?,
        paint: document.blocks_of("paint").map(read_paint).collect(),
        terrain: document
            .blocks_of("terrain")
            .map(read_terrain)
            .collect::<Result<Vec<_>, _>>()?,
        fields: document
            .blocks_of("field")
            .map(read_field)
            .collect::<Result<Vec<_>, _>>()?,
        decals: document
            .blocks_of("decal")
            .map(read_decal)
            .collect::<Result<Vec<_>, _>>()?,
        meshes: document
            .blocks_of("mesh")
            .map(|block| read_placement(block, "source"))
            .collect::<Result<Vec<_>, _>>()?,
        placed: Vec::new(),
        points: document.blocks_of("point").map(read_point).collect(),
        shared: HashMap::new(),
        moving: Vec::new(),
        water: document.blocks_of("water").map(read_water).collect(),
    })
}
