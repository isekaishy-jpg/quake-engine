//! Compiles maps from the command line: `engine-compile <content> <map> ...`.
//!
//! Writes `maps/<map>.ebake` and the binary `maps/<map>.ebin` in the content
//! directory. With `--occlusion`
//! first, the names are meshes, and it writes `meshes/<mesh>.eocc`. With
//! `--textures` it compiles the pictures materials use, and with `--pack <file>`
//! it does that and writes the whole content folder as one pack.

#![expect(clippy::print_stdout, reason = "this tool's output is its report")]
#![expect(clippy::print_stderr, reason = "this tool reports its failures")]

use std::error::Error;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use engine_assets::{DirectoryArchive, FileSystem};
use engine_compile::{
    BakeInput, MaterialLibrary, bake_folder, bake_light, compile, map_hash, occluders_of,
    pack_content, pack_textures, read_map, side_files, write_bake, write_geometry, write_occlusion,
};
use engine_format::{from_text, to_binary};
use engine_map::{MipLevel, Texture};

/// Writes the occlusion sidecar of the mesh called `name`.
fn write_sidecar(
    content: &Path,
    filesystem: &FileSystem,
    name: &str,
) -> Result<(), Box<dyn Error>> {
    let mut library = MaterialLibrary::new(filesystem);
    let loaded = library.mesh(name)?;
    let spheres = occluders_of(&loaded.mesh);
    let out = content.join("meshes").join(format!("{name}.eocc"));
    std::fs::write(&out, write_occlusion(&spheres))?;
    println!("{name}: {} spheres", spheres.len());
    Ok(())
}

/// Compiles one map.
fn compile_map(content: &Path, filesystem: &FileSystem, name: &str) -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    let path = format!("maps/{name}.emap");
    let bytes = filesystem.read(&path)?;
    let text = std::str::from_utf8(&bytes)?;
    let document = from_text(text)?;
    let mut working = read_map(&document)?;
    let mut library = MaterialLibrary::new(filesystem);
    let used = library.prepare(&mut working)?;
    let mut failure = None;
    let mut textures = |material: &str| {
        library.load(material).unwrap_or_else(|error| {
            failure = Some(error);
            Texture::default()
        })
    };
    let mut map = compile(&working, &mut textures)?;
    if let Some(error) = failure {
        return Err(error.into());
    }
    let hash = map_hash(&document, &used);
    let mut library = MaterialLibrary::new(filesystem);
    let mut images = |path: &str| -> Result<Arc<Vec<MipLevel>>, engine_compile::CompileError> {
        library.image_at(path)
    };
    let mut input = BakeInput {
        working: &working,
        images: &mut images,
    };
    bake_light(&mut map, &mut input)?;
    // The bake was all that needed a copy's triangles.
    map.drop_copy_faces();
    let kept = content.join("maps").join(format!("{name}.egeo"));
    std::fs::write(&kept, write_geometry(&map, hash))?;
    let out = content.join("maps").join(format!("{name}.ebake"));
    std::fs::write(&out, write_bake(&map, hash))?;
    let beside = side_files(&map);
    if !beside.is_empty() {
        let folder = content.join(bake_folder(name));
        std::fs::create_dir_all(&folder)?;
        for (file, bytes) in beside {
            std::fs::write(folder.join(file), bytes)?;
        }
    }
    // What ships: the same document, in binary, which plays with no text.
    let binary = content.join("maps").join(format!("{name}.ebin"));
    std::fs::write(&binary, to_binary(&document))?;
    println!(
        "{name}: {} faces, {} light samples, in {:.1} s",
        map.faces.len(),
        map.light.len(),
        started.elapsed().as_secs_f32()
    );
    Ok(())
}

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let Some(content) = arguments.next() else {
        eprintln!("usage: engine-compile <content directory> [--occlusion] <name> ...");
        eprintln!("       engine-compile <content directory> --textures");
        eprintln!("       engine-compile <content directory> --pack <file>");
        return ExitCode::FAILURE;
    };
    let content = Path::new(&content);
    let mut arguments = arguments.peekable();
    if arguments.next_if(|first| first == "--textures").is_some() {
        return match pack_textures(content) {
            Ok((compiled, all)) => {
                println!("compiled {compiled} of {all} pictures");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("textures: {error}");
                ExitCode::FAILURE
            }
        };
    }
    if arguments.next_if(|first| first == "--pack").is_some() {
        let Some(out) = arguments.next() else {
            eprintln!("--pack needs a file to write");
            return ExitCode::FAILURE;
        };
        return match pack_content(content, Path::new(&out)) {
            Ok(summary) => {
                println!(
                    "packed {} files, {} of them shared, {} bytes down to {}",
                    summary.files, summary.shared, summary.bytes, summary.stored
                );
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("pack: {error}");
                ExitCode::FAILURE
            }
        };
    }
    let mut filesystem = FileSystem::new();
    filesystem.mount(Box::new(DirectoryArchive::new(content)));
    let mut failed = false;
    let sidecars = arguments.next_if(|first| first == "--occlusion").is_some();
    for name in arguments {
        let done = if sidecars {
            write_sidecar(content, &filesystem, &name)
        } else {
            compile_map(content, &filesystem, &name)
        };
        if let Err(error) = done {
            eprintln!("{name}: {error}");
            failed = true;
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
