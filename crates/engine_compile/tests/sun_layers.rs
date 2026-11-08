//! A sun on layers of its own, so that a day can change it.

use std::sync::Arc;

use engine_compile::{BakeInput, CompileError, bake_light, compile, placeholder_texture, read_map};
use engine_format::from_text;
use engine_map::{BOUNCE_STYLE, Face, Map, MipLevel, SUN_BOUNCE_STYLE, SUN_STYLE, SunMode};

/// A yard open to the sun, with a wall to bounce off and a lamp, and `sun` in its world.
fn yard(sun: &str) -> String {
    format!(
        "format 1 map
entity world {{ type = worldspawn sun_intensity = 200 light_spacing = 32 {sun} }}
sky {{ sun_direction = -0.5 0.1 1 }}
brush {{ material = ground cuboid = -256 -256 -32 256 256 0 }}
brush {{ material = wall cuboid = 100 -256 0 132 256 256 }}
entity lamp {{ type = light origin = 0 0 64 intensity = 150 }}
"
    )
}

/// `text`, compiled and baked.
fn baked(text: &str) -> Result<Map, CompileError> {
    let working = read_map(&from_text(text).expect("parses"))?;
    let mut source = |name: &str| placeholder_texture(name);
    let mut map = compile(&working, &mut source)?;
    let mut images = |path: &str| -> Result<Arc<Vec<MipLevel>>, CompileError> {
        Err(CompileError::Bad(path.to_owned()))
    };
    let mut input = BakeInput {
        working: &working,
        images: &mut images,
    };
    bake_light(&mut map, &mut input)?;
    Ok(map)
}

/// The face of `material` that faces straight up.
fn facing_up<'a>(map: &'a Map, material: &str) -> &'a Face {
    let number = map
        .textures
        .iter()
        .position(|texture| texture.name == material);
    let number = u32::try_from(number.expect("a material")).expect("few");
    map.faces
        .iter()
        .filter(|face| face.texture == number && face.light.is_some())
        .max_by_key(|face| {
            let first = face.first_point as usize;
            let points = &map.points[first..first + face.point_count as usize];
            // The highest face of it.
            #[expect(clippy::cast_possible_truncation, reason = "small heights")]
            let height = points.iter().map(|point| point.z as i32).min().unwrap_or(0);
            // And of those the biggest, which is the side of the wall that faces the sun.
            let size = face
                .light
                .as_ref()
                .map_or(0, |light| light.width * light.height);
            (height, size)
        })
        .expect("a face")
}

/// How much light a face has on the layer of `style`, or `None` without one.
fn layer_total(map: &Map, face: &Face, style: u8) -> Option<u64> {
    let light = face.light.as_ref()?;
    let slot = light.styles.iter().position(|known| *known == style)?;
    let size = (light.width * light.height) as usize;
    let first = light.first_sample as usize + slot * size;
    let samples = &map.light[first..first + size];
    // As the game reads it, with what a bake brought down multiplied back.
    let total: u64 = samples.iter().flatten().map(|part| u64::from(*part)).sum();
    Some(total << map.light_shift)
}

/// What a level has always done is what it does unless it says.
#[test]
fn a_sun_is_fixed_unless_told() {
    let map = baked(&yard("")).expect("bakes");
    assert_eq!(map.sun_mode, SunMode::Fixed);
    let ground = facing_up(&map, "ground");
    assert!(layer_total(&map, ground, SUN_STYLE).is_none());
    assert!(layer_total(&map, ground, SUN_BOUNCE_STYLE).is_none());
    assert!(layer_total(&map, ground, 0).expect("baked in") > 0);
}

/// A baked sun has layers of its own, with the rest of the light apart from it.
#[test]
fn a_baked_sun_has_its_own_layers() {
    let map = baked(&yard("sun = baked")).expect("bakes");
    assert_eq!(map.sun_mode, SunMode::Baked);
    let ground = facing_up(&map, "ground");
    assert!(layer_total(&map, ground, SUN_STYLE).expect("its own") > 0);
    assert!(layer_total(&map, ground, SUN_BOUNCE_STYLE).expect("what it bounces") > 0);
    assert!(layer_total(&map, ground, 0).expect("the lamp") > 0);
    assert!(layer_total(&map, ground, BOUNCE_STYLE).is_some());
}

/// All of a baked sun's light is what it was, only kept apart.
#[test]
fn a_baked_sun_is_as_bright_as_a_fixed_one() {
    let fixed = baked(&yard("")).expect("bakes");
    let own = baked(&yard("sun = baked")).expect("bakes");
    let total = |map: &Map, styles: &[u8]| -> u64 {
        let ground = facing_up(map, "ground");
        styles
            .iter()
            .filter_map(|style| layer_total(map, ground, *style))
            .sum()
    };
    let before = total(&fixed, &[0, BOUNCE_STYLE]);
    let after = total(&own, &[0, BOUNCE_STYLE, SUN_STYLE, SUN_BOUNCE_STYLE]);
    let off = before.abs_diff(after) as f64 / before as f64;
    assert!(off < 0.02, "{before} {after}");
}

/// What's lit only by the lamp has none of the sun's bounce in the lamp's.
#[test]
fn the_rest_of_the_bounce_has_no_sun_in_it() {
    let lit = baked(&yard("sun = baked")).expect("bakes");
    let dark = baked(&yard("sun = baked sun_intensity = 1")).expect("bakes");
    let ground = |map: &Map| layer_total(map, facing_up(map, "ground"), BOUNCE_STYLE);
    let (lit, dark) = (
        ground(&lit).expect("bounced"),
        ground(&dark).expect("bounced"),
    );
    let off = lit.abs_diff(dark) as f64 / lit.max(1) as f64;
    assert!(off < 0.1, "{lit} {dark}");
}

/// A live sun isn't baked, but what it bounces is.
#[test]
fn a_live_sun_leaves_only_its_bounce() {
    let map = baked(&yard("sun = live")).expect("bakes");
    assert_eq!(map.sun_mode, SunMode::Live);
    let ground = facing_up(&map, "ground");
    assert!(layer_total(&map, ground, SUN_STYLE).is_none());
    assert!(layer_total(&map, ground, SUN_BOUNCE_STYLE).expect("what it bounces") > 0);
}

/// Probes say how much of their light is the sun's, for a day to work with.
#[test]
fn probes_know_the_suns_share() {
    let own = baked(&yard("sun = baked")).expect("bakes");
    let probes = own.probes.as_ref().expect("probes");
    assert_eq!(probes.sun_shares.len(), probes.colors.len());
    assert!(probes.sun_shares.iter().any(|share| *share > 128));
    let fixed = baked(&yard("")).expect("bakes");
    let probes = fixed.probes.as_ref().expect("probes");
    assert!(probes.sun_shares.iter().all(|share| *share == 0));
}

/// A live sun lights models as it goes, so it's not in the probes as well.
#[test]
fn a_live_sun_is_not_in_the_probes() {
    let total = |map: &Map| -> u64 {
        let probes = map.probes.as_ref().expect("probes");
        let all = probes.colors.iter().flatten().flatten();
        all.map(|part| u64::from(*part)).sum()
    };
    let baked_sun = baked(&yard("sun = baked")).expect("bakes");
    let live = baked(&yard("sun = live")).expect("bakes");
    assert!(total(&live) * 10 < total(&baked_sun) * 9);
}

/// A sun on its own layer gets one on every face it reaches, however many lamps there are.
#[test]
fn a_sun_always_gets_its_layer() {
    // A dim sun, and bright lamps, so that only putting the sun first gives it a layer.
    let mut text = yard("sun = baked").replace("sun_intensity = 200", "sun_intensity = 20");
    for style in 1..6 {
        let at = style * 30 - 90;
        text.push_str(&format!(
            "entity lamp{style} {{ type = light origin = {at} 0 40 intensity = 600 style = {style} }}\n"
        ));
    }
    let map = baked(&text).expect("bakes");
    let ground = facing_up(&map, "ground");
    assert!(layer_total(&map, ground, SUN_STYLE).is_some());
    assert!(layer_total(&map, ground, SUN_BOUNCE_STYLE).is_some());
}

/// A sharp sun can't go on a layer of its own, as those go to whichever light needs them.
#[test]
fn a_sun_of_its_own_is_not_crisp() {
    let map = baked(&yard("sun = baked sun_crisp = true")).expect("bakes");
    let ground = facing_up(&map, "ground");
    assert!(layer_total(&map, ground, SUN_STYLE).is_some());
}

/// What isn't a kind of sun is the author's to hear about.
#[test]
fn a_sun_has_to_be_a_kind_of_sun() {
    let error = baked(&yard("sun = moon")).expect_err("no such sun");
    assert!(error.to_string().contains("moon"), "{error}");
}
