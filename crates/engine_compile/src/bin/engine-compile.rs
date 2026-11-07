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
    BakeInput, BakeLevel, BakeQuality, MaterialLibrary, bake_files, bake_is_for, bake_light_as,
    compile, map_hash, occluders_of, pack_content, pack_textures, read_map, write_geometry,
    write_occlusion,
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

/// What the arguments say of a bake, which is `None` for no bake at all.
fn bake_wanted(
    arguments: &mut std::iter::Peekable<impl Iterator<Item = String>>,
) -> Result<Option<BakeQuality>, String> {
    let mut quality = BakeQuality::default();
    if arguments.next_if(|first| first == "--bake").is_some() {
        match arguments.next().as_deref() {
            Some("quick") => quality.level = BakeLevel::Quick,
            Some("usual") => quality.level = BakeLevel::Usual,
            Some("final") => quality.level = BakeLevel::Final,
            Some("none") => return Ok(None),
            other => {
                return Err(format!(
                    "--bake is quick, usual, final or none, not {other:?}"
                ));
            }
        }
    }
    if arguments.next_if(|first| first == "--area").is_some() {
        if quality.level != BakeLevel::Final {
            return Err("--area is for a final bake".to_owned());
        }
        let mut number = || -> Result<f64, String> {
            let text = arguments.next().ok_or("--area needs six numbers")?;
            text.parse().map_err(|_| format!("{text} isn't a number"))
        };
        let low = [number()?, number()?, number()?];
        let high = [number()?, number()?, number()?];
        quality.area = Some([low, high]);
    }
    Ok(Some(quality))
}

/// Compiles one map.
fn compile_map(
    content: &Path,
    filesystem: &FileSystem,
    name: &str,
    bake: Option<BakeQuality>,
) -> Result<(), Box<dyn Error>> {
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
    let missing = library.missing_pictures();
    if !missing.is_empty() {
        eprintln!(
            "{name}: placeholders for {} materials with no pictures here",
            missing.len()
        );
    }
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
    if let Some(quality) = bake {
        bake_light_as(&mut map, &mut input, quality)?;
    }
    // The bake was all that needed a copy's triangles.
    map.drop_copy_faces();
    let kept = content.join("maps").join(format!("{name}.egeo"));
    std::fs::write(&kept, write_geometry(&map, hash))?;
    match bake {
        Some(quality) => {
            for (file, bytes) in bake_files(&map, name, hash, quality.level) {
                let out = content.join(file);
                if let Some(folder) = out.parent() {
                    std::fs::create_dir_all(folder)?;
                }
                std::fs::write(&out, bytes)?;
            }
        }
        // An old bake would only say that it's out of date.
        None => {
            let _gone = std::fs::remove_file(content.join("maps").join(format!("{name}.ebake")));
        }
    }
    let finished = content.join("maps").join(format!("{name}.efinal"));
    if std::fs::read(finished).is_ok_and(|bytes| !bake_is_for(&bytes, hash)) {
        println!("{name}: its final bake is out of date, and a build's is used");
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
        eprintln!("       engine-compile <content directory> --bake <how> <name> ...");
        eprintln!("         how is quick, usual, final or none");
        eprintln!(
            "       engine-compile <content directory> --bake final --area <low> <high> <name>"
        );
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
    let bake = match bake_wanted(&mut arguments) {
        Ok(bake) => bake,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    for name in arguments {
        let done = if sidecars {
            write_sidecar(content, &filesystem, &name)
        } else {
            compile_map(content, &filesystem, &name, bake)
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
