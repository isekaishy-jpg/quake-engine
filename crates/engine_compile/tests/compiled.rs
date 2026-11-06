//! A compiled map's geometry, kept and read back.

use engine_compile::{
    TextureSource, compile, compile_known, read_geometry, read_map_text, write_geometry,
};
use engine_map::{Map, MipLevel, Texture, TextureKind};

/// A map with a little of everything geometry has: faces of two materials, a
/// moving part with its own model, and a piece of hull.
const TEXT: &str = "format 1 map
entity world { type = worldspawn }
brush { material = floor cuboid = -256 -256 -32 256 256 0 }
brush { material = wall cuboid = -256 -256 0 -224 256 128 }
brush { material = wall cuboid = 64 -64 0 128 64 96 }
entity door_1 {
    type = func_door
    brush { material = wall cuboid = 0 0 0 16 64 96 }
}
";

/// Plain textures, named as they're asked for, noting how many were.
#[derive(Default)]
struct Plain {
    asked: Vec<String>,
}

impl TextureSource for Plain {
    fn texture(&mut self, name: &str) -> Texture {
        self.asked.push(name.to_owned());
        Texture {
            name: name.to_owned(),
            kind: TextureKind::Solid,
            mips: vec![MipLevel {
                width: 1,
                height: 1,
                rgba: vec![128, 128, 128, 255],
            }],
            ..Texture::default()
        }
    }
}

/// The map, compiled in the ordinary way.
fn compiled() -> Map {
    let working = read_map_text(TEXT).expect("reads");
    compile(&working, &mut Plain::default()).expect("compiles")
}

/// Kept geometry gives the map a compile would.
#[test]
fn kept_geometry_makes_the_same_map() {
    let map = compiled();
    let bytes = write_geometry(&map, 7);
    let geometry = read_geometry(&bytes, 7).expect("reads");
    let working = read_map_text(TEXT).expect("reads");
    let again = compile_known(&working, &mut Plain::default(), geometry).expect("compiles");
    assert_eq!(map.points, again.points);
    assert_eq!(map.faces, again.faces);
    assert_eq!(map.models, again.models);
    assert_eq!(map.tree, again.tree);
    assert_eq!(map.entities, again.entities);
    assert_eq!(map.textures, again.textures);
    assert_eq!(map, again);
}

/// Geometry from other source text isn't used.
#[test]
fn geometry_for_another_hash_is_refused() {
    let bytes = write_geometry(&compiled(), 7);
    assert!(read_geometry(&bytes, 8).is_none());
}

/// A file cut short is refused, not half read, so a load falls back to compiling.
#[test]
fn a_damaged_file_is_refused() {
    let bytes = write_geometry(&compiled(), 7);
    for keep in [0, 3, bytes.len() / 2, bytes.len() - 1] {
        assert!(read_geometry(&bytes[..keep], 7).is_none(), "{keep}");
    }
}

/// Overlapping brushes reach each other's leaves, and if each copied what the last
/// put there they'd multiply, as they did to 75 million nodes in Vesper Deep.
#[test]
fn overlapping_brushes_share_their_trees() {
    let mut text = String::from(
        "format 1 map
entity world { type = worldspawn }
",
    );
    for index in 0..14 {
        let step = index * 6;
        let (low, high) = (step - 100, step + 100);
        let (top, bottom) = (step * 2 - 40, step * 2 + 200);
        text.push_str(&format!(
            "brush {{ material = wall cuboid = {low} {top} {step} {high} {bottom} {} }}
",
            step + 120
        ));
    }
    let working = read_map_text(&text).expect("reads");
    let map = compile(&working, &mut Plain::default()).expect("compiles");
    let nodes = map.tree.clip_nodes.len();
    assert!(nodes < 4000, "{nodes} clip nodes");
}
