//! Compiling shaders and packing content, for a game to read (proposals 0106, 0131 to 0133).

use std::collections::BTreeSet;
use std::path::Path;

use engine_assets::{Archive, DirectoryArchive, FileIds, FileSystem, PackSummary, write_pack_of};

use crate::content_pack::play_map;
use crate::error::CompileError;
use crate::material::document_of;
use crate::shader_graph::{graph_code, read_graph};
use crate::texture_pack::{pack_textures, packed_path};

/// Whether a game has any use for the file at `path`. What's only for making
/// content stays behind, as does a text map with its binary form beside it,
/// and a picture that's compiled and that no level reads as pixels.
fn shipped(path: &str, all: &BTreeSet<String>, pixels: &BTreeSet<String>) -> bool {
    if path.ends_with(".py") || path.ends_with(".json") || path == FILE_ID_TABLE {
        return false;
    }
    if let Some(stem) = path.strip_suffix(".emap") {
        return !all.contains(&format!("{stem}.ebin"));
    }
    !path.ends_with(".png") || pixels.contains(path) || !all.contains(&packed_path(path))
}

/// The name of the map at `path`, unless a text form of it is there to use.
fn map_at<'a>(path: &'a str, all: &BTreeSet<String>) -> Option<&'a str> {
    let file = path.strip_prefix("maps/")?;
    if let Some(name) = file.strip_suffix(".emap") {
        return Some(name);
    }
    let name = file.strip_suffix(".ebin")?;
    (!all.contains(&format!("maps/{name}.emap"))).then_some(name)
}

/// The table of file ids, in the content.
const FILE_ID_TABLE: &str = "fids.txt";

/// Compiles every shader a game could want, into `compiled/shaders` under `root`, and says
/// how many. A game built without a compiler reads nothing else.
///
/// # Errors
///
/// Returns what's wrong with a shader, or why it can't be written.
pub fn compile_shaders(root: &Path) -> Result<usize, CompileError> {
    let failed = |error: std::io::Error| CompileError::Bad(format!("shaders: {error}"));
    let mut codes = Vec::new();
    if let Ok(folder) = std::fs::read_dir(root.join("shaders")) {
        for file in folder {
            let path = file.map_err(failed)?.path();
            if path.extension().is_some_and(|kind| kind == "wgsl") {
                codes.push(std::fs::read_to_string(path).map_err(failed)?);
            } else if path.extension().is_some_and(|kind| kind == "egraph") {
                // A WGSL file of the same name goes ahead of it, so this one's not used.
                if !path.with_extension("wgsl").exists() {
                    let text = std::fs::read_to_string(&path).map_err(failed)?;
                    let made = read_graph(&text)
                        .map_err(|problem| vec![problem])
                        .and_then(|graph| graph_code(&graph));
                    let bad = |problems: Vec<String>| {
                        CompileError::Bad(format!("{}: {}", path.display(), problems.join("; ")))
                    };
                    codes.push(made.map_err(bad)?);
                }
            }
        }
    }
    let (particles, rest): (Vec<&str>, Vec<&str>) = codes
        .iter()
        .map(String::as_str)
        .partition(|code| code.contains("fn shader_particle"));
    let (fields, rest): (Vec<&str>, Vec<&str>) = rest
        .into_iter()
        .partition(|code| code.contains("fn shader_field"));
    let (posts, surfaces): (Vec<&str>, Vec<&str>) = rest
        .into_iter()
        .partition(|code| code.contains("fn shader_post"));
    let out = root.join("compiled").join("shaders");
    if out.exists() {
        std::fs::remove_dir_all(&out).map_err(failed)?;
    }
    std::fs::create_dir_all(&out).map_err(failed)?;
    let modules = engine_shaders::all_modules(&surfaces, &posts, &fields, &particles);
    for module in &modules {
        let words = engine_shaders::compile(module)
            .map_err(|error| CompileError::Bad(format!("{}: {error}", module.path())))?;
        std::fs::write(
            root.join(module.path()),
            engine_shaders::compiled_file(&words),
        )
        .map_err(failed)?;
    }
    Ok(modules.len())
}

/// Writes the content at `root` to the container at `out`, with its pictures compiled.
/// New files get ids in the table beside the content, which is kept in the repository.
///
/// # Errors
///
/// Returns an error if something can't be read, a map is broken, or writing
/// fails.
pub fn pack_content(root: &Path, out: &Path) -> Result<PackSummary, CompileError> {
    let unread = |error: engine_assets::AssetError| CompileError::Bad(error.to_string());
    pack_textures(root)?;
    let _compiled = compile_shaders(root)?;
    let archive = DirectoryArchive::new(root);
    let mut filesystem = FileSystem::new();
    filesystem.mount(Box::new(DirectoryArchive::new(root)));
    let all: BTreeSet<String> = archive.paths().into_iter().collect();
    let mut pixels = BTreeSet::new();
    for path in &all {
        if let Some(name) = map_at(path, &all) {
            let document = document_of(&archive.read(path).map_err(unread)?)?;
            pixels.extend(play_map(&filesystem, name, &document)?.pictures);
        }
    }
    let wanted = all
        .iter()
        .filter(|path| shipped(path, &all, &pixels))
        .cloned()
        .collect();
    let failed = |error: std::io::Error| CompileError::Bad(format!("the container: {error}"));
    let table = root.join(FILE_ID_TABLE);
    let mut ids = match std::fs::read_to_string(&table) {
        Ok(text) => FileIds::parse(&text).map_err(unread)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => FileIds::default(),
        Err(error) => return Err(failed(error)),
    };
    let mut file = std::fs::File::create(out).map_err(failed)?;
    let summary = write_pack_of(&archive, wanted, &mut ids, &mut file).map_err(unread)?;
    std::fs::write(&table, ids.to_text()).map_err(failed)?;
    Ok(summary)
}
