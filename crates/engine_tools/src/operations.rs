//! The named operations. The editor, the command line and MCP all call these.

use std::path::Path;
use std::sync::Arc;

use engine_assets::{DirectoryArchive, FileSystem};
use engine_compile::{
    BakeInput, CompileError, MaterialLibrary, WorkingMap, bake_light, compile, map_hash, read_map,
    side_files, write_bake,
};
use engine_format::{Block, Value, to_binary};
use engine_map::{Map, MipLevel, Texture};
use serde_json::{Value as Json, json};

use crate::game_run::GameRunner;
use crate::json::{block_to_json, move_block, numbers, point_from_json, value_from_json};
use crate::prefab::{build_prefab, prefab_names};
use crate::session::{ID_FIELD, Session, id_of};
use crate::{checks, features};
use crate::{plan, wear};

/// What an argument is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgKind {
    /// Text.
    Text,
    /// A number.
    Number,
    /// Yes or no.
    Flag,
    /// Three numbers.
    Point,
    /// A list of numbers.
    Numbers,
    /// A list of anything.
    List,
    /// Named values.
    Object,
    /// Any value.
    Any,
}

/// One argument of an operation.
#[derive(Clone, Copy, Debug)]
pub struct Arg {
    /// Its name.
    pub name: &'static str,
    /// What it is.
    pub kind: ArgKind,
    /// Whether it has to be there.
    pub required: bool,
    /// What it means.
    pub help: &'static str,
}

/// An argument that has to be there.
const fn needs(name: &'static str, kind: ArgKind, help: &'static str) -> Arg {
    Arg {
        name,
        kind,
        required: true,
        help,
    }
}

/// An argument that can be left out.
const fn may(name: &'static str, kind: ArgKind, help: &'static str) -> Arg {
    Arg {
        name,
        kind,
        required: false,
        help,
    }
}

/// One operation, for listing.
#[derive(Clone, Copy, Debug)]
pub struct OperationInfo {
    /// Its name.
    pub name: &'static str,
    /// What it does.
    pub help: &'static str,
    /// What it takes.
    pub args: &'static [Arg],
}

impl OperationInfo {
    /// What it takes, as a JSON schema, so that an agent needn't guess.
    #[must_use]
    pub fn schema(&self) -> Json {
        let mut properties = serde_json::Map::new();
        for arg in self.args {
            let mut property = match arg.kind {
                ArgKind::Text => json!({ "type": "string" }),
                ArgKind::Number => json!({ "type": "number" }),
                ArgKind::Flag => json!({ "type": "boolean" }),
                ArgKind::Point => json!({
                    "type": "array",
                    "items": { "type": "number" },
                    "minItems": 3,
                    "maxItems": 3,
                }),
                ArgKind::Numbers => json!({ "type": "array", "items": { "type": "number" } }),
                ArgKind::List => json!({ "type": "array" }),
                ArgKind::Object => json!({ "type": "object" }),
                ArgKind::Any => json!({}),
            };
            property["description"] = json!(arg.help);
            properties.insert(arg.name.to_owned(), property);
        }
        let required: Vec<&str> = self
            .args
            .iter()
            .filter(|arg| arg.required)
            .map(|arg| arg.name)
            .collect();
        json!({ "type": "object", "properties": properties, "required": required })
    }
}

/// Every operation.
pub const OPERATIONS: &[OperationInfo] = &[
    OperationInfo {
        name: "new",
        help: "Starts an empty map.",
        args: &[],
    },
    OperationInfo {
        name: "open",
        help: "Opens a map.",
        args: &[needs(
            "path",
            ArgKind::Text,
            "The map, from the content, such as maps/hall.emap",
        )],
    },
    OperationInfo {
        name: "save",
        help: "Saves the map.",
        args: &[may(
            "path",
            ArgKind::Text,
            "Where, from the content. Where it was last saved if left out",
        )],
    },
    OperationInfo {
        name: "describe",
        help: "Lists what's in the map, with IDs.",
        args: &[
            may("id", ArgKind::Number, "Only this thing"),
            may(
                "kind",
                ArgKind::Text,
                "Only blocks of this kind: brush, entity, mesh and so on",
            ),
            may("type", ArgKind::Text, "Only entities of this type"),
            may("prefab", ArgKind::Text, "Only what came from this prefab"),
            may("near", ArgKind::Point, "Only what has an origin near here"),
            may(
                "radius",
                ArgKind::Number,
                "How near, in units. 512 if left out",
            ),
            may(
                "limit",
                ArgKind::Number,
                "The most to return. 200 if left out, and 0 for all",
            ),
            may("skip", ArgKind::Number, "How many to pass over first"),
        ],
    },
    OperationInfo {
        name: "add_brush",
        help: "Adds a box.",
        args: &[
            needs(
                "cuboid",
                ArgKind::Numbers,
                "Six numbers: the low corner then the high",
            ),
            may("material", ArgKind::Text, "A material, by name"),
            may(
                "contents",
                ArgKind::Text,
                "solid, water, slime, lava or sky",
            ),
        ],
    },
    OperationInfo {
        name: "add_entity",
        help: "Adds an entity.",
        args: &[
            needs(
                "type",
                ArgKind::Text,
                "What it is, such as light or info_player_start",
            ),
            may("name", ArgKind::Text, "A label for it"),
            may("origin", ArgKind::Point, "Where it is"),
            may("fields", ArgKind::Object, "The entity's other fields"),
        ],
    },
    OperationInfo {
        name: "place_prefab",
        help: "Places a prefab.",
        args: &[
            needs("prefab", ArgKind::Text, "A prefab, by name"),
            needs("at", ArgKind::Point, "Where it goes"),
            may("params", ArgKind::Object, "The prefab's own settings"),
        ],
    },
    OperationInfo {
        name: "set_field",
        help: "Sets a field on a thing.",
        args: &[
            needs("id", ArgKind::Number, "The thing"),
            needs("key", ArgKind::Text, "The field"),
            needs("value", ArgKind::Any, "What it becomes"),
        ],
    },
    OperationInfo {
        name: "move",
        help: "Moves a thing.",
        args: &[
            needs("id", ArgKind::Number, "The thing"),
            needs("by", ArgKind::Point, "How far, each way"),
        ],
    },
    OperationInfo {
        name: "delete",
        help: "Removes a thing.",
        args: &[needs("id", ArgKind::Number, "The thing")],
    },
    OperationInfo {
        name: "undo",
        help: "Goes back a step.",
        args: &[],
    },
    OperationInfo {
        name: "redo",
        help: "Goes forward a step.",
        args: &[],
    },
    OperationInfo {
        name: "prefabs",
        help: "Lists the prefabs.",
        args: &[],
    },
    OperationInfo {
        name: "add_block",
        help: "Adds any block.",
        args: &[
            needs("kind", ArgKind::Text, "A kind of block a map can have"),
            may("fields", ArgKind::Object, "Its fields"),
        ],
    },
    OperationInfo {
        name: "set_block",
        help: "Makes or changes a block a map has one of, such as sky, fog or water.",
        args: &[
            needs("kind", ArgKind::Text, "sky, fog, water or tile_grid"),
            may("fields", ArgKind::Object, "Its fields"),
        ],
    },
    OperationInfo {
        name: "place_mesh",
        help: "Places a mesh.",
        args: &[
            needs(
                "source",
                ArgKind::Text,
                "A mesh, by name, as list gives them",
            ),
            may("origin", ArgKind::Point, "Where it goes"),
            may("angles", ArgKind::Point, "How it turns"),
            may("scale", ArgKind::Number, "How big"),
            may(
                "light",
                ArgKind::Text,
                "probe to light it from one place, which also shares its triangles with other copies",
            ),
        ],
    },
    OperationInfo {
        name: "add_decal",
        help: "Presses an image on surfaces.",
        args: &[
            needs("material", ArgKind::Text, "A decal material"),
            needs("origin", ArgKind::Point, "Where"),
            needs("normal", ArgKind::Point, "Which way it faces"),
            may("size", ArgKind::Numbers, "Width and height"),
            may("angle", ArgKind::Number, "How far it turns, in degrees"),
            may(
                "depth",
                ArgKind::Number,
                "How far in front and behind it reaches",
            ),
        ],
    },
    OperationInfo {
        name: "add_stamp",
        help: "Carves or raises surfaces.",
        args: &[
            needs("image", ArgKind::Text, "A picture"),
            needs("origin", ArgKind::Point, "Where"),
            needs("normal", ArgKind::Point, "Which way it faces"),
            may("size", ArgKind::Numbers, "Width and height"),
            may("depth", ArgKind::Number, "How deep"),
            may("detail", ArgKind::Number, "How fine"),
            may("inner", ArgKind::Text, "What shows inside"),
        ],
    },
    OperationInfo {
        name: "paint",
        help: "Adds a stroke of a layer.",
        args: &[
            needs("points", ArgKind::List, "The points of the stroke"),
            may("layer", ArgKind::Number, "Which layer"),
            may("radius", ArgKind::Number, "How wide"),
            may("strength", ArgKind::Number, "How strong"),
        ],
    },
    OperationInfo {
        name: "wear",
        help: "Makes the level look used, with paint and decals where its real surfaces meet.",
        args: &[
            may("amount", ArgKind::Number, "How much, from 0 to 1"),
            may("seed", ArgKind::Number, "Which of its variations"),
        ],
    },
    OperationInfo {
        name: "add_spline",
        help: "Adds a line for movement.",
        args: &[
            needs("name", ArgKind::Text, "What to call it"),
            needs("points", ArgKind::List, "Its points"),
            may("closed", ArgKind::Flag, "Whether it joins up"),
        ],
    },
    OperationInfo {
        name: "append_point",
        help: "Adds a point to a stroke or spline.",
        args: &[
            needs("id", ArgKind::Number, "The stroke or spline"),
            needs("point", ArgKind::Point, "The point"),
        ],
    },
    OperationInfo {
        name: "set_material",
        help: "Sets the material of a thing, on all of a brush's faces.",
        args: &[
            needs("id", ArgKind::Number, "The thing"),
            needs("material", ArgKind::Text, "A material, by name"),
        ],
    },
    OperationInfo {
        name: "list",
        help: "Lists the content's materials, meshes and prefabs.",
        args: &[],
    },
    OperationInfo {
        name: "read_script",
        help: "Reads the map's own script.",
        args: &[],
    },
    OperationInfo {
        name: "write_script",
        help: "Writes the map's own script.",
        args: &[needs("text", ArgKind::Text, "The whole script")],
    },
    OperationInfo {
        name: "read_material",
        help: "Reads a material's text.",
        args: &[needs("name", ArgKind::Text, "A material, by name")],
    },
    OperationInfo {
        name: "write_material",
        help: "Writes a material's text, which must parse.",
        args: &[
            needs("name", ArgKind::Text, "A material, by name"),
            needs("text", ArgKind::Text, "Its whole text, which must parse"),
        ],
    },
    OperationInfo {
        name: "derive_maps",
        help: "Works out normal and specular maps from a material's picture, and adds them to it.",
        args: &[
            may("material", ArgKind::Text, "A material, by name"),
            may(
                "prefix",
                ArgKind::Text,
                "Or every material starting with this",
            ),
            may("strength", ArgKind::Number, "How strong the normals are"),
        ],
    },
    OperationInfo {
        name: "check_textures",
        help: "Checks materials and their pictures against the texture standard.",
        args: &[may(
            "prefix",
            ArgKind::Text,
            "Only materials starting with this",
        )],
    },
    OperationInfo {
        name: "check_shaders",
        help: "Compiles every shader in the content, and says what's wrong with any that don't.",
        args: &[],
    },
    OperationInfo {
        name: "check_effects",
        help: "Reads every effect in the content, and says what's wrong with any that don't make sense.",
        args: &[],
    },
    OperationInfo {
        name: "import_gltf",
        help: "Brings a glTF model in, as a mesh with its materials and pictures.",
        args: &[
            needs("path", ArgKind::Text, "The .gltf or .glb"),
            needs("name", ArgKind::Text, "What to call it"),
            may(
                "scale",
                ArgKind::Number,
                "World units to a metre. 32 if left out",
            ),
        ],
    },
    OperationInfo {
        name: "plan_read",
        help: "What the level's floor plan is, and what's still missing, in plain words.",
        args: &[],
    },
    OperationInfo {
        name: "plan_paint",
        help: "Paints squares of the floor plan.",
        args: &[
            needs("cells", ArgKind::List, "A list of column and row pairs"),
            needs("kind", ArgKind::Text, "floor, water or none"),
        ],
    },
    OperationInfo {
        name: "plan_thing",
        help: "Puts a thing on squares of the floor plan.",
        args: &[
            needs("cells", ArgKind::List, "A list of column and row pairs"),
            needs(
                "thing",
                ArgKind::Text,
                "start, light, door, health, ammo, enemy, fire or none",
            ),
        ],
    },
    OperationInfo {
        name: "plan_look",
        help: "Changes what the floor plan's level is made of.",
        args: &[needs(
            "look",
            ArgKind::Text,
            "The start of its materials' names",
        )],
    },
    OperationInfo {
        name: "validate",
        help: "Compiles the map in memory and reports problems.",
        args: &[],
    },
    OperationInfo {
        name: "compile",
        help: "Bakes the saved map's light into a .ebake file.",
        args: &[],
    },
    OperationInfo {
        name: "shot",
        help: "Runs the game on the saved map and takes a screenshot.",
        args: &[
            needs("path", ArgKind::Text, "Where the picture goes"),
            may("from", ArgKind::Point, "Where to look from"),
            may("yaw", ArgKind::Number, "Which way, in degrees"),
            may("pitch", ArgKind::Number, "How far up or down, in degrees"),
            may("commands", ArgKind::List, "Console lines to run first"),
        ],
    },
    OperationInfo {
        name: "play",
        help: "Opens the game on the saved map for someone to play, and leaves it open.",
        args: &[],
    },
    OperationInfo {
        name: "playtest",
        help: "Runs a real server on the saved map for some seconds, and reports.",
        args: &[may("seconds", ArgKind::Number, "How long. 5 if left out")],
    },
    OperationInfo {
        name: "read_shader",
        help: "Reads a shader's text.",
        args: &[needs("name", ArgKind::Text, "A shader, by name")],
    },
    OperationInfo {
        name: "write_shader",
        help: "Writes a shader, refusing one that doesn't compile.",
        args: &[
            needs("name", ArgKind::Text, "A shader, by name"),
            needs("text", ArgKind::Text, "Its whole text, which must compile"),
        ],
    },
    OperationInfo {
        name: "read_effect",
        help: "Reads an effect's text.",
        args: &[needs("name", ArgKind::Text, "An effect, by name")],
    },
    OperationInfo {
        name: "write_effect",
        help: "Writes an effect, refusing one that doesn't make sense.",
        args: &[
            needs("name", ArgKind::Text, "An effect, by name"),
            needs(
                "text",
                ArgKind::Text,
                "Its whole text, which must make sense",
            ),
        ],
    },
    OperationInfo {
        name: "read_prefab",
        help: "Reads a prefab's script.",
        args: &[needs("name", ArgKind::Text, "A prefab, by name")],
    },
    OperationInfo {
        name: "write_prefab",
        help: "Writes a prefab, refusing one that doesn't build.",
        args: &[
            needs("name", ArgKind::Text, "A prefab, by name"),
            needs("text", ArgKind::Text, "Its whole script, which must build"),
        ],
    },
    OperationInfo {
        name: "compile_textures",
        help: "Compiles the pictures materials use.",
        args: &[],
    },
    OperationInfo {
        name: "compile_shaders",
        help: "Compiles every shader a game could want.",
        args: &[],
    },
    OperationInfo {
        name: "pack",
        help: "Writes all of the content to one container.",
        args: &[needs("path", ArgKind::Text, "Where the container goes")],
    },
];

/// A required text argument.
fn text_arg<'a>(args: &'a Json, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Json::as_str)
        .ok_or_else(|| format!("{key} is required"))
}

/// A required ID argument.
fn id_arg(args: &Json) -> Result<u64, String> {
    args.get("id")
        .and_then(Json::as_u64)
        .ok_or_else(|| "id is required".to_owned())
}

/// A required point argument.
fn point_arg(args: &Json, key: &str) -> Result<[f64; 3], String> {
    args.get(key)
        .and_then(point_from_json)
        .ok_or_else(|| format!("{key} needs three numbers"))
}

/// Runs an operation by name. On an error the map is left as it was.
///
/// # Errors
///
/// Returns a message saying what went wrong.
pub fn run(session: &mut Session, name: &str, args: &Json) -> Result<Json, String> {
    match name {
        "new" => {
            let content = session.content.clone();
            *session = Session::new(&content);
            Ok(json!({ "ok": true }))
        }
        "open" => {
            let content = session.content.clone();
            let path = session.in_content(Path::new(text_arg(args, "path")?))?;
            *session = Session::open(&path, &content)?;
            Ok(summary(session))
        }
        "save" => {
            let path = args
                .get("path")
                .and_then(Json::as_str)
                .map(|path| session.in_content(Path::new(path)))
                .transpose()?;
            let saved = session.save(path.as_deref())?;
            Ok(json!({ "path": saved.display().to_string() }))
        }
        "describe" => Ok(describe(session, args)),
        "add_brush" => add_brush(session, args),
        "add_entity" => add_entity(session, args),
        "place_prefab" => place_prefab(session, args),
        "set_field" => set_field(session, args),
        "move" => move_thing(session, args),
        "delete" => delete(session, args),
        "undo" => Ok(json!({ "changed": session.undo() })),
        "redo" => Ok(json!({ "changed": session.redo() })),
        "prefabs" => Ok(json!({ "prefabs": prefab_names(&session.content) })),
        "derive_maps" => features::derive_material_maps(session, args),
        "check_textures" => features::check_textures(session, args),
        "check_shaders" => features::check_shaders(session, args),
        "import_gltf" => features::import_gltf(session, args),
        "check_effects" => features::check_effects(session, args),
        "plan_read" => Ok(plan::plan_read(session)),
        "plan_paint" => plan::plan_paint(session, args),
        "plan_thing" => plan::plan_thing(session, args),
        "plan_look" => plan::plan_look(session, args),
        "add_block" => features::add_block(session, args),
        "set_block" => features::set_block(session, args),
        "place_mesh" => features::place_mesh(session, args),
        "add_decal" => features::add_decal(session, args),
        "add_stamp" => features::add_stamp(session, args),
        "paint" => features::paint(session, args),
        "wear" => wear::wear(session, args),
        "add_spline" => features::add_spline(session, args),
        "append_point" => features::append_point(session, args),
        "set_material" => features::set_material(session, args),
        "list" => Ok(features::list(session)),
        "read_script" => features::read_script(session),
        "write_script" => features::write_script(session, args),
        "read_material" => features::read_material(session, args),
        "write_material" => features::write_material(session, args),
        "read_shader" => features::read_text(session, "shaders", ".wgsl", args),
        "write_shader" => features::write_shader(session, args),
        "read_effect" => features::read_text(session, "effects", ".eeffect", args),
        "write_effect" => features::write_effect(session, args),
        "read_prefab" => features::read_text(session, "prefabs", ".luau", args),
        "write_prefab" => features::write_prefab(session, args),
        "compile_textures" => features::compile_textures(session),
        "compile_shaders" => features::compile_all_shaders(session),
        "pack" => features::pack(session, args),
        "validate" => validate(session),
        "compile" => bake(session),
        "shot" => GameRunner::from_environment(session)?.shot(args),
        "play" => GameRunner::from_environment(session)?.play(),
        "playtest" => GameRunner::from_environment(session)?.playtest(args),
        other => Err(format!("unknown operation {other}")),
    }
}

/// How many things a description holds, unless it's asked for more.
const DESCRIBED: u64 = 200;

/// How near counts as near, unless it's said.
const NEAR: f64 = 512.0;

/// Where `block` is, if it says.
fn origin_of(block: &Block) -> Option<[f64; 3]> {
    let found = block.get("origin").or_else(|| block.get("at"))?;
    let numbers = found.list()?;
    match numbers {
        [x, y, z] => Some([x.number()?, y.number()?, z.number()?]),
        _ => None,
    }
}

/// Whether `block` is what `args` asks for.
fn wanted(block: &Block, args: &Json) -> bool {
    let text = |key: &str| args.get(key).and_then(Json::as_str);
    let says = |key: &str, value: &str| block.get(key).and_then(Value::text) == Some(value);
    if let Some(id) = args.get("id").and_then(Json::as_u64)
        && id_of(block) != Some(id)
    {
        return false;
    }
    if text("kind").is_some_and(|kind| block.kind != kind)
        || text("type").is_some_and(|kind| !says("type", kind))
        || text("prefab").is_some_and(|prefab| !says("prefab", prefab))
    {
        return false;
    }
    let Some(near) = args.get("near").and_then(point_from_json) else {
        return true;
    };
    let radius = args.get("radius").and_then(Json::as_f64).unwrap_or(NEAR);
    origin_of(block).is_some_and(|place| {
        let [x, y, z] = [place[0] - near[0], place[1] - near[1], place[2] - near[2]];
        x * x + y * y + z * z <= radius * radius
    })
}

/// How many of each kind of thing the map holds, and where it's saved.
fn summary(session: &Session) -> Json {
    let mut counts = serde_json::Map::new();
    for block in &session.document.blocks {
        let count = counts.get(&block.kind).and_then(Json::as_u64).unwrap_or(0);
        counts.insert(block.kind.clone(), json!(count + 1));
    }
    json!({
        "path": session.path.as_ref().map(|path| path.display().to_string()),
        "dirty": session.dirty,
        "counts": counts,
    })
}

/// What's in the map, as far as `args` asks. A real level is far too much to take at once.
fn describe(session: &Session, args: &Json) -> Json {
    let skip = args.get("skip").and_then(Json::as_u64).unwrap_or(0);
    let limit = args
        .get("limit")
        .and_then(Json::as_u64)
        .unwrap_or(DESCRIBED);
    let matching: Vec<&Block> = session
        .document
        .blocks
        .iter()
        .filter(|block| wanted(block, args))
        .collect();
    let skip = usize::try_from(skip).unwrap_or(usize::MAX);
    let take = match usize::try_from(limit) {
        Ok(0) | Err(_) => usize::MAX,
        Ok(limit) => limit,
    };
    let things: Vec<Json> = matching
        .iter()
        .skip(skip)
        .take(take)
        .map(|block| block_to_json(block))
        .collect();
    let mut answer = summary(session);
    answer["matching"] = json!(matching.len());
    answer["things"] = Json::Array(things);
    answer
}

/// Adds `block`, with an ID.
fn add(session: &mut Session, mut block: Block) -> u64 {
    let id = session.give_id(&mut block);
    session.document.blocks.push(block);
    id
}

/// `add_brush`.
fn add_brush(session: &mut Session, args: &Json) -> Result<Json, String> {
    let cuboid = args
        .get("cuboid")
        .and_then(Json::as_array)
        .filter(|items| items.len() == 6)
        .ok_or("cuboid needs six numbers")?;
    let values: Option<Vec<f64>> = cuboid.iter().map(Json::as_f64).collect();
    let values = values.ok_or("cuboid needs six numbers")?;
    if (0..3).any(|axis| values[axis] >= values[axis + 3]) {
        return Err("a cuboid's low corner has to be under its high one".to_owned());
    }
    let mut block = Block::new("brush");
    if let Some(material) = args.get("material").and_then(Json::as_str) {
        block.set("material", Value::Text(material.to_owned()));
    }
    if let Some(contents) = args.get("contents").and_then(Json::as_str) {
        block.set("contents", Value::Text(contents.to_owned()));
    }
    block.set("cuboid", numbers(&values));
    session.checkpoint();
    Ok(json!({ "id": add(session, block) }))
}

/// `add_entity`.
fn add_entity(session: &mut Session, args: &Json) -> Result<Json, String> {
    let kind = text_arg(args, "type")?;
    let mut block = args
        .get("name")
        .and_then(Json::as_str)
        .map_or_else(|| Block::new("entity"), |name| Block::named("entity", name));
    block.set("type", Value::Text(kind.to_owned()));
    if let Ok(origin) = point_arg(args, "origin") {
        block.set("origin", numbers(&origin));
    }
    if let Some(fields) = args.get("fields").and_then(Json::as_object) {
        for (key, value) in fields {
            let value = value_from_json(value).ok_or_else(|| format!("bad value for {key}"))?;
            block.set(key, value);
        }
    }
    session.checkpoint();
    Ok(json!({ "id": add(session, block) }))
}

/// `place_prefab`.
fn place_prefab(session: &mut Session, args: &Json) -> Result<Json, String> {
    let name = text_arg(args, "prefab")?;
    let at = point_arg(args, "at")?;
    let params = args.get("params").cloned().unwrap_or_else(|| json!({}));
    let output = build_prefab(&session.content, name, &params, at)?;
    session.checkpoint();
    let ids: Vec<u64> = output
        .blocks
        .into_iter()
        .map(|mut block| {
            block.set("prefab", Value::Text(name.to_owned()));
            add(session, block)
        })
        .collect();
    Ok(json!({ "ids": ids }))
}

/// `set_field`.
fn set_field(session: &mut Session, args: &Json) -> Result<Json, String> {
    let id = id_arg(args)?;
    let key = text_arg(args, "key")?;
    if key == ID_FIELD {
        return Err("an ID can't be set".to_owned());
    }
    let value = args
        .get("value")
        .and_then(value_from_json)
        .ok_or("value is required")?;
    if session.find_mut(id).is_none() {
        return Err(format!("nothing has ID {id}"));
    }
    session.checkpoint();
    let block = session.find_mut(id).ok_or("it went away")?;
    if let Some(field) = block.fields.iter_mut().find(|(name, _)| name == key) {
        field.1 = value;
    } else {
        block.set(key, value);
    }
    Ok(json!({ "ok": true }))
}

/// Moves `block` and anything inside it.
fn shift(block: &mut Block, by: [f64; 3]) {
    move_block(block, by);
    for child in &mut block.children {
        shift(child, by);
    }
}

/// `move`.
fn move_thing(session: &mut Session, args: &Json) -> Result<Json, String> {
    let id = id_arg(args)?;
    let by = point_arg(args, "by")?;
    if session.find_mut(id).is_none() {
        return Err(format!("nothing has ID {id}"));
    }
    session.checkpoint();
    shift(session.find_mut(id).ok_or("it went away")?, by);
    Ok(json!({ "ok": true }))
}

/// `delete`.
fn delete(session: &mut Session, args: &Json) -> Result<Json, String> {
    let id = id_arg(args)?;
    if session.find_mut(id).is_none() {
        return Err(format!("nothing has ID {id}"));
    }
    session.checkpoint();
    session
        .document
        .blocks
        .retain(|block| id_of(block) != Some(id));
    Ok(json!({ "ok": true }))
}

/// The content as a file system.
fn content_files(session: &Session) -> FileSystem {
    let mut filesystem = FileSystem::new();
    filesystem.mount(Box::new(DirectoryArchive::new(&session.content)));
    filesystem
}

/// `validate`: what's wrong with the map. Anything in `problems` has to be put right.
fn validate(session: &Session) -> Result<Json, String> {
    let mut problems = checks::problems_in(session);
    let built = built_map(session);
    let (map, _, _) = match built {
        Ok(built) => built,
        Err(error) => {
            problems.push(error);
            return Ok(json!({ "ok": false, "problems": problems }));
        }
    };
    let missing: Vec<String> = map
        .textures
        .iter()
        .filter(|texture| texture.material.is_none())
        .map(|texture| texture.name.clone())
        .collect();
    problems.extend(missing.iter().map(|name| format!("no material {name}")));
    let starts = map
        .entities
        .iter()
        .filter(|entity| entity.get("classname") == Some("info_player_start"))
        .count();
    if starts == 0 {
        problems.push("nowhere to start: no info_player_start".to_owned());
    }
    Ok(json!({
        "ok": problems.is_empty(),
        "problems": problems,
        "faces": map.faces.len(),
        "models": map.models.len(),
        "entities": map.entities.len(),
        "missing_materials": missing,
        "has_start": starts > 0,
    }))
}

/// `compile`.
/// The map as it would compile, with what was read to make it and the materials it
/// uses.
pub(crate) fn built_map(session: &Session) -> Result<(Map, WorkingMap, String), String> {
    let mut working = read_map(&session.document).map_err(|error| error.to_string())?;
    let filesystem = content_files(session);
    let mut library = MaterialLibrary::new(&filesystem);
    let used = library
        .prepare(&mut working)
        .map_err(|error| error.to_string())?;
    let mut failure = None;
    let mut textures = |material: &str| {
        library.load(material).unwrap_or_else(|error| {
            failure = Some(error.to_string());
            Texture::default()
        })
    };
    let map = compile(&working, &mut textures).map_err(|error| error.to_string())?;
    if let Some(error) = failure {
        return Err(error);
    }
    Ok((map, working, used))
}

/// `compile`: bakes the saved map's light, and writes what the game loads.
fn bake(session: &Session) -> Result<Json, String> {
    let path = session.path.as_ref().ok_or("save the map first")?;
    let problems = checks::problems_in(session);
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    let (mut map, working, used) = built_map(session)?;
    let filesystem = content_files(session);
    let mut images_library = MaterialLibrary::new(&filesystem);
    let mut images = |image: &str| -> Result<Arc<Vec<MipLevel>>, CompileError> {
        images_library.image_at(image)
    };
    let mut input = BakeInput {
        working: &working,
        images: &mut images,
    };
    bake_light(&mut map, &mut input).map_err(|error| error.to_string())?;
    let out = path.with_extension("ebake");
    std::fs::write(&out, write_bake(&map, map_hash(&session.document, &used)))
        .map_err(|error| error.to_string())?;
    let beside = side_files(&map);
    if !beside.is_empty() {
        let folder = path.with_extension("").join("bake");
        std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
        for (file, bytes) in beside {
            std::fs::write(folder.join(file), bytes).map_err(|error| error.to_string())?;
        }
    }
    std::fs::write(path.with_extension("ebin"), to_binary(&session.document))
        .map_err(|error| error.to_string())?;
    Ok(json!({
        "path": out.display().to_string(),
        "samples": map.light.len(),
    }))
}
