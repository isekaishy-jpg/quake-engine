//! Drawing with a real device. Each test skips itself if there's no Vulkan.

mod common;

use std::sync::Arc;

use common::map;
use engine_core::canvas::{Image, Quad};
use engine_core::entity::ModelInstance;
use engine_core::math::Vec3;
use engine_map::{
    AliasCorner, AliasFrame, AliasModel, AliasVertex, AlphaMode, Fog, Map, Material, MaterialLayer,
    MipLevel, PAINT_MASK, Pool, PoolLook, ProbeGrid, RayStencil, Shading, Sky, Sprite,
    SpriteFacing, SpriteFrame, SpriteImage, TextureKind,
};
use engine_render::{
    BatchKind, CapsuleDraw, Capture, Drawables, EntityDraw, LightDraw, LiveLight, ParticleColor,
    ParticleDraw, PostSettings, Renderer, SpriteDraw, Target, View, build_world,
};

/// A renderer with no window, or `None` with no Vulkan device. Any other
/// failure fails the test, so a broken shader can't pass as a missing device.
#[expect(clippy::print_stderr, reason = "a skipped test should say so")]
fn renderer() -> Option<Renderer> {
    match Renderer::new(Target::Offscreen, (64, 64)) {
        Ok(renderer) => Some(renderer),
        Err(error) if error.is_no_device() => {
            eprintln!("skipped: no GPU here");
            None
        }
        Err(error) => panic!("{error}"),
    }
}

/// A view from above the square, looking straight down at its middle.
fn view_from_above() -> View {
    View {
        origin: Vec3::new(64.0, 64.0, 64.0),
        yaw: 0.0,
        pitch: 89.9,
        roll: 0.0,
        fov: 90.0,
        time: 0.0,
        styles: [1.0; 64],
        gamma: 1.0,
    }
}

/// Only `instances`.
fn with_instance(instances: &[ModelInstance]) -> Drawables<'_> {
    Drawables {
        instances,
        ..Drawables::default()
    }
}

/// Only `entities`.
fn with_entity(entities: &[EntityDraw]) -> Drawables<'_> {
    Drawables {
        entities,
        ..Drawables::default()
    }
}

/// The pixel at the centre of a capture.
fn centre(capture: &Capture) -> [u8; 4] {
    let at = ((capture.height / 2 * capture.width + capture.width / 2) * 4) as usize;
    [
        capture.rgba[at],
        capture.rgba[at + 1],
        capture.rgba[at + 2],
        capture.rgba[at + 3],
    ]
}

/// How many pixels in rows `rows` aren't the clear colour.
fn lit_pixels(capture: &Capture, rows: std::ops::Range<u32>) -> usize {
    let row_bytes = capture.width as usize * 4;
    let bytes = &capture.rgba[rows.start as usize * row_bytes..rows.end as usize * row_bytes];
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[1] > 32)
        .count()
}

/// Looking level from below the square's height puts it at the top of the
/// picture, and looking level from above puts it at the bottom. This is the
/// test that catches an upside-down picture.
#[test]
fn what_is_above_the_view_is_at_the_top_of_the_picture() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let mut view = view_from_above();
    view.origin = Vec3::new(-32.0, 64.0, -48.0);
    view.pitch = 0.0;
    let capture = renderer
        .capture(&view, Drawables::default(), 64, 64)
        .expect("capture");
    assert!(lit_pixels(&capture, 0..32) > 0, "the top half shows it");
    assert_eq!(lit_pixels(&capture, 32..64), 0, "the bottom half doesn't");
    view.origin.z = 48.0;
    let capture = renderer
        .capture(&view, Drawables::default(), 64, 64)
        .expect("capture");
    assert_eq!(lit_pixels(&capture, 0..32), 0, "the top half doesn't");
    assert!(lit_pixels(&capture, 32..64) > 0, "the bottom half shows it");
}

/// With yaw 0 looking along x, y runs to the left of the picture.
#[test]
fn what_is_left_of_the_view_is_at_the_left_of_the_picture() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let mut view = view_from_above();
    view.origin = Vec3::new(-32.0, -96.0, 48.0);
    view.pitch = 0.0;
    let capture = renderer
        .capture(&view, Drawables::default(), 64, 64)
        .expect("capture");
    let lit_at = |columns: std::ops::Range<usize>| {
        let pixels = capture.rgba.as_chunks::<4>().0;
        pixels
            .chunks(64)
            .flat_map(|row| &row[columns.clone()])
            .filter(|pixel| pixel[1] > 32)
            .count()
    };
    assert!(lit_at(0..32) > 0, "the left half shows it");
    assert_eq!(lit_at(32..64), 0, "the right half doesn't");
}

#[test]
fn a_picture_draws_where_its_quad_is_and_blends_over_the_view() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let view = view_from_above();
    let image = Image {
        width: 2,
        height: 1,
        rgba: vec![255, 0, 0, 255, 0, 0, 0, 0],
    };
    let picture = renderer.add_picture(&image).expect("upload");
    let quads = [
        Quad {
            picture,
            place: [0.0, 0.0, 32.0, 16.0],
            source: [0.0, 0.0, 1.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
        },
        Quad {
            picture: 0,
            place: [32.0, 16.0, 64.0, 32.0],
            source: [0.0, 0.0, 1.0, 1.0],
            color: [0.0, 0.0, 1.0, 0.5],
        },
    ];
    let drawables = Drawables {
        quads: &quads,
        ..Drawables::default()
    };
    let capture = renderer.capture(&view, drawables, 64, 32).expect("capture");
    let pixel = |x: usize, y: usize| capture.rgba.as_chunks::<4>().0[y * 64 + x];
    assert_eq!(pixel(4, 4)[..3], [255, 0, 0], "the opaque half");
    let clear = pixel(60, 4);
    assert_eq!(pixel(24, 4), clear, "the clear half shows what's behind");
    assert!(pixel(48, 24)[2] > 100, "a solid colour blends");
    assert_eq!(pixel(4, 24), clear, "nothing else changes");
}

/// A model is drawn with its position added, and not at all unless listed.
#[test]
fn a_brush_model_draws_where_its_instance_is() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut two = map(2);
    two.tree.leaves[1].face_count = 0;
    two.tree.leaf_faces.clear();
    renderer.load_map(Arc::new(two)).expect("load");
    let view = view_from_above();
    let lit = |capture: &Capture| centre(capture)[1] > 32;
    let empty = renderer
        .capture(&view, Drawables::default(), 64, 64)
        .expect("capture");
    assert!(!lit(&empty), "unlisted");
    let mut instance = ModelInstance {
        model: 1,
        origin: Vec3::ZERO,
        visible: true,
        frame: 0,
        solid: true,
    };
    let shown = renderer
        .capture(&view, with_instance(&[instance]), 64, 64)
        .expect("capture");
    assert!(lit(&shown), "in place");
    instance.origin = Vec3::new(1000.0, 0.0, 0.0);
    let moved = renderer
        .capture(&view, with_instance(&[instance]), 64, 64)
        .expect("capture");
    assert!(!lit(&moved), "moved away");
    instance.origin = Vec3::ZERO;
    instance.visible = false;
    let hidden = renderer
        .capture(&view, with_instance(&[instance]), 64, 64)
        .expect("capture");
    assert!(!lit(&hidden), "hidden");
}

#[test]
fn a_capture_has_the_size_asked_for() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let capture = renderer
        .capture(&view_from_above(), Drawables::default(), 32, 16)
        .expect("capture");
    assert_eq!((capture.width, capture.height), (32, 16));
    assert_eq!(capture.rgba.len(), 32 * 16 * 4);
}

/// The sample is 200, 100, 50. Doubled over 255 on a white texture, that's
/// the original's overbright rule.
#[test]
fn a_lit_white_surface_shows_its_doubled_light() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let capture = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    let [r, g, b, _] = centre(&capture);
    assert_eq!(r, 255, "400 of 255 clamps");
    assert!((199..=201).contains(&g), "green {g}");
    assert!((99..=101).contains(&b), "blue {b}");
}

#[test]
fn a_dark_style_darkens_the_surface() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let mut view = view_from_above();
    view.styles[0] = 0.0;
    let capture = renderer
        .capture(&view, Drawables::default(), 64, 64)
        .expect("capture");
    assert_eq!(centre(&capture)[..3], [0, 0, 0]);
}

#[test]
fn a_view_with_the_surface_behind_shows_the_clear_colour() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let mut view = view_from_above();
    view.pitch = -89.9;
    let capture = renderer
        .capture(&view, Drawables::default(), 64, 64)
        .expect("capture");
    assert!(centre(&capture)[0] < 16);
}

/// A flat white triangle in the x-y plane, 400 units across, in one pose.
fn triangle() -> AliasModel {
    let vertex = |x, y| AliasVertex {
        position: [x, y, 0],
        normal: [127, 0, 0],
    };
    let corner = |vertex| AliasCorner {
        vertex,
        uv: [0.5, 0.5],
    };
    AliasModel {
        scale: Vec3::new(4.0, 4.0, 1.0),
        offset: Vec3::new(-200.0, -200.0, 0.0),
        skin_width: 1,
        skin_height: 1,
        skins: vec![vec![200, 200, 200, 255]],
        corners: vec![corner(0), corner(2), corner(1)],
        poses: vec![vec![vertex(0, 0), vertex(100, 0), vertex(50, 100)]],
        frames: vec![AliasFrame {
            first_pose: 0,
            pose_count: 1,
            pose_ends: Vec::new(),
        }],
        spins: false,
        flags: 0,
    }
}

/// A draw of `model` under the view, in neutral light.
fn draw_of(model: u32) -> EntityDraw {
    EntityDraw {
        model,
        origin: Vec3::new(64.0, 64.0, 32.0),
        ambient: 1.0,
        ..EntityDraw::default()
    }
}

#[test]
fn a_model_draws_where_it_is_with_its_light() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let number = renderer.add_model(Arc::new(triangle())).expect("add");
    let lit = renderer
        .capture(&view_from_above(), with_entity(&[draw_of(number)]), 64, 64)
        .expect("capture");
    let [r, ..] = centre(&lit);
    assert!(
        (198..=202).contains(&r),
        "neutral light shows the skin: {r}"
    );
    let dim = EntityDraw {
        ambient: 0.5,
        ..draw_of(number)
    };
    let dark = renderer
        .capture(&view_from_above(), with_entity(&[dim]), 64, 64)
        .expect("capture");
    assert!((98..=102).contains(&centre(&dark)[0]));
    let away = EntityDraw {
        origin: Vec3::new(5000.0, 64.0, 32.0),
        ..draw_of(number)
    };
    let gone = renderer
        .capture(&view_from_above(), with_entity(&[away]), 64, 64)
        .expect("capture");
    assert_ne!(centre(&gone)[0], r);
}

#[test]
fn a_model_kept_in_front_shows_through_what_is_nearer() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let number = renderer.add_model(Arc::new(triangle())).expect("add");
    let below = EntityDraw {
        origin: Vec3::new(64.0, 64.0, -32.0),
        ..draw_of(number)
    };
    let hidden = renderer
        .capture(&view_from_above(), with_entity(&[below]), 64, 64)
        .expect("capture");
    assert_eq!(centre(&hidden)[0], 255, "the map's square is nearer");
    let in_front = EntityDraw {
        in_front: true,
        ..below
    };
    let shown = renderer
        .capture(&view_from_above(), with_entity(&[in_front]), 64, 64)
        .expect("capture");
    assert!((198..=202).contains(&centre(&shown)[0]));
}

/// A magenta sprite 32 units square, with a hole in its top half.
fn half_sprite() -> Sprite {
    let mut rgba = [255, 0, 255, 255].repeat(32 * 32);
    rgba[..32 * 16 * 4].fill(0);
    Sprite {
        facing: SpriteFacing::View,
        images: vec![SpriteImage {
            left: -16,
            top: 16,
            width: 32,
            height: 32,
            rgba,
        }],
        frames: vec![SpriteFrame {
            first_image: 0,
            image_count: 1,
            image_ends: Vec::new(),
        }],
    }
}

/// A view looking along x at the point `(100, 0, 0)`.
fn level_view() -> View {
    View {
        origin: Vec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        ..view_from_above()
    }
}

/// How many pixels in `rows` are magenta.
fn magenta(capture: &Capture, rows: std::ops::Range<u32>) -> usize {
    let row_bytes = capture.width as usize * 4;
    let bytes = &capture.rgba[rows.start as usize * row_bytes..rows.end as usize * row_bytes];
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[..3] == [255, 0, 255])
        .count()
}

/// A sprite stands the right way up, facing the view, with holes in it.
#[test]
fn a_sprite_faces_the_view_and_keeps_its_holes() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let number = renderer
        .add_sprite(Arc::new(half_sprite()))
        .expect("sprite");
    let sprites = [SpriteDraw {
        sprite: number,
        image: 0,
        origin: Vec3::new(100.0, 0.0, 0.0),
        angles: Vec3::ZERO,
    }];
    let drawables = Drawables {
        sprites: &sprites,
        ..Drawables::default()
    };
    let capture = renderer
        .capture(&level_view(), drawables, 64, 64)
        .expect("capture");
    assert_eq!(magenta(&capture, 0..32), 0, "the top is a hole");
    assert!(magenta(&capture, 32..64) > 20, "the bottom shows");
}

/// A particle is a square of its own colour, at its own place.
#[test]
fn a_particle_shows_its_colour() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let particles = [ParticleDraw {
        origin: Vec3::new(100.0, 0.0, 0.0),
        color: ParticleColor {
            red: 255,
            green: 0,
            blue: 255,
        },
    }];
    let drawables = Drawables {
        particles: &particles,
        ..Drawables::default()
    };
    let capture = renderer
        .capture(&level_view(), drawables, 240, 240)
        .expect("capture");
    assert_eq!(&centre(&capture)[..3], [255, 0, 255]);
    let pixels = magenta(&capture, 0..240);
    assert!(
        (4..=16).contains(&pixels),
        "2.56 pixels from the middle: {pixels}"
    );
}

/// A dynamic light adds to the light on a surface, near it and not far off.
#[test]
fn a_dynamic_light_brightens_what_it_reaches() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let view = view_from_above();
    let plain = centre(
        &renderer
            .capture(&view, Drawables::default(), 64, 64)
            .expect("capture"),
    );
    let light_at = |origin| LightDraw {
        origin,
        radius: 200.0,
        min_light: 0.0,
    };
    let near = [light_at(Vec3::new(64.0, 64.0, 16.0))];
    let lit = centre(
        &renderer
            .capture(
                &view,
                Drawables {
                    lights: &near,
                    ..Drawables::default()
                },
                64,
                64,
            )
            .expect("capture"),
    );
    assert!(lit[2] > plain[2] + 50, "{plain:?} to {lit:?}");
    let far = [light_at(Vec3::new(64.0, 64.0, 500.0))];
    let unlit = centre(
        &renderer
            .capture(
                &view,
                Drawables {
                    lights: &far,
                    ..Drawables::default()
                },
                64,
                64,
            )
            .expect("capture"),
    );
    assert_eq!(unlit, plain);
}

/// A brush entity with a frame shows its texture's alternate, as a used button
/// does, and the world never does.
#[test]
fn a_frame_swaps_a_brush_model_to_its_alternate_texture() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut two = map(2);
    two.tree.leaves[1].face_count = 0;
    two.tree.leaf_faces.clear();
    let mut other = two.textures[0].clone();
    other.mips[0].rgba.fill(0);
    two.textures.push(other);
    two.textures[0].alternate = Some(1);
    renderer.load_map(Arc::new(two)).expect("load");
    let view = view_from_above();
    let mut instance = ModelInstance {
        model: 1,
        origin: Vec3::ZERO,
        visible: true,
        solid: true,
        frame: 0,
    };
    let plain = renderer
        .capture(&view, with_instance(&[instance]), 64, 64)
        .expect("capture");
    assert!(centre(&plain)[1] > 32, "lit");
    instance.frame = 1;
    let used = renderer
        .capture(&view, with_instance(&[instance]), 64, 64)
        .expect("capture");
    assert!(centre(&used)[1] < 8, "the black alternate");
}

/// The far side of a model isn't drawn, so a weapon the near plane cuts open
/// doesn't show its inside.
#[test]
fn a_model_is_only_seen_from_outside() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let mut inside_out = triangle();
    inside_out.corners.swap(1, 2);
    let number = renderer.add_model(Arc::new(inside_out)).expect("model");
    let capture = renderer
        .capture(&view_from_above(), with_entity(&[draw_of(number)]), 64, 64)
        .expect("capture");
    let plain = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    assert_eq!(centre(&capture), centre(&plain));
}

/// A stencil colour the lit square has room to show: it's full of red already.
const BLUE: [u8; 4] = [0, 0, 255, 255];

/// A map with a stencil high over the square, lit from above, whose left half
/// is `left` and whose right half is clear.
fn stencil_map(left: [u8; 4]) -> Map {
    let mut rgba = Vec::new();
    for _ in 0..8 {
        for x in 0..8 {
            rgba.extend_from_slice(if x < 4 { &left } else { &[0, 0, 0, 0] });
        }
    }
    let mut built = map(1);
    built.stencils.push(RayStencil {
        corner: Vec3::new(0.0, 0.0, 128.0),
        across: Vec3::new(128.0, 0.0, 0.0),
        down: Vec3::new(0.0, 128.0, 0.0),
        light: Vec3::new(0.0, 0.0, 1.0),
        sun: true,
        path: Vec::new(),
        period: 0.0,
        strength: 4.0,
        reach: 512.0,
        image: Arc::new(vec![MipLevel {
            width: 8,
            height: 8,
            rgba,
        }]),
        boxes: vec![[0.0, 0.0, 1.0, 1.0]],
    });
    built
}

/// How bright the middle of a picture of `built` is, with `masks` on it.
fn stencil_centre(built: Map, masks: &[[f32; 256]], rays: u8) -> Option<[u8; 4]> {
    let mut renderer = renderer()?;
    renderer.load_map(Arc::new(built)).expect("load");
    renderer.set_post(PostSettings {
        rays,
        ..PostSettings::default()
    });
    let drawables = Drawables {
        stencil_masks: masks,
        ..Drawables::default()
    };
    let view = View {
        origin: Vec3::new(32.0, 64.0, 64.0),
        ..view_from_above()
    };
    let capture = renderer.capture(&view, drawables, 64, 64).expect("capture");
    Some(centre(&capture))
}

/// Light through a coloured stencil shows in the air, tinted.
#[test]
fn a_stencil_tints_the_air_below_it() {
    let plain = stencil_centre(stencil_map(BLUE), &[], 0);
    let shown = stencil_centre(stencil_map(BLUE), &[], 3);
    let (Some(plain), Some(shown)) = (plain, shown) else {
        return;
    };
    assert!(shown[2] > plain[2] + 20, "{plain:?} {shown:?}");
    assert_eq!(shown[1], plain[1], "no green in a blue stencil");
}

/// A stencil's clear part lets nothing through, so a frame cuts the light.
#[test]
fn a_frame_cuts_the_rays() {
    let plain = stencil_centre(stencil_map([0, 0, 0, 0]), &[], 0);
    let shown = stencil_centre(stencil_map([0, 0, 0, 0]), &[], 3);
    if let (Some(plain), Some(shown)) = (plain, shown) {
        assert_eq!(plain, shown);
    }
}

/// A closed box takes its rays away.
#[test]
fn a_closed_box_stops_its_rays() {
    let plain = stencil_centre(stencil_map(BLUE), &[], 0);
    let closed = stencil_centre(stencil_map(BLUE), &[[0.0; 256]], 3);
    if let (Some(plain), Some(closed)) = (plain, closed) {
        assert_eq!(plain, closed);
    }
}

/// A capsule shades what's behind it from its light, and nothing elsewhere.
#[test]
fn a_capsule_shades_what_is_behind_it() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let view = view_from_above();
    let lit = renderer
        .capture(&view, Drawables::default(), 64, 64)
        .expect("capture");
    let capsule = |toward_light: Vec3| CapsuleDraw {
        origin: Vec3::new(64.0, 64.0, 24.0),
        radius: 16.0,
        toward_light,
        strength: 1.0,
    };
    let over = [capsule(Vec3::new(0.0, 0.0, 1.0))];
    let shaded = renderer
        .capture(
            &view,
            Drawables {
                capsules: &over,
                ..Drawables::default()
            },
            64,
            64,
        )
        .expect("capture");
    assert!(centre(&shaded)[0] < centre(&lit)[0], "shaded below it");
    let under = [capsule(Vec3::new(0.0, 0.0, -1.0))];
    let clear = renderer
        .capture(
            &view,
            Drawables {
                capsules: &under,
                ..Drawables::default()
            },
            64,
            64,
        )
        .expect("capture");
    assert_eq!(centre(&clear), centre(&lit), "light from below passes");
}

/// A liquid with a blended material draws through the material pipeline, which
/// needs its material uploaded: without that a device is lost.
#[test]
fn a_blended_liquid_draws() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map(1);
    built.textures[0].kind = TextureKind::Liquid;
    built.textures[0].material = Some(Arc::new(Material {
        alpha: AlphaMode::Blend,
        shading: Shading::Unlit,
        ..Material::default()
    }));
    renderer.load_map(Arc::new(built)).expect("load");
    let capture = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    assert_eq!(capture.width, 64);
}

/// The middle of a picture of `built` from above, with `post`.
fn post_centre(built: Map, post: PostSettings) -> Option<[u8; 4]> {
    let mut renderer = renderer()?;
    renderer.load_map(Arc::new(built)).expect("load");
    renderer.set_post(post);
    let capture = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    Some(centre(&capture))
}

/// A pool around the view that tints as `look` says.
fn pool_map(look: PoolLook) -> Map {
    let mut built = map(1);
    built.pools.push(Pool {
        low: Vec3::new(-64.0, -64.0, -8.0),
        high: Vec3::new(192.0, 192.0, 128.0),
        look,
    });
    built
}

/// Depth takes things towards a pool's colour, more the further they are.
#[test]
fn depth_tints_towards_a_pools_colour() {
    let look = PoolLook {
        color: [0.0, 0.0, 1.0],
        absorption: 0.02,
        caustics: 0.0,
    };
    let thick = PoolLook {
        absorption: 0.2,
        ..look
    };
    let plain = post_centre(map(1), PostSettings::default());
    let tinted = post_centre(pool_map(look), PostSettings::default());
    let deep = post_centre(pool_map(thick), PostSettings::default());
    let (Some(plain), Some(tinted), Some(deep)) = (plain, tinted, deep) else {
        return;
    };
    assert!(tinted[0] < plain[0], "{plain:?} {tinted:?}");
    assert!(tinted[2] > plain[2], "{plain:?} {tinted:?}");
    assert!(deep[0] < tinted[0], "{tinted:?} {deep:?}");
    assert!(deep[2] > 240, "all the way to its colour: {deep:?}");
}

/// A map with thick fog, and probes all of `level`.
fn fog_map(level: u8) -> Map {
    let mut built = map(1);
    built.fog = Some(Fog {
        color: [1.0, 1.0, 1.0],
        density: 0.1,
        height: 0.0,
        falloff: 0.0001,
    });
    built.probes = Some(ProbeGrid {
        origin: Vec3::new(-64.0, -64.0, -64.0),
        spacing: 128.0,
        size: [3, 3, 3],
        colors: vec![[[level; 3]; 6]; 27],
        directions: vec![[128, 128, 255]; 27],
    });
    built
}

/// Fog shows the light in the probes, when it's asked to.
#[test]
fn the_fog_grid_shows_baked_light() {
    let grid = PostSettings {
        fog_grid: true,
        ..PostSettings::default()
    };
    let bright = post_centre(fog_map(120), grid);
    let dark = post_centre(fog_map(0), grid);
    let plain = post_centre(fog_map(0), PostSettings::default());
    let (Some(bright), Some(dark), Some(plain)) = (bright, dark, plain) else {
        return;
    };
    assert!(bright[1] > 150, "lit fog: {bright:?}");
    assert!(dark[1] < 30, "unlit fog: {dark:?}");
    assert!(
        plain[1] > 200,
        "fog with no grid has its own colour: {plain:?}"
    );
}

/// An image of `size` pixels square, all from `color`.
fn image_of(size: u32, color: impl Fn(u32, u32) -> [u8; 4]) -> Vec<MipLevel> {
    let mut rgba = Vec::new();
    for y in 0..size {
        for x in 0..size {
            rgba.extend_from_slice(&color(x, y));
        }
    }
    vec![MipLevel {
        width: size,
        height: size,
        rgba,
    }]
}

/// A mirror looking up at the sky takes on some of its colour.
#[test]
fn a_polished_surface_mirrors_the_sky() {
    let mut built = map(1);
    built.textures[0].mips = image_of(8, |_, _| [40, 40, 40, 255]);
    built.textures[0].material = Some(Arc::new(Material {
        shading: Shading::Unlit,
        reflect: 1.0,
        ..Material::default()
    }));
    built.sky = Some(Sky {
        zenith: [0.0, 0.0, 1.0],
        horizon: [0.0, 0.0, 1.0],
        ..Sky::default()
    });
    let mirrored = post_centre(built, PostSettings::default());
    let Some(mirrored) = mirrored else {
        return;
    };
    // Straight on is when a mirror shows least: a quarter of the sky.
    assert!(mirrored[2] > 80, "the sky shows: {mirrored:?}");
    assert!(
        mirrored[0] < 40,
        "it replaces some of the surface: {mirrored:?}"
    );
}

/// A surface with no shine is left as it is, even under a blend.
#[test]
fn a_plain_surface_mirrors_nothing() {
    let mut built = map(1);
    built.textures[0].mips = image_of(8, |_, _| [40, 40, 40, 255]);
    built.textures[0].material = Some(Arc::new(Material {
        shading: Shading::Unlit,
        ..Material::default()
    }));
    built.sky = Some(Sky {
        zenith: [0.0, 0.0, 1.0],
        horizon: [0.0, 0.0, 1.0],
        ..Sky::default()
    });
    if let Some(plain) = post_centre(built, PostSettings::default()) {
        assert!(plain[2] < 60, "no sky in it: {plain:?}");
    }
}

/// A flow map moves a surface along it as time passes.
#[test]
fn a_flow_map_moves_its_surface() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map(1);
    built.textures[0].mips = image_of(8, |x, _| {
        if x < 4 {
            [255, 0, 0, 255]
        } else {
            [0, 0, 255, 255]
        }
    });
    built.textures[0].material = Some(Arc::new(Material {
        shading: Shading::Unlit,
        slots: vec![Arc::new(image_of(2, |_, _| [255, 128, 0, 255]))],
        flow: Some(1),
        flow_strength: 0.5,
        flow_speed: 0.2,
        ..Material::default()
    }));
    renderer.load_map(Arc::new(built)).expect("load");
    let at = |time: f32, renderer: &mut Renderer| {
        let view = View {
            time,
            ..view_from_above()
        };
        let capture = renderer
            .capture(&view, Drawables::default(), 64, 64)
            .expect("capture");
        centre(&capture)
    };
    let first = at(0.0, &mut renderer);
    let later = at(1.25, &mut renderer);
    assert_ne!(first, later);
}

/// A model's back faces show as an outline when its hull is on, in the
/// outline's colour, where nothing shows otherwise.
#[test]
fn a_hull_shows_the_outline_colour() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let mut inside_out = triangle();
    inside_out.corners.swap(1, 2);
    let number = renderer.add_model(Arc::new(inside_out)).expect("model");
    let draws = [draw_of(number)];
    let plain = renderer
        .capture(&view_from_above(), with_entity(&draws), 64, 64)
        .expect("capture");
    renderer.set_post(PostSettings {
        outline_hull: true,
        outline_color: [0.0, 1.0, 0.0],
        ..PostSettings::default()
    });
    let outlined = renderer
        .capture(&view_from_above(), with_entity(&draws), 64, 64)
        .expect("capture");
    assert_ne!(centre(&plain)[..3], [0, 255, 0], "nothing with it off");
    assert_eq!(centre(&outlined)[..3], [0, 255, 0], "the hull shows");
}

/// A view from high enough to see the square's edges, turned so they slant.
fn slanted_view() -> View {
    View {
        origin: Vec3::new(64.0, 64.0, 160.0),
        roll: 17.0,
        ..view_from_above()
    }
}

/// With several samples a pixel a scene still draws the same, and a slanted
/// edge gains shades between its two sides.
#[test]
fn samples_soften_an_edge() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let view = slanted_view();
    let shades = |capture: &Capture| {
        let (pixels, _) = capture.rgba.as_chunks::<4>();
        let mut seen: Vec<[u8; 3]> = pixels.iter().map(|[r, g, b, _]| [*r, *g, *b]).collect();
        seen.sort_unstable();
        seen.dedup();
        seen.len()
    };
    let plain = renderer
        .capture(&view, Drawables::default(), 96, 96)
        .expect("capture");
    renderer.set_antialiasing(4).expect("samples");
    let soft = renderer
        .capture(&view, Drawables::default(), 96, 96)
        .expect("capture");
    assert_eq!(centre(&plain), centre(&soft), "the same surface");
    assert!(shades(&soft) > shades(&plain), "edges have shades between");
}

/// Blended faces draw far to near, so the nearer shows over the further.
#[test]
fn blended_faces_draw_far_to_near() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let glass = Arc::new(Material {
        alpha: AlphaMode::Blend,
        shading: Shading::Unlit,
        ..Material::default()
    });
    let mut built = map(1);
    built.textures[0].mips = image_of(2, |_, _| [255, 0, 0, 128]);
    built.textures[0].material = Some(glass);
    let mesh = build_world(&built).expect("a mesh");
    let blended = mesh.batches.iter().filter(|b| b.kind == BatchKind::Blended);
    assert!(blended.count() > 1, "each face stands alone");
    renderer.load_map(Arc::new(built)).expect("load");
    let capture = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    assert!(centre(&capture)[0] > 100);
}

/// Searching along edges changes pixels on one, and leaves flat ones alone.
#[test]
fn edge_search_only_touches_edges() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let view = slanted_view();
    let plain = renderer
        .capture(&view, Drawables::default(), 96, 96)
        .expect("capture");
    renderer.set_post(PostSettings {
        edge_smoothing: 2,
        ..PostSettings::default()
    });
    let soft = renderer
        .capture(&view, Drawables::default(), 96, 96)
        .expect("capture");
    assert_eq!(centre(&plain), centre(&soft), "flat pixels stay");
    assert_ne!(plain.rgba, soft.rgba, "edges change");
}

/// A light over the surface, in blue, which it has room to show.
fn blue_light(origin: Vec3, shadows: bool) -> LiveLight {
    LiveLight {
        origin,
        radius: 300.0,
        color: [0.0, 0.0, 0.5],
        shadows,
        ..LiveLight::default()
    }
}

/// The pixel at `x` and `y` of a capture.
fn pixel(capture: &Capture, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * capture.width + x) * 4) as usize;
    [
        capture.rgba[at],
        capture.rgba[at + 1],
        capture.rgba[at + 2],
        capture.rgba[at + 3],
    ]
}

/// A moving light adds its colour where it reaches, and nothing further off.
#[test]
fn a_live_light_brightens_what_it_reaches() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let at = |renderer: &mut Renderer, lights: &[LiveLight]| {
        let drawables = Drawables {
            live_lights: lights,
            ..Drawables::default()
        };
        let view = view_from_above();
        centre(&renderer.capture(&view, drawables, 64, 64).expect("capture"))
    };
    let plain = at(&mut renderer, &[]);
    let near = at(
        &mut renderer,
        &[blue_light(Vec3::new(64.0, 64.0, 40.0), false)],
    );
    let far = at(
        &mut renderer,
        &[blue_light(Vec3::new(2000.0, 64.0, 40.0), false)],
    );
    assert!(near[2] > plain[2] + 30, "{plain:?} {near:?}");
    assert_eq!(near[1], plain[1], "no green in a blue light");
    assert_eq!(far, plain, "out of reach");
}

/// A spot only lights what's in its cone.
#[test]
fn a_live_spot_keeps_to_its_cone() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let spot = LiveLight {
        spot: Vec3::new(0.0, 0.0, -1.0),
        spot_cosine: 0.97,
        ..blue_light(Vec3::new(64.0, 64.0, 60.0), false)
    };
    let lights = [spot];
    let drawables = Drawables {
        live_lights: &lights,
        ..Drawables::default()
    };
    let capture = renderer
        .capture(&view_from_above(), drawables, 64, 64)
        .expect("capture");
    let plain = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    assert!(pixel(&capture, 32, 32)[2] > pixel(&plain, 32, 32)[2]);
    assert_eq!(
        pixel(&capture, 4, 32),
        pixel(&plain, 4, 32),
        "outside its cone"
    );
}

/// A model between a light and a surface shades it, when the light has shadows.
#[test]
fn a_live_shadow_falls_behind_what_blocks_it() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(2))).expect("load");
    renderer.set_post(PostSettings {
        live_shadows: true,
        ..PostSettings::default()
    });
    let blocker = [ModelInstance {
        model: 1,
        origin: Vec3::new(64.0, 0.0, 30.0),
        visible: true,
        ..ModelInstance::default()
    }];
    let at = |renderer: &mut Renderer, shadows: bool| {
        let lights = [blue_light(Vec3::new(128.0, 64.0, 60.0), shadows)];
        let drawables = Drawables {
            instances: &blocker,
            live_lights: &lights,
            ..Drawables::default()
        };
        let capture = renderer
            .capture(&view_from_above(), drawables, 64, 64)
            .expect("capture");
        pixel(&capture, 16, 32)
    };
    let open = at(&mut renderer, false);
    let shaded = at(&mut renderer, true);
    assert!(shaded[2] + 20 < open[2], "{open:?} {shaded:?}");
}

/// A shadow is kept while nothing moves, and redrawn when something does.
#[test]
fn a_shadow_is_drawn_only_when_it_changes() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(2))).expect("load");
    renderer.set_post(PostSettings {
        live_shadows: true,
        ..PostSettings::default()
    });
    let lights = [blue_light(Vec3::new(64.0, 64.0, 60.0), true)];
    let frame = |renderer: &mut Renderer, height: f32| {
        let blocker = [ModelInstance {
            model: 1,
            origin: Vec3::new(0.0, 0.0, height),
            visible: true,
            ..ModelInstance::default()
        }];
        let drawables = Drawables {
            instances: &blocker,
            live_lights: &lights,
            ..Drawables::default()
        };
        renderer
            .capture(&view_from_above(), drawables, 16, 16)
            .expect("capture");
        renderer.shadow_redraws()
    };
    assert_eq!(frame(&mut renderer, 30.0), 1, "the first frame draws it");
    assert_eq!(frame(&mut renderer, 30.0), 0, "nothing moved");
    assert_eq!(frame(&mut renderer, 31.0), 1, "the model moved");
    assert_eq!(frame(&mut renderer, 31.0), 0, "and stopped");
}

/// However many lights change, a frame only redraws its budget of shadows.
#[test]
fn shadow_redraws_stay_within_budget() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    renderer.set_post(PostSettings {
        live_shadows: true,
        ..PostSettings::default()
    });
    let lights: Vec<LiveLight> = (0_u16..6)
        .map(|index| LiveLight {
            id: u32::from(index),
            ..blue_light(Vec3::new(f32::from(index) * 20.0, 64.0, 60.0), true)
        })
        .collect();
    let mut frame = || {
        let drawables = Drawables {
            live_lights: &lights,
            ..Drawables::default()
        };
        renderer
            .capture(&view_from_above(), drawables, 16, 16)
            .expect("capture");
        renderer.shadow_redraws()
    };
    let counts = [frame(), frame(), frame(), frame()];
    assert_eq!(counts, [2, 2, 2, 0], "two a frame until they're all done");
}

/// A layer masked by paint shows where the map's paint says, and not elsewhere.
#[test]
fn a_painted_layer_follows_the_paint() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let material = Material {
        shading: Shading::Unlit,
        slots: vec![Arc::new(image_of(2, |_, _| [0, 255, 0, 255]))],
        layers: vec![MaterialLayer {
            base: 1,
            mask: PAINT_MASK,
        }],
        ..Material::default()
    };
    let painted = |weight: u8| {
        let mut built = map(1);
        built.textures[0].mips = image_of(8, |_, _| [255, 0, 0, 255]);
        built.textures[0].material = Some(Arc::new(material.clone()));
        built.paint = vec![[weight, 0, 0, 0]; built.light.len()];
        built
    };
    let at = |renderer: &mut Renderer, built: Map| {
        renderer.load_map(Arc::new(built)).expect("load");
        let capture = renderer
            .capture(&view_from_above(), Drawables::default(), 64, 64)
            .expect("capture");
        centre(&capture)
    };
    let bare = at(&mut renderer, painted(0));
    let full = at(&mut renderer, painted(255));
    assert!(bare[0] > 200 && bare[1] < 50, "{bare:?}");
    assert!(full[1] > 200 && full[0] < 50, "{full:?}");
}
