//! How much work a bake does, and where it goes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use engine_compile::{
    BakeInput, BakeLevel, BakeQuality, CompileError, apply_saved_bake, bake_files, bake_is_for,
    bake_light, bake_light_as, compile, final_folder, placeholder_texture, read_map,
};
use engine_format::from_text;
use engine_map::{BOUNCE_STYLE, Map, MipLevel, SideFiles};

/// Two rooms a long way apart. The floor of each is its own material, and a wall stands between
/// a lamp and part of the first, so that part only gets what bounces.
const TWO_ROOMS: &str = "format 1 map
entity world { type = worldspawn light_spacing = 16 }
brush { material = near_floor cuboid = -256 -256 -32 256 256 0 }
brush { material = near_top cuboid = -256 -256 256 256 256 288 }
brush { material = near_a cuboid = -288 -256 0 -256 256 256 }
brush { material = near_b cuboid = 256 -256 0 288 256 256 }
brush { material = near_c cuboid = -256 -288 0 256 -256 256 }
brush { material = near_d cuboid = -256 256 0 256 288 256 }
brush { material = screen cuboid = 0 -256 0 16 256 200 }
entity lamp { type = light origin = -128 0 128 intensity = 400 }
brush { material = far_floor cuboid = 2000 -256 -32 2512 256 0 }
brush { material = far_top cuboid = 2000 -256 256 2512 256 288 }
brush { material = far_a cuboid = 1968 -256 0 2000 256 256 }
brush { material = far_b cuboid = 2512 -256 0 2544 256 256 }
brush { material = far_c cuboid = 2000 -288 0 2512 -256 256 }
brush { material = far_d cuboid = 2000 256 0 2512 288 256 }
entity far_lamp { type = light origin = 2256 0 128 intensity = 400 }
";

/// A level compiled with no light.
fn plain() -> (Map, engine_compile::WorkingMap) {
    let working = read_map(&from_text(TWO_ROOMS).expect("parses")).expect("reads");
    let mut source = |name: &str| placeholder_texture(name);
    let map = compile(&working, &mut source).expect("compiles");
    (map, working)
}

/// The level, baked as `quality` says.
fn baked(quality: BakeQuality) -> Map {
    let (mut map, working) = plain();
    let mut images = |path: &str| -> Result<Arc<Vec<MipLevel>>, CompileError> {
        Err(CompileError::Bad(path.to_owned()))
    };
    let mut input = BakeInput {
        working: &working,
        images: &mut images,
    };
    bake_light_as(&mut map, &mut input, quality).expect("bakes");
    map
}

/// A bake at `level`, of all of the level.
fn at(level: BakeLevel) -> Map {
    baked(BakeQuality { level, area: None })
}

/// How many samples the faces of `material` have, all told.
fn samples_of(map: &Map, material: &str) -> u32 {
    let number = map
        .textures
        .iter()
        .position(|texture| texture.name == material);
    let number = u32::try_from(number.expect("a material")).expect("few");
    map.faces
        .iter()
        .filter(|face| face.texture == number)
        .filter_map(|face| face.light.as_ref())
        .map(|light| light.width * light.height)
        .sum()
}

/// How much bounced light there is anywhere.
fn bounced(map: &Map) -> u64 {
    let mut total = 0;
    for light in map.faces.iter().filter_map(|face| face.light.as_ref()) {
        let size = (light.width * light.height) as usize;
        let slot = light.styles.iter().position(|style| *style == BOUNCE_STYLE);
        if let Some(slot) = slot {
            let first = light.first_sample as usize + slot * size;
            for sample in &map.light[first..first + size] {
                total += sample.iter().map(|part| u64::from(*part)).sum::<u64>();
            }
        }
    }
    total
}

/// What a build does hasn't changed.
#[test]
fn usual_is_what_a_build_always_did() {
    let (mut map, working) = plain();
    let mut images = |path: &str| -> Result<Arc<Vec<MipLevel>>, CompileError> {
        Err(CompileError::Bad(path.to_owned()))
    };
    let mut input = BakeInput {
        working: &working,
        images: &mut images,
    };
    bake_light(&mut map, &mut input).expect("bakes");
    assert_eq!(map.light, at(BakeLevel::Usual).light);
}

/// A quick bake has far fewer samples, and a final one has more.
#[test]
fn levels_of_bake_differ_in_how_fine_they_are() {
    let (quick, usual, last) = (
        at(BakeLevel::Quick),
        at(BakeLevel::Usual),
        at(BakeLevel::Final),
    );
    let floor = |map: &Map| samples_of(map, "near_floor");
    assert!(
        floor(&quick) * 4 < floor(&usual),
        "{} {}",
        floor(&quick),
        floor(&usual)
    );
    assert!(
        floor(&usual) * 2 < floor(&last),
        "{} {}",
        floor(&usual),
        floor(&last)
    );
}

/// A quick bake doesn't bounce, which is much of what it saves.
#[test]
fn a_quick_bake_has_nothing_bounced() {
    assert!(bounced(&at(BakeLevel::Usual)) > 0);
    assert_eq!(bounced(&at(BakeLevel::Quick)), 0);
}

/// A final bake takes care only over the part it's told to.
#[test]
fn a_final_bake_can_be_of_part_of_a_level() {
    let usual = at(BakeLevel::Usual);
    let part = baked(BakeQuality {
        level: BakeLevel::Final,
        area: Some([[-300.0, -300.0, -40.0], [300.0, 300.0, 300.0]]),
    });
    assert!(samples_of(&part, "near_floor") > samples_of(&usual, "near_floor") * 2);
    assert_eq!(
        samples_of(&part, "far_floor"),
        samples_of(&usual, "far_floor")
    );
}

/// What just touches the part counts, and what's clear of it on one side doesn't.
#[test]
fn a_part_goes_by_all_three_axes() {
    let usual = at(BakeLevel::Usual);
    let part = baked(BakeQuality {
        level: BakeLevel::Final,
        area: Some([[-300.0, -300.0, 500.0], [300.0, 300.0, 600.0]]),
    });
    assert_eq!(
        samples_of(&part, "near_floor"),
        samples_of(&usual, "near_floor")
    );
}

/// A final bake's files have names of their own, so a build can't touch them.
#[test]
fn a_final_bake_keeps_its_own_files() {
    let map = at(BakeLevel::Quick);
    let usual: Vec<String> = bake_files(&map, "yard", 1, BakeLevel::Usual)
        .into_iter()
        .map(|(file, _)| file)
        .collect();
    let last: Vec<String> = bake_files(&map, "yard", 1, BakeLevel::Final)
        .into_iter()
        .map(|(file, _)| file)
        .collect();
    assert_eq!(usual[0], "maps/yard.ebake");
    assert_eq!(last[0], "maps/yard.efinal");
    assert!(usual.iter().all(|file| !last.contains(file)));
    assert_eq!(final_folder("yard"), "maps/yard/final/");
}

/// A bake says which map it's of.
#[test]
fn a_bake_knows_its_map() {
    let map = at(BakeLevel::Quick);
    let (_, bytes) = bake_files(&map, "yard", 77, BakeLevel::Final).remove(0);
    assert!(bake_is_for(&bytes, 77));
    assert!(!bake_is_for(&bytes, 78));
    assert!(!bake_is_for(&[1, 2, 3], 77));
}

/// What reads files from `files`.
fn reader(files: Vec<(String, Vec<u8>)>) -> SideFiles {
    let files: Mutex<HashMap<String, Vec<u8>>> = Mutex::new(files.into_iter().collect());
    Arc::new(move |path| files.lock().expect("locks").get(path).cloned())
}

/// A map with the same faces as `like`, and no light on it.
fn unlit() -> Map {
    plain().0
}

/// The game takes a final bake over a build's.
#[test]
fn a_final_bake_comes_first() {
    let (quick, last) = (at(BakeLevel::Quick), at(BakeLevel::Final));
    let mut files = bake_files(&quick, "yard", 5, BakeLevel::Usual);
    files.extend(bake_files(&last, "yard", 5, BakeLevel::Final));
    let mut map = unlit();
    assert!(apply_saved_bake(&mut map, "yard", 5, &reader(files)).expect("applies"));
    assert_eq!(map.light.len(), last.light.len());
}

/// A final bake of an older version of a level is passed over.
#[test]
fn a_stale_final_bake_is_passed_over() {
    let (quick, last) = (at(BakeLevel::Quick), at(BakeLevel::Final));
    let mut files = bake_files(&quick, "yard", 5, BakeLevel::Usual);
    files.extend(bake_files(&last, "yard", 4, BakeLevel::Final));
    let mut map = unlit();
    assert!(apply_saved_bake(&mut map, "yard", 5, &reader(files)).expect("applies"));
    assert_eq!(map.light.len(), quick.light.len());
}

/// With only a build's, that's what's used, and with neither the level is left as it is.
#[test]
fn a_level_can_have_no_bake() {
    let quick = at(BakeLevel::Quick);
    let files = bake_files(&quick, "yard", 5, BakeLevel::Usual);
    let mut map = unlit();
    assert!(apply_saved_bake(&mut map, "yard", 5, &reader(files)).expect("applies"));
    let mut bare = unlit();
    assert!(!apply_saved_bake(&mut bare, "yard", 5, &reader(Vec::new())).expect("nothing"));
    assert!(bare.light.is_empty());
}
