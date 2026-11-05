//! Baking light into compiled maps.

use std::sync::Arc;

use engine_compile::{
    BakeInput, CompileError, apply_bake, bake_light, compile, placeholder_texture, read_map,
    write_bake,
};
use engine_core::math::Vec3;
use engine_format::from_text;
use engine_map::{
    AlphaMode, BOUNCE_STYLE, CRISP_STYLE, Face, Map, Material, MaterialLayer, MipLevel, PAINT_MASK,
    Texture, TextureKind,
};
use engine_physics::CollisionWorld;

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
    let material = Material {
        alpha: AlphaMode::Blend,
        slots: vec![Arc::new(vec![red.clone()])],
        tint: Some(1),
        ..Material::default()
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

/// A crisp light keeps its light whole on a layer of its own, and says where
/// its shadow's edge falls.
#[test]
fn a_crisp_light_records_where_its_shadow_ends() {
    let text = format!(
        "{ROOM}\nbrush {{ material = pillar cuboid = -32 -32 -256 32 32 256 }}\n\
         entity lamp {{ type = light origin = -200 0 0 intensity = 900 crisp = true }}\n"
    );
    let map = baked(&text, &[]);
    assert_eq!(map.light_edges.len(), map.light.len());
    let number = map
        .textures
        .iter()
        .position(|texture| texture.name == "floor")
        .expect("a texture");
    // The pillar cuts the floor into pieces, so all of them count.
    let mut places = Vec::new();
    let on_top = |face: &&Face| {
        let first = face.first_point as usize;
        map.points[first..first + face.point_count as usize]
            .iter()
            .all(|point| (point.z + 256.0).abs() < 0.01)
    };
    let faces = map.faces.iter().filter(|f| f.texture as usize == number);
    for face in faces.filter(on_top) {
        let Some(light) = face.light else {
            continue;
        };
        let Some(layer) = light.styles.iter().position(|s| *s == CRISP_STYLE) else {
            continue;
        };
        let count = (light.width * light.height) as usize;
        let first = light.first_sample as usize + layer * count;
        places.extend(first..first + count);
    }
    let edge = |at: &usize| map.light_edges[*at];
    let lit = places.iter().filter(|at| edge(at) > 128).count();
    let shaded = places.iter().filter(|at| edge(at) < 128).count();
    assert!(lit > 0 && shaded > 0, "{lit} lit, {shaded} shaded");
    let near = places
        .iter()
        .filter(|at| edge(at).abs_diff(128) < 60)
        .count();
    assert!(near > 0, "some samples are near the edge");
    // Light behind the pillar is still there, for the edge to cut.
    let shadowed = places.iter().find(|at| edge(at) < 100).expect("in shadow");
    let color = map.light[*shadowed];
    assert!(color.iter().any(|part| *part > 0), "{color:?}");
}

/// With no crisp lights, edges say nothing is cut.
#[test]
fn other_lights_have_no_edges() {
    let text = format!("{ROOM}\nentity lamp {{ type = light origin = 0 0 0 intensity = 300 }}\n");
    let map = baked(&text, &[]);
    assert!(map.light_edges.iter().all(|edge| *edge == 255));
}

/// A keyframed light is a track of its styles, and tells scripts its first.
#[test]
fn keyframes_make_a_track() {
    let text = format!(
        "{ROOM}\nentity swing {{ type = light intensity = 300 period = 6 \
         keyframes = -100 0 0  0 0 0  100 0 0 }}\n"
    );
    let map = baked(&text, &[]);
    let track = map.light_tracks[0];
    assert_eq!(track.count, 3);
    assert!((track.period - 6.0).abs() < 0.001);
    let swing = map
        .entities
        .iter()
        .find(|e| {
            e.fields
                .iter()
                .any(|(k, v)| k == "targetname" && v == "swing")
        })
        .expect("the entity");
    let field = swing.fields.iter().find(|(k, _)| k == "keyframe_style");
    assert_eq!(field.map(|(_, v)| v.as_str()), Some("48"));
}

/// A stroke leaves its layer's weight along its line and nowhere else, fading
/// to its edge.
#[test]
fn paint_follows_its_stroke() {
    let text = format!(
        "{ROOM}\nentity lamp {{ type = light origin = 0 0 0 intensity = 300 }}\n\
         paint {{ layer = 1 radius = 64 points = -256 0 -256 256 0 -256 }}\n"
    );
    let map = baked(&text, &[]);
    assert_eq!(map.paint.len(), map.light.len());
    let floor = face_of(&map, "floor");
    let light = floor.light.expect("lit");
    let (width, height) = (light.width as usize, light.height as usize);
    let first = light.first_sample as usize;
    let weights = &map.paint[first..first + width * height];
    let strongest = weights.iter().map(|w| w[1]).max().expect("some");
    assert!(strongest > 240, "{strongest}");
    assert!(weights.iter().any(|w| w[1] == 0), "not everywhere");
    assert!(weights.iter().any(|w| w[1] > 10 && w[1] < 240), "it fades");
    assert!(
        weights.iter().all(|w| w[0] == 0 && w[2] == 0),
        "only its layer"
    );
}

/// A material marked `mega` with a painted layer.
fn mega_texture() -> Texture {
    let image = |color: [u8; 4]| {
        vec![MipLevel {
            width: 4,
            height: 4,
            rgba: color.repeat(16),
        }]
    };
    Texture {
        name: "floor".to_owned(),
        kind: TextureKind::Solid,
        mips: image([200, 0, 0, 255]),
        animation: None,
        alternate: None,
        material: Some(Arc::new(Material {
            mega: true,
            slots: vec![Arc::new(image([0, 200, 0, 255]))],
            layers: vec![MaterialLayer {
                base: 1,
                mask: PAINT_MASK,
            }],
            ..Material::default()
        })),
    }
}

/// A mega surface draws from one image with its paint mixed in, and its light
/// stays where it was.
#[test]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "texel coordinates are small and positive"
)]
fn a_mega_surface_holds_its_paint() {
    let text = format!(
        "{ROOM}\nentity lamp {{ type = light origin = 0 0 0 intensity = 500 }}\n\
         paint {{ layer = 0 radius = 40 points = -256 0 -256 256 0 -256 }}\n"
    );
    let map = baked(&text, &[mega_texture()]);
    let mega = map
        .textures
        .iter()
        .position(|t| t.name == "*mega")
        .expect("the image");
    let top = map
        .faces
        .iter()
        .find(|face| {
            face.texture as usize == mega
                && map
                    .face_points(face)
                    .iter()
                    .all(|p| (p.z + 256.0).abs() < 0.01)
        })
        .expect("the floor moved there");
    let image = &map.textures[mega].mips[0];
    let at = |point: Vec3| {
        let (s, t) = (top.axes[0].project(point), top.axes[1].project(point));
        let index = (t as usize * image.width as usize + s as usize) * 4;
        [
            image.rgba[index],
            image.rgba[index + 1],
            image.rgba[index + 2],
        ]
    };
    let painted = at(Vec3::new(0.0, 0.0, -256.0));
    let bare = at(Vec3::new(0.0, 200.0, -256.0));
    assert!(painted[1] > 150 && painted[0] < 60, "{painted:?}");
    assert!(bare[0] > 150 && bare[1] < 60, "{bare:?}");
    let light = top.light.expect("still lit");
    let s = top.axes[0].project(Vec3::new(0.0, 0.0, -256.0));
    assert!(
        s >= light.texel_origin[0] as f32,
        "light moved with the coordinates"
    );
}

/// What's not marked keeps its own texture.
#[test]
fn other_surfaces_stay_out_of_the_mega_texture() {
    let text = format!("{ROOM}\nentity lamp {{ type = light origin = 0 0 0 intensity = 500 }}\n");
    let map = baked(&text, &[]);
    assert!(map.mega.is_none());
    assert!(map.textures.iter().all(|t| t.name != "*mega"));
}

/// Two coplanar faces meeting along an edge agree there, with no seam in light.
#[test]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "sample positions are small and positive"
)]
fn coplanar_faces_agree_along_their_edge() {
    let text = "format 1 map
brush { material = left cuboid = -256 -128 -32 0 128 0 }
brush { material = right cuboid = 0 -128 -32 256 128 0 }
entity lamp { type = light origin = 0 0 96 intensity = 400 }
";
    let map = baked(text, &[]);
    let near_edge = |material: &str, x: f32| {
        let face = map
            .faces
            .iter()
            .find(|face| {
                let name = &map.textures[face.texture as usize].name;
                name == material && map.face_points(face).iter().all(|p| p.z.abs() < 0.01)
            })
            .expect("a top");
        let light = face.light.expect("lit");
        let step = light.texels_per_sample as f32;
        let column = ((x - light.texel_origin[0] as f32) / step).round() as usize;
        let row = (light.height / 2) as usize;
        let at = light.first_sample as usize + row * light.width as usize;
        map.light[at + column.min(light.width as usize - 1)][0]
    };
    let left = near_edge("left", 0.0);
    let right = near_edge("right", 0.0);
    assert!(left > 0);
    assert!(left.abs_diff(right) <= 2, "{left} {right}");
}

/// Two rooms with a wall between, and `gap` as text for a brush to leave out.
const TWO_ROOMS: &str = "format 1 map
brush { material = floor cuboid = -1024 -256 -32 1024 256 0 }
brush { material = roof cuboid = -1024 -256 256 1024 256 288 }
brush { material = wall cuboid = -1056 -288 -32 -1024 288 288 }
brush { material = wall cuboid = 1024 -288 -32 1056 288 288 }
brush { material = wall cuboid = -1024 -288 -32 1024 -256 288 }
brush { material = wall cuboid = -1024 256 -32 1024 288 288 }
";

/// Whether a point in `map` sees another.
fn can_see(map: &Map, from: Vec3, to: Vec3) -> bool {
    let mut seen = vec![false; map.tree.leaves.len()];
    map.tree.mark_visible(map.tree.leaf_at(from), &mut seen);
    seen[map.tree.leaf_at(to) as usize]
}

/// Sealed rooms can't see each other, and a doorway lets them.
#[test]
fn walls_stop_sight_and_doorways_let_it_through() {
    let left = Vec3::new(-800.0, 0.0, 64.0);
    let right = Vec3::new(800.0, 0.0, 64.0);
    let sealed = format!("{TWO_ROOMS}brush {{ material = wall cuboid = -32 -256 0 32 256 256 }}\n");
    let map = baked(&sealed, &[]);
    assert!(map.tree.leaves.len() > 8, "cells have leaves");
    assert!(!can_see(&map, left, right), "a wall");
    assert!(
        can_see(&map, left, left + Vec3::new(100.0, 0.0, 0.0)),
        "its own room"
    );
    let doorway = format!(
        "{TWO_ROOMS}brush {{ material = wall cuboid = -32 -256 0 32 -64 256 }}\n\
         brush {{ material = wall cuboid = -32 64 0 32 256 256 }}\n"
    );
    let map = baked(&doorway, &[]);
    assert!(can_see(&map, left, right), "through the doorway");
}

/// With no bake there are cells, and from each of them everything shows.
#[test]
fn a_map_with_no_bake_shows_everything() {
    let document = from_text(TWO_ROOMS).expect("parses");
    let working = read_map(&document).expect("reads");
    let mut source = |name: &str| placeholder_texture(name);
    let map = compile(&working, &mut source).expect("compiles");
    assert!(map.tree.visibility.is_empty());
    assert!(can_see(
        &map,
        Vec3::new(-800.0, 0.0, 64.0),
        Vec3::new(800.0, 0.0, 64.0)
    ));
    let listed: std::collections::HashSet<u32> = map.tree.leaf_faces.iter().copied().collect();
    let world = &map.models[0];
    assert_eq!(
        listed.len(),
        world.face_count as usize,
        "every face is in a leaf"
    );
}

/// A room with a wall across it, and what's said to go in it.
fn walled(extra: &str) -> Map {
    baked(&format!("{TWO_ROOMS}{extra}"), &[])
}

/// Whether a line stays clear of the world for a player.
fn walks_clear(map: &Arc<Map>, path: &[Vec3]) -> bool {
    let world = CollisionWorld::new(Arc::clone(map));
    path.windows(2).all(|pair| {
        let lift = Vec3::new(0.0, 0.0, 4.0);
        world
            .with(&[])
            .trace(pair[0] + lift, pair[1] + lift)
            .fraction
            >= 1.0
    })
}

/// A way goes through a doorway, and never through the wall beside it.
#[test]
fn a_path_goes_through_a_doorway() {
    let map = Arc::new(walled(
        "brush { material = wall cuboid = -32 -256 0 32 -64 256 }\n\
         brush { material = wall cuboid = -32 64 0 32 256 256 }\n",
    ));
    let nav = map.nav.as_ref().expect("baked");
    let from = Vec3::new(-800.0, 200.0, 24.0);
    let to = Vec3::new(800.0, 200.0, 24.0);
    let path = nav.find_path(from, to).expect("a way");
    assert!(path.len() >= 3, "it bends: {path:?}");
    let crossing = path
        .windows(2)
        .find(|pair| pair[0].x < 0.0 && pair[1].x >= 0.0);
    let pair = crossing.expect("it crosses over");
    let share = -pair[0].x / (pair[1].x - pair[0].x);
    let y = pair[0].y + (pair[1].y - pair[0].y) * share;
    assert!(y.abs() < 64.0, "through the gap: {y}");
    assert!(walks_clear(&map, &path), "{path:?}");
}

/// A sealed room has no way out.
#[test]
fn a_sealed_room_has_no_path() {
    let map = walled("brush { material = wall cuboid = -32 -256 0 32 256 256 }\n");
    let nav = map.nav.as_ref().expect("baked");
    let from = Vec3::new(-800.0, 0.0, 24.0);
    assert!(nav.find_path(from, Vec3::new(800.0, 0.0, 24.0)).is_none());
    assert!(
        nav.find_path(from, Vec3::new(-300.0, 100.0, 24.0))
            .is_some()
    );
}

/// In the open a way is straight, however many places it crosses.
#[test]
fn an_open_path_is_straight() {
    let map = walled("");
    let nav = map.nav.as_ref().expect("baked");
    let path = nav
        .find_path(
            Vec3::new(-800.0, -100.0, 24.0),
            Vec3::new(800.0, 100.0, 24.0),
        )
        .expect("a way");
    assert_eq!(path.len(), 2, "{path:?}");
}

/// Stairs are climbed, and a floor over another keeps its own places.
#[test]
fn stairs_are_climbed_and_floors_stack() {
    let mut steps = String::new();
    for step in 0..8 {
        let (x, z) = (-200 + step * 32, (step + 1) * 16);
        steps.push_str(&format!(
            "brush {{ material = wall cuboid = {x} -128 0 {} 128 {z} }}\n",
            x + 32
        ));
    }
    steps.push_str("brush { material = wall cuboid = 56 -128 0 700 128 128 }\n");
    let map = walled(&steps);
    let nav = map.nav.as_ref().expect("baked");
    let path = nav
        .find_path(Vec3::new(-500.0, 0.0, 24.0), Vec3::new(500.0, 0.0, 152.0))
        .expect("up the stairs");
    let last = path.last().expect("an end");
    assert!((last.z - 152.0).abs() < 2.0, "on top: {last:?}");
    let under = nav.nearest(Vec3::new(500.0, 200.0, 24.0)).expect("a place");
    let over = nav.nearest(Vec3::new(500.0, 0.0, 152.0)).expect("a place");
    assert_ne!(under, over);
}

/// Lava isn't a place to stand, so a way goes round it.
#[test]
fn a_path_goes_round_lava() {
    let map = Arc::new(walled(
        "brush { contents = lava material = lava cuboid = -128 -128 -32 128 256 -8 }\n",
    ));
    let nav = map.nav.as_ref().expect("baked");
    assert!(nav.places.iter().all(|place| {
        let over = place.point.x.abs() < 100.0 && place.point.y > -100.0 && place.point.y < 230.0;
        !(over && place.point.z < 10.0)
    }));
}

/// A bake keeps its walkable grid.
#[test]
fn a_bake_keeps_the_nav_grid() {
    let working = read_map(&from_text(TWO_ROOMS).expect("parses")).expect("reads");
    let baked_map = baked(TWO_ROOMS, &[]);
    let bytes = write_bake(&baked_map, 7);
    let mut source = |name: &str| placeholder_texture(name);
    let mut map = compile(&working, &mut source).expect("compiles");
    assert!(apply_bake(&mut map, &bytes, 7).expect("reads"));
    assert_eq!(map.nav, baked_map.nav);
}

/// Probes in the open hold a map's ambient light, so things aren't black where
/// only that reaches.
#[test]
fn probes_hold_ambient_light() {
    let text = format!("{ROOM}\nentity world {{ type = worldspawn ambient = 40 }}\n");
    let map = baked(&text, &[]);
    let probes = map.probes.as_ref().expect("probes");
    let sample = probes.sample(Vec3::new(0.0, 0.0, 0.0)).expect("a sample");
    assert!(sample.brightness > 30.0, "{}", sample.brightness);
}

/// With two crisp lights, the second bakes with its own shadows, and isn't cut
/// by the first's edge.
#[test]
fn only_one_light_is_crisp_on_a_face() {
    let one = format!(
        "{ROOM}\nbrush {{ material = pillar cuboid = -32 -32 -256 32 32 256 }}\n\
         entity lamp {{ type = light origin = -200 0 0 intensity = 900 crisp = true }}\n"
    );
    let two = format!(
        "{one}entity other {{ type = light origin = 200 0 0 intensity = 900 crisp = true }}\n"
    );
    let plain = baked(&one, &[]);
    let both = baked(&two, &[]);
    let usual = |map: &Map| {
        let floor = face_of(map, "floor");
        let light = floor.light.expect("lit");
        light.styles.iter().filter(|style| **style == 0).count()
    };
    assert_eq!(usual(&plain), 0, "one crisp light has no usual layer");
    assert_eq!(usual(&both), 1, "the second has one");
}
