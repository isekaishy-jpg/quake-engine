//! Compiles maps from the command line: `engine-compile <content> <map> ...`.
//!
//! Writes `maps/<map>.ebake` in the content directory.

#![expect(clippy::print_stdout, reason = "this tool's output is its report")]
#![expect(clippy::print_stderr, reason = "this tool reports its failures")]

use std::error::Error;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use engine_assets::{DirectoryArchive, FileSystem};
use engine_compile::{
    BakeInput, MaterialLibrary, bake_light, compile, read_map, source_hash, write_bake,
};
use engine_format::from_text;
use engine_map::{MipLevel, Texture};

/// Compiles one map.
fn compile_map(content: &Path, filesystem: &FileSystem, name: &str) -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    let path = format!("maps/{name}.emap");
    let bytes = filesystem.read(&path)?;
    let text = std::str::from_utf8(&bytes)?;
    let working = read_map(&from_text(text)?)?;
    let mut library = MaterialLibrary::new(filesystem);
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
    std::fs::write(&out, write_bake(&map, source_hash(text)))?;
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
        eprintln!("usage: engine-compile <content directory> <map> ...");
        return ExitCode::FAILURE;
    };
    let content = Path::new(&content);
    let mut filesystem = FileSystem::new();
    filesystem.mount(Box::new(DirectoryArchive::new(content)));
    let mut failed = false;
    for name in arguments {
        if let Err(error) = compile_map(content, &filesystem, &name) {
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
