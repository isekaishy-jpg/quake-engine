//! Compiled pictures, and a content folder as one pack.

use std::fs;
use std::path::{Path, PathBuf};

use engine_assets::{Archive, DirectoryArchive, FileSystem, PackArchive};
use engine_compile::{
    MaterialLibrary, Role, bytes_hash, encode_png, pack_content, pack_picture, pack_textures,
    packed_path, picture_roles, play_map, read_packed,
};
use engine_format::from_text;
use engine_map::PixelForm;

/// A picture of `side` pixels a side, with something in it.
fn picture(side: u32) -> Vec<u8> {
    (0..side * side)
        .flat_map(|at| {
            let (x, y) = (at % side, at / side);
            [
                u8::try_from(x * 255 / side).expect("a byte"),
                u8::try_from(y * 255 / side).expect("a byte"),
                128,
                255,
            ]
        })
        .collect()
}

/// A flat normal map of `side` pixels a side.
fn flat_normals(side: u32) -> Vec<u8> {
    [128_u8, 128, 255, 255].repeat((side * side) as usize)
}

/// A fresh folder for `name`, with the folders content has.
fn folder(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("engine-packing-{name}-{}", std::process::id()));
    let _gone = fs::remove_dir_all(&root);
    for inner in ["textures", "materials", "maps"] {
        fs::create_dir_all(root.join(inner)).expect("directory");
    }
    root
}

/// Writes a 16 pixel PNG to `path` under `root`.
fn write_picture(root: &Path, path: &str, rgba: &[u8]) {
    let bytes = encode_png(16, 16, rgba).expect("encodes");
    fs::write(root.join(path), bytes).expect("write");
}

/// Writes a material.
fn write_material(root: &Path, name: &str, text: &str) {
    let path = root.join("materials").join(format!("{name}.emat"));
    fs::write(path, format!("format 1 material\nsurface {{\n{text}\n}}\n")).expect("write");
}

/// A file system over `root`.
fn mounted(root: &Path) -> FileSystem {
    let mut filesystem = FileSystem::new();
    filesystem.mount(Box::new(DirectoryArchive::new(root)));
    filesystem
}

#[test]
fn a_compiled_picture_has_every_mip_in_blocks() {
    let rgba = picture(20);
    let bytes = pack_picture(20, 20, &rgba, Role::Colour, 99).expect("packs");
    let (hash, mips) = read_packed(&bytes).expect("reads");
    assert_eq!(hash, 99);
    let sizes: Vec<_> = mips.iter().map(|mip| (mip.width, mip.height)).collect();
    assert_eq!(sizes, [(20, 20), (10, 10), (5, 5), (2, 2), (1, 1)]);
    for mip in &mips {
        assert_eq!(mip.form, PixelForm::Bc7Srgb);
        let blocks = mip.width.div_ceil(4) * mip.height.div_ceil(4);
        assert_eq!(mip.rgba.len(), blocks as usize * 16, "16 bytes a block");
    }
}

#[test]
fn each_role_has_its_own_form() {
    let form = |role, rgba: &[u8]| {
        let bytes = pack_picture(16, 16, rgba, role, 0).expect("packs");
        read_packed(&bytes).expect("reads").1[0].form
    };
    assert_eq!(form(Role::Colour, &picture(16)), PixelForm::Bc7Srgb);
    assert_eq!(form(Role::Data, &picture(16)), PixelForm::Bc7);
    assert_eq!(form(Role::Normal, &flat_normals(16)), PixelForm::Bc5);
}

#[test]
fn something_else_isnt_a_compiled_picture() {
    assert!(read_packed(b"nonsense").is_none());
}

#[test]
fn a_materials_pictures_have_their_roles() {
    let text = "format 1 material\nsurface {\n base = a.png\n normal = b.png\n specular = c.png\n emissive = d.png\n layer { base = e.png mask = f.png }\n}\n";
    let roles = picture_roles(&from_text(text).expect("reads"));
    let role = |name: &str| {
        let found = roles.iter().find(|(path, _)| path == name);
        found.expect("listed").1
    };
    assert_eq!(role("a.png"), Role::Colour);
    assert_eq!(role("b.png"), Role::Normal);
    assert_eq!(role("c.png"), Role::Data);
    assert_eq!(role("d.png"), Role::Colour);
    assert_eq!(role("e.png"), Role::Colour);
    assert_eq!(role("f.png"), Role::Data);
}

#[test]
fn a_game_reads_compiled_pictures_and_a_bake_reads_pixels() {
    let root = folder("play");
    write_picture(&root, "textures/wall.png", &picture(16));
    write_picture(&root, "textures/wall_normal.png", &flat_normals(16));
    write_material(
        &root,
        "wall",
        "base = textures/wall.png\nnormal = textures/wall_normal.png",
    );
    assert_eq!(pack_textures(&root).expect("compiles"), (2, 2));
    assert_eq!(
        pack_textures(&root).expect("compiles"),
        (0, 2),
        "up to date"
    );
    let filesystem = mounted(&root);
    let played = MaterialLibrary::for_play(&filesystem)
        .load("wall")
        .expect("loads");
    assert_eq!(played.mips[0].form, PixelForm::Bc7Srgb);
    let material = played.material.expect("a material");
    assert_eq!(material.slots[0][0].form, PixelForm::Bc5);
    let baked = MaterialLibrary::new(&filesystem)
        .load("wall")
        .expect("loads");
    assert_eq!(baked.mips[0].form, PixelForm::Rgba);
    let mut library = MaterialLibrary::for_play(&filesystem);
    let pixels = library.load_pixels("wall").expect("loads");
    assert_eq!(pixels.mips[0].form, PixelForm::Rgba);
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_compiled_picture_behind_its_source_isnt_used() {
    let root = folder("stale");
    write_picture(&root, "textures/wall.png", &picture(16));
    write_material(&root, "wall", "base = textures/wall.png");
    pack_textures(&root).expect("compiles");
    write_picture(&root, "textures/wall.png", &flat_normals(16));
    let filesystem = mounted(&root);
    let texture = MaterialLibrary::for_play(&filesystem)
        .load("wall")
        .expect("loads");
    assert_eq!(texture.mips[0].form, PixelForm::Rgba, "the source wins");
    assert_eq!(pack_textures(&root).expect("compiles"), (1, 1));
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_game_with_nothing_compiled_reads_pixels() {
    let root = folder("plain");
    write_picture(&root, "textures/wall.png", &picture(16));
    write_material(&root, "wall", "base = textures/wall.png");
    let filesystem = mounted(&root);
    let texture = MaterialLibrary::for_play(&filesystem)
        .load("wall")
        .expect("loads");
    assert_eq!(texture.mips[0].form, PixelForm::Rgba);
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn layers_are_mixed_from_pixels_even_when_compiled() {
    let root = folder("layers");
    write_picture(&root, "textures/wall.png", &picture(16));
    write_picture(&root, "textures/moss.png", &flat_normals(16));
    write_picture(&root, "textures/mask.png", &picture(16));
    write_material(
        &root,
        "wall",
        "base = textures/wall.png\nlayer { base = textures/moss.png mask = textures/mask.png }",
    );
    pack_textures(&root).expect("compiles");
    let filesystem = mounted(&root);
    let texture = MaterialLibrary::for_play(&filesystem)
        .load("wall")
        .expect("loads");
    assert_eq!(texture.mips[0].form, PixelForm::Rgba);
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_picture_with_two_jobs_isnt_compiled() {
    let root = folder("two");
    write_picture(&root, "textures/both.png", &picture(16));
    write_picture(&root, "textures/wall.png", &picture(16));
    write_material(&root, "one", "base = textures/both.png");
    write_material(
        &root,
        "two",
        "base = textures/wall.png\nspecular = textures/both.png",
    );
    assert_eq!(pack_textures(&root).expect("compiles"), (1, 1));
    assert!(!root.join(packed_path("textures/both.png")).exists());
    let filesystem = mounted(&root);
    let texture = MaterialLibrary::for_play(&filesystem)
        .load("one")
        .expect("loads");
    assert_eq!(texture.mips[0].form, PixelForm::Rgba);
    fs::remove_dir_all(&root).expect("clean up");
}

/// A room with a stencil, whose pictures are read here and not by the GPU.
const STENCIL_MAP: &str = "format 1 map
entity world { type = worldspawn }
brush { material = wall cuboid = -256 -256 -32 256 256 0 }
entity shaft {
    type = ray_stencil
    image = cut
    corner = 0 0 64
    across = 32 0 0
    down = 0 32 0
    light = 0 0 128
}
";

/// A folder with a map that uses a compiled picture and a stencil.
fn stencil_folder(name: &str) -> PathBuf {
    let root = folder(name);
    write_picture(&root, "textures/wall.png", &picture(16));
    write_picture(&root, "textures/wall_normal.png", &flat_normals(16));
    write_picture(&root, "textures/cut.png", &picture(16));
    write_material(
        &root,
        "wall",
        "base = textures/wall.png\nnormal = textures/wall_normal.png",
    );
    write_material(&root, "cut", "base = textures/cut.png");
    fs::write(root.join("maps").join("room.emap"), STENCIL_MAP).expect("write");
    root
}

#[test]
fn what_a_level_reads_as_pixels_comes_in_pixels() {
    let root = stencil_folder("stencil");
    pack_textures(&root).expect("compiles");
    let filesystem = mounted(&root);
    let document = from_text(STENCIL_MAP).expect("reads");
    let played = play_map(&filesystem, "room", &document).expect("compiles");
    assert_eq!(played.map.stencils[0].image[0].form, PixelForm::Rgba);
    assert_eq!(played.pictures, ["textures/cut.png"]);
    let wall = played
        .map
        .textures
        .iter()
        .find(|texture| texture.name == "wall");
    assert_eq!(wall.expect("the wall").mips[0].form, PixelForm::Bc7Srgb);
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_pack_keeps_only_what_a_game_needs() {
    let root = stencil_folder("whole");
    fs::write(root.join("maps").join("room.ebin"), STENCIL_MAP).expect("write");
    fs::write(root.join("make_things.py"), "print('hello')").expect("write");
    fs::write(root.join("settings.json"), "{}").expect("write");
    fs::write(root.join("textures").join("other.txt"), "same").expect("write");
    fs::write(root.join("maps").join("other.txt"), "same").expect("write");
    let out = root.with_extension("epak");
    let summary = pack_content(&root, &out).expect("packs");
    assert_eq!(
        summary.shared, 2,
        "the text files, and two compiled pictures"
    );
    let pack = PackArchive::open(&out).expect("opens");
    assert!(pack.contains(&packed_path("textures/wall.png")));
    assert!(!pack.contains("textures/wall.png"), "compiled, so left out");
    assert!(pack.contains("textures/cut.png"), "read as pixels");
    assert!(!pack.contains("maps/room.emap"), "the binary form is there");
    assert!(pack.contains("maps/room.ebin"));
    assert!(!pack.contains("make_things.py"));
    assert!(!pack.contains("settings.json"));
    let mut filesystem = FileSystem::new();
    filesystem.mount(Box::new(pack));
    let texture = MaterialLibrary::for_play(&filesystem)
        .load("wall")
        .expect("loads with no source");
    assert_eq!(texture.mips[0].form, PixelForm::Bc7Srgb);
    fs::remove_dir_all(&root).expect("clean up");
    fs::remove_file(&out).expect("clean up");
}

#[test]
fn mips_of_a_colour_average_in_linear_light() {
    // Half black and half white is about 188 as sRGB, where bytes would say 128.
    let rgba: Vec<u8> = (0..16 * 16)
        .flat_map(|at| {
            let value = if at % 2 == 0 { 0 } else { 255 };
            [value, value, value, 255]
        })
        .collect();
    let bytes = pack_picture(16, 16, &rgba, Role::Colour, 0).expect("packs");
    let (_, mips) = read_packed(&bytes).expect("reads");
    let level = mips.iter().find(|mip| mip.width == 8).expect("a level");
    // With no decoder here, the blocks are compared with those of each grey.
    let grey = |value: u8| [value, value, value, 255].repeat(16 * 16);
    let block = |value: u8| {
        let bytes = pack_picture(16, 16, &grey(value), Role::Colour, 0).expect("packs");
        read_packed(&bytes).expect("reads").1[0].rgba[..16].to_vec()
    };
    assert_eq!(level.rgba[..16], block(188)[..]);
    assert_ne!(level.rgba[..16], block(128)[..]);
}

#[test]
fn a_hash_follows_the_bytes() {
    assert_eq!(bytes_hash(b"stone"), bytes_hash(b"stone"));
    assert_ne!(bytes_hash(b"stone"), bytes_hash(b"stonf"));
}

/// A finished game reads only what's compiled, so all of it has to be there.
#[test]
fn every_shader_a_game_could_want_is_compiled() {
    let root = std::env::temp_dir().join(format!("compiled_shaders_{}", std::process::id()));
    let _gone = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("shaders")).expect("folder");
    let code = "fn shader_uv(uv: vec2<f32>, world: vec3<f32>) -> vec2<f32> { return uv * 2.0; }
";
    fs::write(root.join("shaders").join("twice.wgsl"), code).expect("write");
    let count = engine_compile::compile_shaders(&root).expect("compiles");
    let modules = engine_shaders::all_modules(&[code], &[], &[], &[]);
    assert_eq!(count, modules.len());
    for module in &modules {
        let bytes = fs::read(root.join(module.path())).expect("it's there");
        let words = engine_shaders::compile(module).expect("compiles");
        assert_eq!(bytes, engine_shaders::compiled_file(&words), "{module:?}");
    }
    fs::write(root.join("shaders").join("bad.wgsl"), "this isn't wgsl").expect("write");
    assert!(engine_compile::compile_shaders(&root).is_err());
    fs::remove_dir_all(&root).expect("clean up");
}

/// Graphs and fields are compiled ahead like any shader, as a finished game has no compiler, and a
/// wrong graph stops a pack.
#[test]
fn graphs_and_fields_are_compiled_ahead() {
    let root = std::env::temp_dir().join(format!("engine-pack-graph-{}", std::process::id()));
    let _gone = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("shaders")).expect("folder");
    let graph = "format 1 graph
node { id = time kind = time }
node { id = wave kind = sin a = time }
outputs { emissive = wave }
";
    let field = "fn shader_field(at: vec2<i32>, size: vec2<i32>) -> vec4<f32> {
    return field_read(at);
}
";
    fs::write(root.join("shaders/wave.egraph"), graph).expect("write");
    fs::write(root.join("shaders/rain.wgsl"), field).expect("write");
    engine_compile::compile_shaders(&root).expect("compiles");
    let made = engine_compile::read_graph(graph).expect("reads");
    let code = engine_compile::graph_code(&made).expect("it's right");
    let surface = engine_shaders::Module::Surface(&code);
    assert!(root.join(surface.path()).exists(), "the graph");
    let stepping = engine_shaders::Module::Field(field);
    assert!(root.join(stepping.path()).exists(), "the field");
    let wrong = graph.replace("a = time", "a = nothing");
    fs::write(root.join("shaders/wave.egraph"), wrong).expect("write");
    let refused = engine_compile::compile_shaders(&root).expect_err("it's wrong");
    assert!(
        refused.to_string().contains("wave reads nothing"),
        "{refused}"
    );
    fs::remove_dir_all(&root).expect("clean up");
}
