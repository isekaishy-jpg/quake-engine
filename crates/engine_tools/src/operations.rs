//! The named operations. The editor, the command line and MCP all call these.

use std::path::Path;
use std::sync::Arc;

use engine_assets::{DirectoryArchive, FileSystem};
use engine_compile::{
    BakeInput, CompileError, MaterialLibrary, bake_light, compile, map_hash, read_map, side_files,
    write_bake,
};
use engine_format::{Block, Value, to_binary};
use engine_map::{MipLevel, Texture};
use serde_json::{Value as Json, json};

use crate::features;
use crate::game_run::GameRunner;
use crate::json::{block_to_json, move_block, numbers, point_from_json, value_from_json};
use crate::prefab::{build_prefab, prefab_names};
use crate::session::{ID_FIELD, Session, id_of};

/// One operation, for listing.
#[derive(Clone, Copy, Debug)]
pub struct OperationInfo {
    /// Its name.
    pub name: &'static str,
    /// What it does and the arguments it takes.
    pub help: &'static str,
}

/// Every operation.
pub const OPERATIONS: &[OperationInfo] = &[
    OperationInfo {
        name: "new",
        help: "Starts an empty map.",
    },
    OperationInfo {
        name: "open",
        help: "Opens a map. path.",
    },
    OperationInfo {
        name: "save",
        help: "Saves the map. Optional path.",
    },
    OperationInfo {
        name: "describe",
        help: "Lists what's in the map, with IDs.",
    },
    OperationInfo {
        name: "add_brush",
        help: "Adds a box. cuboid: six numbers. Optional material, contents.",
    },
    OperationInfo {
        name: "add_entity",
        help: "Adds an entity. type, optional name, origin, fields: an object.",
    },
    OperationInfo {
        name: "place_prefab",
        help: "Places a prefab. prefab, at: three numbers, optional params.",
    },
    OperationInfo {
        name: "set_field",
        help: "Sets a field on a thing. id, key, value.",
    },
    OperationInfo {
        name: "move",
        help: "Moves a thing. id, by: three numbers.",
    },
    OperationInfo {
        name: "delete",
        help: "Removes a thing. id.",
    },
    OperationInfo {
        name: "undo",
        help: "Goes back a step.",
    },
    OperationInfo {
        name: "redo",
        help: "Goes forward a step.",
    },
    OperationInfo {
        name: "prefabs",
        help: "Lists the prefabs.",
    },
    OperationInfo {
        name: "add_block",
        help: "Adds any block. kind, fields.",
    },
    OperationInfo {
        name: "set_block",
        help: "Makes or changes a block a map has one of, such as sky, fog or water. kind, fields.",
    },
    OperationInfo {
        name: "place_mesh",
        help: "Places a mesh. source, optional origin, angles, scale.",
    },
    OperationInfo {
        name: "add_decal",
        help: "Presses an image on surfaces. material, origin, normal, optional size, angle, depth.",
    },
    OperationInfo {
        name: "add_stamp",
        help: "Carves or raises surfaces. image, origin, normal, optional size, depth, detail, inner.",
    },
    OperationInfo {
        name: "paint",
        help: "Adds a stroke of a layer. points, optional layer, radius, strength.",
    },
    OperationInfo {
        name: "add_spline",
        help: "Adds a line for movement. name, points, optional closed.",
    },
    OperationInfo {
        name: "append_point",
        help: "Adds a point to a stroke or spline. id, point.",
    },
    OperationInfo {
        name: "set_material",
        help: "Sets the material of a thing, on all of a brush's faces. id, material.",
    },
    OperationInfo {
        name: "list",
        help: "Lists the content's materials, meshes and prefabs.",
    },
    OperationInfo {
        name: "read_script",
        help: "Reads the map's own script.",
    },
    OperationInfo {
        name: "write_script",
        help: "Writes the map's own script. text.",
    },
    OperationInfo {
        name: "read_material",
        help: "Reads a material's text. name.",
    },
    OperationInfo {
        name: "write_material",
        help: "Writes a material's text, which must parse. name, text.",
    },
    OperationInfo {
        name: "derive_maps",
        help: "Works out normal and specular maps from a material's picture, and adds them to it. material: a name, or prefix: the start of several. Optional strength.",
    },
    OperationInfo {
        name: "validate",
        help: "Compiles the map in memory and reports problems.",
    },
    OperationInfo {
        name: "compile",
        help: "Bakes the saved map's light into a .ebake file.",
    },
    OperationInfo {
        name: "shot",
        help: "Runs the game on the saved map and takes a screenshot. path, optional commands: a list of console lines.",
    },
    OperationInfo {
        name: "playtest",
        help: "Runs a real server on the saved map for some seconds, and reports. Optional seconds.",
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
            *session = Session::open(Path::new(text_arg(args, "path")?), &content)?;
            Ok(describe(session))
        }
        "save" => {
            let path = args.get("path").and_then(Json::as_str).map(Path::new);
            let saved = session.save(path)?;
            Ok(json!({ "path": saved.display().to_string() }))
        }
        "describe" => Ok(describe(session)),
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
        "add_block" => features::add_block(session, args),
        "set_block" => features::set_block(session, args),
        "place_mesh" => features::place_mesh(session, args),
        "add_decal" => features::add_decal(session, args),
        "add_stamp" => features::add_stamp(session, args),
        "paint" => features::paint(session, args),
        "add_spline" => features::add_spline(session, args),
        "append_point" => features::append_point(session, args),
        "set_material" => features::set_material(session, args),
        "list" => Ok(features::list(session)),
        "read_script" => features::read_script(session),
        "write_script" => features::write_script(session, args),
        "read_material" => features::read_material(session, args),
        "write_material" => features::write_material(session, args),
        "validate" => validate(session),
        "compile" => bake(session),
        "shot" => GameRunner::from_environment(session)?.shot(args),
        "playtest" => GameRunner::from_environment(session)?.playtest(args),
        other => Err(format!("unknown operation {other}")),
    }
}

/// A summary of the map.
fn describe(session: &Session) -> Json {
    let things: Vec<Json> = session.document.blocks.iter().map(block_to_json).collect();
    json!({
        "path": session.path.as_ref().map(|path| path.display().to_string()),
        "dirty": session.dirty,
        "things": things,
    })
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

/// `validate`.
fn validate(session: &Session) -> Result<Json, String> {
    let mut working = read_map(&session.document).map_err(|error| error.to_string())?;
    let filesystem = content_files(session);
    let mut library = MaterialLibrary::new(&filesystem);
    library
        .prepare(&mut working)
        .map_err(|error| error.to_string())?;
    let mut failure = None;
    let mut missing = Vec::new();
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
    for texture in &map.textures {
        if texture.material.is_none() {
            missing.push(texture.name.clone());
        }
    }
    let starts = map
        .entities
        .iter()
        .filter(|entity| entity.get("classname") == Some("info_player_start"))
        .count();
    Ok(json!({
        "faces": map.faces.len(),
        "models": map.models.len(),
        "entities": map.entities.len(),
        "missing_materials": missing,
        "has_start": starts > 0,
    }))
}

/// `compile`.
fn bake(session: &Session) -> Result<Json, String> {
    let path = session.path.as_ref().ok_or("save the map first")?;
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
    let mut map = compile(&working, &mut textures).map_err(|error| error.to_string())?;
    if let Some(error) = failure {
        return Err(error);
    }
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
