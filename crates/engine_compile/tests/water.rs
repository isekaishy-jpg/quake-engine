//! What a level says of its water.

use engine_compile::{CompileError, compile, read_map, read_terrain_data, write_terrain};
use engine_core::entity::effect_id;
use engine_core::math::Vec3;
use engine_format::from_text;
use engine_map::{Contents, Map, TerrainBrush, TerrainData, TerrainStroke, Texture, stroke_land};

/// A level with `text` in it.
fn level(text: &str) -> Result<Map, CompileError> {
    let text = format!(
        "format 1 map
entity world {{ type = worldspawn }}
brush {{ material = rock cuboid = -64 -64 -96 64 64 -64 }}
{text}
"
    );
    let working = read_map(&from_text(&text).expect("parses"))?;
    compile(&working, &mut |_: &str| Texture::default())
}

/// Each body has the look it says, and those that don't say have the one with no name.
#[test]
fn a_brush_says_which_water_it_is() {
    let map = level(
        "water { absorption = 0.1 }
water canal { absorption = 0.2 deep = 0.5 view_effect = motes }
water pond { absorption = 0.3 }
brush { material = rock contents = water water = pond cuboid = 0 0 0 10 10 10 }
brush { material = rock contents = water cuboid = 20 0 0 30 10 10 }
brush { material = rock contents = water water = canal cuboid = 40 0 0 50 10 10 }",
    )
    .expect("compiles");
    let looks: Vec<f32> = map.pools.iter().map(|pool| pool.look.absorption).collect();
    assert_eq!(looks, [0.3, 0.1, 0.2]);
    assert_eq!(map.pools[2].look.deep, 0.5);
    assert_eq!(map.pools[2].look.view_effect, effect_id("motes"));
    assert_eq!(map.pools[0].look.view_effect, 0);
}

/// A name nobody gave is a mistake.
#[test]
fn a_water_has_to_be_there() {
    let told =
        level("brush { material = rock contents = water water = gone cuboid = 0 0 0 9 9 9 }");
    let told = told.expect_err("turned down").to_string();
    assert!(told.contains("gone"), "{told}");
}

/// With no water that fits, it's what such a liquid usually is.
#[test]
fn a_liquid_with_nothing_said_has_its_usual_look() {
    let map = level(
        "water canal { absorption = 0.2 }
brush { material = rock contents = slime cuboid = 0 0 0 10 10 10 }",
    )
    .expect("compiles");
    assert_eq!(map.pools[0].look.absorption, 0.02);
}

/// A level from files, with a mesh to be the surface of something.
fn level_with_mesh(text: &str) -> Map {
    let root = std::env::temp_dir().join(format!(
        "engine-water-{}-{}",
        std::process::id(),
        text.len()
    ));
    let _gone = std::fs::remove_dir_all(&root);
    for folder in ["maps", "meshes"] {
        std::fs::create_dir_all(root.join(folder)).expect("folder");
    }
    std::fs::write(
        root.join("meshes/river.emesh"),
        "format 1 mesh
part top {
    material = rock
    points = 0 0 20 100 0 0 100 40 0 0 40 20
    uvs = 0 0 1 0 1 1 0 1
    triangles = 0 1 2 0 2 3
}
",
    )
    .expect("write");
    let level = format!(
        "format 1 map
entity world {{ type = worldspawn }}
brush {{ material = rock cuboid = -64 -64 -96 64 64 -64 }}
{text}
"
    );
    std::fs::write(root.join("maps/level.emap"), &level).expect("write");
    let mut filesystem = engine_assets::FileSystem::new();
    filesystem.mount(Box::new(engine_assets::DirectoryArchive::new(&root)));
    let document = engine_compile::document_of(level.as_bytes()).expect("reads");
    let played = engine_compile::play_map(&filesystem, "level", &document).expect("loads");
    let _gone = std::fs::remove_dir_all(&root);
    played.map
}

/// A mesh can be the top of a body of water: it slopes as the mesh does, and isn't drawn as one.
#[test]
fn a_mesh_can_be_the_surface_of_water() {
    let map = level_with_mesh(
        "water stream { cell = 10 depth = 30 absorption = 0.2 }
mesh { source = river water = stream origin = 200 0 50 }",
    );
    let [sheet] = &map.water_sheets[..] else {
        panic!("one body");
    };
    assert_eq!(sheet.cell, 10.0);
    assert_eq!(sheet.depth, 30.0);
    assert_eq!(sheet.look.absorption, 0.2);
    assert_eq!(sheet.contents, Contents::Water);
    let top = |x: f32| sheet.height_at(x, 20.0).expect("over it");
    assert!((top(200.0) - 70.0).abs() < 0.01, "{}", top(200.0));
    assert!((top(250.0) - 60.0).abs() < 0.01, "{}", top(250.0));
    assert!((top(300.0) - 50.0).abs() < 0.01, "{}", top(300.0));
    assert!(sheet.height_at(150.0, 20.0).is_none());
    assert!(!sheet.triangles().is_empty());
    let plain = level_with_mesh("mesh { source = river origin = 200 0 50 }");
    assert!(plain.water_sheets.is_empty());
    assert!(plain.faces.len() > map.faces.len(), "as a mesh it's drawn");
}

/// A body can be any liquid, and has to be one.
#[test]
fn a_body_says_what_it_is() {
    let map = level_with_mesh(
        "water hot { contents = lava }
mesh { source = river water = hot }",
    );
    assert_eq!(map.water_sheets[0].contents, Contents::Lava);
    assert_eq!(
        map.water_sheets[0].look.waves, 0.0,
        "it looks like that too"
    );
}

/// Land with a dip in it, and water in the dip, as a file.
fn land_with_a_pond() -> Vec<u8> {
    let mut data = TerrainData::flat([1, 1], 32.0);
    let across = data.across();
    for y in 0..across {
        for x in 0..across {
            let at = (y * across + x) as usize;
            let away = (x as f32 - 8.0).hypot(y as f32 - 8.0);
            data.heights[at] = (away - 4.0).min(0.0) * 8.0;
        }
    }
    let brush = TerrainBrush {
        middle: [8.0 * 32.0, 8.0 * 32.0],
        radius: 4.0 * 32.0,
        soft: 0.0,
    };
    stroke_land(&mut data, &brush, TerrainStroke::Water(20.0));
    write_terrain(&data)
}

/// A level with land that has water on it.
fn level_with_land(text: &str) -> Map {
    let root =
        std::env::temp_dir().join(format!("engine-pond-{}-{}", std::process::id(), text.len()));
    let _gone = std::fs::remove_dir_all(&root);
    for folder in ["maps", "terrain"] {
        std::fs::create_dir_all(root.join(folder)).expect("folder");
    }
    std::fs::write(root.join("terrain/pond.eterrain"), land_with_a_pond()).expect("write");
    let level = format!(
        "format 1 map
entity world {{ type = worldspawn }}
brush {{ material = rock cuboid = -64 -64 -96 64 64 -64 }}
{text}
"
    );
    std::fs::write(root.join("maps/level.emap"), &level).expect("write");
    let mut filesystem = engine_assets::FileSystem::new();
    filesystem.mount(Box::new(engine_assets::DirectoryArchive::new(&root)));
    let document = engine_compile::document_of(level.as_bytes()).expect("reads");
    let played = engine_compile::play_map(&filesystem, "level", &document).expect("loads");
    let _gone = std::fs::remove_dir_all(&root);
    played.map
}

/// Water on land goes where it's said to, and carries on a sample past that, for the land to hide.
#[test]
fn land_can_have_water_on_it() {
    let map = level_with_land(
        "water pond { absorption = 0.3 }
terrain { source = pond water = pond origin = 100 200 50 }",
    );
    let [sheet] = &map.water_sheets[..] else {
        panic!("one body");
    };
    assert_eq!(sheet.look.absorption, 0.3);
    assert_eq!(sheet.cell, 32.0);
    let land = &map.terrain[0];
    let middle = (100.0 + 8.0 * 32.0, 200.0 + 8.0 * 32.0);
    let ground = land.height_at(middle.0, middle.1).expect("over it");
    let top = sheet.height_at(middle.0, middle.1).expect("over it");
    assert!((top - ground - 20.0).abs() < 0.01, "{top} over {ground}");
    assert!(sheet.holds(Vec3::new(middle.0, middle.1, ground + 5.0)));
    let far = (100.0 + 32.0, 200.0 + 32.0);
    assert!(
        sheet.height_at(far.0, far.1).is_none(),
        "it's only where it's wet"
    );
    let (x, y) = (100.0 + 13.0 * 32.0, 200.0 + 8.0 * 32.0);
    let wet = sheet.height_at(x - 32.0, y).expect("in the water");
    let shore = sheet.height_at(x, y).expect("a sample past it");
    assert!(
        wet - shore < 25.0 && shore > wet - 25.0,
        "{wet} and {shore}"
    );
    assert!(sheet.height_at(x + 32.0, y).is_none(), "and no further");
    assert!(!sheet.triangles().is_empty());
}

/// Land with no water is nothing, and water comes through a file.
#[test]
fn land_with_no_water_has_no_body() {
    let flat = write_terrain(&TerrainData::flat([1, 1], 32.0));
    assert!(read_terrain_data(&flat).expect("reads").water.is_empty());
    let read = read_terrain_data(&land_with_a_pond()).expect("reads");
    assert_eq!(read.water.len(), read.heights.len());
    assert!(read.water.contains(&20.0));
}

/// A level from files, with pictures for its water.
fn level_with_pictures(text: &str) -> Map {
    let root =
        std::env::temp_dir().join(format!("engine-foam-{}-{}", std::process::id(), text.len()));
    let _gone = std::fs::remove_dir_all(&root);
    for folder in ["maps", "textures"] {
        std::fs::create_dir_all(root.join(folder)).expect("folder");
    }
    let picture = |side: u32| {
        let pixels = [200, 100, 50, 255].repeat((side * side) as usize);
        engine_compile::encode_png(side, side, &pixels).expect("encodes")
    };
    std::fs::write(root.join("textures/small.png"), picture(16)).expect("write");
    std::fs::write(root.join("textures/large.png"), picture(64)).expect("write");
    let level = format!(
        "format 1 map
entity world {{ type = worldspawn }}
brush {{ material = rock cuboid = -64 -64 -96 64 64 -64 }}
{text}
"
    );
    std::fs::write(root.join("maps/level.emap"), &level).expect("write");
    let mut filesystem = engine_assets::FileSystem::new();
    filesystem.mount(Box::new(engine_assets::DirectoryArchive::new(&root)));
    let document = engine_compile::document_of(level.as_bytes()).expect("reads");
    let played = engine_compile::play_map(&filesystem, "level", &document).expect("loads");
    let _gone = std::fs::remove_dir_all(&root);
    played.map
}

/// What's on water is read once each, at one size, and each look knows where.
#[test]
fn a_water_says_what_is_on_it() {
    let map = level_with_pictures(
        "water scum {
    foam = 12
    foam_image = textures/small.png
    foam_size = 40
    shore = 6
    overlay { image = textures/large.png size = 90 flow = 3 4 strength = 0.5 }
    overlay { image = textures/small.png }
}
brush { material = rock contents = water water = scum cuboid = 0 0 0 9 9 9 }",
    );
    assert_eq!(map.water_pictures.len(), 2);
    assert_eq!(map.water_pictures[0][0].width, 64);
    assert_eq!(map.water_pictures[1][0].width, 64);
    let look = map.pools[0].look;
    assert_eq!(look.overlays[0].picture, 1);
    assert_eq!(look.overlays[0].size, 90.0);
    assert_eq!(look.overlays[0].flow, [3.0, 4.0]);
    assert_eq!(look.overlays[0].strength, 0.5);
    assert_eq!(look.overlays[1].picture, 2);
    assert_eq!(look.foam, 12.0);
    assert_eq!(look.foam_look.picture, 2);
    assert_eq!(look.foam_look.size, 40.0);
    assert_eq!(look.shore, 6.0);
}
