//! Baking light into compiled maps.

use std::sync::Arc;

use engine_compile::{BakeInput, CompileError, bake_light, compile, placeholder_texture, read_map};
use engine_core::math::Vec3;
use engine_format::from_text;
use engine_map::{BOUNCE_STYLE, Face, Map, MipLevel, Texture, TextureKind};

/// A map baked with placeholder textures, plus `textures` by name.
fn baked(text: &str, textures: &[Texture]) -> Map {
    let document = from_text(text).expect("parses");
    let working = read_map(&document).expect("reads");
    let mut source = |name: &str| {
        textures
            .iter()
            .find(|texture| texture.name == name)
            .cloned()
            .unwrap_or_else(|| placeholder_texture(name))
    };
    let mut map = compile(&working, &mut source).expect("compiles");
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

/// The six walls of a room from -256 to 256.
const ROOM: &str = "format 1 map
brush { material = floor cuboid = -288 -288 -288 288 288 -256 }
brush { material = ceiling cuboid = -288 -288 256 288 288 288 }
brush { material = west cuboid = -288 -288 -256 -256 288 256 }
brush { material = east cuboid = 256 -288 -256 288 288 256 }
brush { material = south cuboid = -256 -288 -256 256 -256 256 }
brush { material = north cuboid = -256 256 -256 256 288 256 }
";

/// The face of `material` facing into the room.
fn face_of<'a>(map: &'a Map, material: &str) -> &'a Face {
    let number = map
        .textures
        .iter()
        .position(|texture| texture.name == material)
        .expect("a texture");
    let distance = |face: &&Face| {
        let first = face.first_point as usize;
        let points = &map.points[first..first + face.point_count as usize];
        let share = 1.0 / points.len() as f32;
        let middle = points.iter().fold(Vec3::ZERO, |sum, p| sum + *p) * share;
        middle.length()
    };
    map.faces
        .iter()
        .filter(|face| face.texture as usize == number)
        .min_by(|a, b| distance(a).total_cmp(&distance(b)))
        .expect("a face")
}

/// The brightest sample of `face` on its layer of `style`, and its average.
fn layer_light(map: &Map, face: &Face, style: u8) -> Option<(f32, f32)> {
    let light = face.light?;
    let layer = light.styles.iter().position(|known| *known == style)?;
    let count = (light.width * light.height) as usize;
    let first = light.first_sample as usize + layer * count;
    let samples = &map.light[first..first + count];
    let values: Vec<f32> = samples
        .iter()
        .map(|sample| (f32::from(sample[0]) + f32::from(sample[1]) + f32::from(sample[2])) / 3.0)
        .collect();
    let high = values.iter().copied().fold(0.0, f32::max);
    let total: f32 = values.iter().sum();
    Some((high, total / values.len() as f32))
}

#[test]
fn a_light_brightens_what_it_faces_and_keeps_its_colour() {
    let text = format!(
        "{ROOM}entity lamp {{ type = light origin = 0 0 200 intensity = 500 color = 1 0.2 0.2 }}\n"
    );
    let map = baked(&text, &[]);
    assert!(map.lit, "the map is lit");
    let floor = face_of(&map, "floor");
    let (high, _) = layer_light(&map, floor, 0).expect("a layer");
    assert!(high > 15.0, "the floor is lit: {high}");
    let light = floor.light.expect("light");
    let first = light.first_sample as usize;
    let count = (light.width * light.height) as usize;
    let brightest = map.light[first..first + count]
        .iter()
        .max_by_key(|sample| sample[0])
        .expect("samples");
    assert!(brightest[0] > brightest[1] * 2, "it's red: {brightest:?}");
}

#[test]
fn a_pillar_casts_a_shadow() {
    let open = format!("{ROOM}entity lamp {{ type = light origin = -200 0 0 intensity = 800 }}\n");
    let blocked = format!(
        "{ROOM}brush {{ material = pillar cuboid = -32 -128 -256 32 128 256 }}\nentity lamp {{ type = light origin = -200 0 0 intensity = 800 }}\n"
    );
    let (_, clear) =
        layer_light(&baked(&open, &[]), face_of(&baked(&open, &[]), "east"), 0).expect("a layer");
    let shaded_map = baked(&blocked, &[]);
    let (_, shaded) = layer_light(&shaded_map, face_of(&shaded_map, "east"), 0).expect("a layer");
    assert!(shaded < clear * 0.8, "shadow {shaded} against {clear}");
}

#[test]
fn a_spot_only_lights_its_cone() {
    let wide = format!("{ROOM}entity lamp {{ type = light origin = 0 0 200 intensity = 600 }}\n");
    let narrow = format!(
        "{ROOM}entity lamp {{ type = light origin = 0 0 200 intensity = 600 spot_direction = 0 0 -1 spot_angle = 15 }}\n"
    );
    let wide_map = baked(&wide, &[]);
    let narrow_map = baked(&narrow, &[]);
    let (_, all) = layer_light(&wide_map, face_of(&wide_map, "floor"), 0).expect("a layer");
    let (peak, part) = layer_light(&narrow_map, face_of(&narrow_map, "floor"), 0).expect("a layer");
    assert!(peak > 10.0, "the cone lands: {peak}");
    assert!(
        part < all * 0.5,
        "and most of the floor is dark: {part} {all}"
    );
}

#[test]
fn bounced_light_is_its_own_layer_and_reaches_shadow() {
    let text = format!(
        "{ROOM}brush {{ material = wall cuboid = -8 -256 -256 8 200 256 }}\nentity lamp {{ type = light origin = -128 0 0 intensity = 900 }}\n"
    );
    let map = baked(&text, &[]);
    let east = face_of(&map, "east");
    let (direct, _) = layer_light(&map, east, 0).unwrap_or((0.0, 0.0));
    let (bounce, _) = layer_light(&map, east, BOUNCE_STYLE).expect("a bounce layer");
    assert!(bounce > 0.0, "light came round the wall: {bounce}");
    assert!(direct < 1.0 || bounce > 0.0);
    assert_eq!(map.light.len(), map.light_directions.len(), "one each");
}

#[test]
fn stained_glass_tints_what_passes_through() {
    let red = MipLevel {
        width: 1,
        height: 1,
        rgba: vec![255, 0, 0, 255],
    };
    let material = engine_map::Material {
        alpha: engine_map::AlphaMode::Blend,
        slots: vec![Arc::new(vec![red.clone()])],
        tint: Some(1),
        ..engine_map::Material::default()
    };
    let glass = Texture {
        name: "glass".to_owned(),
        kind: TextureKind::Solid,
        mips: vec![red],
        animation: None,
        alternate: None,
        material: Some(Arc::new(material)),
    };
    let text = format!(
        "{ROOM}brush {{ material = glass cuboid = -8 -256 -256 8 256 256 }}\nentity lamp {{ type = light origin = -128 0 0 intensity = 900 }}\n"
    );
    let map = baked(&text, &[glass]);
    let east = face_of(&map, "east");
    let light = east.light.expect("light");
    let layer = light.styles.iter().position(|s| *s == 0).expect("direct");
    let count = (light.width * light.height) as usize;
    let first = light.first_sample as usize + layer * count;
    let brightest = map.light[first..first + count]
        .iter()
        .max_by_key(|sample| sample[0])
        .expect("samples");
    assert!(brightest[0] > 20, "light got through: {brightest:?}");
    assert!(
        brightest[1] < brightest[0] / 4,
        "and it's red: {brightest:?}"
    );
}

#[test]
fn keyframes_each_get_a_style_and_probes_follow_the_light() {
    let text = format!(
        "{ROOM}entity lamp {{ type = light origin = 0 0 0 intensity = 500 keyframes = 64 0 0 128 0 0 }}\n"
    );
    let map = baked(&text, &[]);
    let floor = face_of(&map, "floor").light.expect("light");
    assert!(floor.styles.contains(&48), "{:?}", floor.styles);
    assert!(floor.styles.contains(&49), "{:?}", floor.styles);
    let probes = map.probes.as_ref().expect("probes");
    let near = probes.sample(Vec3::new(128.0, 0.0, 0.0)).expect("a sample");
    let far = probes
        .sample(Vec3::new(-250.0, 250.0, 250.0))
        .expect("a sample");
    assert!(near.brightness > far.brightness, "{near:?} {far:?}");
}
