//! Operations for what surfaces and worlds can do beyond brushes and entities:
//! meshes, decals, stamps, paint, splines, and a map's own script and materials
//! (proposal 0091).

use std::path::PathBuf;

use engine_format::{Block, Value};
use serde_json::{Value as Json, json};

use crate::json::{numbers, value_from_json};
use crate::session::Session;

/// A block's fields from `args`, all of them but what's named in `skip`.
fn set_fields(block: &mut Block, args: &Json, skip: &[&str]) -> Result<(), String> {
    let Some(fields) = args.as_object() else {
        return Ok(());
    };
    for (key, value) in fields {
        if skip.contains(&key.as_str()) {
            continue;
        }
        let value = value_from_json(value).ok_or_else(|| format!("bad value for {key}"))?;
        block.set(key, value);
    }
    Ok(())
}

/// Adds a block of `kind`, with the fields in `args`.
fn add_kind(session: &mut Session, kind: &str, args: &Json) -> Result<Json, String> {
    let name = args.get("name").and_then(Json::as_str);
    let mut block = name.map_or_else(|| Block::new(kind), |name| Block::named(kind, name));
    set_fields(&mut block, args, &["name"])?;
    session.checkpoint();
    let id = session.give_id(&mut block);
    session.document.blocks.push(block);
    Ok(json!({ "id": id }))
}

/// `add_block`: anything the format allows, with its fields.
pub(crate) fn add_block(session: &mut Session, args: &Json) -> Result<Json, String> {
    let kind = args
        .get("kind")
        .and_then(Json::as_str)
        .ok_or("kind is required")?
        .to_owned();
    let fields = args.get("fields").cloned().unwrap_or_else(|| json!({}));
    add_kind(session, &kind, &fields)
}

/// `set_block`: a block a map has one of, such as `sky` or `fog`, made if it's
/// not there and otherwise changed.
pub(crate) fn set_block(session: &mut Session, args: &Json) -> Result<Json, String> {
    let kind = args
        .get("kind")
        .and_then(Json::as_str)
        .ok_or("kind is required")?
        .to_owned();
    let fields = args.get("fields").cloned().unwrap_or_else(|| json!({}));
    if !session
        .document
        .blocks
        .iter()
        .any(|block| block.kind == kind)
    {
        return add_kind(session, &kind, &fields);
    }
    session.checkpoint();
    let found = session
        .document
        .blocks
        .iter_mut()
        .find(|block| block.kind == kind);
    let block = found.ok_or("it went away")?;
    set_fields(block, &fields, &[])?;
    Ok(json!({ "ok": true }))
}

/// `place_mesh`.
pub(crate) fn place_mesh(session: &mut Session, args: &Json) -> Result<Json, String> {
    args.get("source")
        .and_then(Json::as_str)
        .ok_or("source is required")?;
    add_kind(session, "mesh", args)
}

/// `add_decal`.
pub(crate) fn add_decal(session: &mut Session, args: &Json) -> Result<Json, String> {
    for key in ["material", "origin", "normal"] {
        args.get(key).ok_or_else(|| format!("{key} is required"))?;
    }
    add_kind(session, "decal", args)
}

/// `add_stamp`.
pub(crate) fn add_stamp(session: &mut Session, args: &Json) -> Result<Json, String> {
    for key in ["image", "origin", "normal"] {
        args.get(key).ok_or_else(|| format!("{key} is required"))?;
    }
    add_kind(session, "stamp", args)
}

/// `paint`: a stroke of a layer.
pub(crate) fn paint(session: &mut Session, args: &Json) -> Result<Json, String> {
    args.get("points").ok_or("points is required")?;
    add_kind(session, "paint", args)
}

/// `add_spline`.
pub(crate) fn add_spline(session: &mut Session, args: &Json) -> Result<Json, String> {
    args.get("name")
        .and_then(Json::as_str)
        .ok_or("a spline needs a name")?;
    args.get("points").ok_or("points is required")?;
    add_kind(session, "spline", args)
}

/// `append_point`: adds a point to a thing's line, as a stroke or a spline.
pub(crate) fn append_point(session: &mut Session, args: &Json) -> Result<Json, String> {
    let id = args
        .get("id")
        .and_then(Json::as_u64)
        .ok_or("id is required")?;
    let point: Vec<f64> = args
        .get("point")
        .and_then(Json::as_array)
        .map(|items| items.iter().filter_map(Json::as_f64).collect())
        .filter(|parts: &Vec<f64>| parts.len() == 3)
        .ok_or("point needs three numbers")?;
    if session.find_mut(id).is_none() {
        return Err(format!("nothing has ID {id}"));
    }
    session.checkpoint();
    let block = session.find_mut(id).ok_or("it went away")?;
    let mut parts = block
        .get("points")
        .and_then(Value::numbers)
        .unwrap_or_default();
    parts.extend(point);
    let count = parts.len() / 3;
    let line = numbers(&parts);
    match block.fields.iter_mut().find(|(key, _)| key == "points") {
        Some(field) => field.1 = line,
        None => block.set("points", line),
    }
    Ok(json!({ "points": count }))
}

/// `set_material`: the material of a thing, on every face of a brush.
pub(crate) fn set_material(session: &mut Session, args: &Json) -> Result<Json, String> {
    let id = args
        .get("id")
        .and_then(Json::as_u64)
        .ok_or("id is required")?;
    let material = args
        .get("material")
        .and_then(Json::as_str)
        .ok_or("material is required")?
        .to_owned();
    if session.find_mut(id).is_none() {
        return Err(format!("nothing has ID {id}"));
    }
    session.checkpoint();
    let block = session.find_mut(id).ok_or("it went away")?;
    let text = Value::Text(material);
    match block.fields.iter_mut().find(|(key, _)| key == "material") {
        Some(field) => field.1 = text.clone(),
        None => block.set("material", text.clone()),
    }
    for face in &mut block.children {
        if face.kind == "face" {
            match face.fields.iter_mut().find(|(key, _)| key == "material") {
                Some(field) => field.1 = text.clone(),
                None => face.set("material", text.clone()),
            }
        }
    }
    Ok(json!({ "ok": true }))
}

/// The names of the files in `folder` of the content, with `extension` off.
fn names_in(session: &Session, folder: &str, extension: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(session.content.join(folder))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            name.strip_suffix(extension).map(str::to_owned)
        })
        .collect();
    names.sort();
    names
}

/// `list`: what the content offers.
pub(crate) fn list(session: &Session) -> Json {
    json!({
        "materials": names_in(session, "materials", ".emat"),
        "meshes": names_in(session, "meshes", ".emesh"),
        "prefabs": names_in(session, "prefabs", ".luau"),
    })
}

/// A safe file name, or a message.
fn checked(name: &str) -> Result<&str, String> {
    if name.is_empty() || name.contains(['/', '\\', '.']) {
        return Err(format!("bad name {name}"));
    }
    Ok(name)
}

/// The path of the map's script, which has the map's name.
fn script_path(session: &Session) -> Result<PathBuf, String> {
    let saved = session.path.as_ref().ok_or("save the map first")?;
    Ok(saved.with_extension("luau"))
}

/// `read_script`.
pub(crate) fn read_script(session: &Session) -> Result<Json, String> {
    let path = script_path(session)?;
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    Ok(json!({ "path": path.display().to_string(), "text": text }))
}

/// `write_script`.
pub(crate) fn write_script(session: &Session, args: &Json) -> Result<Json, String> {
    let text = args
        .get("text")
        .and_then(Json::as_str)
        .ok_or("text is required")?;
    let path = script_path(session)?;
    std::fs::write(&path, text).map_err(|error| error.to_string())?;
    Ok(json!({ "path": path.display().to_string() }))
}

/// `read_material`.
pub(crate) fn read_material(session: &Session, args: &Json) -> Result<Json, String> {
    let name = args
        .get("name")
        .and_then(Json::as_str)
        .ok_or("name is required")?;
    let path = session
        .content
        .join("materials")
        .join(format!("{}.emat", checked(name)?));
    let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    Ok(json!({ "text": text }))
}

/// `write_material`.
pub(crate) fn write_material(session: &Session, args: &Json) -> Result<Json, String> {
    let name = args
        .get("name")
        .and_then(Json::as_str)
        .ok_or("name is required")?;
    let text = args
        .get("text")
        .and_then(Json::as_str)
        .ok_or("text is required")?;
    engine_format::from_text(text).map_err(|error| error.to_string())?;
    let path = session
        .content
        .join("materials")
        .join(format!("{}.emat", checked(name)?));
    std::fs::write(&path, text).map_err(|error| error.to_string())?;
    Ok(json!({ "ok": true }))
}
