//! What land's painted with, from its file to the pictures a map holds (proposal 0144).

use std::sync::Arc;

use engine_assets::{DirectoryArchive, FileSystem};
use engine_compile::{
    MaterialLibrary, TextureSource, compile, document_of, encode_png, pack_land, play_map,
    read_map_text, read_terrain_data, write_terrain,
};
use engine_map::{Material, MipLevel, PixelForm, TerrainData, TerrainLayer, Texture, mip_chain};

/// A picture of `side` of one pixel over and over.
fn picture(side: u32, pixel: [u8; 4]) -> Vec<MipLevel> {
    mip_chain(side, side, pixel.repeat((side * side) as usize))
}

/// A material for each name: "big" is eight wide and the rest four, and "bumpy" has a normal, a
/// height and a shine.
struct Pictures;

impl TextureSource for Pictures {
    fn texture(&mut self, name: &str) -> Texture {
        let side = if name == "big" { 8 } else { 4 };
        let material = (name == "bumpy").then(|| {
            Arc::new(Material {
                slots: vec![
                    Arc::new(picture(4, [10, 20, 255, 255])),
                    Arc::new(picture(4, [200, 0, 0, 255])),
                    Arc::new(picture(4, [255, 255, 255, 255])),
                ],
                normal: Some(1),
                height: Some(2),
                specular: Some(3),
                specular_strength: 0.5,
                gloss: 12.0,
                ..Material::default()
            })
        });
        Texture {
            name: name.to_owned(),
            mips: picture(side, [100, 100, 100, 255]),
            material,
            ..Texture::default()
        }
    }
}

/// Land with `materials` for layers.
fn land(materials: &[&str]) -> TerrainData {
    let mut data = TerrainData::flat([1, 1], 32.0);
    for (number, material) in materials.iter().enumerate() {
        let layer = TerrainLayer {
            material: (*material).to_owned(),
            size: 256.0 + number as f32,
            hex: number == 1,
        };
        assert!(data.set_layer(number, layer));
    }
    data
}

#[test]
fn paint_is_kept_in_a_file() {
    let mut data = land(&["rock", "moss"]);
    data.paint[7] = [5, 250, 0, 0, 0, 0, 0, 0];
    let again = read_terrain_data(&write_terrain(&data)).expect("reads");
    assert_eq!(again, data);
}

#[test]
fn land_with_no_paint_reads_as_it_did() {
    let data = land(&[]);
    let again = read_terrain_data(&write_terrain(&data)).expect("reads");
    assert_eq!(again, data);
    assert!(again.paint.is_empty());
}

#[test]
fn land_short_of_paint_is_refused() {
    let mut data = land(&["rock"]);
    data.paint.pop();
    let read = read_terrain_data(&write_terrain(&data));
    assert!(read.is_err(), "{read:?}");
}

/// A map on `data`, which is held as it would be from its file.
fn map_on(data: TerrainData) -> engine_map::Map {
    let mut working = read_map_text(
        "format 1 map
entity world { type = worldspawn }
brush { material = stone cuboid = -64 -64 -96 64 64 -64 }
terrain { source = hills origin = [0 0 0] material = ground }
",
    )
    .expect("reads");
    working.terrain[0].data = data;
    compile(&working, &mut Pictures).expect("compiles")
}

#[test]
fn a_map_holds_each_picture_once_all_of_one_size() {
    let map = map_on(land(&["small", "big", "small"]));
    let names: Vec<&str> = map
        .land_pictures
        .iter()
        .map(|p| p.material.as_str())
        .collect();
    assert_eq!(names, ["small", "big"]);
    assert_eq!(map.terrain[0].pictures, [0, 1, 0]);
    for picture in &map.land_pictures {
        assert_eq!(picture.color[0].width, 8, "{}", picture.material);
        assert_eq!(picture.surface[0].width, 8, "{}", picture.material);
        assert_eq!(picture.color[0].rgba.len(), 8 * 8 * 4);
        assert_eq!(picture.color[0].form, PixelForm::Rgba);
        assert_eq!(picture.color.last().map(|top| top.width), Some(1));
    }
}

#[test]
fn a_surface_is_a_materials_bumps_height_and_shine() {
    let map = map_on(land(&["bumpy"]));
    let picture = &map.land_pictures[0];
    let surface = &picture.surface[0].rgba;
    assert_eq!(surface[..3], [10, 20, 200]);
    assert!(
        surface[3].abs_diff(128) < 2,
        "half of white: {}",
        surface[3]
    );
    assert!((picture.gloss - 12.0).abs() < 0.01);
}

#[test]
fn a_plain_material_is_flat_stands_by_its_brightness_and_doesnt_shine() {
    let map = map_on(land(&["plain"]));
    let surface = &map.land_pictures[0].surface[0].rgba;
    assert_eq!(surface[..2], [128, 128]);
    assert!(surface[2] > 100, "{}", surface[2]);
    assert_eq!(surface[3], 0);
}

#[test]
fn land_with_no_layers_loads_no_pictures() {
    let map = map_on(land(&[]));
    assert!(map.land_pictures.is_empty());
    assert!(map.terrain[0].pictures.is_empty());
}

/// A content folder with two materials and land painted with them, on a map.
fn content(name: &str, data: &TerrainData) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("engine-land-{name}-{}", std::process::id()));
    let _gone = std::fs::remove_dir_all(&root);
    for folder in ["maps", "materials", "textures", "terrain"] {
        std::fs::create_dir_all(root.join(folder)).expect("folder");
    }
    for (material, pixel) in [("rock", [120, 90, 60, 255]), ("moss", [40, 140, 50, 255])] {
        let png = encode_png(8, 8, &pixel.repeat(64)).expect("encodes");
        std::fs::write(root.join(format!("textures/{material}.png")), png).expect("picture");
        let text = format!("format 1 material\nsurface {{ base = textures/{material}.png }}\n");
        std::fs::write(root.join(format!("materials/{material}.emat")), text).expect("material");
    }
    std::fs::write(root.join("terrain/hills.eterrain"), write_terrain(data)).expect("land");
    std::fs::write(
        root.join("maps/hills.emap"),
        "format 1 map
entity world { type = worldspawn }
brush { material = rock cuboid = -64 -64 -96 64 64 -64 }
terrain { source = hills origin = [0 0 0] material = rock }
",
    )
    .expect("map");
    root
}

/// What `root` holds, as files.
fn files_of(root: &std::path::Path) -> FileSystem {
    let mut filesystem = FileSystem::new();
    filesystem.mount(Box::new(DirectoryArchive::new(root)));
    filesystem
}

/// The level in `root`, as a game reads it.
fn played(root: &std::path::Path) -> engine_map::Map {
    let filesystem = files_of(root);
    let bytes = filesystem.read("maps/hills.emap").expect("map");
    let document = document_of(&bytes).expect("reads");
    play_map(&filesystem, "hills", &document)
        .expect("loads")
        .map
}

#[test]
fn a_game_reads_land_pictures_as_they_were_compiled() {
    let root = content("packed", &land(&["rock", "moss"]));
    let plain = played(&root);
    assert_eq!(plain.land_pictures[0].color[0].form, PixelForm::Rgba);
    assert_eq!(pack_land(&root).expect("packs"), (2, 2));
    assert_eq!(pack_land(&root).expect("packs"), (0, 2), "they're current");
    let map = played(&root);
    assert_eq!(map.land_pictures.len(), 2);
    for picture in &map.land_pictures {
        assert_eq!(picture.color[0].form, PixelForm::Bc7Srgb);
        assert_eq!(picture.surface[0].form, PixelForm::Bc7);
    }
    // A change to a picture puts them behind, and pixels stand in till they're compiled again.
    let png = encode_png(8, 8, &[9, 9, 9, 255].repeat(64)).expect("encodes");
    std::fs::write(root.join("textures/moss.png"), png).expect("picture");
    let map = played(&root);
    assert_eq!(map.land_pictures[0].color[0].form, PixelForm::Rgba);
    assert_eq!(pack_land(&root).expect("packs"), (1, 2));
    let _gone = std::fs::remove_dir_all(&root);
}

/// What a bake's hash covers of the level in `root`.
fn baked_from(root: &std::path::Path) -> String {
    let filesystem = files_of(root);
    let bytes = filesystem.read("maps/hills.emap").expect("map");
    let text = std::str::from_utf8(&bytes).expect("text");
    let mut working = read_map_text(text).expect("reads");
    let mut library = MaterialLibrary::new(&filesystem);
    library.prepare(&mut working).expect("prepares")
}

#[test]
fn a_bake_lasts_through_paint_but_not_through_a_change_of_shape() {
    let data = land(&["rock", "moss"]);
    let root = content("hash", &data);
    let before = baked_from(&root);
    let file = root.join("terrain/hills.eterrain");
    let mut painted = data.clone();
    painted.paint[9] = [1, 254, 0, 0, 0, 0, 0, 0];
    std::fs::write(&file, write_terrain(&painted)).expect("land");
    assert_eq!(baked_from(&root), before, "paint");
    let mut shaped = data;
    shaped.heights[9] = 40.0;
    std::fs::write(&file, write_terrain(&shaped)).expect("land");
    assert_ne!(baked_from(&root), before, "shape");
    let _gone = std::fs::remove_dir_all(&root);
}
