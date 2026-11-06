//! Operations for what surfaces and worlds can do beyond brushes and entities:
//! meshes, decals, stamps, paint, splines, and a map's own script and materials
//! (proposal 0091).

use engine_shaders::{check_post_code, check_surface_code};
use std::path::{Path, PathBuf};

use engine_compile::{DeriveSettings, Repeat, check_picture, decode_png, derive_maps, encode_png};
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
    let model = engine_compile::import_gltf(Path::new(path), name, scale)
        .map_err(|error| error.to_string())?;
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
        let checked = if code.contains("fn shader_post") {
            check_post_code(&code)
        } else {
            check_surface_code(&code)
        };
        if let Err(problem) = checked {
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
