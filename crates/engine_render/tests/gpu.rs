//! Drawing with a real device. Each test skips itself if there's no Vulkan.

mod common;

use std::sync::Arc;

use common::map;
use engine_core::canvas::{Image, Quad};
use engine_core::entity::ModelInstance;
use engine_core::math::Vec3;
use engine_map::{
    AliasCorner, AliasFrame, AliasModel, AliasVertex, AlphaMode, Clouds, Fog, LightTiles, Map,
    Material, MaterialLayer, MeshCorner, MeshInstance, MetalRough, MipLevel, MovingMesh, NO_MOVE,
    PAINT_MASK, PixelForm, Pool, PoolLook, ProbeGrid, RESIDENT, RayStencil, Rows, ShaderCode,
    Shading, SharedMesh, SharedMorph, SharedPart, SideFiles, Skeleton, Sky, Sprite, SpriteFacing,
    SpriteFrame, SpriteImage, TerrainData, TerrainSheet, Texture, TextureKind, TileLight,
};
use engine_render::{
    BatchKind, CapsuleDraw, Capture, Drawables, EntityDraw, GpuParticles, LightDraw, LiveLight,
    MovingDraw, ParticleColor, ParticleDraw, ParticleShape, PostSettings, Renderer, SpriteDraw,
    Target, TileStreamer, View, build_world,
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
        light_shift: [0.0; 3],
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
    // Doubled to 400, 200 and 100, then brought down together to fit.
    assert_eq!(r, 255, "400 of 255 fits");
    assert!((126..=129).contains(&g), "green {g}");
    assert!((62..=66).contains(&b), "blue {b}");
}

/// A BC7 block of one opaque colour, whose parts are 7 bits each: mode 6, with
/// every pixel on the first end of its line.
fn solid_block(color: [u8; 3]) -> Vec<u8> {
    let mut bits = 1_u128 << 6;
    for (number, part) in color.into_iter().chain([127]).enumerate() {
        bits |= u128::from(part) << (7 + number * 14);
    }
    bits |= 3 << 63;
    bits.to_le_bytes().to_vec()
}

/// A compiled picture goes up as blocks, in the format they're in. Sent as
/// anything else, a copy reads past its bytes and the device is lost.
#[test]
fn a_compressed_texture_draws_its_colour() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map(1);
    built.textures[0].mips = vec![
        MipLevel {
            width: 8,
            height: 8,
            rgba: solid_block([100, 40, 20]).repeat(4),
            form: PixelForm::Bc7Srgb,
        },
        MipLevel {
            width: 4,
            height: 4,
            rgba: solid_block([100, 40, 20]),
            form: PixelForm::Bc7Srgb,
        },
        MipLevel {
            width: 2,
            height: 2,
            rgba: solid_block([100, 40, 20]),
            form: PixelForm::Bc7Srgb,
        },
        MipLevel {
            width: 1,
            height: 1,
            rgba: solid_block([100, 40, 20]),
            form: PixelForm::Bc7Srgb,
        },
    ];
    renderer.load_map(Arc::new(built)).expect("load");
    let capture = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    let [r, g, b, _] = centre(&capture);
    assert!(r > g && g > b, "red over green over blue: {r} {g} {b}");
    assert!(r > 0, "it drew");
}

/// The centre of the square, drawn with a material whose shader has `code`.
fn with_shader(renderer: &mut Renderer, code: &str, params: [f32; 8]) -> [u8; 4] {
    let mut built = map(1);
    built.textures[0].material = Some(Arc::new(Material {
        shader: Some(Arc::new(ShaderCode {
            name: "test".to_owned(),
            code: code.to_owned(),
        })),
        shader_params: params,
        shading: Shading::Unlit,
        ..Material::default()
    }));
    renderer.load_map(Arc::new(built)).expect("load");
    let capture = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    centre(&capture)
}

/// The centre of the square, drawn with `metal_rough`, in a dim red light.
fn with_metal(renderer: &mut Renderer, metal_rough: Option<MetalRough>) -> [u8; 4] {
    let mut built = map(1);
    // Dim enough that nothing's held to what the screen can show.
    built.light = vec![[60, 30, 15]; 81];
    built.textures[0].material = Some(Arc::new(Material {
        metal_rough,
        ..Material::default()
    }));
    renderer.load_map(Arc::new(built)).expect("load");
    let capture = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    centre(&capture)
}

#[test]
fn metal_shows_the_room_tinted_by_its_colour() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let plain = with_metal(&mut renderer, None);
    let dull = |metallic, roughness| {
        Some(MetalRough {
            metallic,
            roughness,
            map: None,
        })
    };
    let not_metal = with_metal(&mut renderer, dull(0.0, 1.0));
    let difference = (i32::from(plain[0]) - i32::from(not_metal[0])).abs();
    assert!(difference <= 8, "{plain:?} {not_metal:?}");
    let metal = with_metal(&mut renderer, dull(1.0, 0.5));
    assert!(metal[0] > 0, "it isn't black: {metal:?}");
    assert_ne!(plain, metal);
}

/// What green a broken shader shows.
const GREEN: [u8; 3] = [0, 255, 0];

#[test]
fn surface_code_changes_a_surface() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let code = "fn shader_surface(surface: Surface) -> Surface {
        var out = surface;
        out.base = vec4<f32>(shader_param(0), 0.0, shader_param(1), 1.0);
        return out;
    }";
    let [r, g, b, _] = with_shader(
        &mut renderer,
        code,
        [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    );
    assert_eq!([r, g, b], [255, 0, 0], "the first number");
    let [r, g, b, _] = with_shader(
        &mut renderer,
        code,
        [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    );
    assert_eq!(
        [r, g, b],
        [0, 0, 255],
        "an instance only differs in its numbers"
    );
    assert!(renderer.take_shader_problems().is_empty());
}

#[test]
fn a_shader_can_be_the_whole_of_a_surface() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let code = "fn shader_whole(in: VertexOut) -> vec4<f32> {
        return vec4<f32>(0.0, 0.0, 1.0, 1.0);
    }";
    let [r, g, b, _] = with_shader(&mut renderer, code, [0.0; 8]);
    assert_eq!([r, g, b], [0, 0, 255]);
}

#[test]
fn a_broken_shader_shows_green_and_says_why() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let [r, g, b, _] = with_shader(&mut renderer, "fn shader_whole( nonsense", [0.0; 8]);
    assert_eq!([r, g, b], GREEN);
    let problems = renderer.take_shader_problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].starts_with("test: "), "{problems:?}");
}

#[test]
fn a_missing_shader_shows_green_and_says_so() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let [r, g, b, _] = with_shader(&mut renderer, "", [0.0; 8]);
    assert_eq!([r, g, b], GREEN);
    assert_eq!(renderer.take_shader_problems().len(), 1);
}

#[test]
fn shaders_survive_a_change_of_samples() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let code = "fn shader_whole(in: VertexOut) -> vec4<f32> {
        return vec4<f32>(0.0, 0.0, 1.0, 1.0);
    }";
    let _first = with_shader(&mut renderer, code, [0.0; 8]);
    renderer.set_antialiasing(4).expect("samples");
    let capture = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    let [r, g, b, _] = centre(&capture);
    assert_eq!([r, g, b], [0, 0, 255]);
}

#[test]
fn moving_vertices_moves_a_surface() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let code = "fn shader_vertex(world: vec3<f32>) -> vec3<f32> {
        return world + vec3<f32>(100000.0, 0.0, 0.0);
    }";
    let [r, g, b, _] = with_shader(&mut renderer, code, [0.0; 8]);
    let plain = with_shader(
        &mut renderer,
        "fn shader_uv(uv: vec2<f32>, world: vec3<f32>) -> vec2<f32> { return uv; }",
        [0.0; 8],
    );
    assert_ne!(
        [r, g, b],
        [plain[0], plain[1], plain[2]],
        "it left the view"
    );
}

#[test]
fn the_final_pass_can_have_code_of_its_own() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map(1);
    built.post_shader = Some(Arc::new(ShaderCode {
        name: "test".to_owned(),
        code: "fn shader_post(color: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
            return vec3<f32>(0.0, 0.0, 1.0);
        }"
        .to_owned(),
    }));
    renderer.load_map(Arc::new(built)).expect("load");
    let capture = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    let [r, g, b, _] = centre(&capture);
    assert_eq!([r, g, b], [0, 0, 255]);
    renderer.load_map(Arc::new(map(1))).expect("load");
    let capture = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    assert_ne!(centre(&capture)[..3], [0, 0, 255], "the next map has none");
}

#[test]
fn broken_final_pass_code_is_left_out_and_noted() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map(1);
    built.post_shader = Some(Arc::new(ShaderCode {
        name: "test".to_owned(),
        code: "fn shader_post( nonsense".to_owned(),
    }));
    renderer.load_map(Arc::new(built)).expect("load");
    assert_eq!(renderer.take_shader_problems().len(), 1);
    let capture = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    assert_eq!(centre(&capture)[0], 255, "drawn as it is");
}

#[test]
fn pipelines_are_kept_for_the_next_run_and_a_bad_file_is_ignored() {
    let path = std::env::temp_dir().join(format!("engine-pipes-{}.cache", std::process::id()));
    let _gone = std::fs::remove_file(&path);
    let saved = Some(path.clone());
    let Ok(first) = Renderer::keeping_pipelines(Target::Offscreen, (64, 64), saved.clone()) else {
        return;
    };
    drop(first);
    let bytes = std::fs::read(&path).expect("kept");
    assert_eq!(&bytes[..4], b"EPIP");
    std::fs::write(&path, &bytes[..bytes.len() / 2]).expect("cut short");
    let second = Renderer::keeping_pipelines(Target::Offscreen, (64, 64), saved);
    drop(second.expect("a file cut short is only ignored"));
    assert_eq!(std::fs::read(&path).expect("kept").len(), bytes.len());
    std::fs::remove_file(&path).expect("clean up");
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
        ..ParticleDraw::default()
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
            form: PixelForm::Rgba,
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

/// A blue wall standing at the far side of a calm pool, seen at a low angle, with
/// the pool mirroring by `reflect`.
fn wall_over_a_pool(renderer: &mut Renderer, reflect: f32) -> Capture {
    let mut built = map(2);
    // The first square is wide enough that no edge of it shows, and the second
    // stands up along its far side, in a light of its own.
    let corners = [
        [-2000.0, -2000.0],
        [2000.0, -2000.0],
        [2000.0, 128.0],
        [-2000.0, 128.0],
    ];
    for (at, [x, y]) in corners.into_iter().enumerate() {
        built.points[at] = Vec3::new(x, y, 0.0);
    }
    let first = built.faces[1].first_point as usize;
    built.points[first] = Vec3::new(-2000.0, 128.0, 0.0);
    built.points[first + 1] = Vec3::new(2000.0, 128.0, 0.0);
    built.points[first + 2] = Vec3::new(2000.0, 128.0, 60.0);
    built.points[first + 3] = Vec3::new(-2000.0, 128.0, 60.0);
    built.light.extend([[0, 0, 200]; 81]);
    if let Some(light) = &mut built.faces[1].light {
        light.first_sample = 81;
    }
    built.pools.push(Pool {
        low: Vec3::new(-2000.0, -2000.0, -8.0),
        high: Vec3::new(2000.0, 127.0, 1.0),
        look: PoolLook {
            color: [0.0, 0.0, 0.1],
            absorption: 0.0,
            caustics: 0.0,
            reflect,
            waves: 0.0,
            refraction: 0.0,
            tint: [0.0; 4],
        },
    });
    renderer.load_map(Arc::new(built)).expect("load");
    let view = View {
        origin: Vec3::new(0.0, -120.0, 24.0),
        yaw: 90.0,
        pitch: 6.0,
        ..view_from_above()
    };
    let instances = [ModelInstance {
        model: 1,
        origin: Vec3::ZERO,
        visible: true,
        ..ModelInstance::default()
    }];
    renderer
        .capture(&view, with_instance(&instances), 128, 128)
        .expect("capture")
}

/// A calm surface mirrors evenly. Noise in how it looked showed as a stipple, on
/// the edge of what it mirrors: here the top of a wall, which runs straight across
/// the view, so pixels side by side should match.
#[test]
fn a_calm_surface_has_no_stipple() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let capture = wall_over_a_pool(&mut renderer, 1.0);
    let mut uneven = 0;
    for y in 0..128 {
        for x in 66..104 {
            let [r, g, b, _] = pixel(&capture, x, y);
            let [nr, ng, nb, _] = pixel(&capture, x + 1, y);
            let gap = |a: u8, b: u8| (i32::from(a) - i32::from(b)).abs();
            if gap(r, nr).max(gap(g, ng)).max(gap(b, nb)) > 6 {
                uneven += 1;
            }
        }
    }
    assert!(
        uneven < 8,
        "{uneven} pixels differ from the one beside them"
    );
}

/// A surface mirrors what its pool says, and one that says nothing doesn't.
#[test]
fn how_much_a_surface_mirrors_is_up_to_its_pool() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let matt = wall_over_a_pool(&mut renderer, 0.0);
    let shiny = wall_over_a_pool(&mut renderer, 1.0);
    let bluer = |capture: &Capture| {
        let [_, _, b, _] = pixel(capture, 84, 70);
        b
    };
    assert!(
        bluer(&shiny) > bluer(&matt),
        "{} {}",
        bluer(&shiny),
        bluer(&matt)
    );
}

/// Depth takes things towards a pool's colour, more the further they are.
#[test]
fn depth_tints_towards_a_pools_colour() {
    let look = PoolLook {
        color: [0.0, 0.0, 1.0],
        absorption: 0.02,
        caustics: 0.0,
        reflect: 0.0,
        waves: 0.0,
        refraction: 0.0,
        tint: [0.0; 4],
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
        form: PixelForm::Rgba,
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

/// Decals lie on solid surfaces, so they don't need a draw for each face, which is what
/// made 800 of them cost more than all the layers in a level.
#[test]
fn decals_share_a_draw() {
    let decal = Arc::new(Material {
        alpha: AlphaMode::Blend,
        shading: Shading::Unlit,
        decal: true,
        ..Material::default()
    });
    let mut built = map(1);
    let face = built.faces[0].clone();
    built.faces.push(face);
    built.textures[0].mips = image_of(2, |_, _| [255, 0, 0, 128]);
    built.textures[0].material = Some(decal);
    let mesh = build_world(&built).expect("a mesh");
    let blended = mesh.batches.iter().filter(|b| b.kind == BatchKind::Blended);
    assert_eq!(blended.count(), 1, "one draw for all of them");
}

/// `map` with its one face's light moved out to tile 0, half red in its first
/// sample's column, with `average` as its colour at a distance.
fn tiled_map(average: [u8; 3]) -> (Map, TileLight) {
    let mut built = map(1);
    let light = built.faces[0].light.as_mut().expect("lit");
    light.tile = 0;
    light.average = average;
    let tile = TileLight {
        light: std::mem::take(&mut built.light),
        ..TileLight::default()
    };
    built.tile_size = 1024.0;
    built.light_tiles = Some(LightTiles::new(1024.0, vec![[0, 0]]));
    (built, tile)
}

/// A tile with no detail draws in its average colour, its detail replaces that,
/// and taking the detail away puts the average back.
#[test]
fn a_tile_shows_its_average_until_its_light_arrives() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let (built, tile) = tiled_map([0, 10, 240]);
    renderer.load_map(Arc::new(built)).expect("load");
    let centre_of = |renderer: &mut Renderer| {
        let capture = renderer
            .capture(&view_from_above(), Drawables::default(), 16, 16)
            .expect("capture");
        centre(&capture)
    };
    let far = centre_of(&mut renderer);
    assert!(far[2] > far[0] + 100, "the average: {far:?}");
    assert!(renderer.show_tile_light(0, &tile).expect("shows"));
    assert_eq!(renderer.shown_tiles(), 1);
    let near = centre_of(&mut renderer);
    assert!(near[0] > near[2] + 50, "its own light: {near:?}");
    assert!(!renderer.show_tile_light(0, &tile).expect("shows"), "once");
    renderer.hide_tile_light(0).expect("hides");
    assert_eq!(renderer.shown_tiles(), 0);
    assert_eq!(centre_of(&mut renderer), far, "back as it was");
    assert!(renderer.show_tile_light(0, &tile).expect("shows"), "again");
    assert_eq!(centre_of(&mut renderer), near, "just the same");
}

/// A streamer brings a tile's light to the view off the main thread, and takes
/// it away when the view goes far off.
#[test]
fn a_tile_streams_in_and_out_as_the_view_moves() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let (mut built, tile) = tiled_map([0, 10, 240]);
    let file = LightTiles::tile_file([0, 0]);
    let bytes = tile.to_bytes();
    let files: SideFiles = Arc::new(move |path| (path == file).then(|| bytes.clone()));
    built.light_tiles.as_mut().expect("tiled").attach("", files);
    let built = Arc::new(built);
    renderer.load_map(Arc::clone(&built)).expect("load");
    let mut streamer = TileStreamer::new(&built).expect("it streams");
    let near = Vec3::new(64.0, 64.0, 64.0);
    let settle = |streamer: &mut TileStreamer, renderer: &mut Renderer, eye: Vec3| {
        for _ in 0..2000 {
            streamer.update(eye, renderer).expect("updates");
            if streamer.settled() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        streamer.update(eye, renderer).expect("updates");
    };
    settle(&mut streamer, &mut renderer, near);
    assert_eq!(renderer.shown_tiles(), 1, "it came");
    let tiles = built.light_tiles.as_ref().expect("tiled");
    assert!(tiles.held(0).is_some(), "light at a point can use it");
    settle(&mut streamer, &mut renderer, Vec3::new(9000.0, 0.0, 0.0));
    assert_eq!(renderer.shown_tiles(), 0, "it went");
    assert!(tiles.held(0).is_none());
    settle(&mut streamer, &mut renderer, near);
    assert_eq!(renderer.shown_tiles(), 1, "it came back");
}

/// `count` faces each with a 256 sample square of light, which takes more than
/// the largest atlas could hold at once.
fn too_much_light(count: u32, tile: u32) -> Map {
    let mut built = map(count);
    for face in &mut built.faces {
        let light = face.light.as_mut().expect("lit");
        light.width = 256;
        light.height = 256;
        light.tile = tile;
    }
    built
}

/// Light that won't fit the atlas stops a map loading, unless it streams, and
/// then only what's near has to fit.
#[test]
fn streamed_light_goes_past_what_an_atlas_holds() {
    let count = 1100;
    assert!(build_world(&too_much_light(count, RESIDENT)).is_none());
    let mut built = too_much_light(count, 0);
    built.tile_size = 1024.0;
    built.light_tiles = Some(LightTiles::new(1024.0, vec![[0, 0]]));
    let mesh = build_world(&built).expect("it builds");
    assert_eq!(mesh.atlas_size, 4096);
}

/// Light too bright to show keeps its colour. Red stopping while green goes on
/// rising used to turn an orange light green near its source.
#[test]
fn bright_light_keeps_its_colour() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let lights = [LiveLight {
        origin: Vec3::new(64.0, 64.0, 40.0),
        radius: 300.0,
        color: [8.0, 4.0, 2.0],
        ..LiveLight::default()
    }];
    let drawables = Drawables {
        live_lights: &lights,
        ..Drawables::default()
    };
    let capture = renderer
        .capture(&view_from_above(), drawables, 16, 16)
        .expect("capture");
    let [red, green, blue, _] = centre(&capture);
    assert!(red > 200, "it's bright: {red}");
    assert!(
        i32::from(green) * 2 < i32::from(red) + 40,
        "{red} {green} {blue}"
    );
    assert!(green > blue, "{red} {green} {blue}");
}

/// Clouds drift smoothly. A hash of `sin` broke them into blocks with hard edges,
/// so neighbouring pixels of open sky jumped.
#[test]
fn clouds_have_no_hard_edges() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map(1);
    built.sky = Some(Sky {
        zenith: [0.1, 0.2, 0.5],
        horizon: [0.1, 0.2, 0.5],
        clouds: Some(Clouds {
            color: [1.0, 1.0, 1.0],
            coverage: 0.5,
            speed: [0.0, 0.0],
            scale: 1.0,
            image: None,
        }),
        ..Sky::default()
    });
    renderer.load_map(Arc::new(built)).expect("load");
    let view = View {
        origin: Vec3::new(0.0, 0.0, -2000.0),
        pitch: -70.0,
        ..view_from_above()
    };
    let capture = renderer
        .capture(&view, Drawables::default(), 256, 256)
        .expect("capture");
    let red = |x: u32, y: u32| i32::from(pixel(&capture, x, y)[0]);
    let (mut jumps, mut low, mut high) = (0, 255, 0);
    for y in 0..256 {
        for x in 0..255 {
            low = low.min(red(x, y));
            high = high.max(red(x, y));
            if (red(x, y) - red(x + 1, y)).abs() > 48 {
                jumps += 1;
            }
        }
    }
    assert!(high - low > 60, "there are clouds: {low} {high}");
    assert!(jumps < 150, "{jumps}");
}

/// How many different colours a capture has.
fn shades(capture: &Capture) -> usize {
    let (pixels, _) = capture.rgba.as_chunks::<4>();
    let found: std::collections::HashSet<[u8; 4]> = pixels.iter().copied().collect();
    found.len()
}

/// SMAA leaves flat pixels alone and puts steps between an edge's two sides.
#[test]
fn smaa_grades_edges_and_leaves_the_rest() {
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
    assert!(
        shades(&soft) > shades(&plain) + 4,
        "{} {}",
        shades(&plain),
        shades(&soft)
    );
    renderer.set_post(PostSettings::default());
    let again = renderer
        .capture(&view, Drawables::default(), 96, 96)
        .expect("capture");
    assert_eq!(
        plain.rgba, again.rgba,
        "turning it off leaves nothing behind"
    );
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

/// How blue the middle of the square is under a light, if it says it faces
/// `normal`.
fn blue_with_normal(renderer: &mut Renderer, normal: [i8; 3]) -> u8 {
    let mut built = map(1);
    built.normals = vec![normal; built.points.len()];
    renderer.load_map(Arc::new(built)).expect("load");
    let lights = [blue_light(Vec3::new(64.0, 64.0, 80.0), false)];
    let drawables = Drawables {
        live_lights: &lights,
        ..Drawables::default()
    };
    let capture = renderer
        .capture(&view_from_above(), drawables, 16, 16)
        .expect("capture");
    centre(&capture)[2]
}

/// The view of the square, with `particles` and `gpu_particles` over it.
fn with_particles(
    renderer: &mut Renderer,
    particles: &[ParticleDraw],
    gpu_particles: &[GpuParticles],
) -> [u8; 4] {
    let drawables = Drawables {
        particles,
        gpu_particles,
        ..Drawables::default()
    };
    let capture = renderer
        .capture(&view_from_above(), drawables, 64, 64)
        .expect("capture");
    centre(&capture)
}

/// A soft blue particle in the middle of the view.
fn soft_blue(adds: bool) -> ParticleDraw {
    ParticleDraw {
        origin: Vec3::new(64.0, 64.0, 32.0),
        color: ParticleColor {
            red: 0,
            green: 0,
            blue: 255,
        },
        alpha: 255,
        size: 80.0,
        shape: ParticleShape::Soft,
        glow: 1.0,
        adds,
        ..ParticleDraw::default()
    }
}

#[test]
fn a_soft_particle_covers_what_is_behind_it() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let [r, _, b, _] = with_particles(&mut renderer, &[soft_blue(false)], &[]);
    assert!(b > r, "{r} {b}");
}

#[test]
fn a_particle_that_adds_keeps_what_is_behind_it() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let plain = with_particles(&mut renderer, &[], &[]);
    let [r, _, b, _] = with_particles(&mut renderer, &[soft_blue(true)], &[]);
    assert!(b > plain[2], "more blue: {} {b}", plain[2]);
    assert!(
        i32::from(r) * 2 > i32::from(plain[0]),
        "still red: {} {r}",
        plain[0]
    );
}

#[test]
fn the_gpu_works_out_particles_from_their_numbers() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let plain = with_particles(&mut renderer, &[], &[]);
    let batch = GpuParticles {
        origin: Vec3::new(64.0, 64.0, 32.0),
        life: 2.0,
        direction: Vec3::new(0.0, 0.0, 1.0),
        spread: -1.0,
        color: [0.0, 0.0, 1.0, 1.0],
        end_color: [0.0, 0.0, 1.0, 1.0],
        age: 5.0,
        size: [30.0, 30.0],
        radius: 8.0,
        count: 500,
        ..GpuParticles::default()
    };
    let [_, _, b, _] = with_particles(&mut renderer, &[], &[batch]);
    assert!(b > plain[2], "more blue: {} {b}", plain[2]);
}

/// A moving light goes by which way a mesh says it faces, not by its triangle, or
/// each triangle of a curve shows as a band.
#[test]
fn a_moving_light_follows_a_meshs_normal() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let toward = blue_with_normal(&mut renderer, [0, 0, 127]);
    let aside = blue_with_normal(&mut renderer, [127, 0, 0]);
    assert!(toward > aside, "{toward} and {aside}");
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

/// A flat, wide model over the surface, as something for shadows to come from.
fn figure(renderer: &mut Renderer) -> u32 {
    let mut model = triangle();
    for pose in &mut model.poses {
        for vertex in pose {
            vertex.position[2] = 0;
        }
    }
    model.scale = Vec3::new(1.0, 1.0, 1.0);
    model.offset = Vec3::ZERO;
    renderer.add_model(Arc::new(model)).expect("model")
}

/// A figure at `origin`, in its pose.
fn figure_at(model: u32, origin: Vec3, pose: u32) -> EntityDraw {
    EntityDraw {
        model,
        pose,
        next_pose: pose,
        origin,
        ambient: 1.0,
        ..EntityDraw::default()
    }
}

/// How much blue a capture of `entities` has, under `light` with and without
/// its shadows, from `view`.
fn blue_with_shadows(
    renderer: &mut Renderer,
    view: &View,
    entities: &[EntityDraw],
    light: Vec3,
) -> [u64; 2] {
    [false, true].map(|shadows| {
        let lights = [blue_light(light, shadows)];
        let drawables = Drawables {
            entities,
            live_lights: &lights,
            ..Drawables::default()
        };
        let capture = renderer.capture(view, drawables, 64, 64).expect("capture");
        capture
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| u64::from(pixel[2]))
            .sum::<u64>()
    })
}

/// A figure blocks a moving light, as a brush model does.
#[test]
fn a_figure_casts_a_live_shadow() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    renderer.set_post(PostSettings {
        live_shadows: true,
        ..PostSettings::default()
    });
    let model = figure(&mut renderer);
    let entities = [figure_at(model, Vec3::new(20.0, 20.0, 30.0), 0)];
    let light = Vec3::new(64.0, 64.0, 60.0);
    let [open, blocked] = blue_with_shadows(&mut renderer, &view_from_above(), &entities, light);
    assert!(blocked + 5000 < open, "{open} {blocked}");
}

/// A lamp overhead throws a figure's shadow on the floor, which is the tile of
/// a light that looks straight down. The eye is higher, to see past the figure.
#[test]
fn a_lamp_overhead_casts_a_figure_on_the_floor() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    renderer.set_post(PostSettings {
        live_shadows: true,
        ..PostSettings::default()
    });
    let model = figure(&mut renderer);
    let entities = [figure_at(model, Vec3::new(20.0, 20.0, 30.0), 0)];
    let view = View {
        origin: Vec3::new(64.0, 64.0, 300.0),
        ..view_from_above()
    };
    let light = Vec3::new(64.0, 64.0, 100.0);
    let [open, blocked] = blue_with_shadows(&mut renderer, &view, &entities, light);
    assert!(blocked + 1000 < open, "{open} {blocked}");
}

/// Standing still costs nothing, and a new pose redraws what it can shade.
#[test]
fn a_figure_redraws_shadows_only_when_it_changes() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    renderer.set_post(PostSettings {
        live_shadows: true,
        ..PostSettings::default()
    });
    let model = figure(&mut renderer);
    let lights = [blue_light(Vec3::new(64.0, 64.0, 60.0), true)];
    let frame = |renderer: &mut Renderer, entities: &[EntityDraw]| {
        let drawables = Drawables {
            entities,
            live_lights: &lights,
            ..Drawables::default()
        };
        renderer
            .capture(&view_from_above(), drawables, 16, 16)
            .expect("capture");
        renderer.shadow_redraws()
    };
    let at = Vec3::new(20.0, 20.0, 30.0);
    let standing = [figure_at(model, at, 0)];
    let far_off = [figure_at(model, Vec3::new(9000.0, 0.0, 0.0), 0)];
    let lit_differently = [EntityDraw {
        ambient: 0.5,
        ..standing[0]
    }];
    let turned = [EntityDraw {
        angles: Vec3::new(0.0, 45.0, 0.0),
        ..standing[0]
    }];
    assert_eq!(frame(&mut renderer, &standing), 1, "the first frame");
    assert_eq!(frame(&mut renderer, &standing), 0, "it stood still");
    assert_eq!(frame(&mut renderer, &lit_differently), 0, "only its light");
    assert_eq!(frame(&mut renderer, &turned), 1, "it turned");
    assert_eq!(frame(&mut renderer, &turned), 0, "and stopped");
    assert_eq!(frame(&mut renderer, &[]), 1, "it left");
    assert_eq!(frame(&mut renderer, &far_off), 0, "out of reach");
}

/// Something that never stops moving can't hold a far light back for ever, and
/// a redraw that had to wait still happens after everything's stopped.
#[test]
fn every_light_gets_its_turn() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    renderer.set_post(PostSettings {
        live_shadows: true,
        ..PostSettings::default()
    });
    let model = figure(&mut renderer);
    let lights: Vec<LiveLight> = (0_u16..4)
        .map(|index| LiveLight {
            id: u32::from(index),
            ..blue_light(Vec3::new(f32::from(index) * 10.0, 64.0, 60.0), true)
        })
        .collect();
    let frame = |renderer: &mut Renderer, entities: &[EntityDraw]| {
        let drawables = Drawables {
            entities,
            live_lights: &lights,
            ..Drawables::default()
        };
        renderer
            .capture(&view_from_above(), drawables, 16, 16)
            .expect("capture");
        renderer.shadow_redraws()
    };
    let at = |height: f32| [figure_at(model, Vec3::new(20.0, 20.0, height), 0)];
    assert_eq!(frame(&mut renderer, &at(30.0)), 2);
    assert_eq!(frame(&mut renderer, &at(30.0)), 2, "the rest of the first");
    assert_eq!(frame(&mut renderer, &at(30.0)), 0, "all drawn");
    assert_eq!(frame(&mut renderer, &at(31.0)), 2, "four lights want it");
    assert_eq!(frame(&mut renderer, &at(31.0)), 2, "the two left waiting");
    assert_eq!(frame(&mut renderer, &at(31.0)), 0, "nothing's owed");
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

/// The sky covers every way a view can look, with nothing of the clear colour
/// showing through.
#[test]
fn the_sky_covers_every_direction() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map(1);
    built.sky = Some(Sky {
        zenith: [0.0, 0.0, 1.0],
        horizon: [0.0, 1.0, 0.0],
        ground: [1.0, 0.0, 0.0],
        ..Sky::default()
    });
    renderer.load_map(Arc::new(built)).expect("load");
    for yaw in (0..360).step_by(30) {
        for pitch in [-80.0, -45.0, -10.0, 0.0, 30.0, 70.0] {
            #[expect(clippy::cast_precision_loss, reason = "small angles")]
            let view = View {
                origin: Vec3::new(5000.0, -4000.0, 900.0),
                yaw: yaw as f32,
                pitch,
                ..view_from_above()
            };
            let capture = renderer
                .capture(&view, Drawables::default(), 48, 48)
                .expect("capture");
            let (pixels, _) = capture.rgba.as_chunks::<4>();
            let clear = pixels
                .iter()
                .filter(|p| p[0] < 8 && p[1] < 8 && p[2] < 12)
                .count();
            assert_eq!(clear, 0, "at yaw {yaw}, pitch {pitch}");
        }
    }
}

/// A count of samples no device has rounds down, and still draws.
#[test]
fn an_odd_sample_count_rounds_down() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let plain = renderer
        .capture(&view_from_above(), Drawables::default(), 32, 32)
        .expect("capture");
    for samples in [3, 5, 7, 64] {
        renderer.set_antialiasing(samples).expect("samples");
        let capture = renderer
            .capture(&view_from_above(), Drawables::default(), 32, 32)
            .expect("capture");
        assert_eq!(centre(&plain), centre(&capture), "with {samples}");
    }
}

/// With shadows turned off, nothing goes on being redrawn.
#[test]
fn turning_shadows_off_stops_redraws() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    renderer.set_post(PostSettings {
        live_shadows: true,
        ..PostSettings::default()
    });
    let lights = [blue_light(Vec3::new(64.0, 64.0, 60.0), true)];
    let drawables = || Drawables {
        live_lights: &lights,
        ..Drawables::default()
    };
    renderer
        .capture(&view_from_above(), drawables(), 16, 16)
        .expect("capture");
    assert_eq!(renderer.shadow_redraws(), 1);
    renderer.set_post(PostSettings::default());
    renderer
        .capture(&view_from_above(), drawables(), 16, 16)
        .expect("capture");
    assert_eq!(renderer.shadow_redraws(), 0);
}

/// A map with a small square that moves, floating over the floor, whose corners go half with
/// each of two joints if `shared`, and otherwise all with the first.
fn map_with_a_moving_square(shared: bool) -> Map {
    let mut map = map(1);
    let corner = |x: f32, y: f32| MeshCorner {
        position: [x, y, 32.0],
        uv: [0.0, 0.0],
        normal: [0, 0, 127],
    };
    let corners = vec![
        corner(56.0, 56.0),
        corner(72.0, 56.0),
        corner(72.0, 72.0),
        corner(56.0, 72.0),
    ];
    let weights = if shared {
        [128, 127, 0, 0]
    } else {
        [255, 0, 0, 0]
    };
    let mesh = SharedMesh {
        joints: vec![[0, 1, 0, 0]; corners.len()],
        weights: vec![weights; corners.len()],
        indices: vec![0, 1, 2, 0, 2, 3],
        levels: vec![vec![SharedPart {
            texture: 0,
            first: 0,
            count: 6,
        }]],
        corners,
        ..SharedMesh::default()
    };
    map.moving_meshes.push(MovingMesh {
        name: "square".to_owned(),
        mesh,
        skeleton: Skeleton::default(),
    });
    map
}

/// The middle of where `a` differs from `b`, and how many pixels that is.
fn middle_of_change(a: &Capture, b: &Capture) -> (f32, f32, u32) {
    let (mut x, mut y, mut count) = (0.0, 0.0, 0);
    let pixels = a
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.rgba.as_chunks::<4>().0);
    for (at, (one, other)) in (0_u32..).zip(pixels) {
        if one[..3] != other[..3] {
            x += f32::from(u16::try_from(at % a.width).expect("small"));
            y += f32::from(u16::try_from(at / a.width).expect("small"));
            count += 1;
        }
    }
    let share = f32::from(u16::try_from(count.max(1)).expect("small"));
    (x / share, y / share, count)
}

/// What moves a corner along X by `by`.
fn along(by: f32) -> Rows {
    let mut rows = NO_MOVE;
    rows[0][3] = by;
    rows
}

/// What's drawn of a mesh placed where it was made, in black, with `palette`.
fn draw_moving(renderer: &mut Renderer, palette: &[Rows]) -> Capture {
    let draw = MovingDraw {
        mesh: 0,
        rows: NO_MOVE,
        first_joint: 0,
        joint_count: u32::try_from(palette.len()).expect("few"),
        light: [0.0; 3],
        center: Vec3::new(64.0, 64.0, 32.0),
        radius: 200.0,
        ..MovingDraw::default()
    };
    let drawables = Drawables {
        moving: &[draw],
        palette,
        ..Drawables::default()
    };
    renderer
        .capture(&view_from_above(), drawables, 64, 64)
        .expect("capture")
}

/// The compute pass really does put corners where the palette says. Looking down, X runs up the
/// picture or across it, so this goes by how far the mesh moved and not which way.
#[test]
fn the_compute_pass_moves_a_mesh_by_its_joints() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer
        .load_map(Arc::new(map_with_a_moving_square(false)))
        .expect("load");
    let empty = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    let still = draw_moving(&mut renderer, &[NO_MOVE, NO_MOVE]);
    let moved = draw_moving(&mut renderer, &[along(16.0), NO_MOVE]);
    let (still_x, still_y, still_count) = middle_of_change(&still, &empty);
    let (moved_x, moved_y, moved_count) = middle_of_change(&moved, &empty);
    assert!(still_count > 20, "it shows: {still_count}");
    assert!(moved_count > 20, "it still shows: {moved_count}");
    let shift = (moved_x - still_x).hypot(moved_y - still_y);
    assert!(shift > 8.0, "it went: {shift}");
}

/// A corner goes by the weighted sum of its joints, as glTF defines it: half with a joint that
/// moves is half as far.
#[test]
fn a_corner_shared_by_joints_goes_by_their_weights() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let shift = |renderer: &mut Renderer, palette: &[Rows]| {
        let empty = renderer
            .capture(&view_from_above(), Drawables::default(), 64, 64)
            .expect("capture");
        let still = draw_moving(renderer, &[NO_MOVE, NO_MOVE]);
        let moved = draw_moving(renderer, palette);
        let (a, b, _) = middle_of_change(&still, &empty);
        let (c, d, _) = middle_of_change(&moved, &empty);
        (c - a).hypot(d - b)
    };
    renderer
        .load_map(Arc::new(map_with_a_moving_square(false)))
        .expect("load");
    let whole = shift(&mut renderer, &[along(16.0), NO_MOVE]);
    renderer
        .load_map(Arc::new(map_with_a_moving_square(true)))
        .expect("load");
    let half = shift(&mut renderer, &[NO_MOVE, along(16.0)]);
    assert!(whole > 8.0, "{whole}");
    assert!((half / whole - 0.5).abs() < 0.15, "{half} of {whole}");
}

/// With no joints to go by, as past the frame's budget, a mesh still shows, where it was made.
#[test]
fn a_mesh_with_no_palette_draws_at_rest() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer
        .load_map(Arc::new(map_with_a_moving_square(false)))
        .expect("load");
    let empty = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    let draw = MovingDraw {
        mesh: 0,
        rows: NO_MOVE,
        first_joint: 9000,
        joint_count: 2,
        light: [0.0; 3],
        center: Vec3::new(64.0, 64.0, 32.0),
        radius: 200.0,
        ..MovingDraw::default()
    };
    let drawables = Drawables {
        moving: &[draw],
        ..Drawables::default()
    };
    let rest = renderer
        .capture(&view_from_above(), drawables, 64, 64)
        .expect("capture");
    let (_, _, count) = middle_of_change(&rest, &empty);
    assert!(count > 20, "{count}");
    assert!(renderer.take_moving_overflow(), "and it says so");
    assert!(!renderer.take_moving_overflow(), "once");
}

/// The square of `map_with_a_moving_square`, with a morph target that moves all of it along X.
fn map_with_a_morphing_square() -> Map {
    let mut map = map_with_a_moving_square(false);
    let moving = &mut map.moving_meshes[0];
    moving.mesh.morph_heads = vec![[0, 1], [1, 1], [2, 1], [3, 1]];
    moving.mesh.morphs = vec![
        SharedMorph {
            target: 0,
            position: [16.0, 0.0, 0.0],
            normal: [0.0; 3],
        };
        4
    ];
    map
}

/// What's drawn of the square with `weights`.
fn draw_morphing(renderer: &mut Renderer, weights: &[f32]) -> Capture {
    let draw = MovingDraw {
        rows: NO_MOVE,
        joint_count: 2,
        weight_count: u32::try_from(weights.len()).expect("few"),
        center: Vec3::new(64.0, 64.0, 32.0),
        radius: 200.0,
        ..MovingDraw::default()
    };
    let drawables = Drawables {
        moving: &[draw],
        palette: &[NO_MOVE, NO_MOVE],
        morph_weights: weights,
        ..Drawables::default()
    };
    renderer
        .capture(&view_from_above(), drawables, 64, 64)
        .expect("capture")
}

/// A morph moves corners by its weight: half the weight is half as far.
#[test]
fn a_morph_moves_corners_by_its_weight() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer
        .load_map(Arc::new(map_with_a_morphing_square()))
        .expect("load");
    let empty = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    let shown = |capture: &Capture| middle_of_change(capture, &empty);
    let (a, b, _) = shown(&draw_morphing(&mut renderer, &[]));
    let (c, d, _) = shown(&draw_morphing(&mut renderer, &[1.0]));
    let (e, f, _) = shown(&draw_morphing(&mut renderer, &[0.5]));
    let whole = (c - a).hypot(d - b);
    let half = (e - a).hypot(f - b);
    assert!(whole > 8.0, "{whole}");
    assert!((half / whole - 0.5).abs() < 0.15, "{half} of {whole}");
}

/// Morphs come before joints, as glTF has it: a morph along X, then a turn, ends up where the
/// turn takes it. The other way round, it would go along X whatever the joint did.
#[test]
fn a_morph_comes_before_the_joints() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer
        .load_map(Arc::new(map_with_a_morphing_square()))
        .expect("load");
    let empty = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    // A joint that takes X to Y, about the square's middle.
    let turn: Rows = [
        [0.0, -1.0, 0.0, 128.0],
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ];
    let draw = |renderer: &mut Renderer, weights: &[f32]| {
        let one = MovingDraw {
            rows: NO_MOVE,
            joint_count: 2,
            weight_count: u32::try_from(weights.len()).expect("few"),
            center: Vec3::new(64.0, 64.0, 32.0),
            radius: 200.0,
            ..MovingDraw::default()
        };
        let drawables = Drawables {
            moving: &[one],
            palette: &[turn, NO_MOVE],
            morph_weights: weights,
            ..Drawables::default()
        };
        let capture = renderer
            .capture(&view_from_above(), drawables, 64, 64)
            .expect("capture");
        middle_of_change(&capture, &empty)
    };
    let (a, b, _) = draw(&mut renderer, &[]);
    let (c, d, _) = draw(&mut renderer, &[1.0]);
    // Without the turn, a morph along X went the other way on the picture.
    let (e, f, _) = middle_of_change(&draw_morphing(&mut renderer, &[]), &empty);
    let (g, h, _) = middle_of_change(&draw_morphing(&mut renderer, &[1.0]), &empty);
    let along = (c - a, d - b);
    let plain = (g - e, h - f);
    assert!(along.0.hypot(along.1) > 8.0, "it moved: {along:?}");
    assert!(plain.0.hypot(plain.1) > 8.0, "it moved: {plain:?}");
    let across = along.0 * plain.0 + along.1 * plain.1;
    assert!(
        across.abs() < 16.0,
        "the morph was turned: {along:?} against {plain:?}"
    );
}

/// A closed box that moves, as an outline needs a back to show.
fn map_with_a_moving_box() -> Map {
    let mut map = map_with_a_moving_square(false);
    let mut corners = Vec::new();
    for (x, y, z) in [
        (-1.0, -1.0, -1.0),
        (1.0, -1.0, -1.0),
        (1.0, 1.0, -1.0),
        (-1.0, 1.0, -1.0),
        (-1.0, -1.0, 1.0),
        (1.0, -1.0, 1.0),
        (1.0, 1.0, 1.0),
        (-1.0, 1.0, 1.0),
    ] {
        #[expect(clippy::cast_possible_truncation, reason = "a normal of 73 either way")]
        let normal = [x, y, z].map(|part: f32| (part * 73.0) as i8);
        corners.push(MeshCorner {
            position: [64.0 + x * 8.0, 64.0 + y * 8.0, 32.0 + z * 8.0],
            uv: [0.0, 0.0],
            normal,
        });
    }
    let moving = &mut map.moving_meshes[0];
    moving.mesh.joints = vec![[0, 1, 0, 0]; corners.len()];
    moving.mesh.weights = vec![[255, 0, 0, 0]; corners.len()];
    moving.mesh.corners = corners;
    moving.mesh.indices = vec![
        0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2, 7, 6,
        3, 0, 4, 3, 4, 7,
    ];
    moving.mesh.levels = vec![vec![SharedPart {
        texture: 0,
        first: 0,
        count: 36,
    }]];
    map
}

/// An outline shows round a moving mesh, as it does round an alias model.
#[test]
fn a_moving_mesh_has_an_outline_hull() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer
        .load_map(Arc::new(map_with_a_moving_box()))
        .expect("load");
    let plain = draw_moving(&mut renderer, &[NO_MOVE, NO_MOVE]);
    renderer.set_post(PostSettings {
        outline_hull: true,
        outline_width: 4.0,
        outline_color: [0.0, 1.0, 0.0],
        ..PostSettings::default()
    });
    let outlined = draw_moving(&mut renderer, &[NO_MOVE, NO_MOVE]);
    let green = |capture: &Capture| {
        let pixels = capture.rgba.as_chunks::<4>().0;
        pixels
            .iter()
            .filter(|pixel| pixel[1] > 200 && pixel[0] < 60)
            .count()
    };
    assert_eq!(green(&plain), 0);
    assert!(green(&outlined) > 10, "{}", green(&outlined));
}

/// Two moving meshes, side by side: one with so many corners that what shows is in its fourth
/// workgroup, and one with few. The first has UVs in the black half of a texture, and the second
/// in the white half.
fn map_with_two_moving_squares() -> Map {
    let mut map = map(1);
    let mut rgba = Vec::new();
    for _row in 0..64 {
        rgba.extend([0, 0, 0, 255].repeat(32));
        rgba.extend([255, 255, 255, 255].repeat(32));
    }
    // Of its own, as a black floor wouldn't show a black square.
    let mut halves = map.textures[0].clone();
    halves.mips[0].rgba = rgba;
    map.textures.push(halves);
    let square = |x: f32, u: f32| {
        [(0.0, 0.0), (12.0, 0.0), (12.0, 12.0), (0.0, 12.0)].map(|(across, up)| MeshCorner {
            position: [x + across, 58.0 + up, 32.0],
            uv: [u, 0.5],
            normal: [0, 0, 127],
        })
    };
    let mesh = |corners: Vec<MeshCorner>| {
        let last = u32::try_from(corners.len()).expect("few") - 4;
        SharedMesh {
            joints: vec![[0, 1, 0, 0]; corners.len()],
            weights: vec![[255, 0, 0, 0]; corners.len()],
            indices: [0, 1, 2, 0, 2, 3].map(|at| last + at).to_vec(),
            levels: vec![vec![SharedPart {
                texture: 1,
                first: 0,
                count: 6,
            }]],
            corners,
            ..SharedMesh::default()
        }
    };
    let mut big = vec![square(0.0, 0.25)[0]; 196];
    big.extend(square(46.0, 0.25));
    let meshes = [mesh(big), mesh(square(70.0, 0.75).to_vec())];
    map.moving_meshes = meshes
        .into_iter()
        .map(|mesh| MovingMesh {
            name: "square".to_owned(),
            mesh,
            skeleton: Skeleton::default(),
        })
        .collect();
    map
}

/// What's drawn of each of the two meshes that's asked for, with its own joints.
fn draw_squares(renderer: &mut Renderer, first: Option<Rows>, second: Option<Rows>) -> Capture {
    let mut palette = Vec::new();
    let mut draws = Vec::new();
    for (mesh, rows) in (0..).zip([first, second]) {
        let Some(rows) = rows else {
            continue;
        };
        draws.push(MovingDraw {
            mesh,
            rows: NO_MOVE,
            first_joint: u32::try_from(palette.len()).expect("few"),
            joint_count: 2,
            light: [1.0; 3],
            center: Vec3::new(64.0, 64.0, 32.0),
            radius: 200.0,
            ..MovingDraw::default()
        });
        palette.extend([rows, NO_MOVE]);
    }
    let drawables = Drawables {
        moving: &draws,
        palette: &palette,
        ..Drawables::default()
    };
    renderer
        .capture(&view_from_above(), drawables, 64, 64)
        .expect("capture")
}

/// One dispatch does every mesh, so each workgroup has to find its own mesh, and its own corners
/// in it.
#[test]
fn meshes_of_different_sizes_each_get_their_own_joints() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer
        .load_map(Arc::new(map_with_two_moving_squares()))
        .expect("load");
    let empty = draw_squares(&mut renderer, None, None);
    let shift = |renderer: &mut Renderer, first: Rows, second: Rows| {
        let still = draw_squares(renderer, Some(NO_MOVE), Some(NO_MOVE));
        let moved = draw_squares(renderer, Some(first), Some(second));
        let (a, b, still_count) = middle_of_change(&still, &empty);
        let (c, d, moved_count) = middle_of_change(&moved, &empty);
        assert!(still_count > 20, "they show: {still_count}");
        // Not exactly, as a square lands differently on the pixels.
        let change = still_count.abs_diff(moved_count);
        assert!(change * 10 < still_count, "{still_count} to {moved_count}");
        (c - a).hypot(d - b)
    };
    let both = shift(&mut renderer, along(4.0), along(4.0));
    let first = shift(&mut renderer, along(4.0), NO_MOVE);
    let second = shift(&mut renderer, NO_MOVE, along(4.0));
    assert!(both > 2.0, "they went: {both}");
    assert!((first - both / 2.0).abs() < 1.0, "{first} of {both}");
    assert!((second - both / 2.0).abs() < 1.0, "{second} of {both}");
}

/// How bright what's different from `empty` is, on average.
fn brightness_of_change(picture: &Capture, empty: &Capture) -> f32 {
    let (mut total, mut count) = (0.0, 0.0);
    let pixels = picture
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(empty.rgba.as_chunks::<4>().0);
    for (pixel, other) in pixels {
        if pixel[..3] != other[..3] {
            total += f32::from(pixel[1]);
            count += 1.0;
        }
    }
    assert!(count > 20.0, "it shows: {count}");
    total / count
}

/// What doesn't move is kept apart from what does, so the two have to stay in step: each mesh
/// reads its own UVs.
#[test]
fn a_moving_mesh_reads_its_own_uvs() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer
        .load_map(Arc::new(map_with_two_moving_squares()))
        .expect("load");
    let empty = draw_squares(&mut renderer, None, None);
    let black = draw_squares(&mut renderer, Some(NO_MOVE), None);
    let white = draw_squares(&mut renderer, None, Some(NO_MOVE));
    let black = brightness_of_change(&black, &empty);
    let white = brightness_of_change(&white, &empty);
    assert!(white > black + 40.0, "{white} against {black}");
}

/// How much blue there is under `light`, with and without its shadows, with `drawables`.
fn blue_with_shadows_of(
    renderer: &mut Renderer,
    drawables: Drawables<'_>,
    light: Vec3,
) -> [u64; 2] {
    [false, true].map(|shadows| {
        let lights = [blue_light(light, shadows)];
        let drawables = Drawables {
            live_lights: &lights,
            ..drawables
        };
        renderer
            .capture(&view_from_above(), drawables, 64, 64)
            .expect("capture")
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| u64::from(pixel[2]))
            .sum::<u64>()
    })
}

/// What's behind the view has a shadow that's in it. Above the eye, just under a lamp, a small
/// thing shades all that the view can see.
#[test]
fn a_moving_mesh_out_of_view_still_casts_a_shadow() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer
        .load_map(Arc::new(map_with_a_moving_square(false)))
        .expect("load");
    renderer.set_post(PostSettings {
        live_shadows: true,
        ..PostSettings::default()
    });
    let mut rows = NO_MOVE;
    rows[2][3] = 158.0;
    let draws = [MovingDraw {
        rows,
        joint_count: 2,
        center: Vec3::new(64.0, 64.0, 190.0),
        radius: 12.0,
        ..MovingDraw::default()
    }];
    let drawables = Drawables {
        moving: &draws,
        palette: &[NO_MOVE, NO_MOVE],
        ..Drawables::default()
    };
    let light = Vec3::new(64.0, 64.0, 200.0);
    let [open, blocked] = blue_with_shadows_of(&mut renderer, drawables, light);
    assert!(blocked + 5000 < open, "{open} {blocked}");
}

/// The same of one of the fixture's models, which aren't drawn when they're not in view.
#[test]
fn a_figure_out_of_view_still_casts_a_shadow() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    renderer.set_post(PostSettings {
        live_shadows: true,
        ..PostSettings::default()
    });
    let mut model = triangle();
    for pose in &mut model.poses {
        for vertex in pose {
            vertex.position[2] = 0;
        }
    }
    // Small, so that it's far enough out of view for all it could reach. Its first pose is a
    // point, as that's what's drawn with no room, and only moved corners can block anything.
    model.scale = Vec3::new(0.1, 0.1, 0.1);
    model.offset = Vec3::ZERO;
    let shape = model.poses[0].clone();
    model.poses = vec![vec![AliasVertex::default(); shape.len()], shape];
    let number = renderer.add_model(Arc::new(model)).expect("model");
    let entities = [figure_at(number, Vec3::new(60.0, 60.0, 190.0), 1)];
    let light = Vec3::new(64.0, 64.0, 200.0);
    let [open, blocked] = blue_with_shadows(&mut renderer, &view_from_above(), &entities, light);
    assert!(blocked + 5000 < open, "{open} {blocked}");
}

/// A mesh's depth goes in ahead of it, and it doesn't write any of its own, so whatever comes
/// after has to find it there.
#[test]
fn a_moving_mesh_hides_what_is_drawn_after_it() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer
        .load_map(Arc::new(map_with_a_moving_square(false)))
        .expect("load");
    let number = renderer.add_model(Arc::new(triangle())).expect("add");
    let below = EntityDraw {
        origin: Vec3::new(14.0, 14.0, 16.0),
        ..draw_of(number)
    };
    let entities = [below];
    let alone = renderer
        .capture(&view_from_above(), with_entity(&entities), 64, 64)
        .expect("capture");
    let draws = [MovingDraw {
        joint_count: 2,
        rows: NO_MOVE,
        center: Vec3::new(64.0, 64.0, 32.0),
        radius: 200.0,
        ..MovingDraw::default()
    }];
    let drawables = Drawables {
        entities: &entities,
        moving: &draws,
        palette: &[NO_MOVE, NO_MOVE],
        ..Drawables::default()
    };
    let both = renderer
        .capture(&view_from_above(), drawables, 64, 64)
        .expect("capture");
    assert!(
        centre(&alone)[0] > 100,
        "the model shows: {:?}",
        centre(&alone)
    );
    assert!(
        centre(&both)[0] < 10,
        "the mesh is over it: {:?}",
        centre(&both)
    );
}

/// Far off, a mesh moves and draws only a coarser level, which is its first corners. Here that's
/// half the square.
#[test]
fn a_moving_mesh_can_move_a_coarser_level() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut map = map_with_a_moving_square(false);
    let mesh = &mut map.moving_meshes[0].mesh;
    let half = SharedPart {
        texture: 0,
        first: 0,
        count: 3,
    };
    mesh.levels.push(vec![half]);
    renderer.load_map(Arc::new(map)).expect("load");
    let empty = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    let at_level = |renderer: &mut Renderer, level: u32, palette: &[Rows]| {
        let draws = [MovingDraw {
            rows: NO_MOVE,
            joint_count: 2,
            level,
            center: Vec3::new(64.0, 64.0, 32.0),
            radius: 200.0,
            ..MovingDraw::default()
        }];
        let drawables = Drawables {
            moving: &draws,
            palette,
            ..Drawables::default()
        };
        renderer
            .capture(&view_from_above(), drawables, 64, 64)
            .expect("capture")
    };
    let finest = at_level(&mut renderer, 0, &[NO_MOVE, NO_MOVE]);
    let coarse = at_level(&mut renderer, 1, &[NO_MOVE, NO_MOVE]);
    let moved = at_level(&mut renderer, 1, &[along(8.0), NO_MOVE]);
    let (_, _, finest_count) = middle_of_change(&finest, &empty);
    let (x, y, coarse_count) = middle_of_change(&coarse, &empty);
    let (a, b, moved_count) = middle_of_change(&moved, &empty);
    assert!(
        coarse_count * 3 < finest_count * 2,
        "{coarse_count} of {finest_count}"
    );
    assert!(
        coarse_count.abs_diff(moved_count) * 10 < coarse_count,
        "it keeps its shape"
    );
    assert!((a - x).hypot(b - y) > 4.0, "and it moved");
}

/// What a matcap shows of a floor whose normal map leans `slope` along V, with its texture squeezed
/// `stretch` times along U. A matcap goes straight by which way a surface faces, so it shows a lean
/// as a colour, red across it and green up it.
fn matcap_on_a_slope(stretch: f32, slope: u8) -> Option<[u8; 4]> {
    let mut renderer = renderer()?;
    let mut built = map(1);
    // The texture and not its light, which is where it was.
    built.faces[0].light_axes = Some(built.faces[0].axes);
    built.faces[0].axes[0].axis = Vec3::new(stretch, 0.0, 0.0);
    built.textures[0].material = Some(Arc::new(Material {
        shading: Shading::Matcap,
        normal: Some(1),
        matcap: Some(2),
        slots: vec![
            Arc::new(image_of(2, |_, _| [128, slope, 0, 255])),
            Arc::new(image_of(64, |x, y| {
                [
                    u8::try_from(x * 4).expect("small"),
                    u8::try_from(y * 4).expect("small"),
                    0,
                    255,
                ]
            })),
        ],
        ..Material::default()
    }));
    renderer.load_map(Arc::new(built)).expect("load");
    let capture = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    Some(centre(&capture))
}

/// How a texture is stretched mustn't change which way a normal map leans. A bake's frame is
/// square, so tangent and bitangent are each a unit long, and scaling them together squashed one.
#[test]
fn a_stretched_normal_map_leans_as_far() {
    let Some(flat) = matcap_on_a_slope(1.0, 128) else {
        return;
    };
    let leaning = matcap_on_a_slope(1.0, 220).expect("it drew");
    let stretched = matcap_on_a_slope(4.0, 220).expect("it drew");
    let other_way = matcap_on_a_slope(0.25, 220).expect("it drew");
    let apart = |a: [u8; 4], b: [u8; 4]| (0..3).map(|at| a[at].abs_diff(b[at])).max().unwrap_or(0);
    assert!(apart(leaning, flat) > 10, "{flat:?} and {leaning:?}");
    assert!(
        apart(stretched, leaning) <= 3,
        "{leaning:?} and {stretched:?}"
    );
    assert!(
        apart(other_way, leaning) <= 3,
        "{leaning:?} and {other_way:?}"
    );
}

/// The middle of a picture with `moving` over the map's floor, with corners coloured `colors`.
fn moving_square_coloured(colors: Vec<[u8; 4]>, material: bool) -> Option<[u8; 4]> {
    let mut renderer = renderer()?;
    let mut map = map_with_a_moving_square(false);
    map.moving_meshes[0].mesh.colors = colors;
    if material {
        map.textures[0].material = Some(Arc::new(Material::default()));
    }
    renderer.load_map(Arc::new(map)).expect("load");
    let draws = [MovingDraw {
        rows: NO_MOVE,
        joint_count: 2,
        light: [1.0; 3],
        center: Vec3::new(64.0, 64.0, 32.0),
        radius: 200.0,
        ..MovingDraw::default()
    }];
    let drawables = Drawables {
        moving: &draws,
        palette: &[NO_MOVE, NO_MOVE],
        ..Drawables::default()
    };
    let capture = renderer
        .capture(&view_from_above(), drawables, 64, 64)
        .expect("capture");
    Some(centre(&capture))
}

/// A mesh's corners' colours multiply its surface, and with none it's as it was.
#[test]
fn a_corners_colour_multiplies_a_mesh() {
    for material in [false, true] {
        let Some(plain) = moving_square_coloured(Vec::new(), material) else {
            return;
        };
        let red = moving_square_coloured(vec![[255, 0, 0, 255]; 4], material).expect("it drew");
        assert!(plain[1] > 100, "it's lit: {plain:?}");
        assert_eq!(red[0], plain[0], "red goes through");
        assert!(red[1] < 5 && red[2] < 5, "and nothing else: {red:?}");
    }
}

/// A baked mesh's faces keep their colours too.
#[test]
fn a_points_colour_multiplies_a_face() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map(1);
    let mut centre_with = |colors: Vec<[u8; 4]>| {
        built.point_colors = colors;
        renderer.load_map(Arc::new(built.clone())).expect("load");
        let capture = renderer
            .capture(&view_from_above(), Drawables::default(), 64, 64)
            .expect("capture");
        centre(&capture)
    };
    let plain = centre_with(Vec::new());
    assert_eq!(
        centre_with(vec![[255; 4]; 4]),
        plain,
        "white changes nothing"
    );
    // The floor's red is past what a screen shows, and that holds its green down, so with no red
    // its green can be higher than it was.
    let green = centre_with(vec![[0, 255, 0, 255]; 4]);
    assert!(green[1] > 100 && green[0] < 5 && green[2] < 5, "{green:?}");
}

/// A texture of glass, half see-through, in `color`.
fn glass(color: [u8; 3]) -> Texture {
    let [red, green, blue] = color;
    let mut texture = map(1).textures[0].clone();
    texture.mips = image_of(2, |_, _| [red, green, blue, 128]);
    texture.material = Some(Arc::new(Material {
        alpha: AlphaMode::Blend,
        shading: Shading::Unlit,
        ..Material::default()
    }));
    texture
}

/// A square over the floor, of `texture`, as a moving mesh.
fn square_of(texture: u32) -> MovingMesh {
    let corner = |x: f32, y: f32| MeshCorner {
        position: [x, y, 0.0],
        uv: [0.0, 0.0],
        normal: [0, 0, 127],
    };
    MovingMesh {
        name: "square".to_owned(),
        mesh: SharedMesh {
            corners: vec![
                corner(48.0, 48.0),
                corner(80.0, 48.0),
                corner(80.0, 80.0),
                corner(48.0, 80.0),
            ],
            joints: vec![[0, 0, 0, 0]; 4],
            weights: vec![[255, 0, 0, 0]; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
            levels: vec![vec![SharedPart {
                texture,
                first: 0,
                count: 6,
            }]],
            ..SharedMesh::default()
        },
        skeleton: Skeleton::default(),
    }
}

/// `mesh` at height `z`.
fn square_at(mesh: u32, z: f32) -> MovingDraw {
    let mut rows = NO_MOVE;
    rows[2][3] = z;
    MovingDraw {
        mesh,
        rows,
        joint_count: 1,
        center: Vec3::new(64.0, 64.0, z),
        radius: 30.0,
        ..MovingDraw::default()
    }
}

/// A red square and a green one, as moving meshes of glass.
fn map_of_glass() -> Map {
    let mut map = map(1);
    map.textures.push(glass([255, 0, 0]));
    map.textures.push(glass([0, 255, 0]));
    map.moving_meshes = vec![square_of(1), square_of(2)];
    map
}

/// What shows through `draws`, in the order given.
fn through_glass(renderer: &mut Renderer, draws: &[MovingDraw]) -> [u8; 4] {
    let palette = [NO_MOVE];
    let drawables = Drawables {
        moving: draws,
        palette: &palette,
        ..Drawables::default()
    };
    let capture = renderer
        .capture(&view_from_above(), drawables, 64, 64)
        .expect("capture");
    centre(&capture)
}

/// What's see-through in a mesh draws.
#[test]
fn a_see_through_mesh_draws() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map_of_glass())).expect("load");
    let bare = through_glass(&mut renderer, &[]);
    let red = through_glass(&mut renderer, &[square_at(0, 30.0)]);
    assert_ne!(red, bare, "it shows");
}

/// See-through meshes draw far to near, whatever order they're given in, so the nearer is over.
#[test]
fn see_through_meshes_draw_far_to_near() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map_of_glass())).expect("load");
    let (red, green) = (square_at(0, 40.0), square_at(1, 20.0));
    let one_way = through_glass(&mut renderer, &[red, green]);
    let other_way = through_glass(&mut renderer, &[green, red]);
    assert_eq!(one_way, other_way, "the order given doesn't matter");
    let swapped = [square_at(0, 20.0), square_at(1, 40.0)];
    let turned_round = through_glass(&mut renderer, &swapped);
    // The floor is so red that red always leads, so this goes by how much green there is.
    assert!(
        turned_round[1] > one_way[1],
        "{one_way:?} and {turned_round:?}"
    );
}

/// A see-through copy draws as well, and each by itself, far to near with what moves.
#[test]
fn see_through_copies_sort_with_what_moves() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map_of_glass();
    let mut rows = NO_MOVE;
    rows[2][3] = 40.0;
    let copy = square_of(1).mesh;
    built.shared_meshes.push(SharedMesh {
        center: Vec3::new(64.0, 64.0, 0.0),
        radius: 30.0,
        level_sizes: Vec::new(),
        ..copy
    });
    // A face for it to be found by, which it stands in for.
    let stand_in = built.faces[0].clone();
    built.faces.push(stand_in);
    built.tree.leaf_faces = vec![0, 1];
    built.tree.leaves[1].face_count = 2;
    built.models[0].face_count = 2;
    built.mesh_instances.push(MeshInstance {
        mesh: 0,
        rows,
        first_face: 1,
        face_count: 1,
    });
    renderer.load_map(Arc::new(built)).expect("load");
    let below = through_glass(&mut renderer, &[square_at(1, 20.0)]);
    let above = through_glass(&mut renderer, &[square_at(1, 50.0)]);
    assert!(above[1] > below[1] + 20, "{below:?} and {above:?}");
}

/// A map with rough land under a view from high up, where its parts are at different levels. Its
/// one face is a stand-in, as the land's own chunk.
fn map_with_land() -> Map {
    let mut built = map(1);
    // With no light in it, what's land is plainly land, and what's dark isn't.
    built.textures[0].material = Some(Arc::new(Material {
        shading: Shading::Unlit,
        ..Material::default()
    }));
    let mut data = TerrainData::flat([2, 2], 32.0);
    let across = data.across();
    for (at, height) in (0_u32..).zip(&mut data.heights) {
        let (x, y) = (at % across, at / across);
        *height = ((x as f32 * 0.7).sin() + (y as f32 * 0.9).cos()) * 60.0;
    }
    let first_face = u32::try_from(built.faces.len()).expect("few");
    let stand_in = built.faces[0].clone();
    built.faces.extend(vec![stand_in; 4]);
    built.tree.leaf_faces = (0..5).collect();
    built.tree.leaves[1].face_count = 5;
    built.models[0].face_count = 5;
    built.models[0].first_face = 0;
    built.terrain.push(TerrainSheet {
        data,
        origin: Vec3::new(-2048.0, -2048.0, 0.0),
        first_face,
        ..TerrainSheet::default()
    });
    built
}

/// Land draws, and where its parts of different levels meet there are no gaps to see through. A
/// gap is an upright slit, so this looks down into them at a slant, across the distance where one
/// level meets the next. Land's white, and anything else in the picture is showing through.
#[test]
fn land_has_no_cracks_between_its_levels() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map_with_land())).expect("load");
    let view = View {
        origin: Vec3::new(-1700.0, -1700.0, 900.0),
        yaw: 45.0,
        pitch: 25.0,
        fov: 40.0,
        ..view_from_above()
    };
    let capture = renderer
        .capture(&view, Drawables::default(), 1024, 1024)
        .expect("capture");
    // The middle of the picture, as its sides can see past the land, and below its skyline.
    let land = |x: u32, y: u32| pixel(&capture, x, y)[0] > 128;
    let mut holes = 0;
    for x in 256..768 {
        let top = (0..1024).find(|y| land(x, *y)).expect("it shows");
        holes += (top..1024).filter(|y| !land(x, *y)).count();
    }
    assert_eq!(holes, 0, "{holes} pixels show through");
}

/// What shows straight down onto the middle of `built`, from high up.
fn land_from_above(built: Map, live: &[LiveLight], shadows: bool) -> Option<[u8; 4]> {
    let mut renderer = renderer()?;
    renderer.load_map(Arc::new(built)).expect("load");
    renderer.set_post(PostSettings {
        live_shadows: shadows,
        ..PostSettings::default()
    });
    let view = View {
        origin: Vec3::new(0.0, 0.0, 400.0),
        ..view_from_above()
    };
    let drawables = Drawables {
        live_lights: live,
        ..Drawables::default()
    };
    let capture = renderer.capture(&view, drawables, 64, 64).expect("capture");
    Some(centre(&capture))
}

/// A hole in the land shows what's under it, in the picture as in collision.
#[test]
fn a_hole_in_land_shows_through() {
    let Some(whole) = land_from_above(map_with_land(), &[], false) else {
        return;
    };
    let mut built = map_with_land();
    let sheet = &mut built.terrain[0];
    for y in 60..68 {
        for x in 60..68 {
            sheet.data.set_hole(x, y, true);
        }
    }
    let holed = land_from_above(built, &[], false).expect("it drew");
    assert!(whole[0] > 200, "land shows: {whole:?}");
    assert!(holed[0] < 50, "and through it, nothing: {holed:?}");
}

/// Land's found through the leaves it's in, so what can't be seen from here isn't drawn.
#[test]
fn land_in_no_leaf_that_can_be_seen_is_not_drawn() {
    let mut built = map_with_land();
    built.tree.leaf_faces = vec![0];
    built.tree.leaves[1].face_count = 1;
    let Some(hidden) = land_from_above(built, &[], false) else {
        return;
    };
    assert!(hidden[0] < 50, "{hidden:?}");
}

/// A sheet high up shades what's under it from a moving light.
#[test]
fn land_casts_a_live_shadow() {
    // Lit land, as the other tests' is unlit, under a roof that a lamp is above.
    let lit = |roof: bool, hole: bool| {
        let mut built = map_with_land();
        built.textures[0].material = Some(Arc::new(Material::default()));
        if roof {
            let mut data = TerrainData::flat([2, 2], 32.0);
            if hole {
                for y in 56..72 {
                    for x in 56..72 {
                        data.set_hole(x, y, true);
                    }
                }
            }
            let first_face = u32::try_from(built.faces.len()).expect("few");
            let stand_in = built.faces[0].clone();
            built.faces.extend(vec![stand_in; 4]);
            built.tree.leaf_faces = (0..9).collect();
            built.tree.leaves[1].face_count = 9;
            built.models[0].face_count = 9;
            built.terrain.push(TerrainSheet {
                data,
                origin: Vec3::new(-2048.0, -2048.0, 300.0),
                first_face,
                ..TerrainSheet::default()
            });
        }
        let lamp = [LiveLight {
            origin: Vec3::new(0.0, 0.0, 330.0),
            radius: 800.0,
            color: [0.0, 0.0, 0.8],
            shadows: true,
            ..LiveLight::default()
        }];
        let view = View {
            origin: Vec3::new(0.0, 0.0, 200.0),
            ..view_from_above()
        };
        let mut renderer = renderer()?;
        renderer.load_map(Arc::new(built)).expect("load");
        renderer.set_post(PostSettings {
            live_shadows: true,
            ..PostSettings::default()
        });
        let drawables = Drawables {
            live_lights: &lamp,
            ..Drawables::default()
        };
        let capture = renderer.capture(&view, drawables, 64, 64).expect("capture");
        Some(centre(&capture)[2])
    };
    let Some(open) = lit(false, false) else {
        return;
    };
    let under = lit(true, false).expect("it drew");
    let through_a_hole = lit(true, true).expect("it drew");
    assert!(under + 30 < open, "the roof shades it: {under} of {open}");
    assert!(
        through_a_hole + 5 > open,
        "unless there's a hole: {through_a_hole} of {open}"
    );
}

/// What the two kinds of model cost on the GPU, drawn and moved. It says nothing is wrong by passing:
/// it's here to be run by hand, with `cargo test --release -p engine_render --test gpu speed_of
/// -- --ignored --nocapture`, and `COPIES` and `CORNERS` to change what's drawn.
#[test]
#[ignore = "prints timings, for reading"]
#[expect(clippy::print_stdout, reason = "the timings are what it's for")]
fn speed_of_the_two_paths() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let number_from = |name: &str, fallback: usize| {
        std::env::var(name)
            .ok()
            .and_then(|text| text.parse().ok())
            .unwrap_or(fallback)
    };
    let copies = number_from("COPIES", 200);
    let corners = number_from("CORNERS", 900);
    let count = u32::try_from(corners).expect("few");
    let corner = |at: usize| MeshCorner {
        position: [(at % 30) as f32 * 0.5, (at / 30) as f32 * 0.5, 0.0],
        uv: [0.0, 0.0],
        normal: [0, 0, 127],
    };
    let mut map = map_with_a_moving_square(false);
    map.moving_meshes[0].mesh = SharedMesh {
        joints: vec![[0, 1, 0, 0]; corners],
        weights: vec![[128, 127, 0, 0]; corners],
        indices: (0..count).collect(),
        levels: vec![vec![SharedPart {
            texture: 0,
            first: 0,
            count,
        }]],
        corners: (0..corners).map(corner).collect(),
        ..SharedMesh::default()
    };
    renderer.load_map(Arc::new(map)).expect("load");
    let mut model = triangle();
    let vertex = |at: usize| AliasVertex {
        position: [
            u8::try_from(at % 30).expect("small"),
            u8::try_from(at / 30 % 256).expect("small"),
            0,
        ],
        normal: [127, 0, 0],
    };
    let pose: Vec<AliasVertex> = (0..corners).map(vertex).collect();
    model.poses = vec![pose.clone(), pose];
    model.corners = (0..count)
        .map(|vertex| AliasCorner {
            vertex,
            uv: [0.5, 0.5],
        })
        .collect();
    model.frames[0].pose_count = 2;
    let number = renderer.add_model(Arc::new(model)).expect("add");
    let aliases: Vec<EntityDraw> = (0..copies)
        .map(|_| EntityDraw {
            pose: 0,
            next_pose: 1,
            blend: 0.5,
            ..draw_of(number)
        })
        .collect();
    let palette: Vec<Rows> = vec![NO_MOVE; copies * 2];
    let draws: Vec<MovingDraw> = (0..copies)
        .map(|at| MovingDraw {
            rows: NO_MOVE,
            first_joint: u32::try_from(at * 2).expect("few"),
            joint_count: 2,
            center: Vec3::new(64.0, 64.0, 32.0),
            radius: 200.0,
            ..MovingDraw::default()
        })
        .collect();
    assert!(renderer.time_gpu(true), "no clock");
    let mut run = |drawables: Drawables<'_>| {
        for _ in 0..200 {
            let _shot = renderer
                .capture(&view_from_above(), drawables, 64, 64)
                .expect("capture");
        }
        renderer.take_gpu_times()
    };
    let _warm = run(Drawables {
        moving: &draws,
        palette: &palette,
        ..Drawables::default()
    });
    let alias = run(with_entity(&aliases));
    let moving = run(Drawables {
        moving: &draws,
        palette: &palette,
        ..Drawables::default()
    });
    println!(
        "SPEED {copies} x {corners}: alias {:.4} + {:.4} / moving {:.4} + {:.4}, scene {:.4} / {:.4}",
        alias.models, alias.deform, moving.moving, moving.deform, alias.scene, moving.scene,
    );
}
