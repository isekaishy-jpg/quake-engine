//! A level as a game reads it, and a content folder as one pack (proposal 0106).

use std::collections::BTreeSet;
use std::path::Path;

use engine_assets::{Archive, DirectoryArchive, FileSystem, PackSummary, write_pack_of};
use engine_format::Document;
use engine_map::{Map, Texture};

use crate::bake::map_hash;
use crate::build::{TextureSource, compile, compile_known};
use crate::compiled::read_geometry;
use crate::error::CompileError;
use crate::material::{MaterialLibrary, document_of};
use crate::source::read_map;
use crate::texture_pack::{pack_textures, packed_path};

/// A level's textures while it loads: compiled for the GPU, and in pixels for
/// what's read here. The first thing that goes wrong is kept for afterwards.
struct PlayTextures<'a> {
    /// Where they come from.
    library: MaterialLibrary<'a>,
    /// What went wrong.
    failure: Option<CompileError>,
}

impl PlayTextures<'_> {
    /// `result`'s texture, or nothing, with the error kept.
    fn kept(&mut self, result: Result<Texture, CompileError>) -> Texture {
        result.unwrap_or_else(|error| {
            self.failure = Some(error);
            Texture::default()
        })
    }
}

impl TextureSource for PlayTextures<'_> {
    fn texture(&mut self, material: &str) -> Texture {
        let result = self.library.load(material);
        self.kept(result)
    }

    fn pixels(&mut self, material: &str) -> Texture {
        let result = self.library.load_pixels(material);
        self.kept(result)
    }
}

/// A level ready to play.
#[derive(Debug)]
pub struct PlayMap {
    /// The map, without its bake.
    pub map: Map,
    /// The hash a bake has to have.
    pub hash: u64,
    /// The pictures it read as pixels, which a pack has to keep.
    pub pictures: Vec<String>,
}

/// Reads the map called `name` from `document` as a game does: with compiled
/// pictures, and the geometry an earlier compile kept, where they're current.
///
/// # Errors
///
/// Returns an error if the map or something it uses is broken.
pub fn play_map(
    filesystem: &FileSystem,
    name: &str,
    document: &Document,
) -> Result<PlayMap, CompileError> {
    let mut working = read_map(document)?;
    let mut library = MaterialLibrary::for_play(filesystem);
    let used = library.prepare(&mut working)?;
    let hash = map_hash(document, &used);
    let mut textures = PlayTextures {
        library,
        failure: None,
    };
    let kept = filesystem.read(&format!("maps/{name}.egeo")).ok();
    let map = match kept.and_then(|bytes| read_geometry(&bytes, hash)) {
        Some(geometry) => compile_known(&working, &mut textures, geometry)?,
        None => compile(&working, &mut textures)?,
    };
    if let Some(error) = textures.failure {
        return Err(error);
    }
    let pictures = textures.library.pixel_pictures();
    Ok(PlayMap {
        map,
        hash,
        pictures,
    })
}

/// Whether a game has any use for the file at `path`. What's only for making
/// content stays behind, as does a text map with its binary form beside it,
/// and a picture that's compiled and that no level reads as pixels.
fn shipped(path: &str, all: &BTreeSet<String>, pixels: &BTreeSet<String>) -> bool {
    if path.ends_with(".py") || path.ends_with(".json") {
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

/// Writes the content at `root` to the pack at `out`, with its pictures
/// compiled.
///
/// # Errors
///
/// Returns an error if something can't be read, a map is broken, or writing
/// fails.
pub fn pack_content(root: &Path, out: &Path) -> Result<PackSummary, CompileError> {
    let unread = |error: engine_assets::AssetError| CompileError::Bad(error.to_string());
    pack_textures(root)?;
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
    let failed = |error: std::io::Error| CompileError::Bad(format!("the pack: {error}"));
    let mut file = std::fs::File::create(out).map_err(failed)?;
    write_pack_of(&archive, wanted, &mut file).map_err(unread)
}
