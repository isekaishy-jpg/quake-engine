//! Baked light brighter than a byte holds, brought down to fit and multiplied back.

use std::sync::Arc;

use engine_compile::{
    BakeInput, BakeLevel, BakeQuality, CompileError, apply_bake, bake_files, bake_light_as,
    compile, placeholder_texture, read_map,
};
use engine_core::math::Vec3;
use engine_format::from_text;
use engine_map::{Map, MipLevel};

/// A floor with a lamp `intensity` bright over it, and `rest` in its world.
fn lamp(intensity: u32, rest: &str) -> String {
    format!(
        "format 1 map
entity world {{ type = worldspawn light_spacing = 32 {rest} }}
brush {{ material = ground cuboid = -256 -256 -32 256 256 0 }}
brush {{ material = roof cuboid = -256 -256 256 256 256 288 }}
entity lamp {{ type = light origin = 0 0 48 intensity = {intensity} }}
"
    )
}

/// `text`, compiled and baked at `level`.
fn baked(text: &str, level: BakeLevel) -> Map {
    let working = read_map(&from_text(text).expect("parses")).expect("reads");
    let mut source = |name: &str| placeholder_texture(name);
    let mut map = compile(&working, &mut source).expect("compiles");
    let mut images = |path: &str| -> Result<Arc<Vec<MipLevel>>, CompileError> {
        Err(CompileError::Bad(path.to_owned()))
    };
    let mut input = BakeInput {
        working: &working,
        images: &mut images,
    };
    bake_light_as(&mut map, &mut input, BakeQuality { level, area: None }).expect("bakes");
    map
}

/// Light a byte holds is kept as it always was.
#[test]
fn light_that_fits_a_byte_is_as_it_was() {
    let map = baked(&lamp(200, ""), BakeLevel::Usual);
    assert_eq!(map.light_shift, 0);
}

/// Light brighter than a byte holds is brought down by a power of two to fit, and comes back
/// brighter than a byte would have let it.
#[test]
fn brighter_light_is_brought_down_to_fit_and_comes_back() {
    let bright = baked(&lamp(3000, ""), BakeLevel::Usual);
    assert!(bright.light_shift >= 1, "{}", bright.light_shift);
    let dim = baked(&lamp(1000, ""), BakeLevel::Usual);
    let under = Vec3::new(0.0, 0.0, 8.0);
    let (bright_at, dim_at) = (
        bright.light_at(under, &[1.0; 64]),
        dim.light_at(under, &[1.0; 64]),
    );
    assert!(bright_at > dim_at * 2.0, "{dim_at} {bright_at}");
    assert!(bright_at > 300.0, "past what a byte holds: {bright_at}");
}

/// A bake doesn't bring light down by more than it needs to.
#[test]
fn light_is_brought_down_no_more_than_it_needs() {
    // Bright enough to need it, and not so bright that it's brought down as far as it goes.
    let map = baked(&lamp(1000, ""), BakeLevel::Usual);
    assert!(map.light_shift >= 1, "{}", map.light_shift);
    let brightest = map
        .light
        .iter()
        .flat_map(|sample| sample.iter().copied())
        .max()
        .unwrap_or(0);
    assert!(brightest > 127, "it fills the byte: {brightest}");
}

/// A saved bake says how far its light was brought down, so one put back is read the same way.
#[test]
fn a_saved_bake_keeps_how_far_its_light_was_brought_down() {
    let text = lamp(3000, "");
    let map = baked(&text, BakeLevel::Usual);
    let (_, bytes) = bake_files(&map, "lamp", 9, BakeLevel::Usual).remove(0);
    let mut again = baked(&lamp(3000, ""), BakeLevel::Quick);
    again.light_shift = 0;
    assert!(apply_bake(&mut again, &bytes, 9).expect("reads"));
    assert_eq!(again.light_shift, map.light_shift);
    let dim = baked(&lamp(200, ""), BakeLevel::Usual);
    let (_, bytes) = bake_files(&dim, "lamp", 9, BakeLevel::Usual).remove(0);
    let mut again = baked(&lamp(200, ""), BakeLevel::Quick);
    again.light_shift = 3;
    assert!(apply_bake(&mut again, &bytes, 9).expect("reads"));
    assert_eq!(again.light_shift, 0);
}

/// Two rooms with a wall between, a lamp `bright` in the first and an ordinary one in the second.
fn two_rooms(bright: u32) -> String {
    format!(
        "format 1 map
entity world {{ type = worldspawn light_spacing = 32 }}
brush {{ material = near_floor cuboid = -512 -256 -32 0 256 0 }}
brush {{ material = far_floor cuboid = 16 -256 -32 528 256 0 }}
brush {{ material = wall cuboid = 0 -288 -32 16 288 288 }}
brush {{ material = roof cuboid = -544 -288 256 560 288 288 }}
brush {{ material = side cuboid = -544 -288 -32 -512 288 288 }}
brush {{ material = side cuboid = 528 -288 -32 560 288 288 }}
brush {{ material = side cuboid = -512 -288 -32 528 -256 288 }}
brush {{ material = side cuboid = -512 256 -32 528 288 288 }}
entity bright {{ type = light origin = -256 0 48 intensity = {bright} }}
entity lamp {{ type = light origin = 272 0 64 intensity = 250 }}
"
    )
}

/// The brightest of the faces of `material`, as stored and as the game reads it.
fn stored_and_read(map: &Map, material: &str) -> (f64, f64) {
    let number = map
        .textures
        .iter()
        .position(|texture| texture.name == material)
        .and_then(|number| u32::try_from(number).ok())
        .expect("a material");
    let mut brightest = 0.0_f64;
    for face in map.faces.iter().filter(|face| face.texture == number) {
        let Some(light) = face.light else {
            continue;
        };
        let size = (light.width * light.height) as usize;
        let first = light.first_sample as usize;
        for sample in &map.light[first..first + size] {
            let mean = sample.iter().map(|part| f64::from(*part)).sum::<f64>() / 3.0;
            brightest = brightest.max(mean);
        }
    }
    (brightest, brightest * f64::from(map.light_scale()))
}

/// Light in a room next to a very bright one is what it would be anyway, once it's multiplied
/// back: it's brought down and multiplied by the same, both on surfaces and in probes.
#[test]
fn light_beside_bright_light_comes_back_as_it_was() {
    let bright = baked(&two_rooms(4000), BakeLevel::Usual);
    let ordinary = baked(&two_rooms(200), BakeLevel::Usual);
    assert!(bright.light_shift >= 1, "{}", bright.light_shift);
    assert_eq!(ordinary.light_shift, 0);
    let ((bright_stored, bright_read), (_, ordinary_read)) = (
        stored_and_read(&bright, "far_floor"),
        stored_and_read(&ordinary, "far_floor"),
    );
    assert!(
        bright_stored < ordinary_read * 0.75,
        "brought down: {bright_stored} {ordinary_read}"
    );
    let off = (bright_read - ordinary_read).abs() / ordinary_read;
    assert!(off < 0.1, "{ordinary_read} {bright_read}");
    let far = Vec3::new(272.0, 0.0, 64.0);
    let (bright_at, ordinary_at) = (
        bright.light_at(far, &[1.0; 64]),
        ordinary.light_at(far, &[1.0; 64]),
    );
    let off = (bright_at - ordinary_at).abs() / ordinary_at;
    assert!(off < 0.1, "probes: {ordinary_at} {bright_at}");
}
