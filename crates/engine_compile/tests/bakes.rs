//! Compiled output beside a map.

use std::sync::Arc;

use engine_compile::{
    BakeInput, CompileError, apply_bake, bake_light, compile, placeholder_texture, read_map,
    source_hash, write_bake,
};
use engine_format::from_text;
use engine_map::{Map, MipLevel};

/// A small lit map.
const TEXT: &str = "format 1 map
brush { material = floor cuboid = -128 -128 -32 128 128 0 }
entity lamp { type = light origin = 0 0 100 intensity = 300 }
paint { layer = 0 radius = 32 points = 0 0 0 }
";

/// `text` compiled, with no light.
fn plain(text: &str) -> Map {
    let working = read_map(&from_text(text).expect("parses")).expect("reads");
    let mut source = |name: &str| placeholder_texture(name);
    compile(&working, &mut source).expect("compiles")
}

/// `text` compiled and baked.
fn lit(text: &str) -> Map {
    let working = read_map(&from_text(text).expect("parses")).expect("reads");
    let mut map = plain(text);
    let mut images = |path: &str| -> Result<Arc<Vec<MipLevel>>, CompileError> {
        Err(CompileError::Bad(path.to_owned()))
    };
    let mut input = BakeInput {
        working: &working,
        images: &mut images,
    };
    bake_light(&mut map, &mut input).expect("bakes");
    map
}

#[test]
fn a_bake_restores_what_was_baked() {
    let baked = lit(TEXT);
    let hash = source_hash(TEXT);
    let bytes = write_bake(&baked, hash);
    let mut map = plain(TEXT);
    assert!(!map.lit);
    assert!(apply_bake(&mut map, &bytes, hash).expect("reads"));
    assert!(map.lit);
    assert_eq!(map.light, baked.light);
    assert_eq!(map.light_directions, baked.light_directions);
    assert_eq!(map.light_edges, baked.light_edges);
    assert_eq!(map.paint, baked.paint);
    assert_eq!(map.probes, baked.probes);
    for (face, baked) in map.faces.iter().zip(&baked.faces) {
        assert_eq!(face.light, baked.light);
    }
}

#[test]
fn a_stale_bake_is_ignored() {
    let bytes = write_bake(&lit(TEXT), source_hash(TEXT));
    let edited = format!("{TEXT}# a comment still changes the text\n");
    let mut map = plain(&edited);
    let applied = apply_bake(&mut map, &bytes, source_hash(&edited)).expect("reads");
    assert!(!applied, "it was made from something else");
    assert!(!map.lit, "so the map stays unlit");
    assert!(map.light.is_empty());
}

#[test]
fn something_else_is_an_error() {
    let mut map = plain(TEXT);
    assert!(apply_bake(&mut map, b"not a bake", 0).is_err());
}
