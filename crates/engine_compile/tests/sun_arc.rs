//! A sun whose shadows are baked at angles along an arc, so that it can move with no new bake.

use std::sync::Arc;

use engine_compile::{
    BakeInput, BakeLevel, BakeQuality, CompileError, apply_bake, bake_files, bake_light_as,
    compile, placeholder_texture, read_map,
};
use engine_format::from_text;
use engine_map::{ARC_ANGLES, Face, Map, MipLevel, SUN_STYLE};

/// A yard with a pillar in the middle, and a day, with `rest` in its world and `more` after.
fn yard(rest: &str, more: &str) -> String {
    format!(
        "format 1 map
entity world {{ type = worldspawn sun_intensity = 200 light_spacing = 32 day_start = 12 {rest} }}
sky {{ sun_direction = 0 -0.5 0.87 }}
brush {{ material = ground cuboid = -256 -256 -32 256 256 0 }}
brush {{ material = pillar cuboid = -16 -16 0 16 16 160 }}
{more}"
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

/// The faces on top of the ground, which the pillar splits.
fn ground_tops(map: &Map) -> Vec<&Face> {
    let number = map
        .textures
        .iter()
        .position(|texture| texture.name == "ground")
        .and_then(|number| u32::try_from(number).ok())
        .expect("ground");
    map.faces
        .iter()
        .filter(|face| face.texture == number && face.light.is_some())
        .filter(|face| {
            let first = face.first_point as usize;
            let points = &map.points[first..first + face.point_count as usize];
            points.iter().all(|point| point.z.abs() < 0.01)
        })
        .collect()
}

/// Each sample of the ground's sun layer: where it is across the ground, and its bits.
fn ground_bits(map: &Map) -> Vec<(f32, f32, u32)> {
    let mut found = Vec::new();
    for face in ground_tops(map) {
        let light = face.light.expect("lit");
        let slot = light
            .styles
            .iter()
            .position(|style| *style == SUN_STYLE)
            .expect("a sun layer");
        let (width, height) = (light.width as usize, light.height as usize);
        let first = light.first_sample as usize + slot * width * height;
        let [u_axis, v_axis] = face.axes;
        for y in 0..height {
            for x in 0..width {
                let at = first + y * width + x;
                let [low, middle, high] = map.light[at];
                let bits = u32::from_le_bytes([low, middle, high, map.light_edges[at]]);
                #[expect(clippy::cast_precision_loss, reason = "small grids")]
                #[expect(
                    clippy::cast_possible_wrap,
                    clippy::cast_possible_truncation,
                    reason = "small grids"
                )]
                let (u, v) = (
                    (light.texel_origin[0] + light.texels_per_sample as i32 * x as i32) as f32,
                    (light.texel_origin[1] + light.texels_per_sample as i32 * y as i32) as f32,
                );
                // Where on the ground has these texels, from the two axes across it.
                let (a, b) = (u_axis.axis, v_axis.axis);
                let (u, v) = (u - u_axis.offset, v - v_axis.offset);
                let det = a.x * b.y - a.y * b.x;
                found.push(((u * b.y - a.y * v) / det, (a.x * v - b.x * u) / det, bits));
            }
        }
    }
    found
}

/// The bits of the sample nearest `x`, `y`.
fn bits_near(samples: &[(f32, f32, u32)], x: f32, y: f32) -> u32 {
    samples
        .iter()
        .min_by(|a, b| {
            let far = |s: &(f32, f32, u32)| (s.0 - x).powi(2) + (s.1 - y).powi(2);
            far(a).total_cmp(&far(b))
        })
        .expect("samples")
        .2
}

/// Whether `bits` has the sun at `angle`.
fn lit(bits: u32, angle: u32) -> bool {
    bits >> angle & 1 == 1
}

/// A final bake keeps, for each sample, the angles the sun reaches it at. A pillar's shadow falls
/// one way in the morning and the other in the evening, and open ground is lit all day.
#[test]
fn a_final_bake_keeps_where_the_sun_reaches_along_its_arc() {
    let map = baked(&yard("sun = arc", ""), BakeLevel::Final);
    assert!(map.sun_arc_baked && map.arc_drawn());
    let samples = ground_bits(&map);
    let (morning, evening) = (4, 27);
    let west = bits_near(&samples, -56.0, 10.0);
    assert!(!lit(west, morning) && lit(west, evening), "{west:032b}");
    let east = bits_near(&samples, 56.0, 10.0);
    assert!(lit(east, morning) && !lit(east, evening), "{east:032b}");
    let open = bits_near(&samples, -200.0, -200.0);
    for angle in 2..30 {
        assert!(lit(open, angle), "{angle}: {open:032b}");
    }
    #[expect(clippy::cast_possible_truncation, reason = "32")]
    let last = ARC_ANGLES as u32 - 1;
    assert!(
        !lit(open, 0) && !lit(open, last),
        "the sun's on the horizon: {open:032b}"
    );
}

/// What's smaller than an angle's slice of the arc doesn't come and go between angles: a small
/// block high in the air leaves the bits as they were.
#[test]
fn what_is_smaller_than_an_angle_stays_out() {
    let block = "brush { material = block cuboid = 150 -150 300 158 -142 308 }\n";
    let with = ground_bits(&baked(&yard("sun = arc", block), BakeLevel::Final));
    let without = ground_bits(&baked(&yard("sun = arc", ""), BakeLevel::Final));
    assert_eq!(with.len(), without.len());
    for (with, without) in with.iter().zip(&without) {
        assert_eq!(with.2, without.2, "at {} {}", with.0, with.1);
    }
}

/// Until a final bake, a sun on an arc is baked as one on its own layer is.
#[test]
fn an_arc_is_baked_as_its_own_layer_till_a_final_bake() {
    let arc = baked(&yard("sun = arc", ""), BakeLevel::Usual);
    let own = baked(&yard("sun = baked", ""), BakeLevel::Usual);
    assert!(!arc.sun_arc_baked && !arc.arc_drawn());
    assert_eq!(arc.light, own.light);
    let own_final = baked(&yard("sun = baked", ""), BakeLevel::Final);
    assert!(!own_final.sun_arc_baked, "only an arc has bits");
}

/// Models take a sun on an arc as they go, so it's not in the probes as well.
#[test]
fn an_arc_sun_is_not_in_the_probes() {
    let total = |map: &Map| -> u64 {
        let probes = map.probes.as_ref().expect("probes");
        let all = probes.colors.iter().flatten().flatten();
        all.map(|part| u64::from(*part)).sum()
    };
    let arc = baked(&yard("sun = arc", ""), BakeLevel::Final);
    let own = baked(&yard("sun = baked", ""), BakeLevel::Final);
    assert!(total(&arc) * 10 < total(&own) * 9);
}

/// A saved bake says whether it has the bits, so that one put back is read the same way.
#[test]
fn a_saved_bake_keeps_its_arc() {
    let text = yard("sun = arc", "");
    for level in [BakeLevel::Final, BakeLevel::Usual] {
        let map = baked(&text, level);
        let (_, bytes) = bake_files(&map, "yard", 5, level).remove(0);
        let mut again = baked(&text, BakeLevel::Quick);
        again.sun_arc_baked = level != BakeLevel::Final;
        assert!(apply_bake(&mut again, &bytes, 5).expect("reads"));
        assert_eq!(again.sun_arc_baked, map.sun_arc_baked, "{level:?}");
        assert_eq!(again.light_edges, map.light_edges);
    }
}

/// A face the sun doesn't reach as it's baked still gets its layer, if some of the arc faces it:
/// the side of the pillar towards the sunrise has the morning.
#[test]
fn a_face_gets_its_layer_if_the_arc_reaches_it() {
    let map = baked(&yard("sun = arc", ""), BakeLevel::Final);
    let number = map
        .textures
        .iter()
        .position(|texture| texture.name == "pillar")
        .and_then(|number| u32::try_from(number).ok())
        .expect("pillar");
    let east = map
        .faces
        .iter()
        .filter(|face| face.texture == number)
        .find(|face| {
            let first = face.first_point as usize;
            let points = &map.points[first..first + face.point_count as usize];
            points.iter().all(|point| (point.x - 16.0).abs() < 0.01)
        })
        .expect("its east side");
    let light = east.light.expect("lit");
    let slot = light
        .styles
        .iter()
        .position(|style| *style == SUN_STYLE)
        .expect("the arc's layer, though the sun as baked is side on to it");
    let size = (light.width * light.height) as usize;
    let first = light.first_sample as usize + slot * size;
    let morning = (first..first + size).any(|at| {
        let [low, middle, high] = map.light[at];
        let bits = u32::from_le_bytes([low, middle, high, map.light_edges[at]]);
        lit(bits, 4) && !lit(bits, 27)
    });
    assert!(morning, "lit in the morning and not the evening");
}

/// A sample on a face's grid that's off the face, under what stands on it, has its bits from on
/// the face, by the pillar, which shades it some of the day.
#[test]
fn a_sample_off_its_face_looks_from_on_it() {
    let map = baked(&yard("sun = arc", ""), BakeLevel::Final);
    let under: Vec<u32> = ground_bits(&map)
        .into_iter()
        .filter(|(x, y, _)| x.abs() < 8.0 && y.abs() < 8.0)
        .map(|(_, _, bits)| bits)
        .collect();
    assert!(!under.is_empty(), "the grid reaches under the pillar");
    let some_of_the_day = |bits: &u32| (2..30).any(|angle| lit(*bits, angle));
    let all_day = |bits: &u32| (2..30).all(|angle| lit(*bits, angle));
    assert!(
        under
            .iter()
            .all(|bits| some_of_the_day(bits) && !all_day(bits)),
        "{under:?}"
    );
}
