//! Reading materials from a container.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use engine_assets::{DirectoryArchive, FileSystem};
use engine_compile::{MaterialLibrary, compile, placeholder_texture, read_map_text};
use engine_core::math::Vec3;
use engine_map::{AlphaMode, Material, MipLevel, PAINT_MASK, Shading, Texture, TextureKind};

/// A one pixel PNG.
fn pixel() -> Vec<u8> {
    colored(&[1, 2, 3, 255])
}

/// A one pixel PNG of `color`.
fn colored(color: &[u8; 4]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("a header");
    writer.write_image_data(color).expect("a pixel");
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

/// Flattening mixes a masked layer into the base, so nothing's left to blend,
/// and leaves a painted layer as it is.
#[test]
fn flattening_mixes_layers_into_the_base() {
    let text = "format 1 material\nsurface {\n    base = textures/red.png\n    flatten = true\n\
        layer { base = textures/blue.png mask = textures/half.png }\n\
        layer { base = textures/blue.png mask = paint }\n}\n";
    let (filesystem, root) = container("flatten", &[("mixed", text)]);
    fs::write(root.join("textures/red.png"), colored(&[255, 0, 0, 255])).expect("write");
    fs::write(root.join("textures/blue.png"), colored(&[0, 0, 255, 255])).expect("write");
    fs::write(
        root.join("textures/half.png"),
        colored(&[128, 128, 128, 255]),
    )
    .expect("write");
    let mut library = MaterialLibrary::new(&filesystem);
    let texture = library.load("mixed").expect("loads");
    let material = texture.material.expect("a material");
    assert_eq!(material.layers.len(), 1, "only the painted layer is left");
    assert_eq!(material.layers[0].mask, PAINT_MASK);
    let base = &texture.mips[0].rgba;
    assert!(
        base[0].abs_diff(127) < 3 && base[2].abs_diff(128) < 3,
        "{base:?}"
    );
    let _gone = fs::remove_dir_all(root);
}

/// A tile's things land in place on the grid, and what it holds is in the text
/// a bake's hash covers.
#[test]
fn tiles_merge_into_place() {
    let (filesystem, root) = container("tiles", &[]);
    fs::create_dir_all(root.join("maps")).expect("directory");
    let tile = "format 1 tile\n\
        brush { material = floor cuboid = 0 0 -32 512 512 0 }\n\
        entity lamp { type = light origin = 100 200 64 }\n\
        mesh { source = none origin = 0 0 0 }\n";
    fs::write(root.join("maps/part.etile"), tile).expect("write");
    let mut text = String::from("format 1 map\ntile_grid { size = 1024 }\n");
    text.push_str("tile { x = 2 y = -1 file = part }\n");
    let mut working = read_map_text(&text).expect("reads");
    working.tiles[0].file = "part".to_owned();
    let mut library = MaterialLibrary::new(&filesystem);
    let missing = library.prepare(&mut working);
    assert!(missing.is_err(), "the mesh isn't there");
    let tile = tile.replace("mesh { source = none origin = 0 0 0 }\n", "");
    fs::write(root.join("maps/part.etile"), &tile).expect("write");
    let mut working = read_map_text(&text).expect("reads");
    let used = library.prepare(&mut working).expect("merges");
    assert!(used.contains("lamp"), "{used}");
    let lamp = working
        .entities
        .iter()
        .find(|e| e.kind == "light")
        .expect("the lamp");
    let origin = lamp
        .fields
        .iter()
        .find(|(key, _)| key == "origin")
        .and_then(|(_, value)| value.numbers())
        .expect("a place");
    assert_eq!(origin, vec![2148.0, -824.0, 64.0]);
    let mut source = |name: &str| placeholder_texture(name);
    let map = compile(&working, &mut source).expect("compiles");
    assert!((map.tile_size - 1024.0).abs() < 0.001);
    assert!(
        (map.wake_reach - 1536.0).abs() < 0.001,
        "a tile and a half by default"
    );
    let world = &map.models[0];
    assert!((world.mins.x - 2048.0).abs() < 0.01, "{:?}", world.mins);
    let (tile, local) = map.tile_of(Vec3::new(2148.0, -824.0, 64.0));
    assert_eq!(tile, [2, -1]);
    assert_eq!(local, Vec3::new(100.0, 200.0, 64.0));
    let _gone = fs::remove_dir_all(root);
}

/// A tile too far out is refused, since positions are `f32`.
#[test]
fn a_tile_too_far_out_is_refused() {
    let (filesystem, root) = container("far", &[]);
    let text = "format 1 map\ntile_grid { size = 4096 }\ntile { x = 40 y = 0 file = part }\n";
    let mut working = read_map_text(text).expect("reads");
    let mut library = MaterialLibrary::new(&filesystem);
    let error = library.prepare(&mut working).expect_err("too far");
    assert!(error.to_string().contains("past"), "{error}");
    let _gone = fs::remove_dir_all(root);
}

/// A material can say how much of the world a repeat covers, so that its pictures
/// can change size without a surface changing size.
#[test]
fn world_size_sets_how_big_a_repeat_is() {
    let texture = Texture {
        material: Some(Arc::new(Material {
            world_size: Some([256.0, 128.0]),
            ..Material::default()
        })),
        mips: vec![MipLevel {
            width: 1024,
            height: 1024,
            rgba: Vec::new(),
            form: engine_map::PixelForm::Rgba,
        }],
        ..Texture::default()
    };
    assert_eq!(texture.tile_size(), Some([256.0, 128.0]));
    let plain = Texture {
        material: None,
        ..texture
    };
    assert_eq!(plain.tile_size(), Some([1024.0, 1024.0]));
}
