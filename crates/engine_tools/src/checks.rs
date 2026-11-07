//! What's wrong with a map, past what it takes to compile (proposal 0137). A map can compile
//! and still do nothing where it was meant to do something, which an agent can't see.

use std::path::Path;

use engine_format::{Block, Value};

use crate::session::Session;

/// The blocks a map can hold, and the fields of those that have a set of them. An empty list
/// is a block whose fields aren't a fixed set.
const BLOCKS: &[(&str, &[&str])] = &[
    ("brush", &[]),
    ("entity", &[]),
    ("mesh", &[]),
    ("sky", &[]),
    ("spline", &[]),
    ("tile", &[]),
    ("stamp", &[]),
    ("paint", &[]),
    ("decal", &[]),
    ("tile_grid", &["size", "id"]),
    ("fog", &["color", "density", "height", "falloff", "id"]),
    (
        "water",
        &[
            "color",
            "absorption",
            "caustics",
            "reflect",
            "waves",
            "refraction",
            "tint",
            "tint_strength",
            "id",
        ],
    ),
];

/// Whether a map can hold a block of `kind`.
pub(crate) fn is_block_kind(kind: &str) -> bool {
    BLOCKS.iter().any(|(known, _)| *known == kind)
}

/// Whether `path` under the content is there.
fn has(session: &Session, path: &str) -> bool {
    session.content.join(path).is_file()
}

/// What's wrong with a brush's shape.
fn brush_problems(block: &Block, problems: &mut Vec<String>) {
    let Some(numbers) = block.get("cuboid").and_then(Value::list) else {
        return;
    };
    let values: Vec<f64> = numbers.iter().filter_map(Value::number).collect();
    if values.len() == 6 && (0..3).any(|axis| values[axis] >= values[axis + 3]) {
        problems.push(format!("a brush at {values:?} is inside out"));
    }
}

/// What's wrong with an entity the engine itself knows.
fn entity_problems(session: &Session, block: &Block, problems: &mut Vec<String>) {
    let name = block.name.as_deref().unwrap_or("an entity");
    let text = |key: &str| block.get(key).and_then(Value::text);
    match text("type") {
        Some("effect") => match text("effect") {
            None => problems.push(format!("{name} is an effect with no `effect`")),
            Some(effect) if !has(session, &format!("effects/{effect}.eeffect")) => {
                problems.push(format!("{name}: no effect called {effect}"));
            }
            Some(_) => {}
        },
        Some("zone") => {
            if !block.children.iter().any(|child| child.kind == "brush") {
                problems.push(format!("{name} is a zone with no shape: give it a brush"));
            }
            if let Some(music) = text("music")
                && music.contains('.')
                && !has(session, music)
            {
                problems.push(format!("{name}: no music at {music}"));
            }
        }
        Some("camera") => {
            if let Some(path) = text("path")
                && !session
                    .document
                    .blocks_of("spline")
                    .any(|spline| spline.name.as_deref() == Some(path))
            {
                problems.push(format!("{name}: no spline called {path}"));
            }
        }
        None => problems.push(format!("{name} has no `type`")),
        Some(_) => {}
    }
}

/// What's wrong with a mesh's placement.
fn mesh_problems(session: &Session, block: &Block, problems: &mut Vec<String>) {
    if let Some(source) = block.get("source").and_then(Value::text)
        && !has(session, &format!("meshes/{source}.emesh"))
    {
        problems.push(format!("no mesh called {source}"));
    }
}

/// Everything wrong with `session`'s map that compiling doesn't say.
pub(crate) fn problems_in(session: &Session) -> Vec<String> {
    let mut problems = Vec::new();
    for block in &session.document.blocks {
        let Some((_, fields)) = BLOCKS.iter().find(|(kind, _)| *kind == block.kind) else {
            problems.push(format!("a map has no block called {}", block.kind));
            continue;
        };
        if !fields.is_empty() {
            for (field, _) in &block.fields {
                if !fields.contains(&field.as_str()) {
                    problems.push(format!("{} has no field {field}", block.kind));
                }
            }
        }
        match block.kind.as_str() {
            "brush" => brush_problems(block, &mut problems),
            "entity" => entity_problems(session, block, &mut problems),
            "mesh" => mesh_problems(session, block, &mut problems),
            _ => {}
        }
    }
    problems
}

/// `name` as a mesh's name, from what an agent may have given.
pub(crate) fn mesh_name(source: &str) -> &str {
    let file = Path::new(source)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(source);
    file.strip_suffix(".emesh").unwrap_or(file)
}
