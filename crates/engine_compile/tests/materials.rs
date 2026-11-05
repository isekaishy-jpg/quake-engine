//! Reading materials from a container.

use std::fs;
use std::path::PathBuf;

use engine_assets::{DirectoryArchive, FileSystem};
use engine_compile::MaterialLibrary;
use engine_map::{AlphaMode, Shading, TextureKind};

/// A one pixel PNG.
fn pixel() -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("a header");
    writer.write_image_data(&[1, 2, 3, 255]).expect("a pixel");
    writer.finish().expect("finished");
    bytes
}

/// A container with `images` PNGs and these materials.
fn container(name: &str, materials: &[(&str, &str)]) -> (FileSystem, PathBuf) {
    let root = std::env::temp_dir().join(format!("engine-mat-{name}-{}", std::process::id()));
    let _gone = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("textures")).expect("directory");
    fs::create_dir_all(root.join("materials")).expect("directory");
    for index in 0..10 {
        fs::write(root.join("textures").join(format!("{index}.png")), pixel()).expect("write");
    }
    for (name, text) in materials {
        fs::write(root.join("materials").join(format!("{name}.emat")), text).expect("write");
    }
    let mut filesystem = FileSystem::new();
    filesystem.mount(Box::new(DirectoryArchive::new(&root)));
    (filesystem, root)
}

#[test]
fn a_material_fills_its_slots_and_shares_repeats() {
    let text = "format 1 material\nsurface {\n base = textures/0.png\n normal = textures/1.png\n specular = textures/2.png\n height = textures/1.png\n shading = cel\n alpha = mask\n gloss = 12\n layer { base = textures/3.png mask = textures/4.png }\n}\n";
    let (filesystem, root) = container("fill", &[("wall", text)]);
    let mut library = MaterialLibrary::new(&filesystem);
    let texture = library.load("wall").expect("loads");
    let material = texture.material.expect("a material");
    assert_eq!(material.shading, Shading::Cel);
    assert_eq!(material.alpha, AlphaMode::Mask);
    assert_eq!(material.gloss, 12.0);
    assert_eq!(material.normal, Some(1));
    assert_eq!(material.specular, Some(2));
    assert_eq!(material.height, Some(1), "the same image takes one slot");
    assert_eq!(material.layers.len(), 1);
    assert_eq!(material.slot_count(), 5);
    assert_eq!(texture.kind, TextureKind::Solid);
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn too_many_images_are_refused() {
    let text = "format 1 material\nsurface {\n base = textures/0.png\n normal = textures/1.png\n specular = textures/2.png\n emissive = textures/3.png\n height = textures/4.png\n tint = textures/5.png\n matcap = textures/6.png\n ramp = textures/7.png\n detail = textures/8.png\n}\n";
    let (filesystem, root) = container("many", &[("wall", text)]);
    let error = MaterialLibrary::new(&filesystem)
        .load("wall")
        .expect_err("too many");
    assert!(error.to_string().contains("more than 8"), "{error}");
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_missing_material_is_a_placeholder_and_a_broken_one_an_error() {
    let cases = [
        ("no_base", "format 1 material\nsurface { gloss = 1 }\n"),
        (
            "no_image",
            "format 1 material\nsurface { base = textures/none.png }\n",
        ),
        (
            "bad_mode",
            "format 1 material\nsurface { base = textures/0.png shading = odd }\n",
        ),
    ];
    let (filesystem, root) = container("gaps", &cases);
    let mut library = MaterialLibrary::new(&filesystem);
    let placeholder = library.load("absent").expect("a stand-in");
    assert!(placeholder.material.is_none());
    assert!(!placeholder.mips.is_empty());
    for (name, _) in cases {
        assert!(library.load(name).is_err(), "{name}");
    }
    fs::remove_dir_all(&root).expect("clean up");
}
