//! Operations for what surfaces and worlds can do beyond brushes and entities:
//! meshes, decals, stamps, paint, splines, and a map's own script and materials
//! (proposal 0091).

use engine_shaders::{check_field_code, check_post_code, check_surface_code};
use std::path::{Path, PathBuf};

use engine_compile::{DeriveSettings, Repeat, check_picture, decode_png, derive_maps, encode_png};
use engine_format::{Block, Value};
use serde_json::{Value as Json, json};

use crate::checks::{is_block_kind, mesh_name};
use crate::json::{numbers, value_from_json};
use crate::prefab::build_prefab;
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
    if !is_block_kind(kind) {
        return Err(format!("a map has no block called {kind}"));
    }
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
    let source = args
        .get("source")
        .and_then(Json::as_str)
        .ok_or("source is required")?;
    let name = mesh_name(source);
    let file = session.content.join("meshes").join(format!("{name}.emesh"));
    if !file.is_file() {
        return Err(format!("no mesh called {name}"));
    }
    let mut args = args.clone();
    args["source"] = json!(name);
    add_kind(session, "mesh", &args)
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
            let stem = name.strip_suffix(extension)?;
            (!stem.starts_with('_')).then(|| stem.to_owned())
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
/// How much a material shines and how glossy it is, going by what it's called.
fn finish_of(name: &str) -> (f32, f64) {
    let metal = ["metal", "plate", "grille", "bronze", "copper", "iron"];
    if metal.iter().any(|word| name.contains(word)) {
        (0.7, 48.0)
    } else if name.contains("wood") || name.contains("door") {
        (0.18, 14.0)
    } else if name.contains("damp") {
        (0.3, 24.0)
    } else {
        (0.14, 12.0)
    }
}

/// The value of a material's `key`, from its text.
fn material_field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim() == key).then(|| value.trim().trim_matches('"'))
    })
}

/// Works out the maps of the material `name`. Returns whether it did: what
/// flows or has no picture is left alone.
fn derive_one(session: &Session, name: &str, strength: f32) -> Result<bool, String> {
    let materials = session.content.join("materials");
    let path = materials.join(format!("{name}.emat"));
    let text = std::fs::read_to_string(&path).map_err(|error| format!("{name}: {error}"))?;
    let Some(base) = material_field(&text, "base") else {
        return Ok(false);
    };
    if matches!(material_field(&text, "kind"), Some("liquid" | "sky")) {
        return Ok(false);
    }
    let bytes = std::fs::read(session.content.join(base)).map_err(|error| error.to_string())?;
    let (width, height, rgba) = decode_png(&bytes).map_err(|error| error.to_string())?;
    let (shine, gloss) = finish_of(name);
    let settings = DeriveSettings { strength, shine };
    let maps = derive_maps(width, height, &rgba, settings);
    let write = |suffix: &str, pixels: &[u8]| -> Result<String, String> {
        let file = format!("textures/{name}_{suffix}.png");
        let png = encode_png(width, height, pixels).map_err(|error| error.to_string())?;
        std::fs::write(session.content.join(&file), png).map_err(|error| error.to_string())?;
        Ok(file)
    };
    let normal = write("normal", &maps.normal)?;
    let specular = write("specular", &maps.specular)?;
    let derived = ["normal", "specular", "specular_strength", "gloss"];
    let mut lines = Vec::new();
    for line in text.lines() {
        let key = line.split_once('=').map(|(key, _)| key.trim());
        if key.is_some_and(|key| derived.contains(&key)) {
            continue;
        }
        lines.push(line.to_owned());
        if key == Some("base") {
            lines.push(format!("    normal = {normal}"));
            lines.push(format!("    specular = {specular}"));
            lines.push("    specular_strength = 1".to_owned());
            lines.push(format!("    gloss = {gloss}"));
        }
    }
    let joined = lines.join("\n") + "\n";
    engine_format::from_text(&joined).map_err(|error| error.to_string())?;
    std::fs::write(&path, joined).map_err(|error| error.to_string())?;
    Ok(true)
}

/// `derive_maps`: normal and specular maps from a material's own picture.
pub(crate) fn derive_material_maps(session: &Session, args: &Json) -> Result<Json, String> {
    #[expect(clippy::cast_possible_truncation, reason = "a strength is small")]
    let strength = args.get("strength").and_then(Json::as_f64).unwrap_or(1.0) as f32;
    let names = match (
        args.get("material").and_then(Json::as_str),
        args.get("prefix").and_then(Json::as_str),
    ) {
        (Some(name), _) => vec![checked(name)?.to_owned()],
        (None, Some(prefix)) => names_in(session, "materials", ".emat")
            .into_iter()
            .filter(|name| name.starts_with(prefix))
            .collect(),
        (None, None) => return Err("material or prefix is required".to_owned()),
    };
    let mut done = Vec::new();
    for name in names {
        if derive_one(session, &name, strength)? {
            done.push(name);
        }
    }
    Ok(json!({ "derived": done }))
}

/// What's wrong with one material, by the texture standard.
fn material_problems(session: &Session, name: &str) -> Result<Vec<String>, String> {
    let path = session
        .content
        .join("materials")
        .join(format!("{name}.emat"));
    let text = std::fs::read_to_string(&path).map_err(|error| format!("{name}: {error}"))?;
    let mut problems = Vec::new();
    if material_field(&text, "world_size").is_none() {
        problems.push("it doesn't say its world_size".to_owned());
    }
    let repeat = match material_field(&text, "repeat").map(Repeat::from_text) {
        Some(Some(repeat)) => repeat,
        Some(None) => {
            problems.push("its repeat isn't both, across or none".to_owned());
            Repeat::None
        }
        None => {
            problems.push("it doesn't say how it repeats".to_owned());
            Repeat::None
        }
    };
    let picture = |path: &str| {
        let bytes = std::fs::read(session.content.join(path)).map_err(|error| error.to_string())?;
        decode_png(&bytes).map_err(|error| error.to_string())
    };
    let Some(base) = material_field(&text, "base") else {
        problems.push("it has no base picture".to_owned());
        return Ok(problems);
    };
    let (width, height, rgba) = picture(base)?;
    problems.extend(check_picture(width, height, &rgba, repeat));
    for key in ["normal", "specular"] {
        if let Some(path) = material_field(&text, key) {
            let (other_width, other_height, _) = picture(path)?;
            if (other_width, other_height) != (width, height) {
                problems.push(format!("its {key} map is a different size"));
            }
        }
    }
    Ok(problems)
}

/// `check_textures`: what's wrong with materials, by the texture standard.
pub(crate) fn check_textures(session: &Session, args: &Json) -> Result<Json, String> {
    let prefix = args.get("prefix").and_then(Json::as_str).unwrap_or("");
    let mut found = serde_json::Map::new();
    for name in names_in(session, "materials", ".emat") {
        if name.starts_with(prefix) {
            let problems = material_problems(session, &name)?;
            if !problems.is_empty() {
                found.insert(name, json!(problems));
            }
        }
    }
    Ok(Json::Object(found))
}

/// `make_levels`: gives a mesh already in the content its coarser levels.
pub(crate) fn make_terrain(session: &Session, args: &Json) -> Result<Json, String> {
    let name = args
        .get("name")
        .and_then(Json::as_str)
        .ok_or("make_terrain needs a name")?;
    let chunks = args
        .get("chunks")
        .and_then(Json::as_f64)
        .ok_or("make_terrain needs a number of chunks")?;
    let hills = args.get("hills").and_then(Json::as_f64).unwrap_or(0.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a small count"
    )]
    let chunks = chunks.clamp(1.0, 64.0) as u32;
    let mut data = engine_map::TerrainData::flat([chunks, chunks], engine_map::USUAL_CELL);
    let across = data.across() as usize;
    #[expect(clippy::cast_precision_loss, reason = "a land's size")]
    for (at, height) in data.heights.iter_mut().enumerate() {
        let (x, y) = ((at % across) as f64, (at / across) as f64);
        let wave = (x * 0.05).sin() * (y * 0.043).cos()
            + (x * 0.017 + y * 0.011).sin() * 1.5
            + (x * 0.13 - y * 0.11).sin() * 0.2;
        #[expect(clippy::cast_possible_truncation, reason = "land is in f32")]
        let rise = (wave * hills * 0.4) as f32;
        *height = rise;
    }
    let folder = session.content.join("terrain");
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let file = folder.join(format!("{name}.eterrain"));
    std::fs::write(&file, engine_compile::write_terrain(&data))
        .map_err(|error| format!("{}: {error}", file.display()))?;
    Ok(json!({ "file": file.display().to_string(), "cells": data.cells[0] }))
}

/// The land called `name`, and where it's kept.
fn land_file(session: &Session, name: &str) -> Result<(engine_map::TerrainData, PathBuf), String> {
    let file = session
        .content
        .join("terrain")
        .join(format!("{name}.eterrain"));
    let bytes = std::fs::read(&file).map_err(|error| format!("{}: {error}", file.display()))?;
    let data = engine_compile::read_terrain_data(&bytes).map_err(|error| error.to_string())?;
    Ok((data, file))
}

/// Keeps `data` in `file`.
fn keep_land(data: &engine_map::TerrainData, file: &Path) -> Result<Json, String> {
    std::fs::write(file, engine_compile::write_terrain(data))
        .map_err(|error| format!("{}: {error}", file.display()))?;
    let (low, high) = data
        .heights
        .iter()
        .fold((f32::MAX, f32::MIN), |(low, high), height| {
            (low.min(*height), high.max(*height))
        });
    Ok(json!({ "low": low, "high": high }))
}

/// A number that's asked for.
fn wanted_number(args: &Json, key: &str) -> Result<f64, String> {
    args.get(key)
        .and_then(Json::as_f64)
        .ok_or_else(|| format!("{key} is required"))
}

/// A place on a sheet, as two or three numbers.
fn sheet_place(args: &Json, key: &str) -> Result<[f32; 2], String> {
    let numbers = args.get(key).and_then(Json::as_array);
    let at = |index: usize| {
        numbers
            .and_then(|list| list.get(index))
            .and_then(Json::as_f64)
    };
    match (at(0), at(1)) {
        #[expect(clippy::cast_possible_truncation, reason = "land is in f32")]
        (Some(x), Some(y)) => Ok([x as f32, y as f32]),
        _ => Err(format!("{key} is required, as a place")),
    }
}

/// The circle in `args`.
fn land_brush(args: &Json) -> Result<engine_map::TerrainBrush, String> {
    #[expect(clippy::cast_possible_truncation, reason = "land is in f32")]
    Ok(engine_map::TerrainBrush {
        middle: sheet_place(args, "at")?,
        radius: wanted_number(args, "radius")? as f32,
        soft: args.get("soft").and_then(Json::as_f64).unwrap_or(0.5) as f32,
    })
}

/// `add_field`.
pub(crate) fn add_field(session: &mut Session, args: &Json) -> Result<Json, String> {
    let shader = text_of(args, "shader")?;
    let shaders = session.content.join("shaders");
    let known = ["wgsl", "egraph"]
        .iter()
        .any(|kind| shaders.join(format!("{shader}.{kind}")).exists());
    if !known {
        return Err(format!("there's no shader called {shader}"));
    }
    add_kind(session, "field", args)
}

/// `add_terrain`.
pub(crate) fn add_terrain(session: &mut Session, args: &Json) -> Result<Json, String> {
    let source = args
        .get("source")
        .and_then(Json::as_str)
        .ok_or("source is required")?;
    let _known = land_file(session, source)?;
    add_kind(session, "terrain", args)
}

/// `shape_terrain`.
pub(crate) fn shape_terrain(session: &Session, args: &Json) -> Result<Json, String> {
    use engine_map::TerrainStroke as Stroke;
    let name = args
        .get("name")
        .and_then(Json::as_str)
        .ok_or("name is required")?;
    let (mut data, file) = land_file(session, name)?;
    #[expect(clippy::cast_possible_truncation, reason = "land is in f32")]
    let amount = args.get("amount").and_then(Json::as_f64).unwrap_or(16.0) as f32;
    let stroke = match args.get("tool").and_then(Json::as_str) {
        Some("raise") => Stroke::Raise(amount),
        Some("lower") => Stroke::Raise(-amount),
        // A height's nothing to guess at.
        #[expect(clippy::cast_possible_truncation, reason = "land is in f32")]
        Some("flatten") => Stroke::Flatten(wanted_number(args, "amount")? as f32),
        Some("smooth") => Stroke::Smooth(amount.clamp(0.0, 1.0)),
        Some("hole") => Stroke::Hole(true),
        Some("fill") => Stroke::Hole(false),
        other => return Err(format!("no land tool called {other:?}")),
    };
    engine_map::stroke_land(&mut data, &land_brush(args)?, stroke);
    keep_land(&data, &file)
}

/// `set_terrain_layer`.
pub(crate) fn set_terrain_layer(session: &Session, args: &Json) -> Result<Json, String> {
    let name = args
        .get("name")
        .and_then(Json::as_str)
        .ok_or("name is required")?;
    let material = args
        .get("material")
        .and_then(Json::as_str)
        .ok_or("material is required")?;
    let (mut data, file) = land_file(session, name)?;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a small count"
    )]
    let number = wanted_number(args, "layer")? as usize;
    #[expect(clippy::cast_possible_truncation, reason = "land is in f32")]
    let size = args.get("size").and_then(Json::as_f64).unwrap_or(512.0) as f32;
    let layer = engine_map::TerrainLayer {
        material: material.to_owned(),
        size,
        hex: args.get("hex").and_then(Json::as_bool).unwrap_or(false),
    };
    if !data.set_layer(number, layer) {
        return Err(format!(
            "layer {number} can't be set: it has {} of {}, and they go in order",
            data.layers.len(),
            engine_map::MOST_LAND_LAYERS
        ));
    }
    keep_land(&data, &file)
}

/// `paint_terrain`.
pub(crate) fn paint_terrain(session: &Session, args: &Json) -> Result<Json, String> {
    let name = args
        .get("name")
        .and_then(Json::as_str)
        .ok_or("name is required")?;
    let (mut data, file) = land_file(session, name)?;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a small count"
    )]
    let layer = wanted_number(args, "layer")? as usize;
    if layer >= data.layers.len() {
        return Err(format!("{name} has {} layers", data.layers.len()));
    }
    #[expect(clippy::cast_possible_truncation, reason = "land is in f32")]
    let amount = args.get("amount").and_then(Json::as_f64).unwrap_or(1.0) as f32;
    let stroke = engine_map::TerrainStroke::Paint(layer, amount);
    engine_map::stroke_land(&mut data, &land_brush(args)?, stroke);
    keep_land(&data, &file)
}

/// `stamp_terrain`.
pub(crate) fn stamp_terrain(session: &Session, args: &Json) -> Result<Json, String> {
    let name = args
        .get("name")
        .and_then(Json::as_str)
        .ok_or("name is required")?;
    let stamp = args
        .get("stamp")
        .and_then(Json::as_str)
        .ok_or("stamp is required")?;
    let (mut data, file) = land_file(session, name)?;
    let (stamp, _) = land_file(session, stamp)?;
    let brush = land_brush(args)?;
    #[expect(clippy::cast_possible_truncation, reason = "land is in f32")]
    let amount = args.get("amount").and_then(Json::as_f64).unwrap_or(1.0) as f32;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "small counts"
    )]
    match args.get("count").and_then(Json::as_f64) {
        Some(count) => {
            let seed = args.get("seed").and_then(Json::as_f64).unwrap_or(1.0) as u32;
            engine_map::splat_land(&mut data, &stamp, &brush, (count as u32, amount, seed));
        }
        None => engine_map::stamp_land(&mut data, &stamp, &brush, amount),
    }
    keep_land(&data, &file)
}

/// `copy_terrain`.
pub(crate) fn copy_terrain(session: &Session, args: &Json) -> Result<Json, String> {
    let name = args
        .get("name")
        .and_then(Json::as_str)
        .ok_or("name is required")?;
    let (mut data, file) = land_file(session, name)?;
    engine_map::copy_land(&mut data, sheet_place(args, "from")?, &land_brush(args)?);
    keep_land(&data, &file)
}

pub(crate) fn make_levels(session: &Session, args: &Json) -> Result<Json, String> {
    let name = args
        .get("name")
        .and_then(Json::as_str)
        .ok_or("make_levels needs a name")?;
    let file = session.content.join("meshes").join(format!("{name}.emesh"));
    let bytes = std::fs::read(&file).map_err(|error| format!("{}: {error}", file.display()))?;
    let document = engine_compile::document_of(&bytes).map_err(|error| error.to_string())?;
    let mut mesh = engine_compile::read_mesh(&document).map_err(|error| error.to_string())?;
    let levels = engine_compile::add_levels(&mut mesh);
    std::fs::write(&file, engine_compile::write_mesh(&mesh))
        .map_err(|error| format!("{}: {error}", file.display()))?;
    let count = |level: usize| -> usize {
        let at = |part: &engine_compile::MeshPart| match level {
            0 => part.triangles.len(),
            _ => part
                .levels
                .get(level - 1)
                .or(part.levels.last())
                .map_or(part.triangles.len(), Vec::len),
        };
        mesh.parts.iter().map(at).sum()
    };
    let triangles: Vec<usize> = (0..=levels).map(count).collect();
    let points = mesh.parts.iter().flat_map(|part| &part.points);
    let (mut low, mut high) = ([f64::MAX; 3], [f64::MIN; 3]);
    for point in points {
        for axis in 0..3 {
            low[axis] = low[axis].min(point[axis]);
            high[axis] = high[axis].max(point[axis]);
        }
    }
    let size = [high[0] - low[0], high[1] - low[1], high[2] - low[2]];
    Ok(json!({ "mesh": name, "levels": levels, "triangles": triangles, "size": size }))
}

/// `import_gltf`: brings a model in: its mesh, materials and pictures.
pub(crate) fn import_gltf(session: &Session, args: &Json) -> Result<Json, String> {
    let path = args
        .get("path")
        .and_then(Json::as_str)
        .ok_or("import_gltf needs a path")?;
    let name = args
        .get("name")
        .and_then(Json::as_str)
        .ok_or("import_gltf needs a name")?;
    let scale = args.get("scale").and_then(Json::as_f64).unwrap_or(32.0);
    let mut model = engine_compile::import_gltf(Path::new(path), name, scale)
        .map_err(|error| error.to_string())?;
    let levels = engine_compile::add_levels(&mut model.mesh);
    let write = |file: PathBuf, bytes: &[u8]| -> Result<(), String> {
        if let Some(folder) = file.parent() {
            std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
        }
        std::fs::write(&file, bytes).map_err(|error| format!("{}: {error}", file.display()))
    };
    let file = session.content.join("meshes").join(format!("{name}.emesh"));
    write(file, &engine_compile::write_mesh(&model.mesh))?;
    for (material, text) in &model.materials {
        let file = format!("{material}.emat");
        write(
            session.content.join("materials").join(file),
            text.as_bytes(),
        )?;
    }
    for (path, png) in &model.pictures {
        write(session.content.join(path), png)?;
    }
    let triangles: usize = model.mesh.parts.iter().map(|p| p.triangles.len()).sum();
    Ok(json!({
        "mesh": name,
        "levels": levels,
        "parts": model.mesh.parts.len(),
        "triangles": triangles,
        "materials": model.materials.iter().map(|(name, _)| name).collect::<Vec<_>>(),
        "pictures": model.pictures.len(),
    }))
}

/// `check_effects`: what's wrong with effects.
pub(crate) fn check_effects(session: &Session, _args: &Json) -> Result<Json, String> {
    let mut found = serde_json::Map::new();
    for name in names_in(session, "effects", ".eeffect") {
        let path = session
            .content
            .join("effects")
            .join(format!("{name}.eeffect"));
        let bytes = std::fs::read(&path).map_err(|error| format!("{name}: {error}"))?;
        let read = engine_compile::document_of(&bytes)
            .and_then(|document| engine_compile::read_effect(&name, &document));
        if let Err(problem) = read {
            found.insert(name, json!(problem.to_string()));
        }
    }
    Ok(Json::Object(found))
}

/// `check_shaders`: what's wrong with shaders, as the compiler sees them.
pub(crate) fn check_shaders(session: &Session, _args: &Json) -> Result<Json, String> {
    let mut found = serde_json::Map::new();
    for name in names_in(session, "shaders", ".wgsl") {
        let path = session.content.join("shaders").join(format!("{name}.wgsl"));
        let code = std::fs::read_to_string(&path).map_err(|error| format!("{name}: {error}"))?;
        if let Err(problem) = check_code(&code) {
            found.insert(name, json!(problem));
        }
    }
    for name in names_in(session, "shaders", ".egraph") {
        let path = session
            .content
            .join("shaders")
            .join(format!("{name}.egraph"));
        let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
        if let Err(problem) = checked_graph(&text) {
            found.insert(name, json!(problem));
        }
    }
    Ok(Json::Object(found))
}

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

/// A required text argument called `key`.
fn text_of<'a>(args: &'a Json, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Json::as_str)
        .ok_or_else(|| format!("{key} is required"))
}

/// Where the file of `name` goes, in `folder` of the content.
fn file_of(
    session: &Session,
    folder: &str,
    extension: &str,
    name: &str,
) -> Result<PathBuf, String> {
    let name = checked(name)?;
    Ok(session
        .content
        .join(folder)
        .join(format!("{name}{extension}")))
}

/// Writes `text` to `file`, making its folder.
fn write_text(file: &Path, text: &str) -> Result<Json, String> {
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    }
    std::fs::write(file, text).map_err(|error| format!("{}: {error}", file.display()))?;
    Ok(json!({ "path": file.display().to_string() }))
}

/// `read_shader`, `read_effect` and `read_prefab`.
pub(crate) fn read_text(
    session: &Session,
    folder: &str,
    extension: &str,
    args: &Json,
) -> Result<Json, String> {
    let name = text_of(args, "name")?;
    let file = file_of(session, folder, extension, name)?;
    let text = std::fs::read_to_string(&file).map_err(|_| format!("no {name} in {folder}"))?;
    Ok(json!({ "text": text }))
}

/// `write_shader`: a shader that doesn't compile is refused, with what the compiler said.
pub(crate) fn write_shader(session: &Session, args: &Json) -> Result<Json, String> {
    let text = text_of(args, "text")?;
    let file = file_of(session, "shaders", ".wgsl", text_of(args, "name")?)?;
    check_code(text)?;
    write_text(&file, text)
}

/// What's wrong with `code`, as whichever sort of shader it is.
fn check_code(code: &str) -> Result<(), String> {
    if code.contains("fn shader_field") {
        check_field_code(code)
    } else if code.contains("fn shader_post") {
        check_post_code(code)
    } else {
        check_surface_code(code)
    }
}

/// What the graph in `text` comes to as code, which has compiled.
fn checked_graph(text: &str) -> Result<String, String> {
    let graph = engine_compile::read_graph(text)?;
    let code = engine_compile::graph_code(&graph).map_err(|problems| problems.join("; "))?;
    check_code(&code)?;
    Ok(code)
}

/// `write_graph`.
pub(crate) fn write_graph(session: &Session, args: &Json) -> Result<Json, String> {
    let text = text_of(args, "text")?;
    let file = file_of(session, "shaders", ".egraph", text_of(args, "name")?)?;
    checked_graph(text)?;
    write_text(&file, text)
}

/// `describe_graph`.
pub(crate) fn describe_graph(session: &Session, args: &Json) -> Result<Json, String> {
    let file = file_of(session, "shaders", ".egraph", text_of(args, "name")?)?;
    let text =
        std::fs::read_to_string(&file).map_err(|error| format!("{}: {error}", file.display()))?;
    Ok(match checked_graph(&text) {
        Ok(code) => json!({ "ok": true, "code": code }),
        Err(problem) => json!({ "ok": false, "problem": problem }),
    })
}

/// `graph_kinds`.
pub(crate) fn graph_kinds() -> Json {
    let kinds: Vec<Json> = engine_compile::GRAPH_KINDS
        .iter()
        .map(|kind| {
            let inputs: Vec<&str> = kind.inputs.iter().map(|(name, _)| *name).collect();
            json!({
                "name": kind.name,
                "group": kind.group,
                "inputs": inputs,
                "settings": kind.settings,
            })
        })
        .collect();
    let outputs: Vec<&str> = engine_compile::GRAPH_OUTPUTS
        .iter()
        .map(|(name, ..)| *name)
        .collect();
    json!({ "kinds": kinds, "outputs": outputs })
}

/// `write_effect`: an effect that doesn't make sense is refused.
pub(crate) fn write_effect(session: &Session, args: &Json) -> Result<Json, String> {
    let name = text_of(args, "name")?;
    let text = text_of(args, "text")?;
    let file = file_of(session, "effects", ".eeffect", name)?;
    engine_compile::document_of(text.as_bytes())
        .and_then(|document| engine_compile::read_effect(name, &document))
        .map_err(|error| error.to_string())?;
    write_text(&file, text)
}

/// `write_prefab`: a prefab that doesn't build with no parameters is refused, and the old one
/// stays.
pub(crate) fn write_prefab(session: &Session, args: &Json) -> Result<Json, String> {
    let name = text_of(args, "name")?;
    let text = text_of(args, "text")?;
    let file = file_of(session, "prefabs", ".luau", name)?;
    let before = std::fs::read_to_string(&file).ok();
    write_text(&file, text)?;
    if let Err(problem) = build_prefab(&session.content, name, &json!({}), [0.0; 3]) {
        let _put_back = match before {
            Some(old) => std::fs::write(&file, old),
            None => std::fs::remove_file(&file),
        };
        return Err(problem);
    }
    Ok(json!({ "path": file.display().to_string() }))
}

/// `compile_textures`.
pub(crate) fn compile_textures(session: &Session) -> Result<Json, String> {
    let (compiled, skipped) =
        engine_compile::pack_textures(&session.content).map_err(|error| error.to_string())?;
    Ok(json!({ "compiled": compiled, "up_to_date": skipped }))
}

/// `compile_shaders`.
pub(crate) fn compile_all_shaders(session: &Session) -> Result<Json, String> {
    let modules =
        engine_compile::compile_shaders(&session.content).map_err(|error| error.to_string())?;
    Ok(json!({ "modules": modules }))
}

/// `pack`.
pub(crate) fn pack(session: &Session, args: &Json) -> Result<Json, String> {
    let path = Path::new(text_of(args, "path")?);
    let summary =
        engine_compile::pack_content(&session.content, path).map_err(|error| error.to_string())?;
    Ok(json!({
        "files": summary.files,
        "shared": summary.shared,
        "bytes": summary.bytes,
        "stored": summary.stored,
    }))
}
