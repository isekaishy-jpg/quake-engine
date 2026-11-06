//! Compiles maps from the command line: `engine-compile <content> <map> ...`.
//!
//! Writes `maps/<map>.ebake` and the binary `maps/<map>.ebin` in the content
//! directory. With `--occlusion`
//! first, the names are meshes, and it writes `meshes/<mesh>.eocc`.

#![expect(clippy::print_stdout, reason = "this tool's output is its report")]
#![expect(clippy::print_stderr, reason = "this tool reports its failures")]

use std::error::Error;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use engine_assets::{DirectoryArchive, FileSystem};
use engine_compile::{
    BakeInput, MaterialLibrary, bake_folder, bake_light, compile, map_hash, occluders_of, read_map,
    side_files, write_bake, write_geometry, write_occlusion,
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
    let kept = content.join("maps").join(format!("{name}.egeo"));
    std::fs::write(&kept, write_geometry(&map, hash))?;
    let mut library = MaterialLibrary::new(filesystem);
    let mut images = |path: &str| -> Result<Arc<Vec<MipLevel>>, engine_compile::CompileError> {
        library.image_at(path)
    };
    let mut input = BakeInput {
        working: &working,
        images: &mut images,
    };
    bake_light(&mut map, &mut input)?;
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
        return ExitCode::FAILURE;
    };
    let content = Path::new(&content);
    let mut filesystem = FileSystem::new();
    filesystem.mount(Box::new(DirectoryArchive::new(content)));
    let mut failed = false;
    let mut arguments = arguments.peekable();
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
