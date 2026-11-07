//! Baking light into compiled maps.

use std::collections::HashMap;
use std::sync::Arc;

use engine_assets::{DirectoryArchive, FileSystem};
use engine_compile::{
    BakeInput, CompileError, LoadedMesh, MaterialLibrary, apply_bake, attach_side_files,
    bake_folder, bake_light, compile, map_hash, placeholder_texture, read_map, read_mesh,
    resolve_meshes, side_files, write_bake,
};
use engine_core::math::Vec3;
use engine_format::from_text;
use engine_map::{
    AlphaMode, BOUNCE_STYLE, CRISP_STYLES, Face, Map, Material, MaterialLayer, MipLevel,
    PAINT_MASK, SideFiles, Texture, TextureKind,
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
    // All in shadow, it has no layer for that light at all.
    let east = face_of(&shaded_map, "east");
    let shaded = layer_light(&shaded_map, east, 0).map_or(0.0, |(_, light)| light);
    assert!(shaded < clear * 0.8, "shadow {shaded} against {clear}");
}

/// How much of a lamp reaches the east wall past a pane placed by `field`.
fn past_a_pane(field: &str) -> f32 {
    let pane = "format 1 mesh
part pane {
    material = stone
    points = 0 -256 -256  0 256 -256  0 256 256  0 -256 256
    uvs = 0 0  1 0  1 1  0 1
    triangles = 0 1 2  0 2 3  2 1 0  3 2 0
}
hull { cuboid = -1 -256 -256 1 256 256 }
";
    let text = format!(
        "{ROOM}mesh {{ source = pane {field} }}
entity lamp {{ type = light origin = -200 0 0 intensity = 800 }}
"
    );
    let mut working = read_map(&from_text(&text).expect("parses")).expect("reads");
    let mesh = read_mesh(&from_text(pane).expect("parses")).expect("a mesh");
    let mut read = |_: &str| {
        Ok(LoadedMesh {
            mesh: Arc::new(mesh.clone()),
            occluders: Vec::new(),
        })
    };
    resolve_meshes(&mut working, &mut read).expect("placed");
    let mut source = |name: &str| placeholder_texture(name);
    let mut map = compile(&working, &mut source).expect("compiles");
    let mut images = |path: &str| -> Result<Arc<Vec<MipLevel>>, CompileError> {
        Err(CompileError::Bad(path.to_owned()))
    };
    let mut input = BakeInput {
        working: &working,
        images: &mut images,
    };
    bake_light(&mut map, &mut input).expect("bakes");
    layer_light(&map, face_of(&map, "east"), 0).map_or(0.0, |(_, light)| light)
}

/// The sun stops at the top of the world, and a mesh taller than any brush is
/// still in its way.
#[test]
fn the_sun_finds_a_mesh_above_every_brush() {
    let tower = "format 1 mesh
part roof {
    material = stone
    points = -600 -600 500  600 -600 500  600 600 500  -600 600 500
    uvs = 0 0  1 0  1 1  0 1
    triangles = 0 1 2  0 2 3  2 1 0  3 2 0
}
hull { cuboid = -600 -600 499 600 600 500 }
";
    let open_sky = "format 1 map
entity world { type = worldspawn sun_intensity = 200 }
sky { sun_direction = 0 0 1 }
brush { material = ground cuboid = -512 -512 -64 512 512 0 }
";
    let bright = |text: &str, meshes: bool| {
        let mut working = read_map(&from_text(text).expect("parses")).expect("reads");
        if meshes {
            let mesh = read_mesh(&from_text(tower).expect("parses")).expect("a mesh");
            let mut read = |_: &str| {
                Ok(LoadedMesh {
                    mesh: Arc::new(mesh.clone()),
                    occluders: Vec::new(),
                })
            };
            resolve_meshes(&mut working, &mut read).expect("placed");
        }
        let mut source = |name: &str| placeholder_texture(name);
        let mut map = compile(&working, &mut source).expect("compiles");
        let mut images = |path: &str| -> Result<Arc<Vec<MipLevel>>, CompileError> {
            Err(CompileError::Bad(path.to_owned()))
        };
        let mut input = BakeInput {
            working: &working,
            images: &mut images,
        };
        bake_light(&mut map, &mut input).expect("bakes");
        layer_light(&map, face_of(&map, "ground"), 0).map_or(0.0, |layer| layer.0)
    };
    let clear = bright(open_sky, false);
    let roofed = bright(
        &format!(
            "{open_sky}mesh {{ source = roof }}
"
        ),
        true,
    );
    assert!(clear > 20.0, "{clear}");
    assert!(roofed < clear * 0.2, "{roofed} against {clear}");
}

/// Each sample of `style` along the column of `face` nearest `x`, as red.
fn column_at(map: &Map, face: &Face, x: i32, style: u8) -> Vec<u8> {
    let light = face.light.expect("lit");
    let (width, height) = (light.width as usize, light.height as usize);
    // With none of that light, a face has no layer for it.
    let Some(layer) = light.styles.iter().position(|known| *known == style) else {
        return vec![0; height];
    };
    let first = light.first_sample as usize + layer * width * height;
    let column = usize::try_from((x - light.texel_origin[0]) / 32).expect("on the face");
    (0..height)
        .map(|row| map.light[first + row * width + column][0])
        .collect()
}

/// Two faces side by side agree where they join, so a shadow crossing the join
/// runs straight on. The join is between samples, as it is more often than not:
/// the left face's last sample lies over the right face.
#[test]
fn a_shadow_runs_straight_over_a_join() {
    let text = "format 1 map
entity world { type = worldspawn light_spacing = 32 }
brush { material = left cuboid = -512 -512 -64 20 512 0 }
brush { material = right cuboid = 20 -512 -64 512 512 0 }
brush { material = post cuboid = 40 -40 0 120 40 200 }
entity lamp { type = light origin = 300 150 120 intensity = 900 }
";
    let map = baked(text, &[]);
    let (left, right) = (face_of(&map, "left"), face_of(&map, "right"));
    let (one, other) = (column_at(&map, left, 32, 0), column_at(&map, right, 32, 0));
    let most = one
        .iter()
        .zip(&other)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .expect("samples");
    assert!(most <= 2, "{one:?} against {other:?}");
    assert!(one.iter().any(|sample| *sample > 20), "{one:?}");
    assert!(one.contains(&0), "the shadow crosses: {one:?}");
}

/// The edges of crisp light down the column of `face` nearest `x`.
fn edges_at(map: &Map, face: &Face, x: i32) -> Vec<u8> {
    let light = face.light.expect("lit");
    let layer = light
        .styles
        .iter()
        .position(|known| *known == 62)
        .expect("a crisp layer");
    let (width, height) = (light.width as usize, light.height as usize);
    let first = light.first_sample as usize + layer * width * height;
    let column = usize::try_from((x - light.texel_origin[0]) / 32).expect("on the face");
    (0..height)
        .map(|row| map.light_edges[first + row * width + column])
        .collect()
}

/// Two faces hold the same samples either side of a join, so what blends between
/// them is the same on both, right down to how far a crisp shadow's edge is.
#[test]
fn faces_hold_the_same_samples_at_a_join() {
    let text = "format 1 map
entity world { type = worldspawn light_spacing = 32 }
brush { material = left cuboid = -512 -512 -64 20 512 0 }
brush { material = right cuboid = 20 -512 -64 512 512 0 }
brush { material = post cuboid = -60 -40 4 60 40 200 }
entity lamp { type = light origin = 240 190 90 intensity = 900 crisp = true }
";
    let map = baked(text, &[]);
    let (left, right) = (face_of(&map, "left"), face_of(&map, "right"));
    let rows = |face: &Face| face.light.expect("lit").texel_origin[1];
    assert_eq!(rows(left), rows(right), "the same rows");
    for x in [0, 32] {
        let one = edges_at(&map, left, x);
        assert_eq!(one, edges_at(&map, right, x), "at {x}");
        assert!(one.iter().any(|edge| *edge < 128), "{one:?}");
        assert!(one.iter().any(|edge| *edge > 128), "{one:?}");
    }
}

/// Each face ranks its layers by how much of each light it gets, so two faces side by
/// side can hold the same lights in a different order. What they share has to go by
/// which light it is, not by where it is on the list.
#[test]
fn faces_with_their_layers_in_another_order_still_agree() {
    let text = "format 1 map
entity world { type = worldspawn light_spacing = 32 }
brush { material = left cuboid = -1024 -512 -64 20 512 0 }
brush { material = right cuboid = 20 -512 -64 1024 512 0 }
entity one { type = light origin = -500 0 100 intensity = 900 }
entity two { type = light origin = 500 0 100 intensity = 900 style = 5 }
";
    let map = baked(text, &[]);
    let (left, right) = (face_of(&map, "left"), face_of(&map, "right"));
    let styles = |face: &Face| face.light.expect("lit").styles;
    assert_ne!(styles(left), styles(right), "in another order");
    for style in [0, 5] {
        let one = column_at(&map, left, 32, style);
        let other = column_at(&map, right, 32, style);
        let most = one.iter().zip(&other).map(|(a, b)| a.abs_diff(*b)).max();
        assert!(
            most.expect("samples") <= 1,
            "{style}: {one:?} and {other:?}"
        );
    }
}

/// A wall stands on the join between two floors. What's lit on one side stays
/// there, though their samples fall in the same places.
#[test]
fn faces_share_no_samples_through_a_wall() {
    let text = "format 1 map
entity world { type = worldspawn light_spacing = 32 }
brush { material = left cuboid = -512 -512 -64 0 512 0 }
brush { material = right cuboid = 0 -512 -64 512 512 0 }
brush { material = wall cuboid = -8 -512 0 8 512 400 }
entity lamp { type = light origin = -100 0 100 intensity = 900 }
";
    let map = baked(text, &[]);
    let (left, right) = (face_of(&map, "left"), face_of(&map, "right"));
    let lit = column_at(&map, left, -32, 0);
    assert!(lit.iter().any(|sample| *sample > 20), "{lit:?}");
    for x in [0, 32, 64] {
        let column = column_at(&map, right, x, 0);
        assert!(
            column.iter().all(|sample| *sample == 0),
            "at {x}: {column:?}"
        );
    }
}

/// Light goes through what's only seen, as it wouldn't through anything else.
#[test]
fn light_goes_through_a_mesh_only_to_be_seen() {
    let blocked = past_a_pane("");
    let clear = past_a_pane("only_seen = 1");
    assert!(blocked < clear * 0.5, "{blocked} against {clear}");
}

/// Faces are shared out among threads as they come free, and each still gets
/// its own light back.
#[test]
fn a_map_of_many_faces_keeps_each_ones_light() {
    let mut text = ROOM.to_owned();
    for index in 0..60 {
        let x = -240 + index * 8;
        text += &format!(
            "brush {{ material = post{index} cuboid = {x} -40 -256 {} 40 200 }}\n",
            x + 4
        );
    }
    text += "entity lamp { type = light origin = -249 0 0 intensity = 300 }\n";
    let map = baked(&text, &[]);
    let brightest = |name: &str| {
        let number = map.textures.iter().position(|texture| texture.name == name);
        map.faces
            .iter()
            .filter(|face| Some(face.texture as usize) == number)
            .filter_map(|face| layer_light(&map, face, 0))
            .map(|(most, _)| most)
            .fold(0.0, f32::max)
    };
    let (near, far) = (brightest("post0"), brightest("post59"));
    assert!(near > far + 1.0, "{near} against {far}");
}

/// Bounced light is worked out on a coarser grid, and what's between is drawn
/// from it, so a wide wall's shading has no steps in it.
#[test]
fn bounced_light_runs_smoothly_along_a_wall() {
    let lamp = "entity lamp { type = light origin = 0 0 240 intensity = 900 }\n";
    let map = baked(&format!("{ROOM}{lamp}"), &[]);
    let face = face_of(&map, "east");
    let light = face.light.expect("lit");
    let layer = light
        .styles
        .iter()
        .position(|style| *style == BOUNCE_STYLE)
        .expect("a bounced layer");
    let count = (light.width * light.height) as usize;
    let first = light.first_sample as usize + layer * count;
    let samples = &map.light[first..first + count];
    let width = light.width as usize;
    let row = &samples[width * (light.height as usize / 2)..][..width];
    // Each end of a row hangs off the face, and really does fall away.
    let row = &row[2..width - 2];
    let bright: Vec<i32> = row.iter().map(|sample| i32::from(sample[0])).collect();
    let steps: Vec<i32> = bright.windows(2).map(|pair| pair[1] - pair[0]).collect();
    let largest = steps.iter().map(|step| step.abs()).max().expect("samples");
    assert!(largest <= 12, "{bright:?}");
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
        form: engine_map::PixelForm::Rgba,
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
        let Some(layer) = light.styles.iter().position(|s| *s == CRISP_STYLES[0]) else {
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
        .find(|e| e.fields.iter().any(|(k, v)| k == "name" && v == "swing"))
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
            form: engine_map::PixelForm::Rgba,
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
                surface: None,
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

/// A mega surface keeps its normal, specular and emissive maps, with each
/// material's strengths in the texels, and they survive a bake.
#[test]
fn a_mega_surface_keeps_its_other_maps() {
    let image = |color: [u8; 4]| {
        Arc::new(vec![MipLevel {
            width: 4,
            height: 4,
            rgba: color.repeat(16),
            form: engine_map::PixelForm::Rgba,
        }])
    };
    let mut texture = mega_texture();
    texture.material = Some(Arc::new(Material {
        mega: true,
        slots: vec![
            image([10, 20, 250, 255]),
            image([200, 100, 0, 255]),
            image([0, 80, 160, 255]),
        ],
        normal: Some(1),
        specular: Some(2),
        emissive: Some(3),
        specular_strength: 2.0,
        gloss: 64.0,
        emissive_strength: 1.0,
        ..Material::default()
    }));
    let text = format!("{ROOM}\nentity lamp {{ type = light origin = 0 0 0 intensity = 500 }}\n");
    let map = baked(&text, &[texture]);
    let mega = map
        .textures
        .iter()
        .find(|t| t.name == "*mega")
        .expect("the image");
    let face = map
        .faces
        .iter()
        .find(|face| map.textures[face.texture as usize].name == "*mega")
        .expect("a face");
    let material = mega.material.as_ref().expect("a material");
    let corner = map.face_points(face)[0];
    let inward = map.face_points(face)[2] - corner;
    let point = corner + inward * 0.5;
    let texel_of = |slot: Option<u8>| {
        let image = &material.slots[usize::from(slot.expect("a slot")) - 1][0];
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "texel coordinates are small and positive"
        )]
        let (s, t) = (
            face.axes[0].project(point) as usize,
            face.axes[1].project(point) as usize,
        );
        let at = (t * image.width as usize + s) * 4;
        [
            image.rgba[at],
            image.rgba[at + 1],
            image.rgba[at + 2],
            image.rgba[at + 3],
        ]
    };
    assert_eq!(texel_of(material.normal), [10, 20, 250, 255]);
    assert_eq!(texel_of(material.specular), [100, 50, 0, 64], "scaled");
    assert_eq!(texel_of(material.emissive), [0, 20, 40, 255], "scaled");
    assert!((material.specular_strength - 4.0).abs() < f32::EPSILON);
}

/// What a layer brings of its own goes into the mega texture where it's painted, as its colour
/// does.
#[test]
fn a_mega_surface_holds_a_layers_bumps_and_shine() {
    let image = |color: [u8; 4]| {
        Arc::new(vec![MipLevel {
            width: 4,
            height: 4,
            rgba: color.repeat(16),
            form: engine_map::PixelForm::Rgba,
        }])
    };
    let mut texture = mega_texture();
    texture.material = Some(Arc::new(Material {
        mega: true,
        slots: vec![image([0, 200, 0, 255]), image([40, 220, 128, 200])],
        layers: vec![MaterialLayer {
            base: 1,
            mask: PAINT_MASK,
            surface: Some(2),
        }],
        gloss: 64.0,
        ..Material::default()
    }));
    let text = format!(
        "{ROOM}\nentity lamp {{ type = light origin = 0 0 0 intensity = 500 }}\n\
         paint {{ layer = 0 radius = 40 points = -256 0 -256 256 0 -256 }}\n"
    );
    let map = baked(&text, &[texture]);
    let mega = map
        .textures
        .iter()
        .find(|t| t.name == "*mega")
        .expect("the image");
    let material = mega.material.as_ref().expect("a material");
    let face = map
        .faces
        .iter()
        .find(|face| {
            map.textures[face.texture as usize].name == "*mega"
                && map
                    .face_points(face)
                    .iter()
                    .all(|point| (point.z + 256.0).abs() < 1.0)
        })
        .expect("the floor");
    let texel_of = |slot: Option<u8>, point: Vec3| {
        let image = &material.slots[usize::from(slot.expect("a slot")) - 1][0];
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "texel coordinates are small and positive"
        )]
        let (s, t) = (
            face.axes[0].project(point) as usize,
            face.axes[1].project(point) as usize,
        );
        let at = (t * image.width as usize + s) * 4;
        [image.rgba[at], image.rgba[at + 1], image.rgba[at + 2]]
    };
    let painted = Vec3::new(0.0, 0.0, -256.0);
    let bare = Vec3::new(200.0, 200.0, -256.0);
    let bump = texel_of(material.normal, painted);
    assert!(bump[0] < 50 && bump[1] > 210, "the layer's: {bump:?}");
    let flat = texel_of(material.normal, bare);
    assert!(
        flat[0].abs_diff(128) < 2,
        "and none of it where it isn't: {flat:?}"
    );
    let shine = texel_of(material.specular, painted);
    assert!(shine[0] > 40, "its shine, scaled: {shine:?}");
    assert_eq!(texel_of(material.specular, bare)[0], 0);
}

/// Reading a bake puts mega surfaces and their light just where baking did.
#[test]
fn a_bake_keeps_mega_surfaces_where_they_were() {
    let text = format!("{ROOM}\nentity lamp {{ type = light origin = 0 0 0 intensity = 500 }}\n");
    let baked_map = baked(&text, &[mega_texture()]);
    let bytes = write_bake(&baked_map, 7);
    let working = read_map(&from_text(&text).expect("parses")).expect("reads");
    let mut source = |name: &str| {
        if name == "floor" {
            mega_texture()
        } else {
            placeholder_texture(name)
        }
    };
    let mut map = compile(&working, &mut source).expect("compiles");
    assert!(apply_bake(&mut map, &bytes, 7).expect("reads"));
    assert_eq!(map.faces.len(), baked_map.faces.len());
    let moved = baked_map
        .faces
        .iter()
        .filter(|face| baked_map.textures[face.texture as usize].name == "*mega");
    assert!(moved.count() > 1, "there's something to compare");
    for (read, made) in map.faces.iter().zip(&baked_map.faces) {
        assert_eq!(read.light, made.light);
        assert_eq!(read.axes, made.axes);
        assert_eq!(read.texture, made.texture);
    }
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

/// A doorway a player fits and a large monster doesn't has a way for one and
/// not the other, and a wider one has a way for both.
#[test]
fn paths_fit_what_walks_them() {
    let gap = |half: u32| {
        format!(
            "brush {{ material = wall cuboid = -32 -256 0 32 -{half} 256 }}\n\
             brush {{ material = wall cuboid = -32 {half} 0 32 256 256 }}\n"
        )
    };
    let (from, to) = (Vec3::new(-800.0, 0.0, 24.0), Vec3::new(800.0, 0.0, 24.0));
    let narrow = walled(&gap(24));
    let wide = walled(&gap(64));
    let way = |map: &Map, width: f32| {
        let nav = map.nav_for(width).expect("baked");
        nav.find_path(from, to).is_some()
    };
    assert!(way(&narrow, 32.0), "a player fits");
    assert!(!way(&narrow, 64.0), "a large monster doesn't");
    assert!(way(&wide, 64.0), "until it's wider");
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

/// A tiled map's grids go beside its bake, and paths on what's read back are
/// the paths it was baked with.
#[test]
fn a_tiled_map_reads_its_grids_from_beside_its_bake() {
    let text = format!("{TWO_ROOMS}tile_grid {{ size = 1024 }}\n");
    let baked_map = baked(&text, &[]);
    assert!(baked_map.tile_size > 0.0, "it's tiled");
    let (from, to) = (Vec3::new(-800.0, 0.0, 24.0), Vec3::new(-300.0, 100.0, 24.0));
    let expected = baked_map.find_path(32.0, from, to).expect("a way");
    let bytes = write_bake(&baked_map, 7);
    let beside: HashMap<String, Vec<u8>> = side_files(&baked_map)
        .into_iter()
        .map(|(name, bytes)| (format!("{}{name}", bake_folder("here")), bytes))
        .collect();
    assert!(!beside.is_empty());
    let working = read_map(&from_text(&text).expect("parses")).expect("reads");
    let mut source = |name: &str| placeholder_texture(name);
    let mut map = compile(&working, &mut source).expect("compiles");
    assert!(apply_bake(&mut map, &bytes, 7).expect("reads"));
    assert!(map.nav.is_none(), "the bake doesn't hold it");
    let files: SideFiles = Arc::new(move |path| beside.get(path).cloned());
    attach_side_files(&mut map, "here", &files);
    assert_eq!(map.find_path(32.0, from, to).expect("a way"), expected);
}

/// A tiled map's light goes beside its bake, tile by tile. Held, a point's light
/// is what it was when baked, and with nothing held it's near that, from the
/// face's average.
#[test]
#[expect(clippy::float_cmp, reason = "the same sums on the same bytes")]
fn a_tiled_map_reads_its_light_from_beside_its_bake() {
    let text = format!(
        "{TWO_ROOMS}tile_grid {{ size = 1024 }}\n\
         entity lamp {{ type = light origin = -600 0 128 intensity = 900 }}\n"
    );
    let baked_map = baked(&text, &[]);
    let styles = [1.0; 64];
    let point = Vec3::new(-600.0, 0.0, 24.0);
    let expected = baked_map.light_at(point, &styles);
    assert!(expected > 10.0, "it's lit: {expected}");
    let bytes = write_bake(&baked_map, 7);
    let beside: HashMap<String, Vec<u8>> = side_files(&baked_map)
        .into_iter()
        .map(|(name, bytes)| (format!("{}{name}", bake_folder("here")), bytes))
        .collect();
    let working = read_map(&from_text(&text).expect("parses")).expect("reads");
    let mut source = |name: &str| placeholder_texture(name);
    let mut map = compile(&working, &mut source).expect("compiles");
    assert!(apply_bake(&mut map, &bytes, 7).expect("reads"));
    assert!(map.light.len() < baked_map.light.len(), "it went elsewhere");
    let files: SideFiles = Arc::new(move |path| beside.get(path).cloned());
    attach_side_files(&mut map, "here", &files);
    let far = map.light_at(point, &styles);
    assert!(far > 0.0, "an average's better than dark");
    let tiles = map.light_tiles.as_ref().expect("tiled");
    assert!(tiles.cells().len() >= 2, "{:?}", tiles.cells());
    for tile in 0..u32::try_from(tiles.cells().len()).expect("few") {
        let read = tiles.read(tile).expect("it's there");
        tiles.hold(tile, Some(Arc::new(read)));
    }
    assert_eq!(map.light_at(point, &styles), expected);
    for tile in 0..u32::try_from(tiles.cells().len()).expect("few") {
        tiles.hold(tile, None);
    }
    assert_eq!(map.light_at(point, &styles), far, "let go again");
}

/// A path across `tile_world`, from its real bake and the files beside it, which
/// goes through the gaps between yards, past where one chunk ends.
#[test]
#[ignore = "needs tile_world baked"]
fn a_path_crosses_the_real_tile_world() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let mut filesystem = FileSystem::new();
    filesystem.mount(Box::new(DirectoryArchive::new(&root)));
    let text = std::fs::read_to_string(root.join("maps/tile_world.emap")).expect("the map");
    let document = from_text(&text).expect("parses");
    let mut working = read_map(&document).expect("reads");
    let mut library = MaterialLibrary::new(&filesystem);
    let used = library.prepare(&mut working).expect("prepares");
    let mut source = |name: &str| library.load(name).expect("loads");
    let mut map = compile(&working, &mut source).expect("compiles");
    let bake = std::fs::read(root.join("maps/tile_world.ebake")).expect("baked");
    assert!(apply_bake(&mut map, &bake, map_hash(&document, &used)).expect("reads"));
    let folder = root.clone();
    let files: SideFiles = Arc::new(move |path| std::fs::read(folder.join(path)).ok());
    attach_side_files(&mut map, "tile_world", &files);
    let (from, to) = (
        Vec3::new(1024.0, 400.0, 24.0),
        Vec3::new(5000.0, 5000.0, 24.0),
    );
    let path = map.find_path(32.0, from, to).expect("a way");
    assert!(path.len() > 2, "it goes round walls: {path:?}");
    let last = *path.last().expect("a point");
    assert!((last - to).length() < 96.0, "{last:?}");
}

/// A room's bounced light doesn't depend on how many tiles a map has, as it would
/// if they all shared a budget.
#[test]
fn far_tiles_do_not_change_bounced_light() {
    let room = |x: i32| {
        format!(
            "brush {{ material = wall cuboid = {} -512 -32 {} 512 0 }}\n\
             brush {{ material = wall cuboid = {} -512 256 {} 512 288 }}\n\
             brush {{ material = wall cuboid = {} -512 0 {} -480 256 }}\n\
             brush {{ material = wall cuboid = {} 480 0 {} 512 256 }}\n\
             brush {{ material = wall cuboid = {} -512 0 {} 512 256 }}\n\
             brush {{ material = wall cuboid = {} -512 0 {} 512 256 }}\n\
             entity {{ type = light origin = {} 0 200 intensity = 500 }}\n",
            x,
            x + 1024,
            x,
            x + 1024,
            x,
            x + 1024,
            x,
            x + 1024,
            x,
            x + 32,
            x + 992,
            x + 1024,
            x + 512,
        )
    };
    // Only the bounced layer, as direct light would hide any difference.
    let mut styles = [0.0; 64];
    styles[usize::from(BOUNCE_STYLE)] = 1.0;
    let bounced = |count: i32| {
        let rooms: String = (0..count).map(|index| room(index * 1024)).collect();
        let text = format!(
            "format 1 map
entity world {{ type = worldspawn ambient = 0 bounce = 1              light_spacing = 16 }}
tile_grid {{ size = 1024 }}
{rooms}"
        );
        let map = baked(&text, &[]);
        [300.0, 700.0].map(|x| map.light_at(Vec3::new(2048.0 + x, -300.0, 24.0), &styles))
    };
    let (few, many) = (bounced(5), bounced(32));
    for (few, many) in few.iter().zip(&many) {
        assert!(*few > 1.0, "it's lit");
        assert!((few - many).abs() < few * 0.05, "{few} {many}");
    }
}

/// A torch hard up against a wall keeps its colour in the light it leaves, where
/// it's far too bright for a byte.
#[test]
fn a_bright_light_keeps_its_colour_in_a_bake() {
    let text = format!(
        "{ROOM}\nentity lamp {{ type = light origin = 0 0 -230 intensity = 6000 color = 1 0.5 0.25 }}\n"
    );
    let map = baked(&text, &[]);
    let hottest = map
        .light
        .iter()
        .max_by_key(|sample| sample[0])
        .expect("light");
    assert_eq!(hottest[0], 255, "it's past what a byte holds");
    // Ambient and bounce are grey, so it's a little less pure than the lamp.
    let [_, green, blue] = hottest.map(i32::from);
    assert!((100..=150).contains(&green), "{hottest:?}");
    assert!(blue < green, "{hottest:?}");
}

/// A wall struck at a shallow angle keeps most of its light, so a lamp near a
/// corner lights the corner, rather than a circle on each wall with nothing
/// between them. A lamp can ask for all of the angle to count.
#[test]
fn a_lamp_near_a_corner_lights_the_corner() {
    let glow = |extra: &str| {
        let text = format!(
            "{ROOM}
entity lamp {{ type = light origin = -200 -200 -216 intensity = 200 {extra} }}
"
        );
        let map = baked(&text, &[]);
        let styles = [1.0; 64];
        let at = |x: f32, y: f32| map.light_at(Vec3::new(x, y, -230.0), &styles);
        // Under the lamp, and out towards the corner of the room.
        (at(-200.0, -200.0), at(-248.0, -248.0), at(-100.0, -200.0))
    };
    let (under, corner, away) = glow("");
    assert!(corner > under * 0.6, "{under} {corner}");
    let (under_hard, _, away_hard) = glow("angle_scale = 1");
    assert!(away > away_hard * 1.2, "{away} {away_hard} {under_hard}");
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
    assert_eq!(map.nav_large, baked_map.nav_large);
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

/// Whether every pair of `points` with a clear line between them is in each
/// other's visible sets, which is what keeps anything from vanishing.
fn sets_hold_every_line(map: &Arc<Map>, eyes: &[Vec3], points: &[Vec3]) -> Result<usize, String> {
    let world = CollisionWorld::new(Arc::clone(map));
    let collision = world.with(&[]);
    let mut lines = 0;
    for from in eyes {
        let mut seen = vec![false; map.tree.leaves.len()];
        map.tree.mark_visible(map.tree.leaf_at(*from), &mut seen);
        for to in points {
            let leaf = map.tree.leaf_at(*to) as usize;
            // A point, not a player: what's near a wall is still in plain sight.
            let line = collision.move_box(*from, Vec3::ZERO, Vec3::ZERO, *to);
            if leaf == 0 || line.fraction < 1.0 {
                continue;
            }
            lines += 1;
            if !seen[leaf] {
                return Err(format!("{from:?} sees {to:?}, which its set hides"));
            }
        }
    }
    Ok(lines)
}

/// Points spread through `low` to `high`, the same every time.
fn scattered(low: Vec3, high: Vec3, count: u32) -> Vec<Vec3> {
    let mut seed = 12345_u32;
    let mut next = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f32::from(u16::try_from(seed >> 16).unwrap_or(0)) / 65535.0
    };
    (0..count)
        .map(|_| {
            Vec3::new(
                low.x + (high.x - low.x) * next(),
                low.y + (high.y - low.y) * next(),
                low.z + (high.z - low.z) * next(),
            )
        })
        .collect()
}

/// Nothing a clear line reaches is hidden, through a doorway and a slit.
#[test]
fn visible_sets_never_hide_what_a_line_reaches() {
    let map = Arc::new(walled(
        "brush { material = wall cuboid = -32 -256 0 32 -64 256 }\n\
         brush { material = wall cuboid = -32 64 0 32 256 256 }\n\
         brush { material = wall cuboid = -32 -64 96 32 64 256 }\n\
         brush { material = wall cuboid = 400 -256 0 432 -8 256 }\n\
         brush { material = wall cuboid = 400 8 0 432 256 256 }\n",
    ));
    let points = scattered(
        Vec3::new(-1000.0, -240.0, 24.0),
        Vec3::new(1000.0, 240.0, 230.0),
        300,
    );
    let lines = sets_hold_every_line(&map, &points, &points).expect("no line is hidden");
    assert!(lines > 1000, "enough of them to mean something: {lines}");
}

/// The same on the showcase, which is too slow to bake with every test run.
#[test]
#[ignore = "bakes a whole map"]
#[expect(clippy::cast_precision_loss, reason = "faces have few corners")]
fn the_showcase_hides_nothing_a_line_reaches() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let mut filesystem = FileSystem::new();
    filesystem.mount(Box::new(DirectoryArchive::new(&root)));
    let text = std::fs::read_to_string(root.join("maps/showcase.emap")).expect("the map");
    let mut working = read_map(&from_text(&text).expect("parses")).expect("reads");
    let mut library = MaterialLibrary::new(&filesystem);
    library.prepare(&mut working).expect("prepares");
    let mut source = |name: &str| library.load(name).expect("loads");
    let mut map = compile(&working, &mut source).expect("compiles");
    let mut images = MaterialLibrary::new(&filesystem);
    let mut read = |path: &str| images.image_at(path);
    let mut input = BakeInput {
        working: &working,
        images: &mut read,
    };
    bake_light(&mut map, &mut input).expect("bakes");
    let map = Arc::new(map);
    // From where a player can stand, to just off every surface.
    let nav = map.nav.as_ref().expect("a nav grid");
    let start = nav
        .nearest(Vec3::new(160.0, 384.0, 24.0))
        .expect("the start");
    let mut reached = vec![false; nav.places.len()];
    reached[start as usize] = true;
    let mut waiting = vec![start];
    while let Some(place) = waiting.pop() {
        for link in nav.places[place as usize].links {
            if link != u32::MAX && !reached[link as usize] {
                reached[link as usize] = true;
                waiting.push(link);
            }
        }
    }
    let standing: Vec<Vec3> = nav
        .places
        .iter()
        .zip(&reached)
        .filter(|(_, reached)| **reached)
        .map(|(place, _)| place.point + Vec3::new(0.0, 0.0, 22.0))
        .collect();
    let step = (standing.len() / 400).max(1);
    let eyes: Vec<Vec3> = standing.iter().step_by(step).copied().collect();
    assert!(eyes.len() > 100, "{}", eyes.len());
    let world = &map.models[0];
    let faces =
        &map.faces[world.first_face as usize..(world.first_face + world.face_count) as usize];
    let mut surfaces = Vec::new();
    for face in faces {
        let points = map.face_points(face);
        let [a, b, c, ..] = points else {
            continue;
        };
        let (u, v) = (*b - *a, *c - *a);
        let normal = Vec3::new(
            u.y * v.z - u.z * v.y,
            u.z * v.x - u.x * v.z,
            u.x * v.y - u.y * v.x,
        )
        .normalized();
        let share = 1.0 / points.len() as f32;
        let middle = points.iter().fold(Vec3::ZERO, |sum, at| sum + *at * share);
        surfaces.push(middle + normal * 2.0);
        surfaces.push(middle - normal * 2.0);
    }
    let lines = sets_hold_every_line(&map, &eyes, &surfaces).expect("no line is hidden");
    assert!(lines > 1000, "{lines}");
}

/// A floor, open above, with `lights` on it, and a thin post `post` wide.
fn floor_with(post: f64, lights: &str) -> String {
    format!(
        "format 1 map
entity world {{ type = worldspawn light_spacing = 32 }}
brush {{ material = floor cuboid = -1024 -1024 -32 1024 1024 0 }}
brush {{ material = post cuboid = 0 0 0 {post} {post} 256 }}
{lights}
"
    )
}

/// The brightness of every sample of the floor's first layer, with how many
/// layers it has.
fn floor_samples(map: &Map) -> (Vec<u32>, usize) {
    let floor = face_of_top(map);
    let light = floor.light.expect("lit");
    let layer = (light.width * light.height) as usize;
    let first = light.first_sample as usize;
    let samples = map.light[first..first + layer]
        .iter()
        .map(|sample| sample.iter().map(|part| u32::from(*part)).sum())
        .collect();
    (samples, light.style_count() as usize)
}

/// The biggest face looking up, which is the floor.
fn face_of_top(map: &Map) -> &Face {
    let up = |face: &&Face| {
        let points = map.face_points(face);
        points.iter().all(|point| point.z == 0.0)
    };
    map.faces
        .iter()
        .filter(up)
        .max_by_key(|face| face.light.map_or(0, |light| light.width * light.height))
        .expect("a floor")
}

#[test]
fn a_thin_shadow_comes_out_as_how_much_of_a_sample_it_covers() {
    let lamp = "entity lamp { type = light origin = 96 8 64 light = 900 }";
    let (shaded, _) = floor_samples(&baked(&floor_with(16.0, lamp), &[]));
    let (open, _) = floor_samples(&baked(&floor_with(0.25, lamp), &[]));
    assert_eq!(shaded.len(), open.len());
    // With one ray a sample, its light is all there or it's none of it.
    let part_shaded = shaded
        .iter()
        .zip(&open)
        .filter(|(shaded, open)| {
            **open > 30 && **shaded * 10 > **open * 2 && **shaded * 10 < **open * 8
        })
        .count();
    assert!(
        part_shaded > 8,
        "only {part_shaded} samples are part shaded"
    );
}

#[test]
fn light_past_a_faces_layers_is_kept() {
    let lamps: String = (0..6)
        .map(|number| {
            format!(
                "entity lamp_{number} {{ type = light origin = {} 300 64 light = 400 style = {} }}\n",
                number * 40,
                number + 1
            )
        })
        .collect();
    let map = baked(&floor_with(4.0, &lamps), &[]);
    let floor = face_of_top(&map);
    let light = floor.light.expect("lit");
    assert!(light.style_count() <= 4);
    // The same lamps, all of one style, so that none is short of a layer.
    let plain: String = (0..6)
        .map(|number| {
            format!(
                "entity lamp_{number} {{ type = light origin = {} 300 64 light = 400 }}\n",
                number * 40
            )
        })
        .collect();
    let whole = baked(&floor_with(4.0, &plain), &[]);
    let total = |map: &Map| -> u64 {
        let light = face_of_top(map).light.expect("lit");
        let layer = (light.width * light.height) as usize;
        let layers = light.style_count() as usize;
        map.light[light.first_sample as usize..][..layer * layers]
            .iter()
            .flatten()
            .map(|part| u64::from(*part))
            .sum()
    };
    let (held, all) = (total(&map), total(&whole));
    assert!(held * 10 > all * 8, "{held} of {all}");
}

#[test]
fn which_lights_get_a_layer_doesnt_depend_on_their_order() {
    let lamp = |number: u32| {
        format!(
            "entity lamp_{number} {{ type = light origin = {} 300 64 light = {} style = {} }}\n",
            number * 40,
            200 + number * 150,
            number + 1
        )
    };
    let forward: String = (0..6).map(lamp).collect();
    let backward: String = (0..6).rev().map(lamp).collect();
    let one = baked(&floor_with(4.0, &forward), &[]);
    let other = baked(&floor_with(4.0, &backward), &[]);
    let styles = |map: &Map| face_of_top(map).light.expect("lit").styles;
    assert_eq!(styles(&one), styles(&other));
}

#[test]
fn each_sharp_light_keeps_an_edge_of_its_own() {
    let lamps = "entity one { type = light origin = 200 8 96 light = 900 crisp = true }
entity two { type = light origin = -200 8 96 light = 900 crisp = true }";
    let map = baked(&floor_with(16.0, lamps), &[]);
    let light = face_of_top(&map).light.expect("lit");
    let layer = (light.width * light.height) as usize;
    let sharp: Vec<usize> = (0..light.style_count() as usize)
        .filter(|number| CRISP_STYLES.contains(&light.styles[*number]))
        .collect();
    assert_eq!(sharp.len(), 2, "styles {:?}", light.styles);
    for number in sharp {
        let first = light.first_sample as usize + number * layer;
        let edges = &map.light_edges[first..first + layer];
        assert!(
            edges.iter().any(|edge| *edge < 128),
            "layer {number} has a shadow"
        );
    }
}

/// The light of `face`'s first layer at `point`, blended between samples as the
/// GPU does.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "a test's small sample numbers"
)]
fn blended_at(map: &Map, face: &Face, point: Vec3) -> f32 {
    let light = face.light.expect("lit");
    let step = light.texels_per_sample as f32;
    let place = [0, 1]
        .map(|axis| (face.axes[axis].project(point) - light.texel_origin[axis] as f32) / step);
    let [x, y] = place.map(f32::floor);
    let [across, down] = [place[0] - x, place[1] - y];
    let sample = |x: f32, y: f32| {
        let (x, y) = (x as u32, y as u32);
        let at =
            light.first_sample + y.min(light.height - 1) * light.width + x.min(light.width - 1);
        f32::from(map.light[at as usize][0])
    };
    let top = sample(x, y) * (1.0 - across) + sample(x + 1.0, y) * across;
    let bottom = sample(x, y + 1.0) * (1.0 - across) + sample(x + 1.0, y + 1.0) * across;
    top * (1.0 - down) + bottom * down
}

/// The biggest face of `map` whose corners all pass `on`.
fn biggest_face<'a>(map: &'a Map, on: &dyn Fn(Vec3) -> bool) -> &'a Face {
    map.faces
        .iter()
        .filter(|face| map.face_points(face).iter().all(|point| on(*point)))
        .max_by_key(|face| face.light.map_or(0, |light| light.width * light.height))
        .expect("a face")
}

/// Where along `y` a face of `shaded` goes dark, compared with the same in `open`,
/// with `on` finding it and `at` turning a place along it into a point.
fn shadow_edge(
    shaded: &Map,
    open: &Map,
    on: &dyn Fn(Vec3) -> bool,
    at: &dyn Fn(f32) -> Vec3,
) -> f32 {
    let (one, other) = (biggest_face(shaded, on), biggest_face(open, on));
    let mut y = -120.0;
    while y < 120.0 {
        let point = at(y);
        let whole = blended_at(open, other, point).max(1.0);
        if blended_at(shaded, one, point) / whole < 0.5 {
            return y;
        }
        y += 0.5;
    }
    y
}

#[test]
fn a_shadow_runs_on_round_a_corner() {
    // A post throws a shadow along the floor and up a wall, and samples are far
    // apart. Where its edge falls has to agree on both sides of the join.
    let text = |post: &str| {
        format!(
            "format 1 map
entity world {{ type = worldspawn light_spacing = 32 }}
brush {{ material = floor cuboid = -512 -512 -32 256 512 0 }}
brush {{ material = wall cuboid = 256 -512 -32 288 512 512 }}
{post}
entity lamp {{ type = light origin = -128 3 100 light = 1800 }}
"
        )
    };
    let shaded = baked(
        &text("brush { material = post cuboid = 0 -11 0 16 13 200 }"),
        &[],
    );
    let open = baked(&text(""), &[]);
    let floor = |point: Vec3| point.z == 0.0;
    let wall = |point: Vec3| point.x == 256.0;
    let on_floor = shadow_edge(&shaded, &open, &floor, &|y| Vec3::new(255.5, y, 0.0));
    let on_wall = shadow_edge(&shaded, &open, &wall, &|y| Vec3::new(256.0, y, 0.5));
    assert!(
        (on_floor - on_wall).abs() < 10.0,
        "the floor's stops at {on_floor} and the wall's at {on_wall}"
    );
}
