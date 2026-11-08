//! Drawing with a real device. Each test skips itself if there's no Vulkan.

mod common;

use std::sync::Arc;

use common::map;
use engine_core::canvas::{Image, Quad};
use engine_core::entity::ModelInstance;
use engine_core::math::Vec3;
use engine_map::{
    AliasCorner, AliasFrame, AliasModel, AliasVertex, AlphaMode, Clouds, Contents, EffectDef,
    FieldDef, Fog, FogVolume, LandPicture, LightTiles, Map, Material, MaterialLayer, MeshCorner,
    MeshInstance, MetalRough, MipLevel, MovingMesh, NO_MOVE, PAINT_MASK, ParticleEmitter,
    ParticleForce, PixelForm, Pool, PoolLook, ProbeGrid, RESIDENT, RayStencil, Rows,
    SUN_BOUNCE_STYLE, SUN_STYLE, ShaderCode, Shading, SharedMesh, SharedMorph, SharedPart,
    SideFiles, Skeleton, Sky, SkyRays, Sprite, SpriteFacing, SpriteFrame, SpriteImage,
    SurfacePoint, TerrainData, TerrainLayer, TerrainSheet, Texture, TextureKind, TileLight,
    ToneCurve, WaterOverlay, WaterPush, WaterSheet, mip_chain, normal_chain,
};
use engine_render::{
    BatchKind, CapsuleDraw, Capture, Drawables, EntityDraw, GpuParticles, LightDraw, LiveLight,
    MovingDraw, ParticleColor, ParticleDraw, ParticleShape, ParticleSim, PostSettings, Renderer,
    ScreenOutput, SimForce, SkyRayFloor, SpriteDraw, SunDraw, Target, TextureFilter, TileStreamer,
    View, build_world,
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
    stencil_centre_reaching(built, masks, &[], rays)
}

/// How bright the middle of a picture of `built` is, with light stopped by `reaches`.
fn stencil_centre_reaching(
    built: Map,
    masks: &[[f32; 256]],
    reaches: &[[f32; 256]],
    rays: u8,
) -> Option<[u8; 4]> {
    let mut renderer = renderer()?;
    renderer.load_map(Arc::new(built)).expect("load");
    renderer.set_post(PostSettings {
        rays,
        ..PostSettings::default()
    });
    let drawables = Drawables {
        stencil_masks: masks,
        stencil_reaches: reaches,
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

/// A beam stops where something's in its way, and the air past that has none of its light.
#[test]
fn a_floor_stops_a_beam() {
    let plain = stencil_centre(stencil_map(BLUE), &[], 0);
    let full = stencil_centre_reaching(stencil_map(BLUE), &[], &[[512.0; 256]], 3);
    // The view looks down from 64 under the stencil, so it's all past this.
    let stopped = stencil_centre_reaching(stencil_map(BLUE), &[], &[[32.0; 256]], 3);
    if let (Some(plain), Some(full), Some(stopped)) = (plain, full, stopped) {
        assert!(full[2] > plain[2] + 20, "{plain:?} {full:?}");
        assert_eq!(plain, stopped);
    }
}

/// A beam stopped short of where the view is still shows, for what's before that.
#[test]
fn a_beam_shows_up_to_where_it_stops() {
    let full = stencil_centre_reaching(stencil_map(BLUE), &[], &[[512.0; 256]], 3);
    let low = stencil_centre_reaching(stencil_map(BLUE), &[], &[[120.0; 256]], 3);
    let plain = stencil_centre(stencil_map(BLUE), &[], 0);
    if let (Some(full), Some(low), Some(plain)) = (full, low, plain) {
        assert!(
            low[2] > plain[2] && low[2] < full[2],
            "{plain:?} {low:?} {full:?}"
        );
    }
}

/// Each stencil has its own reach, and one's doesn't stop another's.
#[test]
fn a_beam_stops_by_its_own_stencil() {
    let two = || {
        let mut built = stencil_map(BLUE);
        let other = RayStencil {
            corner: Vec3::new(4000.0, 0.0, 128.0),
            ..built.stencils[0].clone()
        };
        built.stencils.insert(0, other);
        built
    };
    let plain = stencil_centre(two(), &[], 0);
    let shown = stencil_centre_reaching(two(), &[], &[[32.0; 256], [512.0; 256]], 3);
    let stopped = stencil_centre_reaching(two(), &[], &[[512.0; 256], [32.0; 256]], 3);
    if let (Some(plain), Some(shown), Some(stopped)) = (plain, shown, stopped) {
        assert!(shown[2] > plain[2] + 20, "{plain:?} {shown:?}");
        assert_eq!(stopped, plain);
    }
}

/// Each part of a stencil has its own reach: the view's under the left half.
#[test]
fn a_beam_stops_by_the_cell_it_goes_through() {
    let half = |left: f32, right: f32| -> [f32; 256] {
        std::array::from_fn(|cell| if cell % 16 < 8 { left } else { right })
    };
    let plain = stencil_centre(stencil_map(BLUE), &[], 0);
    let under = stencil_centre_reaching(stencil_map(BLUE), &[], &[half(32.0, 512.0)], 3);
    let beside = stencil_centre_reaching(stencil_map(BLUE), &[], &[half(512.0, 32.0)], 3);
    if let (Some(plain), Some(under), Some(beside)) = (plain, under, beside) {
        assert_eq!(under, plain);
        assert!(beside[2] > plain[2] + 20, "{plain:?} {beside:?}");
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
            deep: 0.0,
            view_effect: 0,
            overlays: [WaterOverlay::NONE; 2],
            foam: 0.0,
            foam_look: WaterOverlay::NONE,
            shore: 0.0,
            splash: 0,
            spray: 0,
            splash_sound: 0,
            trail: 0,
            trail_size: 28.0,
            rain_rings: 0.0,
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

/// A pool that thickens with depth hides what's under it faster.
#[test]
fn water_can_thicken_with_depth() {
    let look = PoolLook {
        color: [0.0, 0.0, 1.0],
        absorption: 0.02,
        caustics: 0.0,
        reflect: 0.0,
        waves: 0.0,
        refraction: 0.0,
        tint: [0.0; 4],
        deep: 0.0,
        view_effect: 0,
        overlays: [WaterOverlay::NONE; 2],
        foam: 0.0,
        foam_look: WaterOverlay::NONE,
        shore: 0.0,
        splash: 0,
        spray: 0,
        splash_sound: 0,
        trail: 0,
        trail_size: 28.0,
        rain_rings: 0.0,
    };
    let even = post_centre(pool_map(look), PostSettings::default());
    let deep = post_centre(
        pool_map(PoolLook { deep: 0.2, ..look }),
        PostSettings::default(),
    );
    let (Some(even), Some(deep)) = (even, deep) else {
        return;
    };
    assert!(deep[2] > even[2] + 20, "{deep:?} and {even:?}");
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
        deep: 0.0,
        view_effect: 0,
        overlays: [WaterOverlay::NONE; 2],
        foam: 0.0,
        foam_look: WaterOverlay::NONE,
        shore: 0.0,
        splash: 0,
        spray: 0,
        splash_sound: 0,
        trail: 0,
        trail_size: 28.0,
        rain_rings: 0.0,
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
        sun_shares: Vec::new(),
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

/// Land below a floor, with its light out in tile 0 and the floor's kept near.
fn tiled_land(average: [u8; 3]) -> (Map, TileLight) {
    let mut built = map_with_land();
    built.textures[0].material = None;
    built.terrain[0].data.heights.fill(-100.0);
    let tile = TileLight {
        light: built.light.clone(),
        ..TileLight::default()
    };
    for face in built.faces.iter_mut().skip(1) {
        let light = face.light.as_mut().expect("lit");
        light.tile = 0;
        light.average = average;
    }
    built.tile_size = 1024.0;
    built.light_tiles = Some(LightTiles::new(1024.0, vec![[0, 0]]));
    (built, tile)
}

/// Land's light comes and goes with its tile, as a face's does, and it leaves the other faces as
/// they were: a chunk has no vertices of its own to write.
#[test]
fn land_shows_its_tiles_light() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let (built, tile) = tiled_land([0, 10, 240]);
    renderer.load_map(Arc::new(built)).expect("load");
    let view = View {
        origin: Vec3::new(64.0, 64.0, 300.0),
        ..view_from_above()
    };
    let shot = |renderer: &mut Renderer| {
        renderer
            .capture(&view, Drawables::default(), 64, 64)
            .expect("capture")
    };
    let far = shot(&mut renderer);
    let (floor, land) = ((32, 32), (4, 4));
    let land_far = pixel_at(&far, land.0, land.1);
    assert!(land_far[2] > land_far[0] + 100, "the average: {land_far:?}");
    assert!(renderer.show_tile_light(0, &tile).expect("shows"));
    let near = shot(&mut renderer);
    let land_near = pixel_at(&near, land.0, land.1);
    assert!(land_near[0] > land_near[2] + 50, "its own: {land_near:?}");
    assert_eq!(
        pixel_at(&near, floor.0, floor.1),
        pixel_at(&far, floor.0, floor.1),
        "the floor's as it was"
    );
    renderer.hide_tile_light(0).expect("hides");
    assert_eq!(shot(&mut renderer).rgba, far.rgba, "and back");
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

/// A plain picture of one colour, for a layer of cloud.
fn cloud_picture(value: u8) -> Arc<Vec<MipLevel>> {
    Arc::new(vec![MipLevel {
        width: 2,
        height: 2,
        rgba: vec![value; 16],
        form: PixelForm::Rgba,
    }])
}

/// A layer of cloud that's a solid colour wherever it's over the view, from its picture.
fn solid_cloud(color: [f32; 3]) -> Clouds {
    Clouds {
        color,
        coverage: 1.0,
        speed: [0.0, 0.0],
        image: Some(cloud_picture(255)),
        ..Clouds::default()
    }
}

/// What's straight up, in a sky with `clouds`, from `origin`.
fn sky_overhead(clouds: Vec<Clouds>, origin: Vec3, pitch: f32) -> Option<[u8; 4]> {
    let mut renderer = renderer()?;
    let mut built = map(1);
    built.sky = Some(Sky {
        zenith: [0.0, 0.0, 0.0],
        horizon: [0.0, 0.0, 0.0],
        sun_size: 0.0,
        sun_color: [0.0, 0.0, 0.0],
        clouds,
        ..Sky::default()
    });
    renderer.load_map(Arc::new(built)).expect("load");
    let view = View {
        origin,
        pitch,
        ..view_from_above()
    };
    let capture = renderer
        .capture(&view, Drawables::default(), 32, 32)
        .expect("capture");
    Some(centre(&capture))
}

/// Where there's nothing else, you see the sky go up.
const UNDER: Vec3 = Vec3::new(0.0, 0.0, -2000.0);

/// The lowest layer is in front of the others.
#[test]
fn the_lowest_clouds_are_in_front() {
    let red = solid_cloud([1.0, 0.0, 0.0]);
    let green = solid_cloud([0.0, 1.0, 0.0]);
    if let Some(shown) = sky_overhead(vec![red.clone(), green.clone()], UNDER, -89.0) {
        assert!(shown[0] > 200 && shown[1] < 20, "{shown:?}");
    }
    if let Some(shown) = sky_overhead(vec![green, red], UNDER, -89.0) {
        assert!(shown[1] > 200 && shown[0] < 20, "{shown:?}");
    }
}

/// A thin layer shows the one above it.
#[test]
fn clouds_show_through_gaps_in_each_other() {
    let gap = Clouds {
        image: Some(cloud_picture(0)),
        ..solid_cloud([1.0, 0.0, 0.0])
    };
    let green = solid_cloud([0.0, 1.0, 0.0]);
    if let Some(shown) = sky_overhead(vec![gap, green], UNDER, -89.0) {
        assert!(shown[1] > 200 && shown[0] < 20, "{shown:?}");
    }
}

/// Each layer reads its own picture, however many there are.
#[test]
fn each_layer_has_its_own_picture() {
    for layer in 0..4 {
        let mut layers = vec![
            Clouds {
                image: Some(cloud_picture(0)),
                ..solid_cloud([1.0, 0.0, 0.0])
            };
            4
        ];
        layers[layer].image = Some(cloud_picture(255));
        if let Some(shown) = sky_overhead(layers, UNDER, -89.0) {
            assert!(shown[0] > 200, "layer {layer}: {shown:?}");
        }
    }
}

/// A layer with no picture among those with one doesn't take up one of theirs.
#[test]
fn a_made_up_layer_leaves_the_pictures_in_order() {
    let none = Clouds {
        coverage: 0.0,
        image: None,
        ..solid_cloud([0.0, 0.0, 1.0])
    };
    // A picture with nothing in it, where the slot after it has something.
    let gap = Clouds {
        image: Some(cloud_picture(0)),
        ..solid_cloud([1.0, 0.0, 0.0])
    };
    if let Some(shown) = sky_overhead(vec![none, gap], UNDER, -89.0) {
        assert!(shown[0] < 20, "{shown:?}");
    }
}

/// A picture with half of it on either side, to see where a layer's looked at.
fn half_cloud_picture() -> Arc<Vec<MipLevel>> {
    let mut rgba = Vec::new();
    for _ in 0..64 {
        for x in 0..64 {
            rgba.extend_from_slice(if x < 32 { &[255; 4] } else { &[0; 4] });
        }
    }
    Arc::new(vec![MipLevel {
        width: 64,
        height: 64,
        rgba,
        form: PixelForm::Rgba,
    }])
}

/// Clouds with a height go by as the view moves, and ones with none stay with it.
#[test]
fn high_clouds_go_by_the_view() {
    let cloud = |height| Clouds {
        height,
        image: Some(half_cloud_picture()),
        ..solid_cloud([1.0, 0.0, 0.0])
    };
    let shown = |height, x| sky_overhead(vec![cloud(height)], Vec3::new(x, 0.0, -2000.0), -89.9);
    let (Some(here), Some(there)) = (shown(1000.0, 250.0), shown(1000.0, 750.0)) else {
        return;
    };
    assert_ne!(here[0] > 128, there[0] > 128, "{here:?} {there:?}");
    let (Some(here), Some(there)) = (shown(0.0, 250.0), shown(0.0, 750.0)) else {
        return;
    };
    assert_eq!(here, there);
}

/// A flat plane goes on for ever, and a dome comes round, so low down they're far apart.
#[test]
fn a_dome_keeps_clouds_big_low_down() {
    let cloud = |curve| Clouds {
        curve,
        scale: 0.15,
        image: Some(half_cloud_picture()),
        ..solid_cloud([1.0, 0.0, 0.0])
    };
    // Looking 12 degrees up, a dome is a sixth of a repeat away, and a plane is well past half.
    let look = |curve| sky_overhead(vec![cloud(curve)], UNDER, -12.0);
    if let (Some(flat), Some(dome)) = (look(0.0), look(1.0)) {
        assert!(flat[0] < 40 && dome[0] > 40, "{flat:?} {dome:?}");
    }
}

/// Lit clouds take the sun's colour, and others keep their own.
#[test]
fn lit_clouds_take_the_sun() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let with_lit = |lit| Sky {
        zenith: [0.0, 0.0, 0.0],
        horizon: [0.0, 0.0, 0.0],
        sun_color: [1.0, 0.0, 0.0],
        sun_size: 0.0,
        sun_direction: Vec3::new(1.0, 0.0, 0.0),
        clouds: vec![Clouds {
            lit,
            ..solid_cloud([1.0, 1.0, 1.0])
        }],
        ..Sky::default()
    };
    let view = View {
        origin: UNDER,
        pitch: -89.0,
        ..view_from_above()
    };
    let mut shot = |lit| {
        let mut built = map(1);
        built.sky = Some(with_lit(lit));
        renderer.load_map(Arc::new(built)).expect("load");
        let capture = renderer
            .capture(&view, Drawables::default(), 32, 32)
            .expect("capture");
        centre(&capture)
    };
    let plain = shot(0.0);
    let lit = shot(1.0);
    assert!(plain[1] > 200, "{plain:?}");
    assert!(lit[0] > 80 && lit[1] < 20, "{lit:?}");
    assert!(
        lit[0] < plain[0],
        "thick cloud is darker: {lit:?} {plain:?}"
    );
}

/// Thin cloud lights up near the sun, if it's lit at all.
#[test]
fn only_lit_clouds_shine_round_the_sun() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let view = View {
        origin: UNDER,
        pitch: -80.0,
        ..view_from_above()
    };
    let mut shot = |clouds: Vec<Clouds>| {
        let mut built = map(1);
        built.sky = Some(Sky {
            zenith: [0.0, 0.0, 0.0],
            horizon: [0.0, 0.0, 0.0],
            sun_color: [1.0, 0.0, 0.0],
            sun_size: 0.0,
            sun_direction: Vec3::new(0.0, 0.0, 1.0),
            clouds,
            ..Sky::default()
        });
        renderer.load_map(Arc::new(built)).expect("load");
        let capture = renderer
            .capture(&view, Drawables::default(), 32, 32)
            .expect("capture");
        centre(&capture)
    };
    // About half as thick as it can be, and black, so that all it shows is what shines.
    let thin = |lit| Clouds {
        lit,
        image: Some(cloud_picture(59)),
        ..solid_cloud([0.0, 0.0, 0.0])
    };
    let plain = shot(vec![thin(0.0)]);
    let lit = shot(vec![thin(1.0)]);
    assert!(lit[0] > plain[0] + 20, "{lit:?} {plain:?}");
}

/// What a sky's rays are looked at under.
#[derive(Clone)]
struct RayShot {
    /// The rays.
    rays: Option<SkyRays>,
    /// The sky's clouds.
    clouds: Vec<Clouds>,
    /// Towards the sun.
    sun: Vec3,
    /// How low the sun's light gets.
    floor: Option<SkyRayFloor>,
    /// How much rays are drawn.
    quality: u8,
}

impl Default for RayShot {
    fn default() -> Self {
        Self {
            rays: None,
            clouds: Vec::new(),
            sun: Vec3::new(0.6, 0.0, 0.8),
            floor: None,
            quality: 3,
        }
    }
}

/// What the middle of a picture from `view` is, under `shot`.
fn ray_picture(shot: RayShot, view: View) -> Option<[u8; 4]> {
    let mut renderer = renderer()?;
    let mut built = map(1);
    built.sky = Some(Sky {
        zenith: [0.0, 0.0, 0.0],
        horizon: [0.0, 0.0, 0.0],
        sun_size: 0.0,
        sun_direction: shot.sun,
        sun_color: [1.0, 1.0, 1.0],
        clouds: shot.clouds,
        rays: shot.rays,
        ..Sky::default()
    });
    renderer.load_map(Arc::new(built)).expect("load");
    renderer.set_post(PostSettings {
        rays: shot.quality,
        ..PostSettings::default()
    });
    let drawables = Drawables {
        sky_ray_floor: shot.floor.as_ref(),
        ..Drawables::default()
    };
    let capture = renderer.capture(&view, drawables, 64, 64).expect("capture");
    Some(centre(&capture))
}

/// Looking up, with only sky to see.
fn looking_up() -> View {
    View {
        origin: UNDER,
        pitch: -89.0,
        ..view_from_above()
    }
}

/// Rays by a picture of `value` all over.
fn rays_by(value: u8) -> SkyRays {
    SkyRays {
        picture: Some(cloud_picture(value)),
        ..SkyRays::default()
    }
}

/// Rays by the clouds.
fn rays_by_clouds() -> SkyRays {
    SkyRays::default()
}

/// Rays show in the sky, as far as their picture lets them.
#[test]
fn a_picture_lets_rays_down() {
    let plain = ray_picture(RayShot::default(), looking_up());
    let open = RayShot {
        rays: Some(rays_by(255)),
        ..RayShot::default()
    };
    let shut = RayShot {
        rays: Some(rays_by(0)),
        ..RayShot::default()
    };
    let (open, shut) = (
        ray_picture(open, looking_up()),
        ray_picture(shut, looking_up()),
    );
    if let (Some(plain), Some(open), Some(shut)) = (plain, open, shut) {
        assert!(open[0] > plain[0] + 40, "{plain:?} {open:?}");
        assert_eq!(shut, plain);
    }
}

/// How bright they are is the author's.
#[test]
fn rays_are_as_strong_as_asked() {
    let at = |strength| RayShot {
        rays: Some(SkyRays {
            strength,
            ..rays_by(255)
        }),
        ..RayShot::default()
    };
    let (weak, strong) = (
        ray_picture(at(0.25), looking_up()),
        ray_picture(at(1.0), looking_up()),
    );
    if let (Some(weak), Some(strong)) = (weak, strong) {
        assert!(strong[0] > weak[0] + 20, "{weak:?} {strong:?}");
    }
}

/// Rays by the clouds come through their gaps, and not through them.
#[test]
fn rays_come_through_gaps_in_clouds() {
    let black = |image: u8| Clouds {
        coverage: 1.0,
        image: Some(cloud_picture(image)),
        ..solid_cloud([0.0, 0.0, 0.0])
    };
    let through = |image| RayShot {
        rays: Some(rays_by_clouds()),
        clouds: vec![black(image)],
        ..RayShot::default()
    };
    let (gap, solid) = (
        ray_picture(through(0), looking_up()),
        ray_picture(through(255), looking_up()),
    );
    if let (Some(gap), Some(solid)) = (gap, solid) {
        assert!(gap[0] > solid[0] + 40, "{gap:?} {solid:?}");
    }
}

/// Every layer has its say: any that's solid stops them, whichever it is.
#[test]
fn any_layer_of_cloud_stops_rays() {
    let black = |image: u8| Clouds {
        coverage: 1.0,
        image: Some(cloud_picture(image)),
        ..solid_cloud([0.0, 0.0, 0.0])
    };
    for layer in 0..3 {
        let mut clouds = vec![black(0); 3];
        clouds[layer] = black(255);
        let shot = RayShot {
            rays: Some(rays_by_clouds()),
            clouds,
            ..RayShot::default()
        };
        if let Some(shown) = ray_picture(shot, looking_up()) {
            assert!(shown[0] < 20, "layer {layer}: {shown:?}");
        }
    }
}

/// Looking down on the square, towards a low sun, and well clear of the horizon, as rays are
/// worked out in big pixels, and bright sky bleeds a little way over what's in front of it.
fn down_on_the_square() -> View {
    View {
        origin: Vec3::new(-140.0, 64.0, 120.0),
        pitch: 30.0,
        ..view_from_above()
    }
}

/// A sun low down, ahead of `down_on_the_square`.
const LOW_SUN: Vec3 = Vec3::new(0.98, 0.0, 0.2);

/// Rays strong enough to see over the short way to something close.
fn strong_rays(into_level: bool) -> SkyRays {
    SkyRays {
        into_level,
        strength: 200.0,
        ..rays_by(255)
    }
}

/// Unless they're told, rays don't come over what's in the level.
#[test]
fn rays_stay_in_the_sky_unless_told() {
    let plain = RayShot {
        sun: LOW_SUN,
        ..RayShot::default()
    };
    let shot = RayShot {
        rays: Some(strong_rays(false)),
        ..plain.clone()
    };
    let plain = ray_picture(plain, down_on_the_square());
    if let (Some(plain), Some(shown)) = (plain, ray_picture(shot, down_on_the_square())) {
        assert_eq!(plain, shown);
    }
}

/// Told, they do.
#[test]
fn rays_can_come_into_the_level() {
    let plain = RayShot {
        sun: LOW_SUN,
        ..RayShot::default()
    };
    let shot = RayShot {
        rays: Some(strong_rays(true)),
        ..plain.clone()
    };
    let plain = ray_picture(plain, down_on_the_square());
    if let (Some(plain), Some(shown)) = (plain, ray_picture(shot, down_on_the_square())) {
        assert!(
            shown[2] > plain[2].saturating_add(20),
            "{plain:?} {shown:?}"
        );
    }
}

/// The sun flares when it's looked at, by as much as the sky says, and clouds can hide it.
#[test]
fn the_sun_flares_when_looked_at() {
    let shot = |flare, image| RayShot {
        rays: Some(SkyRays {
            flare,
            strength: 0.0,
            ..rays_by(image)
        }),
        // Near it, and not on it, which would be bright with or without.
        sun: Vec3::new(0.3, 0.0, 0.95),
        ..RayShot::default()
    };
    let view = looking_up();
    let (none, full) = (
        ray_picture(shot(0.0, 255), view),
        ray_picture(shot(1.0, 255), view),
    );
    let hidden = ray_picture(shot(1.0, 0), view);
    if let (Some(none), Some(full), Some(hidden)) = (none, full, hidden) {
        assert!(full[0] > none[0] + 40, "{none:?} {full:?}");
        assert_eq!(hidden, none);
    }
}

/// Further from the sun the flare has gone, though rays are still there.
#[test]
fn the_flare_is_tighter_than_the_rays() {
    let shot = |flare| RayShot {
        rays: Some(SkyRays {
            flare,
            ..rays_by(255)
        }),
        sun: Vec3::new(0.7, 0.0, 0.7),
        ..RayShot::default()
    };
    let (none, full) = (
        ray_picture(shot(0.0), looking_up()),
        ray_picture(shot(1.0), looking_up()),
    );
    if let (Some(none), Some(full)) = (none, full) {
        assert!(full[0] < none[0] + 16, "{none:?} {full:?}");
    }
}

/// Rays gather round the sun, and looking away from it there are none.
#[test]
fn rays_gather_round_the_sun() {
    // What they add, as the sky has a glow round the sun of its own.
    let added = |sun: Vec3| {
        let plain = RayShot {
            sun,
            ..RayShot::default()
        };
        let shot = RayShot {
            rays: Some(SkyRays {
                flare: 0.0,
                ..rays_by(255)
            }),
            ..plain.clone()
        };
        let plain = ray_picture(plain, looking_up())?;
        let shown = ray_picture(shot, looking_up())?;
        Some(i32::from(shown[0]) - i32::from(plain[0]))
    };
    let near = added(Vec3::new(0.3, 0.0, 0.95));
    let further = added(Vec3::new(0.8, 0.0, 0.6));
    let away = added(Vec3::new(0.98, 0.0, 0.2));
    if let (Some(near), Some(further), Some(away)) = (near, further, away) {
        assert!(near > further + 10, "{near} {further}");
        assert_eq!(away, 0);
    }
}

/// A floor with each cell's own number for a height.
fn numbered_floor() -> SkyRayFloor {
    #[expect(clippy::cast_precision_loss, reason = "a small grid")]
    let heights = std::array::from_fn(|cell| cell as f32);
    SkyRayFloor {
        area: [0.0, 0.0, 10.0, 0.0],
        heights,
    }
}

/// As the view moves, what's known of each place stays with that place.
#[test]
fn what_the_sun_gets_to_stays_where_it_is() {
    let mut floor = numbered_floor();
    floor.move_to([20.0, 10.0, 10.0, 0.0]);
    // Two across and one up, so the first cell is what was at (2, 1).
    assert!(
        (floor.heights[0] - 18.0).abs() < 0.001,
        "{}",
        floor.heights[0]
    );
    assert!(
        (floor.heights[17] - 35.0).abs() < 0.001,
        "{}",
        floor.heights[17]
    );
    assert!((floor.area[0] - 20.0).abs() < 0.001);
}

/// Going the other way works too.
#[test]
fn what_the_sun_gets_to_goes_back() {
    let mut floor = numbered_floor();
    floor.move_to([-10.0, 0.0, 10.0, 0.0]);
    assert!((floor.heights[1]).abs() < 0.001, "{}", floor.heights[1]);
    assert!(
        (floor.heights[2] - 1.0).abs() < 0.001,
        "{}",
        floor.heights[2]
    );
}

/// What's come into the grid starts as the nearest there was, and not as nothing.
#[test]
fn what_is_new_to_the_grid_takes_the_nearest() {
    let mut floor = numbered_floor();
    floor.move_to([10.0, 0.0, 10.0, 0.0]);
    assert!(
        (floor.heights[15] - 15.0).abs() < 0.001,
        "{}",
        floor.heights[15]
    );
    assert!(
        (floor.heights[14] - 15.0).abs() < 0.001,
        "{}",
        floor.heights[14]
    );
    let mut far = numbered_floor();
    far.move_to([-10000.0, 0.0, 10.0, 0.0]);
    assert!((far.heights[5]).abs() < 0.001, "{}", far.heights[5]);
}

/// With the sun down there's none, even looking right at where it is.
#[test]
fn rays_go_with_the_sun() {
    let sun = Vec3::new(0.6, 0.0, -0.8);
    let view = View {
        origin: UNDER,
        pitch: 53.0,
        ..view_from_above()
    };
    let plain = RayShot {
        sun,
        ..RayShot::default()
    };
    let shot = RayShot {
        rays: Some(rays_by(255)),
        ..plain.clone()
    };
    if let (Some(plain), Some(shown)) = (ray_picture(plain, view), ray_picture(shot, view)) {
        assert_eq!(plain, shown);
    }
}

/// A player who's turned rays off gets none from the sky.
#[test]
fn rays_can_be_turned_off() {
    let plain = ray_picture(RayShot::default(), looking_up());
    let off = RayShot {
        rays: Some(rays_by(255)),
        quality: 0,
        ..RayShot::default()
    };
    if let (Some(plain), Some(off)) = (plain, ray_picture(off, looking_up())) {
        assert_eq!(plain, off);
    }
}

/// The square, lit by a layer of `style`, under `tint`.
fn tinted_square(style: u8, tint: Option<[f32; 3]>) -> Option<[u8; 4]> {
    let mut renderer = renderer()?;
    let mut built = map(1);
    for face in &mut built.faces {
        if let Some(light) = &mut face.light {
            light.styles = [style, 255, 255, 255];
        }
    }
    renderer.load_map(Arc::new(built)).expect("load");
    let drawables = Drawables {
        sun_tint: tint,
        ..Drawables::default()
    };
    let capture = renderer
        .capture(&view_from_above(), drawables, 32, 32)
        .expect("capture");
    Some(centre(&capture))
}

/// A day changes the sun's layers, and what's lit by anything else stays as it is.
#[test]
fn a_tint_changes_the_suns_layers_only() {
    let dark = Some([0.25, 0.25, 0.25]);
    for style in [SUN_STYLE, SUN_BOUNCE_STYLE] {
        let (plain, dimmed) = (tinted_square(style, None), tinted_square(style, dark));
        if let (Some(plain), Some(dimmed)) = (plain, dimmed) {
            assert!(dimmed[0] + 20 < plain[0], "{style}: {plain:?} {dimmed:?}");
        }
    }
    let (plain, dimmed) = (tinted_square(0, None), tinted_square(0, dark));
    assert_eq!(plain, dimmed);
}

/// A tint has a colour, so a sun can go orange and not only dim.
#[test]
fn a_tint_has_a_colour() {
    let (plain, tinted) = (
        tinted_square(SUN_STYLE, None),
        tinted_square(SUN_STYLE, Some([1.0, 0.2, 0.2])),
    );
    if let (Some(plain), Some(tinted)) = (plain, tinted) {
        assert!(tinted[1] + 20 < plain[1], "{plain:?} {tinted:?}");
        assert!(
            tinted[0] >= plain[0].saturating_sub(20),
            "{plain:?} {tinted:?}"
        );
    }
}

/// What a sky of `color` comes out as under `tone`, as the sky's a scene's easiest colour to set.
fn toned_sky(color: [f32; 3], tone: ToneCurve) -> Option<[u8; 4]> {
    let mut renderer = renderer()?;
    let mut built = map(1);
    built.look.tone = tone;
    built.sky = Some(Sky {
        zenith: color,
        horizon: color,
        sun_size: 0.0,
        sun_color: [0.0, 0.0, 0.0],
        ..Sky::default()
    });
    renderer.load_map(Arc::new(built)).expect("load");
    let view = View {
        origin: UNDER,
        pitch: -89.0,
        ..view_from_above()
    };
    let capture = renderer
        .capture(&view, Drawables::default(), 32, 32)
        .expect("capture");
    Some(centre(&capture))
}

/// A neutral curve doesn't touch what a level has, up to where it starts.
#[test]
fn the_neutral_curve_leaves_what_is_not_bright() {
    for color in [[0.1, 0.2, 0.3], [0.5, 0.4, 0.2], [0.7, 0.7, 0.7]] {
        let hold = toned_sky(color, ToneCurve::Hold);
        let neutral = toned_sky(color, ToneCurve::Neutral);
        assert_eq!(hold, neutral, "{color:?}");
    }
}

/// Past that it rolls off, so that brighter is still brighter, where holding stops.
#[test]
fn the_neutral_curve_keeps_what_is_bright_apart() {
    let shot = |value: f32, tone| toned_sky([value, value, value], tone);
    let (bright, brighter) = (shot(1.0, ToneCurve::Neutral), shot(1.5, ToneCurve::Neutral));
    let (held, held_more) = (shot(1.0, ToneCurve::Hold), shot(1.5, ToneCurve::Hold));
    if let (Some(bright), Some(brighter), Some(held), Some(held_more)) =
        (bright, brighter, held, held_more)
    {
        assert_eq!(held, held_more, "it stops at the top");
        assert!(brighter[0] > bright[0], "{bright:?} {brighter:?}");
        assert!(bright[0] < held[0], "{bright:?} {held:?}");
    }
}

/// Very bright colours go towards white, as holding them never does.
#[test]
fn the_neutral_curve_whitens_what_is_very_bright() {
    let color = [4.0, 0.2, 0.2];
    let (neutral, hold) = (
        toned_sky(color, ToneCurve::Neutral),
        toned_sky(color, ToneCurve::Hold),
    );
    if let (Some(neutral), Some(hold)) = (neutral, hold) {
        assert!(neutral[1] > hold[1] + 10, "{neutral:?} {hold:?}");
    }
}

/// A film's curve changes all of it, down to the middle.
#[test]
fn the_film_curve_has_a_look_of_its_own() {
    let color = [0.4, 0.4, 0.4];
    let (film, hold) = (
        toned_sky(color, ToneCurve::Film),
        toned_sky(color, ToneCurve::Hold),
    );
    if let (Some(film), Some(hold)) = (film, hold) {
        assert!(film[0].abs_diff(hold[0]) > 5, "{film:?} {hold:?}");
    }
    let (bright, brighter) = (
        toned_sky([1.0; 3], ToneCurve::Film),
        toned_sky([1.5; 3], ToneCurve::Film),
    );
    if let (Some(bright), Some(brighter)) = (bright, brighter) {
        assert!(brighter[0] > bright[0], "{bright:?} {brighter:?}");
    }
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
        clouds: vec![Clouds {
            speed: [0.0, 0.0],
            ..Clouds::default()
        }],
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

/// What the square looks like from above under `sun`, with `blocker` over it.
fn sunlit(sun: Option<SunDraw>, blocker: bool) -> Option<Capture> {
    let mut renderer = renderer()?;
    renderer.load_map(Arc::new(map(2))).expect("load");
    let over = [ModelInstance {
        model: 1,
        origin: Vec3::new(64.0, 0.0, 30.0),
        visible: true,
        ..ModelInstance::default()
    }];
    let drawables = Drawables {
        instances: if blocker { &over } else { &[] },
        sun,
        ..Drawables::default()
    };
    Some(
        renderer
            .capture(&view_from_above(), drawables, 64, 64)
            .expect("capture"),
    )
}

/// A blue sun straight up, or towards `toward`.
fn blue_sun(toward: Vec3) -> Option<SunDraw> {
    Some(SunDraw {
        toward,
        color: [0.0, 0.0, 1.0],
    })
}

/// How blue a picture is, all told.
fn blueness(capture: &Capture) -> u64 {
    let mut total = 0;
    for y in 0..64 {
        for x in 0..64 {
            total += u64::from(pixel(capture, x, y)[2]);
        }
    }
    total
}

/// A sun that isn't baked lights what faces it, and none when it's down.
#[test]
fn a_live_sun_lights_what_faces_it() {
    let up = Vec3::new(0.0, 0.0, 1.0);
    let (dark, lit) = (sunlit(None, false), sunlit(blue_sun(up), false));
    let down = sunlit(blue_sun(Vec3::new(0.0, 0.0, -1.0)), false);
    if let (Some(dark), Some(lit), Some(down)) = (dark, lit, down) {
        assert!(blueness(&lit) > blueness(&dark) + 64 * 64 * 20);
        assert_eq!(blueness(&down), blueness(&dark));
    }
}

/// Looking up at the square's underside, under `sun`.
fn underside(sun: Option<SunDraw>) -> Option<u64> {
    let mut renderer = renderer()?;
    renderer.load_map(Arc::new(map(2))).expect("load");
    let view = View {
        origin: Vec3::new(64.0, 64.0, -64.0),
        pitch: -89.9,
        ..view_from_above()
    };
    let drawables = Drawables {
        sun,
        ..Drawables::default()
    };
    let capture = renderer.capture(&view, drawables, 64, 64).expect("capture");
    Some(blueness(&capture))
}

/// What faces away from the sun, and anything with the sun under it, isn't lit by it.
#[test]
fn a_live_sun_lights_only_what_faces_it() {
    let dark = underside(None);
    let above = underside(blue_sun(Vec3::new(0.0, 0.0, 1.0)));
    let below = underside(blue_sun(Vec3::new(0.0, 0.0, -1.0)));
    if let (Some(dark), Some(above), Some(below)) = (dark, above, below) {
        assert_eq!(above, dark, "its back");
        assert_eq!(below, dark, "from under the ground");
    }
}

/// Light coming in low lights less of a floor than light from straight up.
#[test]
fn a_low_sun_lights_less() {
    let high = sunlit(blue_sun(Vec3::new(0.0, 0.0, 1.0)), false);
    let low = sunlit(blue_sun(Vec3::new(1.0, 0.0, 0.3)), false);
    if let (Some(high), Some(low)) = (high, low) {
        assert!(blueness(&low) + 64 * 64 * 10 < blueness(&high));
    }
}

/// What's between the sun and a surface shades it.
#[test]
fn a_live_sun_casts_a_shadow() {
    // At a slant, so that the shadow falls where the blocker doesn't hide it from above.
    let up = Vec3::new(1.0, 0.0, 1.0);
    let (open, shaded) = (sunlit(blue_sun(up), false), sunlit(blue_sun(up), true));
    if let (Some(open), Some(shaded)) = (open, shaded) {
        let darker = (0..64)
            .flat_map(|y| (0..64).map(move |x| (x, y)))
            .filter(|(x, y)| pixel(&shaded, *x, *y)[2].saturating_add(30) < pixel(&open, *x, *y)[2])
            .count();
        assert!(darker > 40, "{darker}");
    }
}

/// The shadow goes the way the light does.
#[test]
fn a_shadow_moves_with_the_sun() {
    let (east, west) = (
        sunlit(blue_sun(Vec3::new(1.0, 0.0, 1.0)), true),
        sunlit(blue_sun(Vec3::new(-1.0, 0.0, 1.0)), true),
    );
    if let (Some(east), Some(west)) = (east, west) {
        let differ = (0..64)
            .flat_map(|y| (0..64).map(move |x| (x, y)))
            .filter(|(x, y)| pixel(&east, *x, *y)[2].abs_diff(pixel(&west, *x, *y)[2]) > 30)
            .count();
        assert!(differ > 40, "{differ}");
    }
}

/// The centre of a picture of `built`, seen from `view`, with `drawables`.
fn centre_of(built: Map, view: &View, drawables: Drawables<'_>) -> Option<[u8; 4]> {
    let mut renderer = renderer()?;
    renderer.load_map(Arc::new(built)).expect("load");
    let capture = renderer.capture(view, drawables, 32, 32).expect("capture");
    Some(centre(&capture))
}

/// A day changes water's colour, by what it multiplies it by.
#[test]
fn a_day_water_tint_changes_water() {
    let look = PoolLook {
        color: [0.0, 0.0, 1.0],
        absorption: 0.2,
        caustics: 0.0,
        reflect: 0.0,
        waves: 0.0,
        refraction: 0.0,
        tint: [0.0; 4],
        deep: 0.0,
        view_effect: 0,
        overlays: [WaterOverlay::NONE; 2],
        foam: 0.0,
        foam_look: WaterOverlay::NONE,
        shore: 0.0,
        splash: 0,
        spray: 0,
        splash_sound: 0,
        trail: 0,
        trail_size: 28.0,
        rain_rings: 0.0,
    };
    let plain = centre_of(pool_map(look), &view_from_above(), Drawables::default());
    let drawables = Drawables {
        water_tint: Some([1.0, 1.0, 0.2]),
        ..Drawables::default()
    };
    let tinted = centre_of(pool_map(look), &view_from_above(), drawables);
    if let (Some(plain), Some(tinted)) = (plain, tinted) {
        assert!(tinted[2] + 30 < plain[2], "{plain:?} {tinted:?}");
    }
}

/// A day changes fog's colour, and not how thick it is.
#[test]
fn a_day_fog_colour_changes_fog() {
    let plain = centre_of(fog_map(0), &view_from_above(), Drawables::default());
    let drawables = Drawables {
        fog_color: Some([1.0, 0.0, 0.0]),
        ..Drawables::default()
    };
    let red = centre_of(fog_map(0), &view_from_above(), drawables);
    if let (Some(plain), Some(red)) = (plain, red) {
        assert!(red[2] + 30 < plain[2], "{plain:?} {red:?}");
        assert!(red[0].saturating_add(10) >= plain[0], "{plain:?} {red:?}");
    }
}

/// A day's sky is drawn in place of the level's.
#[test]
fn a_days_sky_is_drawn() {
    let mut built = map(1);
    built.sky = Some(Sky {
        zenith: [0.0, 0.0, 1.0],
        horizon: [0.0, 0.0, 1.0],
        sun_size: 0.0,
        ..Sky::default()
    });
    let now = Sky {
        zenith: [1.0, 0.0, 0.0],
        horizon: [1.0, 0.0, 0.0],
        ..built.sky.clone().expect("a sky")
    };
    let view = View {
        origin: UNDER,
        pitch: -89.0,
        ..view_from_above()
    };
    let as_level = centre_of(built.clone(), &view, Drawables::default());
    let drawables = Drawables {
        sky_now: Some(&now),
        ..Drawables::default()
    };
    let as_day = centre_of(built, &view, drawables);
    if let (Some(as_level), Some(as_day)) = (as_level, as_day) {
        assert!(as_level[2] > 200 && as_level[0] < 30, "{as_level:?}");
        assert!(as_day[0] > 200 && as_day[2] < 30, "{as_day:?}");
    }
}

/// Light a day adds shows on surfaces, and its highlights can be turned down.
#[test]
fn a_days_light_is_added() {
    let built = map(1);
    let plain = centre_of(built.clone(), &view_from_above(), Drawables::default());
    let drawables = Drawables {
        day_light: Some([0.0, 0.0, 0.3, 1.0]),
        ..Drawables::default()
    };
    let lit = centre_of(built, &view_from_above(), drawables);
    if let (Some(plain), Some(lit)) = (plain, lit) {
        assert!(lit[2] > plain[2].saturating_add(20), "{plain:?} {lit:?}");
    }
}

/// Stars show where the sky's dark, and not in daylight.
#[test]
fn stars_show_at_night() {
    let sky = |zenith: [f32; 3], stars: f32| {
        let mut built = map(1);
        built.sky = Some(Sky {
            zenith,
            horizon: zenith,
            sun_size: 0.0,
            sun_color: [0.0; 3],
            stars,
            ..Sky::default()
        });
        built
    };
    let lit_pixels = |built: Map| -> Option<usize> {
        let mut renderer = renderer()?;
        renderer.load_map(Arc::new(built)).expect("load");
        let view = View {
            origin: UNDER,
            pitch: -89.0,
            ..view_from_above()
        };
        let capture = renderer
            .capture(&view, Drawables::default(), 128, 128)
            .expect("capture");
        Some(
            (0..128)
                .flat_map(|y| (0..128).map(move |x| (x, y)))
                .filter(|(x, y)| pixel(&capture, *x, *y)[0] > 120)
                .count(),
        )
    };
    let (night, none, day) = (
        lit_pixels(sky([0.0; 3], 1.0)),
        lit_pixels(sky([0.0; 3], 0.0)),
        lit_pixels(sky([0.3, 0.4, 0.8], 1.0)),
    );
    if let (Some(night), Some(none), Some(day)) = (night, none, day) {
        // What's lit with no stars is the rest of the view, and stars add to it.
        assert!(night > none + 3, "{night} {none}");
        assert!(day < night, "{day} {night}");
    }
}

/// The sky's sun is where a day has it, and the moon on the far side.
#[test]
fn the_sky_shows_the_sun_and_moon_where_they_are() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map(1);
    built.sky = Some(Sky {
        zenith: [0.0; 3],
        horizon: [0.0; 3],
        sun_color: [1.0, 0.0, 0.0],
        sun_size: 0.02,
        moon_color: [0.0, 0.0, 1.0],
        moon_size: 0.02,
        sun_direction: Vec3::new(1.0, 0.0, 0.0),
        ..Sky::default()
    });
    renderer.load_map(Arc::new(built)).expect("load");
    let view = View {
        origin: UNDER,
        pitch: -89.9,
        ..view_from_above()
    };
    let mut shot = |toward: Option<Vec3>| {
        let drawables = Drawables {
            sun_toward: toward,
            ..Drawables::default()
        };
        let capture = renderer.capture(&view, drawables, 32, 32).expect("capture");
        centre(&capture)
    };
    let sun_up = shot(Some(Vec3::new(0.0, 0.0, 1.0)));
    let moon_up = shot(Some(Vec3::new(0.0, 0.0, -1.0)));
    let as_level = shot(None);
    assert!(sun_up[0] > 200 && sun_up[2] < 30, "{sun_up:?}");
    assert!(moon_up[2] > 200 && moon_up[0] < 30, "{moon_up:?}");
    assert!(as_level[0] < 30 && as_level[2] < 30, "{as_level:?}");
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
            surface: None,
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

/// The middle of a floor with a matcap on it, under a layer whose surface leans by `slope`,
/// painted on by `paint`.
fn matcap_under_a_layer(slope: u8, paint: u8) -> Option<[u8; 4]> {
    let mut renderer = renderer()?;
    let mut built = map(1);
    built.textures[0].material = Some(Arc::new(Material {
        shading: Shading::Matcap,
        matcap: Some(1),
        slots: vec![
            Arc::new(image_of(64, |x, y| {
                [
                    u8::try_from(x * 4).expect("small"),
                    u8::try_from(y * 4).expect("small"),
                    0,
                    255,
                ]
            })),
            Arc::new(image_of(2, |_, _| [255, 255, 255, 255])),
            Arc::new(image_of(2, |_, _| [128, slope, 128, 0])),
        ],
        layers: vec![MaterialLayer {
            base: 2,
            mask: PAINT_MASK,
            surface: Some(3),
        }],
        ..Material::default()
    }));
    built.paint = vec![[paint, 0, 0, 0]; built.light.len()];
    renderer.load_map(Arc::new(built)).expect("load");
    let capture = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    Some(centre(&capture))
}

/// A layer's own bumps lean a surface where it's painted, and only there.
#[test]
fn a_layer_leans_a_surface_by_its_own_bumps() {
    let Some(flat) = matcap_under_a_layer(128, 255) else {
        return;
    };
    let leaning = matcap_under_a_layer(230, 255).expect("it drew");
    let unpainted = matcap_under_a_layer(230, 0).expect("it drew");
    let apart = |a: [u8; 4], b: [u8; 4]| (0..3).map(|at| a[at].abs_diff(b[at])).max().unwrap_or(0);
    assert!(apart(leaning, flat) > 10, "{flat:?} and {leaning:?}");
    assert!(apart(unpainted, flat) < 25, "{flat:?} and {unpainted:?}");
    assert!(
        apart(unpainted, leaning) > 10,
        "{unpainted:?} and {leaning:?}"
    );
}

/// The middle of a black floor under a white light, painted over with a layer that shines this
/// much.
fn shine_of_a_layer(shine: u8) -> Option<[u8; 4]> {
    let mut renderer = renderer()?;
    let mut built = map(1);
    built.textures[0].mips = image_of(2, |_, _| [0, 0, 0, 255]);
    built.textures[0].material = Some(Arc::new(Material {
        specular_strength: 1.0,
        gloss: 32.0,
        slots: vec![
            Arc::new(image_of(2, |_, _| [0, 0, 0, 255])),
            Arc::new(image_of(2, |_, _| [128, 128, 128, shine])),
        ],
        layers: vec![MaterialLayer {
            base: 1,
            mask: PAINT_MASK,
            surface: Some(2),
        }],
        ..Material::default()
    }));
    built.paint = vec![[255, 0, 0, 0]; built.light.len()];
    renderer.load_map(Arc::new(built)).expect("load");
    let lights = [LightDraw {
        origin: Vec3::new(64.0, 64.0, 40.0),
        radius: 300.0,
        min_light: 0.0,
    }];
    let drawables = Drawables {
        lights: &lights,
        ..Drawables::default()
    };
    let capture = renderer
        .capture(&view_from_above(), drawables, 64, 64)
        .expect("capture");
    Some(centre(&capture))
}

/// A layer's own shine shows under a light, with nothing of the material's to go on.
#[test]
fn a_layer_shines_by_its_own_map() {
    let Some(dull) = shine_of_a_layer(0) else {
        return;
    };
    let bright = shine_of_a_layer(255).expect("it drew");
    assert!(dull[0] < 10, "{dull:?}");
    assert!(bright[0] > 60, "{bright:?}");
}

/// The middle of black land under a white light, painted with a layer that shines this much.
fn shine_of_land(shine: u8) -> Option<[u8; 4]> {
    let mut renderer = renderer()?;
    let mut built = painted_land([128, 128], |_| [255, 0]);
    let black = [0, 0, 0, 255].repeat(16);
    built.land_pictures[0].color = Arc::new(mip_chain(4, 4, black));
    let surface = [128, 128, 128, shine].repeat(16);
    built.land_pictures[0].surface = Arc::new(mip_chain(4, 4, surface));
    renderer.load_map(Arc::new(built)).expect("load");
    let lights = [LightDraw {
        origin: Vec3::new(0.0, 0.0, 100.0),
        radius: 600.0,
        min_light: 0.0,
    }];
    let drawables = Drawables {
        lights: &lights,
        ..Drawables::default()
    };
    let view = View {
        origin: Vec3::new(0.0, 0.0, 300.0),
        ..view_from_above()
    };
    let capture = renderer.capture(&view, drawables, 64, 64).expect("capture");
    Some(centre(&capture))
}

#[test]
fn land_shines_by_its_layers() {
    let Some(dull) = shine_of_land(0) else {
        return;
    };
    let bright = shine_of_land(255).expect("it drew");
    assert!(dull[0] < 10, "{dull:?}");
    assert!(bright[0] > 60, "{bright:?}");
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

/// A brush at work shows at once, heights and holes, with no map built again.
#[test]
fn land_can_be_changed_while_it_is_drawn() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let built = map_with_land();
    let mut sheet = built.terrain[0].clone();
    renderer.load_map(Arc::new(built)).expect("load");
    let view = View {
        origin: Vec3::new(0.0, 0.0, 400.0),
        ..view_from_above()
    };
    let shot = |renderer: &mut Renderer| {
        renderer
            .capture(&view, Drawables::default(), 64, 64)
            .expect("capture")
    };
    assert!(centre(&shot(&mut renderer))[0] > 200, "land shows");
    for y in 60..68 {
        for x in 60..68 {
            sheet.data.set_hole(x, y, true);
        }
    }
    renderer.update_land(0, &sheet).expect("it changes");
    assert!(
        centre(&shot(&mut renderer))[0] < 50,
        "the hole's there at once"
    );
    for y in 60..68 {
        for x in 60..68 {
            sheet.data.set_hole(x, y, false);
        }
    }
    // Pushed up past the eye, so that the middle of the view is under it and sees nothing there.
    sheet.data.heights.fill(600.0);
    renderer.update_land(0, &sheet).expect("it changes");
    assert!(centre(&shot(&mut renderer))[0] < 50, "and so are heights");
}

/// The editor's view is part of its window, and a picture has to show it there, or what's drawn
/// over the view, like a brush's circle, won't line up with it.
#[test]
fn a_picture_has_the_scene_where_the_window_has_it() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map_with_land())).expect("load");
    let view = View {
        origin: Vec3::new(0.0, 0.0, 400.0),
        ..view_from_above()
    };
    renderer.set_view_area(Some([32, 0, 32, 64]));
    let shot = renderer
        .capture(&view, Drawables::default(), 64, 64)
        .expect("capture");
    let at = |x: usize| shot.rgba[(32 * 64 + x) * 4];
    assert!(at(16) < 50, "nothing left of the view: {}", at(16));
    assert!(at(48) > 200, "and the land's in it: {}", at(48));
}

/// A picture for land of one colour, standing as high as `height` says.
fn land_picture(name: &str, color: [u8; 3], height: u8) -> LandPicture {
    let pixels = |pixel: [u8; 4]| pixel.repeat(16);
    LandPicture {
        material: name.to_owned(),
        color: Arc::new(mip_chain(4, 4, pixels([color[0], color[1], color[2], 255]))),
        surface: Arc::new(mip_chain(4, 4, pixels([128, 128, height, 0]))),
        gloss: 32.0,
    }
}

/// Flat land painted red on its low half along X, and blue on the other, from layers that stand
/// as high as `heights` says, under `paint`.
fn painted_land(heights: [u8; 2], paint: impl Fn(u32) -> [u8; 2]) -> Map {
    let mut built = map_with_land();
    let sheet = &mut built.terrain[0];
    sheet.data.heights.fill(0.0);
    for (number, material) in ["red", "blue"].iter().enumerate() {
        let layer = TerrainLayer {
            material: (*material).to_owned(),
            size: 256.0,
            hex: false,
        };
        assert!(sheet.data.set_layer(number, layer));
    }
    let across = sheet.data.across();
    for (at, weights) in (0_u32..).zip(&mut sheet.data.paint) {
        let [first, second] = paint(at % across);
        weights[..2].copy_from_slice(&[first, second]);
    }
    sheet.pictures = vec![0, 1];
    built.land_pictures = vec![
        land_picture("red", [255, 0, 0], heights[0]),
        land_picture("blue", [0, 0, 255], heights[1]),
    ];
    built
}

/// The land looked down on from `height`, as a picture.
fn painted_from_above(renderer: &mut Renderer, built: Map, height: f32) -> Capture {
    renderer.load_map(Arc::new(built)).expect("load");
    let view = View {
        origin: Vec3::new(0.0, 0.0, height),
        ..view_from_above()
    };
    renderer
        .capture(&view, Drawables::default(), 64, 64)
        .expect("capture")
}

/// A pixel's colour.
fn pixel_at(shot: &Capture, x: u32, y: u32) -> [u8; 3] {
    let at = ((y * shot.width + x) * 4) as usize;
    [shot.rgba[at], shot.rgba[at + 1], shot.rgba[at + 2]]
}

#[test]
fn paint_shows_each_layer_where_it_is() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let half = |x: u32| if x < 64 { [255, 0] } else { [0, 255] };
    let shot = painted_from_above(&mut renderer, painted_land([128, 128], half), 300.0);
    // Up the middle of the view, as that's along X, one end is one colour and the other's the
    // other.
    let [low, high] = [(32, 8), (32, 56)].map(|(x, y)| pixel_at(&shot, x, y));
    let reddest = if low[0] > low[2] { low } else { high };
    let bluest = if low[0] > low[2] { high } else { low };
    assert!(reddest[0] > 150 && reddest[2] < 40, "{low:?} {high:?}");
    assert!(bluest[2] > 150 && bluest[0] < 40, "{low:?} {high:?}");
}

#[test]
fn where_layers_are_even_the_one_that_stands_higher_shows() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let even = |_| [128, 127];
    let shot = painted_from_above(&mut renderer, painted_land([230, 40], even), 300.0);
    let red = centre(&shot);
    assert!(red[0] > 150 && red[2] < 30, "the red's higher: {red:?}");
    let shot = painted_from_above(&mut renderer, painted_land([40, 230], even), 300.0);
    let blue = centre(&shot);
    assert!(
        blue[2] > 150 && blue[0] < 30,
        "and now the blue is: {blue:?}"
    );
    let shot = painted_from_above(&mut renderer, painted_land([128, 128], even), 300.0);
    let both = centre(&shot);
    assert!(
        both[0] > 100 && both[2] > 100,
        "and level, they mix: {both:?}"
    );
}

#[test]
fn paint_changes_while_it_is_drawn() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let built = painted_land([128, 128], |_| [255, 0]);
    let mut sheet = built.terrain[0].clone();
    let shot = painted_from_above(&mut renderer, built, 300.0);
    assert!(centre(&shot)[0] > 150 && centre(&shot)[2] < 40, "red");
    for paint in &mut sheet.data.paint {
        paint[..2].copy_from_slice(&[0, 255]);
    }
    renderer.update_land(0, &sheet).expect("it changes");
    let view = View {
        origin: Vec3::new(0.0, 0.0, 300.0),
        ..view_from_above()
    };
    let shot = renderer
        .capture(&view, Drawables::default(), 64, 64)
        .expect("capture");
    assert!(centre(&shot)[2] > 150 && centre(&shot)[0] < 40, "blue");
}

/// Far off, a layer's the mean of its picture, not a pattern. So that a picture's own mips can't
/// come to the same thing, its top differs from its last, and a repeat's so wide that it would
/// read its top from there.
#[test]
fn far_land_reads_the_mean_of_a_picture() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = painted_land([128, 128], |_| [255, 0]);
    let mut mips = built.land_pictures[0].color.as_ref().clone();
    for mip in mips.iter_mut().take(1) {
        mip.rgba = [0, 255, 0, 255].repeat(mip.rgba.len() / 4);
    }
    built.land_pictures[0].color = Arc::new(mips);
    built.terrain[0].data.layers[0].size = 200_000.0;
    let near = painted_from_above(&mut renderer, built.clone(), 300.0);
    assert!(centre(&near)[1] > 150, "the top of it: {:?}", centre(&near));
    let far = painted_from_above(&mut renderer, built, 9000.0);
    assert!(centre(&far)[0] > 150, "the last of it: {:?}", centre(&far));
}

/// A picture red in its low rows and blue in its high ones, which shows which way it lies.
fn striped_picture() -> LandPicture {
    let rows = |top: [u8; 4], bottom: [u8; 4]| [top.repeat(32), bottom.repeat(32)].concat();
    let color = rows([255, 0, 0, 255], [0, 0, 255, 255]);
    let surface = [128, 128, 128, 0].repeat(64);
    LandPicture {
        material: "striped".to_owned(),
        color: Arc::new(mip_chain(8, 8, color)),
        surface: Arc::new(mip_chain(8, 8, surface)),
        gloss: 32.0,
    }
}

/// How many times the colour turns from red to blue or back along `pixels`.
fn colour_changes(pixels: impl Iterator<Item = [u8; 3]>) -> usize {
    let sides: Vec<bool> = pixels.map(|pixel| pixel[0] > pixel[2]).collect();
    sides.windows(2).filter(|pair| pair[0] != pair[1]).count()
}

/// A cliff's picture stands up on it, and isn't drawn out down it from the top of the land.
#[test]
fn a_cliff_has_its_picture_on_its_side() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = painted_land([128, 128], |_| [255, 0]);
    built.land_pictures[0] = striped_picture();
    built.land_pictures[1] = striped_picture();
    let sheet = &mut built.terrain[0];
    let across = sheet.data.across();
    for (at, height) in (0_u32..).zip(&mut sheet.data.heights) {
        *height = if at % across >= 64 { 3000.0 } else { 0.0 };
    }
    renderer.load_map(Arc::new(built)).expect("load");
    let view = View {
        origin: Vec3::new(-300.0, 0.0, 1500.0),
        pitch: 0.0,
        ..view_from_above()
    };
    let shot = renderer
        .capture(&view, Drawables::default(), 64, 64)
        .expect("capture");
    let down = colour_changes((0..64).map(|y| pixel_at(&shot, 32, y)));
    let along = colour_changes((0..64).map(|x| pixel_at(&shot, x, 32)));
    assert!(down >= 2, "stripes go up it: {down}");
    assert_eq!(along, 0, "and not across it");
}

/// Hexagon tiles break up a repeat near the view, and stop further off.
#[test]
fn hexagon_tiles_are_for_a_layer_that_asks_near_the_view() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let striped = |hex: bool| {
        let mut built = painted_land([128, 128], |_| [255, 0]);
        built.land_pictures[0] = striped_picture();
        built.terrain[0].data.layers[0].hex = hex;
        built
    };
    let rows = |shot: &Capture| {
        (0..64)
            .map(|x| colour_changes((0..64).map(|y| pixel_at(shot, x, y))))
            .collect::<Vec<usize>>()
    };
    // Plain stripes run straight across, so every column is the same.
    let plain = painted_from_above(&mut renderer, striped(false), 300.0);
    let shots = rows(&plain);
    assert!(shots.iter().all(|count| *count == shots[0]), "{shots:?}");
    let broken = painted_from_above(&mut renderer, striped(true), 300.0);
    assert_ne!(plain.rgba, broken.rgba, "tiles move them");
    let far_plain = painted_from_above(&mut renderer, striped(false), 2600.0);
    let far_hex = painted_from_above(&mut renderer, striped(true), 2600.0);
    assert_eq!(far_plain.rgba, far_hex.rgba, "and they stop");
}

/// A floor that shows one place of a field called "grid", as a shade of grey.
fn map_with_field(code: &str, size: u32, rate: f32, read_at: f32) -> Map {
    let mut built = map(1);
    let surface = format!(
        "fn shader_surface(surface: Surface) -> Surface {{\n\
         var out = surface;\n\
         let seen = shader_image(0, vec2<f32>({read_at:?}, 0.5)).r;\n\
         out.base = vec4<f32>(seen, seen, seen, 1.0);\n\
         return out;\n}}\n"
    );
    built.textures[0].material = Some(Arc::new(Material {
        shading: Shading::Unlit,
        shader: Some(Arc::new(ShaderCode {
            name: "shows_field".to_owned(),
            code: surface,
        })),
        slots: vec![Arc::new(image_of(1, |_, _| [0, 0, 0, 255]))],
        shader_images: [Some(1), None],
        field_images: vec![(1, "grid".to_owned())],
        ..Material::default()
    }));
    built.fields.push(FieldDef {
        name: "grid".to_owned(),
        shader: Arc::new(ShaderCode {
            name: "grid".to_owned(),
            code: code.to_owned(),
        }),
        size,
        rate,
        params: [0.25, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    });
    built
}

/// What the floor shows at `time`.
fn shade_at(renderer: &mut Renderer, time: f32) -> u8 {
    let view = View {
        time,
        ..view_from_above()
    };
    let shot = renderer
        .capture(&view, Drawables::default(), 16, 16)
        .expect("capture");
    centre(&shot)[0]
}

/// A field that climbs by its first number at every step.
const CLIMBS: &str = "fn shader_field(at: vec2<i32>, size: vec2<i32>) -> vec4<f32> {
    return field_read(at) + vec4<f32>(shader_param(0), 0.0, 0.0, 0.0);
}
";

/// A field steps by the clock, keeps what it had, and a surface reads it.
#[test]
fn a_field_steps_at_its_rate_and_a_surface_reads_it() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer
        .load_map(Arc::new(map_with_field(CLIMBS, 8, 10.0, 0.5)))
        .expect("load");
    let start = shade_at(&mut renderer, 0.0);
    assert!(start < 5, "it starts empty: {start}");
    let one = shade_at(&mut renderer, 0.1);
    assert!(one > 40, "a step: {one}");
    assert_eq!(
        shade_at(&mut renderer, 0.1),
        one,
        "none's due at the same time"
    );
    assert_eq!(shade_at(&mut renderer, 0.15), one, "or before the next");
    let two = shade_at(&mut renderer, 0.2);
    assert!(two > one + 10, "another: {one} and {two}");
    assert!(renderer.take_shader_problems().is_empty());
}

/// However long a frame takes, a field doesn't try to catch up all at once.
#[test]
fn a_long_frame_takes_only_so_many_steps() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let code = CLIMBS.replace("shader_param(0)", "0.05");
    renderer
        .load_map(Arc::new(map_with_field(&code, 8, 10.0, 0.5)))
        .expect("load");
    let _start = shade_at(&mut renderer, 0.0);
    let shade = shade_at(&mut renderer, 100.0);
    // Four steps of 0.05, through the curve from light to a byte, and not a thousand.
    assert!(shade > 20 && shade < 200, "{shade}");
}

/// What a step reads is what the last one left, so nothing spreads faster than a cell a step.
#[test]
fn a_step_reads_only_what_the_last_one_left() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let spreads = "fn shader_field(at: vec2<i32>, size: vec2<i32>) -> vec4<f32> {
    if at.x == 0 {
        return vec4<f32>(1.0);
    }
    return field_read(at - vec2<i32>(1, 0));
}
";
    // The third cell of four, along its middle.
    renderer
        .load_map(Arc::new(map_with_field(spreads, 4, 10.0, 0.625)))
        .expect("load");
    let empty = shade_at(&mut renderer, 0.0);
    assert!(empty < 5, "{empty}");
    let one = shade_at(&mut renderer, 0.1);
    assert!(one < 5, "the first cell's lit, and not this one: {one}");
    let two = shade_at(&mut renderer, 0.2);
    assert!(two < 5, "the second is: {two}");
    let three = shade_at(&mut renderer, 0.3);
    assert!(three > 200, "and now this one: {three}");
}

/// Past the edge is what's at the edge, so a sum over neighbours needs no care there.
#[test]
fn a_field_read_past_its_edge_is_its_edge() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let reads_out = "fn shader_field(at: vec2<i32>, size: vec2<i32>) -> vec4<f32> {
    if shader_time() < 0.15 {
        return vec4<f32>(1.0);
    }
    return field_read(at + vec2<i32>(1, 0));
}
";
    // The last cell of four, along its middle.
    renderer
        .load_map(Arc::new(map_with_field(reads_out, 4, 10.0, 0.875)))
        .expect("load");
    let _start = shade_at(&mut renderer, 0.0);
    let lit = shade_at(&mut renderer, 0.1);
    assert!(lit > 200, "{lit}");
    let after = shade_at(&mut renderer, 0.2);
    assert_eq!(after, lit, "it read itself");
}

/// A field that won't compile doesn't step and says so, and what reads it goes on.
#[test]
fn a_broken_field_says_so_and_stays_still() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let broken = map_with_field("this isn't code", 8, 10.0, 0.5);
    renderer.load_map(Arc::new(broken)).expect("load");
    let told = renderer.take_shader_problems().join(" ");
    assert!(told.contains("field grid"), "{told}");
    let shade = shade_at(&mut renderer, 0.5);
    assert!(shade < 5, "{shade}");
}

/// A map with its square, and `pictures` for particles to show.
fn map_with_pictures(pictures: Vec<Vec<MipLevel>>) -> Map {
    let mut built = map(1);
    built.effect_pictures = pictures.into_iter().map(Arc::new).collect();
    built
}

/// The floor seen with `particles`, as a picture, with what's in the map loaded.
fn floor_with(renderer: &mut Renderer, built: Map, particles: &[ParticleDraw]) -> Capture {
    renderer.load_map(Arc::new(built)).expect("load");
    let drawables = Drawables {
        particles,
        ..Drawables::default()
    };
    renderer
        .capture(&view_from_above(), drawables, 64, 64)
        .expect("capture")
}

/// A soft particle, of `color`, in the middle of the view, `high` above the floor.
fn particle_over(high: f32, color: [u8; 3], size: f32) -> ParticleDraw {
    ParticleDraw {
        origin: Vec3::new(64.0, 64.0, high),
        color: ParticleColor {
            red: color[0],
            green: color[1],
            blue: color[2],
        },
        alpha: 255,
        size,
        shape: ParticleShape::Soft,
        glow: 1.0,
        ..ParticleDraw::default()
    }
}

/// A particle shows its picture, not a soft dot.
#[test]
fn a_particle_shows_its_picture() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    // White on its left, and nothing on its right.
    let half = image_of(8, |x, _| {
        if x < 4 {
            [255, 255, 255, 255]
        } else {
            [255, 255, 255, 0]
        }
    });
    let particle = ParticleDraw {
        picture: 1,
        ..particle_over(10.0, [0, 255, 0], 40.0)
    };
    let plain = floor_with(&mut renderer, map(1), &[]);
    let shot = floor_with(&mut renderer, map_with_pictures(vec![half]), &[particle]);
    let shows = (0..64)
        .filter(|x| {
            let (was, is) = (pixel_at(&plain, *x, 32), pixel_at(&shot, *x, 32));
            was.iter().zip(is).any(|(a, b)| a.abs_diff(b) > 40)
        })
        .count();
    // It's a quarter of the width of the view, and half of it is there.
    assert!((6..=17).contains(&shows), "{shows}");
}

/// A sheet's frames are read by number, and between two it's a mix of them.
#[test]
fn a_particle_shows_the_frame_of_its_sheet() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    // Two frames: red on its left, and blue on its right.
    let sheet = || {
        image_of(8, |x, _| {
            if x < 4 {
                [255, 0, 0, 255]
            } else {
                [0, 0, 255, 255]
            }
        })
    };
    let on_frame = |renderer: &mut Renderer, frame: f32| {
        let particle = ParticleDraw {
            picture: 1,
            frame,
            sheet: [2, 1],
            ..particle_over(40.0, [255, 255, 255], 60.0)
        };
        centre(&floor_with(
            renderer,
            map_with_pictures(vec![sheet()]),
            &[particle],
        ))
    };
    let first = on_frame(&mut renderer, 0.0);
    assert!(first[0] > 200 && first[2] < 30, "{first:?}");
    let second = on_frame(&mut renderer, 1.0);
    assert!(second[2] > 200 && second[0] < 30, "{second:?}");
    let between = on_frame(&mut renderer, 0.5);
    assert!(between[0] > 80 && between[2] > 80, "{between:?}");
}

/// What's behind a wall isn't drawn, though nothing's attached to test it.
#[test]
fn a_particle_behind_something_is_hidden() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let plain = floor_with(&mut renderer, map(1), &[]);
    let under = particle_over(-40.0, [0, 0, 255], 40.0);
    let shot = floor_with(&mut renderer, map(1), &[under]);
    assert_eq!(centre(&shot), centre(&plain));
}

/// Where a particle meets a surface it fades, by `soft`, and with none it's cut off.
#[test]
fn a_soft_particle_fades_into_what_is_behind_it() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let blue_of = |renderer: &mut Renderer, soft: f32| {
        let particle = ParticleDraw {
            soft,
            ..particle_over(10.0, [0, 0, 255], 60.0)
        };
        centre(&floor_with(renderer, map(1), &[particle]))[2]
    };
    let hard = blue_of(&mut renderer, 0.0);
    let faded = blue_of(&mut renderer, 40.0);
    let more = blue_of(&mut renderer, 400.0);
    assert!(hard > faded + 40, "{hard} and {faded}");
    assert!(faded > more, "{faded} and {more}");
}

/// In one list, what covers goes over what's behind it and what adds adds, in the order given.
#[test]
fn what_covers_and_what_adds_go_in_one_order() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let cover = particle_over(40.0, [255, 0, 0], 90.0);
    let adding = ParticleDraw {
        adds: true,
        ..particle_over(40.0, [0, 0, 255], 90.0)
    };
    let adds_last = centre(&floor_with(&mut renderer, map(1), &[cover, adding]));
    assert!(
        adds_last[0] > 150 && adds_last[2] > 150,
        "both: {adds_last:?}"
    );
    let covers_last = centre(&floor_with(&mut renderer, map(1), &[adding, cover]));
    assert!(
        covers_last[0] > 150 && covers_last[2] < 60,
        "covered: {covers_last:?}"
    );
}

/// A particle with a shader of its own goes through it, in its place in the list.
#[test]
fn a_shader_reaches_a_particle() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map(1);
    built.effect_shaders.push(Arc::new(ShaderCode {
        name: "greener".to_owned(),
        code: "fn shader_particle(look: ParticleLook) -> vec4<f32> {
    return vec4<f32>(0.0, 1.0, 0.0, look.color.a);
}
"
        .to_owned(),
    }));
    let particle = ParticleDraw {
        shader: 1,
        ..particle_over(40.0, [255, 0, 0], 60.0)
    };
    let shown = centre(&floor_with(&mut renderer, built, &[particle]));
    assert!(shown[1] > 200 && shown[0] < 60, "{shown:?}");
    assert!(renderer.take_shader_problems().is_empty());
}

/// A map with a pane of red glass high over its floor, facing up.
fn map_with_glass() -> Map {
    let mut built = map(1);
    let mut glass = built.textures[0].clone();
    glass.name = "glass".to_owned();
    glass.mips = image_of(2, |_, _| [255, 0, 0, 200]);
    glass.material = Some(Arc::new(Material {
        alpha: AlphaMode::Blend,
        shading: Shading::Unlit,
        ..Material::default()
    }));
    built.textures.push(glass);
    let first = u32::try_from(built.points.len()).expect("few");
    let pane = built.faces[0].point_count;
    let points: Vec<Vec3> = built.face_points(&built.faces[0]).to_vec();
    built
        .points
        .extend(points.iter().map(|point| Vec3::new(point.x, point.y, 40.0)));
    let mut face = built.faces[0].clone();
    face.first_point = first;
    face.point_count = pane;
    face.texture = 1;
    built.faces.push(face);
    let count = u32::try_from(built.faces.len()).expect("few");
    built.tree.leaf_faces = (0..count).collect();
    built.tree.leaves[1].face_count = count;
    built.models[0].face_count = count;
    built
}

/// What's behind glass is seen through it, and what's in front goes over it.
#[test]
fn glass_goes_over_what_is_behind_it_and_under_what_is_in_front() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let glass = centre(&floor_with(&mut renderer, map_with_glass(), &[]));
    assert!(glass[0] > 100, "there's glass: {glass:?}");
    let below = particle_over(20.0, [0, 0, 255], 60.0);
    let through = centre(&floor_with(&mut renderer, map_with_glass(), &[below]));
    assert!(through[0] > through[2], "red over it: {through:?}");
    let above = particle_over(52.0, [0, 0, 255], 14.0);
    let over = centre(&floor_with(&mut renderer, map_with_glass(), &[above]));
    assert!(over[2] > over[0], "blue over the red: {over:?}");
    let both = centre(&floor_with(
        &mut renderer,
        map_with_glass(),
        &[below, above],
    ));
    assert!(both[2] > both[0], "and both at once: {both:?}");
}

/// A shader that won't compile doesn't stop what it's on from drawing, and says so.
#[test]
fn a_broken_particle_shader_says_so() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map(1);
    built.effect_shaders.push(Arc::new(ShaderCode {
        name: "wrong".to_owned(),
        code: "fn shader_particle(look: ParticleLook) -> vec4<f32> { nonsense }".to_owned(),
    }));
    let particle = ParticleDraw {
        shader: 1,
        ..particle_over(40.0, [255, 0, 0], 60.0)
    };
    let shown = centre(&floor_with(&mut renderer, built, &[particle]));
    assert!(shown[0] > 200, "{shown:?}");
    let told = renderer.take_shader_problems().join(" ");
    assert!(told.contains("wrong"), "{told}");
}

/// A map whose effects have compute particles, so that it has room for them.
fn map_with_sims() -> Map {
    let mut built = map(1);
    built.effects.push(Arc::new(EffectDef {
        name: "sim".to_owned(),
        emitters: vec![ParticleEmitter {
            sim: true,
            ..ParticleEmitter::default()
        }],
        timeline: Vec::new(),
    }));
    built
}

/// One compute particle going up from the middle of the view at 60 units a second.
fn one_sim() -> ParticleSim {
    ParticleSim {
        id: 1,
        origin: Vec3::new(64.0, 64.0, 20.0),
        seconds: 1.0 / 60.0,
        direction: Vec3::new(0.0, 0.0, 1.0),
        spread: 1.0,
        speed: [60.0, 60.0],
        life: [100.0, 100.0],
        count: 4,
        spawn: 1,
        ramp: [[0.0, 0.0, 1.0, 1.0]; 8],
        sizes: [30.0; 8],
        sheet: [1, 1],
        ..ParticleSim::default()
    }
}

/// Runs `sim` for `steps`, with one starting at the first, and the state of its first slot.
fn run_sim(renderer: &mut Renderer, sim: ParticleSim, steps: u32) -> [f32; 8] {
    let view = view_from_above();
    for step in 0..steps {
        let now = ParticleSim {
            spawn: if step == 0 { sim.spawn } else { 0 },
            time: step as f32 / 60.0,
            seed: step,
            ..sim
        };
        let sims = [now];
        let drawables = Drawables {
            particle_sims: &sims,
            ..Drawables::default()
        };
        let _shot = renderer.capture(&view, drawables, 32, 32).expect("capture");
    }
    renderer.read_sim(sim.id).expect("reads")[0]
}

/// Compute keeps what it works out from step to step.
#[test]
fn compute_moves_a_particle_on_from_where_it_was() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map_with_sims())).expect("load");
    let slot = run_sim(&mut renderer, one_sim(), 31);
    assert!((slot[2] - 50.0).abs() < 0.5, "half a second up: {slot:?}");
    assert!(
        (slot[3] - 0.5).abs() < 0.02,
        "and half a second old: {slot:?}"
    );
    assert!((slot[6] - 60.0).abs() < 0.01, "as fast as it was: {slot:?}");
    let falling = ParticleSim {
        id: 2,
        gravity: 120.0,
        ..one_sim()
    };
    let slot = run_sim(&mut renderer, falling, 31);
    assert!((slot[6] - 0.0).abs() < 0.1, "gravity took it all: {slot:?}");
}

/// Forces in compute are the map's sums, step by step.
#[test]
fn compute_pushes_as_the_map_says_forces_do() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let forces = [
        (
            ParticleForce::Attract {
                at: Vec3::new(40.0, 0.0, 0.0),
                strength: 90.0,
                reach: 200.0,
            },
            SimForce {
                kind: 1.0,
                strength: 90.0,
                extra: 200.0,
                at: Vec3::new(40.0, 0.0, 0.0),
                axis: Vec3::ZERO,
            },
        ),
        (
            ParticleForce::Vortex {
                at: Vec3::new(-30.0, 0.0, 0.0),
                axis: Vec3::new(0.0, 0.0, 1.0),
                strength: 80.0,
                inward: 20.0,
            },
            SimForce {
                kind: 2.0,
                strength: 80.0,
                extra: 20.0,
                at: Vec3::new(-30.0, 0.0, 0.0),
                axis: Vec3::new(0.0, 0.0, 1.0),
            },
        ),
        (
            ParticleForce::Wind {
                blowing: Vec3::new(20.0, 40.0, 0.0),
                gusts: 0.5,
            },
            SimForce {
                kind: 3.0,
                extra: 0.5,
                at: Vec3::new(20.0, 40.0, 0.0),
                ..SimForce::default()
            },
        ),
    ];
    for (number, (on_map, on_gpu)) in (10..).zip(forces) {
        renderer.load_map(Arc::new(map_with_sims())).expect("load");
        let mut sim = one_sim();
        sim.id = number;
        sim.forces[0] = on_gpu;
        let seconds = sim.seconds;
        let mut place = sim.origin;
        let mut moving = Vec3::new(0.0, 0.0, 60.0);
        // The first step starts it, and those after move it.
        for step in 1..40_u32 {
            let push = on_map.push(place, sim.origin, step as f32 / 60.0);
            moving = moving + push * seconds;
            place = place + moving * seconds;
        }
        let slot = run_sim(&mut renderer, sim, 40);
        let found = Vec3::new(slot[0], slot[1], slot[2]);
        assert!(
            (found - place).length() < 0.25,
            "{on_map:?}: {found:?} and {place:?}"
        );
    }
}

/// When its slots run out, what starts goes in the oldest, and what's gone draws nothing.
#[test]
fn a_ring_of_slots_goes_round_and_the_dead_draw_nothing() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map_with_sims())).expect("load");
    let sim = ParticleSim {
        life: [0.05, 0.05],
        speed: [0.0, 0.0],
        spawn: 2,
        count: 3,
        ..one_sim()
    };
    let plain = {
        let shot = renderer
            .capture(&view_from_above(), Drawables::default(), 32, 32)
            .expect("capture");
        centre(&shot)
    };
    let mut drew = false;
    for step in 0..8_u32 {
        let now = ParticleSim {
            spawn: if step < 3 { 2 } else { 0 },
            seed: step,
            ..sim
        };
        let sims = [now];
        let drawables = Drawables {
            particle_sims: &sims,
            ..Drawables::default()
        };
        let shot = renderer
            .capture(&view_from_above(), drawables, 32, 32)
            .expect("capture");
        drew |= centre(&shot) != plain;
    }
    assert!(drew, "they showed");
    let slots = renderer.read_sim(sim.id).expect("reads");
    assert_eq!(slots.len(), 3);
    assert!(
        slots.iter().all(|slot| slot[3] >= slot[7]),
        "all gone: {slots:?}"
    );
    let sims = [ParticleSim { spawn: 0, ..sim }];
    let drawables = Drawables {
        particle_sims: &sims,
        ..Drawables::default()
    };
    let shot = renderer
        .capture(&view_from_above(), drawables, 32, 32)
        .expect("capture");
    assert_eq!(centre(&shot), plain, "and nothing's drawn of them");
}

/// One a step goes round the slots, and what starts next goes in the oldest.
#[test]
fn a_ring_fills_and_then_takes_the_oldest() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map_with_sims())).expect("load");
    let sim = ParticleSim {
        speed: [0.0, 0.0],
        ..one_sim()
    };
    let step = |renderer: &mut Renderer, seed: u32| {
        let sims = [ParticleSim { seed, ..sim }];
        let drawables = Drawables {
            particle_sims: &sims,
            ..Drawables::default()
        };
        let _shot = renderer
            .capture(&view_from_above(), drawables, 32, 32)
            .expect("capture");
        let slots = renderer.read_sim(sim.id).expect("reads");
        slots.iter().map(|slot| slot[3]).collect::<Vec<_>>()
    };
    let (one, two) = (1.0 / 60.0, 2.0 / 60.0);
    for (seed, wanted) in [
        (0, [0.0, 1.0, 1.0, 1.0]),
        (1, [one, 0.0, 1.0, 1.0]),
        (2, [two, one, 0.0, 1.0]),
    ] {
        let ages = step(&mut renderer, seed);
        for (age, should) in ages.iter().zip(wanted) {
            assert!((age - should).abs() < 0.001, "{ages:?} and {wanted:?}");
        }
    }
    let _fourth = step(&mut renderer, 3);
    let ages = step(&mut renderer, 4);
    assert!(ages[0] < 0.001, "the first went round again: {ages:?}");
    assert!(ages[1] > ages[2] && ages[2] > ages[3], "{ages:?}");
}

/// Drawn with a shader of its own, it looks as that says.
#[test]
fn a_compute_particle_has_its_own_shader() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map_with_sims();
    built.effect_shaders.push(Arc::new(ShaderCode {
        name: "green".to_owned(),
        code: "fn shader_particle(look: ParticleLook) -> vec4<f32> {
    return vec4<f32>(0.0, 1.0, 0.0, 1.0);
}
"
        .to_owned(),
    }));
    renderer.load_map(Arc::new(built)).expect("load");
    let sims = [ParticleSim {
        speed: [0.0, 0.0],
        shader: 1,
        ..one_sim()
    }];
    let drawables = Drawables {
        particle_sims: &sims,
        ..Drawables::default()
    };
    let shot = renderer
        .capture(&view_from_above(), drawables, 64, 64)
        .expect("capture");
    let green = centre(&shot);
    assert!(green[1] > 200 && green[2] < 60, "{green:?}");
}

/// What goes out of the list stops, and its slots go to what comes next.
#[test]
fn an_emitter_that_ends_gives_its_slots_up() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map_with_sims())).expect("load");
    let _first = run_sim(&mut renderer, one_sim(), 2);
    let _none = renderer
        .capture(&view_from_above(), Drawables::default(), 32, 32)
        .expect("capture");
    assert!(renderer.read_sim(1).expect("reads").is_empty());
    let next = ParticleSim { id: 2, ..one_sim() };
    let slot = run_sim(&mut renderer, next, 2);
    assert!(slot[7] > 50.0, "new in its place: {slot:?}");
}

/// A compute particle draws where it is, and its colour is its own.
#[test]
fn a_compute_particle_shows() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map_with_sims())).expect("load");
    let sim = ParticleSim {
        speed: [0.0, 0.0],
        ..one_sim()
    };
    let sims = [sim];
    let drawables = Drawables {
        particle_sims: &sims,
        ..Drawables::default()
    };
    let shot = renderer
        .capture(&view_from_above(), drawables, 64, 64)
        .expect("capture");
    let blue = centre(&shot);
    assert!(blue[2] > 200, "{blue:?}");
}

/// What falls onto what the view can see comes off it.
#[test]
fn a_compute_particle_bounces_off_what_the_view_sees() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map_with_sims())).expect("load");
    let sim = ParticleSim {
        direction: Vec3::new(0.0, 0.0, -1.0),
        speed: [300.0, 300.0],
        bounce: 0.5,
        collide: 1,
        ..one_sim()
    };
    let slot = run_sim(&mut renderer, sim, 16);
    assert!(slot[2] > 0.0, "it stayed above the floor: {slot:?}");
    assert!(slot[6] > 0.0, "and it's going up: {slot:?}");
    assert!(
        (slot[6] - 150.0).abs() < 2.0,
        "with half its speed: {slot:?}"
    );
}

/// A level's own motion shader goes into the step, and can read a field.
#[test]
fn a_motion_shader_moves_what_it_is_on() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map_with_sims();
    built.effect_motions.push(Arc::new(ShaderCode {
        name: "sideways".to_owned(),
        code: "fn shader_motion(particle: SimParticle, seconds: f32) -> SimParticle {
    var out = particle;
    out.place.x = out.place.x + 60.0 * seconds + field_at(vec2<f32>(0.5)).x * 60.0 * seconds;
    return out;
}
"
        .to_owned(),
    }));
    built.fields.push(FieldDef {
        name: "grid".to_owned(),
        shader: Arc::new(ShaderCode {
            name: "grid".to_owned(),
            code: "fn shader_field(at: vec2<i32>, size: vec2<i32>) -> vec4<f32> {
    return vec4<f32>(1.0);
}
"
            .to_owned(),
        }),
        size: 4,
        rate: 1000.0,
        params: [0.0; 8],
    });
    renderer.load_map(Arc::new(built)).expect("load");
    let sim = ParticleSim {
        motion: 1,
        ..one_sim()
    };
    let without_field = run_sim(&mut renderer, sim, 31);
    assert!(without_field[0] - 64.0 > 25.0, "{without_field:?}");
    assert!(
        without_field[0] - 64.0 < 35.0,
        "and no field: {without_field:?}"
    );
    let next = ParticleSim {
        id: 2,
        field: 1,
        ..sim
    };
    // The field wants to have stepped, and capture's time comes from the view.
    let view = View {
        time: 1.0,
        ..view_from_above()
    };
    let _stepped = renderer
        .capture(&view, Drawables::default(), 32, 32)
        .expect("capture");
    let with_field = run_sim(&mut renderer, next, 31);
    assert!(with_field[0] - 64.0 > 50.0, "{with_field:?}");
    assert!(renderer.take_shader_problems().is_empty());
}

/// The slots of `sim` once all of it has started, in one step.
fn started(renderer: &mut Renderer, sim: ParticleSim) -> Vec<[f32; 8]> {
    let sims = [ParticleSim {
        spawn: sim.count,
        ..sim
    }];
    let drawables = Drawables {
        particle_sims: &sims,
        ..Drawables::default()
    };
    let view = view_from_above();
    let _shot = renderer.capture(&view, drawables, 32, 32).expect("capture");
    renderer.read_sim(sim.id).expect("reads")
}

/// Particles start in the shape they're told to, all of it.
#[test]
fn compute_starts_particles_in_their_shape() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map_with_sims())).expect("load");
    let many = ParticleSim {
        count: 512,
        origin: Vec3::ZERO,
        ..one_sim()
    };
    let in_box = started(
        &mut renderer,
        ParticleSim {
            id: 2,
            shape: 5,
            extent: Vec3::new(10.0, 20.0, 30.0),
            ..many
        },
    );
    for (axis, half) in [10.0, 20.0, 30.0].into_iter().enumerate() {
        let most = in_box.iter().map(|slot| slot[axis]).fold(-1e9, f32::max);
        let least = in_box.iter().map(|slot| slot[axis]).fold(1e9, f32::min);
        assert!(most <= half && most > half * 0.9, "{axis}: {most}");
        assert!(least >= -half && least < -half * 0.9, "{axis}: {least}");
    }
    let flat = in_box
        .iter()
        .filter(|slot| (slot[1] - slot[0] * 2.0).abs() > 1.0);
    assert!(flat.count() > 256, "and not along a line");
    let on_line = started(
        &mut renderer,
        ParticleSim {
            id: 3,
            shape: 4,
            extent: Vec3::new(10.0, 20.0, 0.0),
            ..many
        },
    );
    assert!(
        on_line
            .iter()
            .all(|slot| (slot[1] - slot[0] * 2.0).abs() < 0.01)
    );
    assert!(on_line.iter().any(|slot| slot[0] > 9.0), "to its end");
    assert!(on_line.iter().any(|slot| slot[0] < -9.0), "and the other");
    let in_cone = started(
        &mut renderer,
        ParticleSim {
            id: 4,
            shape: 3,
            radius: 10.0,
            extent: Vec3::new(0.0, 0.0, 40.0),
            ..many
        },
    );
    for slot in &in_cone {
        let out = slot[0].hypot(slot[1]);
        assert!(
            out <= slot[2] * 0.25 + 0.01,
            "it widens as it rises: {slot:?}"
        );
    }
    assert!(in_cone.iter().any(|slot| slot[2] > 36.0), "up to its top");
    let in_ring = started(
        &mut renderer,
        ParticleSim {
            id: 5,
            shape: 1,
            radius: 10.0,
            ..many
        },
    );
    assert!(
        in_ring
            .iter()
            .all(|slot| (slot[0].hypot(slot[1]) - 10.0).abs() < 0.01)
    );
    let in_disc = started(
        &mut renderer,
        ParticleSim {
            id: 6,
            shape: 2,
            radius: 10.0,
            ..many
        },
    );
    assert!(in_disc.iter().all(|slot| slot[2] == 0.0));
    assert!(
        in_disc.iter().any(|slot| slot[0].hypot(slot[1]) < 5.0),
        "and inside"
    );
    let in_ball = started(
        &mut renderer,
        ParticleSim {
            id: 7,
            shape: 0,
            radius: 10.0,
            ..many
        },
    );
    assert!(
        in_ball
            .iter()
            .all(|slot| Vec3::new(slot[0], slot[1], slot[2]).length() <= 10.01)
    );
    assert!(in_ball.iter().any(|slot| slot[2] < -5.0), "below as well");
}

/// What starts on a mesh starts at places the map holds, and those of each emitter are its own.
#[test]
fn compute_starts_particles_on_a_surface() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let up = Vec3::new(0.0, 0.0, 1.0);
    let side = Vec3::new(1.0, 0.0, 0.0);
    let point = |x: f32, facing: Vec3| SurfacePoint {
        place: Vec3::new(x, 0.0, 0.0),
        facing,
    };
    let mut built = map(1);
    built.effects.push(Arc::new(EffectDef {
        name: "sim".to_owned(),
        emitters: vec![
            ParticleEmitter {
                sim: true,
                surface: vec![point(5.0, up), point(6.0, up)],
                ..ParticleEmitter::default()
            },
            ParticleEmitter {
                sim: true,
                surface: vec![point(70.0, side)],
                ..ParticleEmitter::default()
            },
        ],
        timeline: Vec::new(),
    }));
    renderer.load_map(Arc::new(built)).expect("load");
    let many = ParticleSim {
        count: 64,
        origin: Vec3::ZERO,
        shape: 6,
        extent: Vec3::new(0.0, 2.0, 0.0),
        outward: true,
        ..one_sim()
    };
    let first = started(&mut renderer, many);
    assert!(first.iter().any(|slot| slot[0] == 5.0), "{first:?}");
    assert!(first.iter().any(|slot| slot[0] == 6.0), "{first:?}");
    assert!(
        first.iter().all(|slot| slot[0] < 7.0 && slot[6] > 59.0),
        "going up: {first:?}"
    );
    let second = ParticleSim {
        id: 2,
        extent: Vec3::new(2.0, 1.0, 0.0),
        ..many
    };
    let second = started(&mut renderer, second);
    assert!(second.iter().all(|slot| slot[0] == 70.0), "{second:?}");
    assert!(
        second.iter().all(|slot| slot[4] > 59.0),
        "going its own way: {second:?}"
    );
    let none = ParticleSim {
        id: 3,
        extent: Vec3::ZERO,
        ..many
    };
    let none = started(&mut renderer, none);
    assert!(none.iter().all(|slot| slot[0] == 0.0), "{none:?}");
}

/// Those told to go outward head away from the middle, whatever way the emitter's facing.
#[test]
fn compute_sends_particles_outward() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map_with_sims())).expect("load");
    let slots = started(
        &mut renderer,
        ParticleSim {
            count: 64,
            origin: Vec3::ZERO,
            shape: 1,
            radius: 10.0,
            outward: true,
            ..one_sim()
        },
    );
    for slot in slots {
        let out = Vec3::new(slot[0], slot[1], slot[2]).normalized();
        let moving = Vec3::new(slot[4], slot[5], slot[6]).normalized();
        assert!(out.dot(moving) > 0.999, "{slot:?}");
    }
}

/// Emitters at the same step don't throw the same way, as their seeds differ.
#[test]
fn what_starts_differs_from_slot_to_slot_and_step_to_step() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map_with_sims())).expect("load");
    let many = ParticleSim {
        count: 64,
        spread: -1.0,
        ..one_sim()
    };
    let one = started(&mut renderer, ParticleSim { seed: 1, ..many });
    let same = started(
        &mut renderer,
        ParticleSim {
            id: 2,
            seed: 1,
            ..many
        },
    );
    let other = started(
        &mut renderer,
        ParticleSim {
            id: 3,
            seed: 9,
            ..many
        },
    );
    assert_eq!(one, same, "a seed says all of it");
    let alike = one.iter().zip(&other).filter(|(a, b)| a == b).count();
    assert_eq!(alike, 0, "and another says otherwise");
    // Anything that's only moved along by a slot would show up here.
    let moved = one[1..].iter().zip(&other).filter(|(a, b)| a == b).count();
    assert_eq!(moved, 0);
}

/// What sticks stays where it hit, and what dies goes.
#[test]
fn a_compute_particle_sticks_or_dies_on_what_it_hits() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map_with_sims())).expect("load");
    let falling = ParticleSim {
        direction: Vec3::new(0.0, 0.0, -1.0),
        speed: [300.0, 300.0],
        gravity: 100.0,
        ..one_sim()
    };
    let stuck = run_sim(
        &mut renderer,
        ParticleSim {
            collide: 3,
            ..falling
        },
        30,
    );
    assert!(
        stuck[2] >= -2.0 && stuck[2] < 8.0,
        "on the floor: {stuck:?}"
    );
    assert!(stuck[3] < stuck[7], "and there still: {stuck:?}");
    let dead = run_sim(
        &mut renderer,
        ParticleSim {
            id: 2,
            collide: 2,
            ..falling
        },
        30,
    );
    assert!(dead[3] >= dead[7], "{dead:?}");
}

/// A particle has the colour its life has come to, between the ones it's told.
#[test]
fn a_compute_particle_goes_along_its_colours() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map_with_sims())).expect("load");
    let blue = [0.0, 0.0, 1.0, 1.0];
    let red = [1.0, 0.0, 0.0, 1.0];
    let sim = ParticleSim {
        speed: [0.0, 0.0],
        life: [1.0, 1.0],
        ramp: [blue, blue, blue, red, red, blue, blue, blue],
        ..one_sim()
    };
    let shade_at = |renderer: &mut Renderer, steps: u32, id: u64| {
        let sim = ParticleSim { id, ..sim };
        let _went = run_sim(renderer, sim, steps);
        let now = [ParticleSim {
            spawn: 0,
            seconds: 0.0,
            ..sim
        }];
        let drawables = Drawables {
            particle_sims: &now,
            ..Drawables::default()
        };
        centre(
            &renderer
                .capture(&view_from_above(), drawables, 64, 64)
                .expect("capture"),
        )
    };
    let young = shade_at(&mut renderer, 2, 2);
    assert!(young[2] > 200 && young[0] < 40, "{young:?}");
    let middle = shade_at(&mut renderer, 31, 3);
    assert!(middle[0] > 200 && middle[2] < 40, "{middle:?}");
    let old = shade_at(&mut renderer, 58, 4);
    assert!(old[2] > 200 && old[0] < 40, "{old:?}");
}

/// Sheets, turning and drawing out work on what compute runs as on the rest.
#[test]
fn a_compute_particle_has_a_frame_a_turn_and_a_length() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    // Two frames: red on its left, and blue on its right.
    let sheet = image_of(8, |x, _| {
        if x < 4 {
            [255, 0, 0, 255]
        } else {
            [0, 0, 255, 255]
        }
    });
    let mut built = map_with_sims();
    built.effect_pictures = vec![Arc::new(sheet)];
    renderer.load_map(Arc::new(built)).expect("load");
    let white = ParticleSim {
        speed: [0.0, 0.0],
        ramp: [[1.0; 4]; 8],
        sizes: [60.0; 8],
        picture: 1,
        sheet: [2, 1],
        flip_rate: 1.0,
        ..one_sim()
    };
    let shot_after = |renderer: &mut Renderer, sim: ParticleSim, steps: u32| {
        let _went = run_sim(renderer, sim, steps);
        let now = [ParticleSim {
            spawn: 0,
            seconds: 0.0,
            ..sim
        }];
        let drawables = Drawables {
            particle_sims: &now,
            ..Drawables::default()
        };
        renderer
            .capture(&view_from_above(), drawables, 64, 64)
            .expect("capture")
    };
    let first = centre(&shot_after(
        &mut renderer,
        ParticleSim { id: 2, ..white },
        2,
    ));
    assert!(first[0] > 150 && first[2] < 90, "{first:?}");
    let second = centre(&shot_after(
        &mut renderer,
        ParticleSim { id: 3, ..white },
        61,
    ));
    assert!(second[2] > 150 && second[0] < 90, "{second:?}");
    // Half red and half white, with its red turned to one side and then the other.
    let halves = image_of(8, |x, _| {
        if x < 4 {
            [255, 0, 0, 255]
        } else {
            [255, 255, 255, 0]
        }
    });
    let mut built = map_with_sims();
    built.effect_pictures = vec![Arc::new(halves)];
    renderer.load_map(Arc::new(built)).expect("load");
    let turning = ParticleSim {
        id: 4,
        sheet: [1, 1],
        spin: [std::f32::consts::PI, std::f32::consts::PI],
        flip_rate: 0.0,
        ..white
    };
    let red_side = |shot: &Capture| {
        let left = pixel_at(shot, 24, 32)[0].saturating_sub(pixel_at(shot, 24, 32)[1]);
        let right = pixel_at(shot, 40, 32)[0].saturating_sub(pixel_at(shot, 40, 32)[1]);
        (left, right)
    };
    let now = red_side(&shot_after(&mut renderer, turning, 2));
    let turned = red_side(&shot_after(
        &mut renderer,
        ParticleSim { id: 5, ..turning },
        61,
    ));
    assert!(now.0.abs_diff(now.1) > 60, "one side: {now:?}");
    assert!(
        (now.0 > now.1) != (turned.0 > turned.1) && turned.0.abs_diff(turned.1) > 60,
        "and the other: {now:?} and {turned:?}"
    );
    // Going up the view and drawn out along it, it's longer than it's wide.
    let mut built = map_with_sims();
    built.effect_pictures = Vec::new();
    renderer.load_map(Arc::new(built)).expect("load");
    let long = ParticleSim {
        id: 6,
        direction: Vec3::new(1.0, 0.0, 0.0),
        speed: [60.0, 60.0],
        sizes: [8.0; 8],
        ramp: [[0.0, 1.0, 0.0, 1.0]; 8],
        stretch: 1.0,
        picture: 0,
        origin: Vec3::new(60.0, 64.0, 20.0),
        flip_rate: 0.0,
        ..white
    };
    let shot = shot_after(&mut renderer, long, 6);
    let plain = renderer
        .capture(&view_from_above(), Drawables::default(), 64, 64)
        .expect("capture");
    let changed = |at: (u32, u32)| {
        let (was, is) = (pixel_at(&plain, at.0, at.1), pixel_at(&shot, at.0, at.1));
        was.iter().zip(is).any(|(a, b)| a.abs_diff(b) > 30)
    };
    let across = (0..64).filter(|x| changed((*x, 32))).count();
    let up = (0..64).filter(|y| changed((32, *y))).count();
    assert!(up > across * 3, "{up} along it and {across} across");
}

/// The depth a step reads was drawn from the last view, whatever the view has done since.
#[test]
fn a_step_reads_depth_as_it_was_seen() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map_with_sims())).expect("load");
    let sim = ParticleSim {
        origin: Vec3::new(64.0, 64.0, 2.0),
        direction: Vec3::new(0.0, 0.0, -1.0),
        speed: [300.0, 300.0],
        bounce: 1.0,
        collide: 1,
        ..one_sim()
    };
    // The floor's seen, and then the view's gone high above it.
    let plain = view_from_above();
    let high = View {
        origin: Vec3::new(64.0, 64.0, 2000.0),
        ..plain
    };
    let steps = [(&plain, 0, 0.0), (&plain, 1, 0.0), (&high, 0, 1.0 / 60.0)];
    for (view, spawn, seconds) in steps {
        let sims = [ParticleSim {
            spawn,
            seconds,
            ..sim
        }];
        let drawables = Drawables {
            particle_sims: &sims,
            ..Drawables::default()
        };
        let _shot = renderer.capture(view, drawables, 32, 32).expect("capture");
    }
    let slot = renderer.read_sim(sim.id).expect("reads")[0];
    assert!(slot[6] > 0.0, "it came off the floor: {slot:?}");
}

/// With another number of samples, what a level has of its own is made for those.
#[test]
fn a_change_of_samples_makes_a_levels_particles_again() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map_with_sims();
    built.effect_motions.push(Arc::new(ShaderCode {
        name: "still".to_owned(),
        code: "fn shader_motion(particle: SimParticle, seconds: f32) -> SimParticle {
    return particle;
}
"
        .to_owned(),
    }));
    built.effect_motions.push(Arc::new(ShaderCode {
        name: "wrong_motion".to_owned(),
        code: "this isn't code".to_owned(),
    }));
    built.effect_shaders.push(Arc::new(ShaderCode {
        name: "wrong_look".to_owned(),
        code: "this isn't code".to_owned(),
    }));
    renderer.load_map(Arc::new(built)).expect("load");
    let _told = renderer.take_shader_problems();
    renderer.set_antialiasing(4).expect("samples");
    let told = renderer.take_shader_problems().join(" ");
    assert!(told.contains("wrong_motion"), "made again: {told}");
    assert!(told.contains("wrong_look"), "both of them: {told}");
    let sim = ParticleSim {
        direction: Vec3::new(0.0, 0.0, -1.0),
        speed: [300.0, 300.0],
        bounce: 0.5,
        collide: 1,
        motion: 1,
        ..one_sim()
    };
    let slot = run_sim(&mut renderer, sim, 16);
    renderer.set_antialiasing(1).expect("samples");
    assert!(slot[6] > 0.0, "the depth it read had samples: {slot:?}");
}

/// A field that won't build takes its place, and the next is still the next.
#[test]
fn a_broken_field_leaves_the_next_where_it_was() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map_with_sims();
    let field = |name: &str, code: &str| FieldDef {
        name: name.to_owned(),
        shader: Arc::new(ShaderCode {
            name: name.to_owned(),
            code: code.to_owned(),
        }),
        size: 4,
        rate: 1000.0,
        params: [0.0; 8],
    };
    built.fields.push(field("broken", "this isn't code"));
    built.fields.push(field(
        "fine",
        "fn shader_field(at: vec2<i32>, size: vec2<i32>) -> vec4<f32> {
    return vec4<f32>(1.0);
}
",
    ));
    built.effect_motions.push(Arc::new(ShaderCode {
        name: "reads".to_owned(),
        code: "fn shader_motion(particle: SimParticle, seconds: f32) -> SimParticle {
    var out = particle;
    out.place.x = field_at(vec2<f32>(0.5)).x;
    return out;
}
"
        .to_owned(),
    }));
    renderer.load_map(Arc::new(built)).expect("load");
    let view = View {
        time: 1.0,
        ..view_from_above()
    };
    let _stepped = renderer
        .capture(&view, Drawables::default(), 32, 32)
        .expect("capture");
    let reader = ParticleSim {
        motion: 1,
        ..one_sim()
    };
    let none = run_sim(&mut renderer, ParticleSim { field: 1, ..reader }, 3);
    assert!(none[0].abs() < 0.01, "there's nothing there: {none:?}");
    let some = run_sim(
        &mut renderer,
        ParticleSim {
            id: 2,
            field: 2,
            ..reader
        },
        3,
    );
    assert!(
        (some[0] - 1.0).abs() < 0.01,
        "the second is the second: {some:?}"
    );
}

/// A motion that won't compile says so, and they go on as if it wasn't there.
#[test]
fn a_broken_motion_says_so_and_they_go_on() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map_with_sims();
    built.effect_motions.push(Arc::new(ShaderCode {
        name: "wrong".to_owned(),
        code: "this isn't code".to_owned(),
    }));
    renderer.load_map(Arc::new(built)).expect("load");
    let told = renderer.take_shader_problems().join(" ");
    assert!(told.contains("wrong"), "{told}");
    let slot = run_sim(
        &mut renderer,
        ParticleSim {
            motion: 1,
            ..one_sim()
        },
        31,
    );
    assert!((slot[2] - 50.0).abs() < 0.5, "{slot:?}");
}

/// Effects can change with a level loaded: their pictures and shaders, and that there's compute.
#[test]
fn effects_change_while_a_level_is_loaded() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let solid = |color: [u8; 4]| image_of(8, move |_, _| color);
    let mut before = map(1);
    before.effect_pictures = vec![Arc::new(solid([255, 0, 0, 255]))];
    renderer.load_map(Arc::new(before)).expect("load");
    let particle = ParticleDraw {
        picture: 1,
        ..particle_over(40.0, [255, 255, 255], 60.0)
    };
    let look = |renderer: &mut Renderer| {
        let particles = [particle];
        let drawables = Drawables {
            particles: &particles,
            ..Drawables::default()
        };
        centre(
            &renderer
                .capture(&view_from_above(), drawables, 64, 64)
                .expect("capture"),
        )
    };
    let red = look(&mut renderer);
    assert!(red[0] > 200 && red[1] < 40, "{red:?}");
    let mut after = map_with_sims();
    after.effect_pictures = vec![Arc::new(solid([0, 255, 0, 255]))];
    renderer.reload_effects(&after).expect("reloads");
    let green = look(&mut renderer);
    assert!(green[1] > 200 && green[0] < 40, "{green:?}");
    let slot = run_sim(&mut renderer, one_sim(), 3);
    assert!(slot[2] > 20.0, "there's compute now: {slot:?}");
    let without = Map {
        effect_pictures: vec![Arc::new(solid([0, 0, 255, 255]))],
        ..map(1)
    };
    renderer.reload_effects(&without).expect("reloads");
    assert!(
        renderer.read_sim(1).expect("reads").is_empty(),
        "and now not"
    );
    let blue = look(&mut renderer);
    assert!(blue[2] > 200 && blue[1] < 40, "{blue:?}");
}

/// A box of green fog, between the eye and the floor it looks at.
fn fog_box() -> FogVolume {
    FogVolume {
        low: Vec3::new(0.0, 0.0, 10.0),
        high: Vec3::new(128.0, 128.0, 50.0),
        color: [0.0, 1.0, 0.0],
        density: 0.2,
        drift: Vec3::ZERO,
        wisp_size: 16.0,
        ..FogVolume::default()
    }
}

/// The middle of the view of `volumes`, at `time`.
fn through_fog(
    renderer: &mut Renderer,
    volumes: &[FogVolume],
    time: f32,
    live: &[LiveLight],
) -> [u8; 4] {
    let mut built = map(1);
    built.fog_volumes = volumes.to_vec();
    renderer.load_map(Arc::new(built)).expect("load");
    let view = View {
        time,
        ..view_from_above()
    };
    let drawables = Drawables {
        live_lights: live,
        ..Drawables::default()
    };
    centre(&renderer.capture(&view, drawables, 64, 64).expect("capture"))
}

/// Fog's in its box, and not out of it.
#[test]
fn fog_is_where_it_says() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let clear = through_fog(&mut renderer, &[], 0.0, &[]);
    let fogged = through_fog(&mut renderer, &[fog_box()], 0.0, &[]);
    assert!(
        fogged[1] > 200 && fogged[0] < clear[0].max(40),
        "{fogged:?} from {clear:?}"
    );
    let aside = FogVolume {
        low: Vec3::new(500.0, 0.0, 10.0),
        high: Vec3::new(628.0, 128.0, 50.0),
        ..fog_box()
    };
    assert_eq!(
        through_fog(&mut renderer, &[aside], 0.0, &[]),
        clear,
        "it's over there"
    );
    let above = FogVolume {
        low: Vec3::new(0.0, 0.0, 100.0),
        high: Vec3::new(128.0, 128.0, 150.0),
        ..fog_box()
    };
    assert_eq!(
        through_fog(&mut renderer, &[above], 0.0, &[]),
        clear,
        "and that's behind the eye"
    );
}

/// Fog thick enough to hide everything shows the colour it says, as a colour on the screen.
#[test]
fn thick_fog_is_the_colour_it_says() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let grey = FogVolume {
        color: [0.5, 0.5, 0.5],
        density: 2.0,
        ..fog_box()
    };
    let shade = through_fog(&mut renderer, &[grey], 0.0, &[]);
    assert!(shade[1].abs_diff(128) < 10, "{shade:?}");
}

/// Soft sides take a volume in slowly.
#[test]
fn fog_thins_towards_its_sides() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let thin = FogVolume {
        density: 0.02,
        ..fog_box()
    };
    let hard = through_fog(&mut renderer, &[thin], 0.0, &[]);
    let soft = FogVolume { edge: 40.0, ..thin };
    let soft = through_fog(&mut renderer, &[soft], 0.0, &[]);
    assert!(soft[1] + 20 < hard[1], "{soft:?} and {hard:?}");
}

/// Wisps leave gaps, and go by with what drifts them.
#[test]
fn wisps_drift() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let wisps = FogVolume {
        density: 0.03,
        edge: 1.0,
        wisps: 1.0,
        drift: Vec3::new(40.0, 0.0, 0.0),
        ..fog_box()
    };
    let shots: Vec<[u8; 4]> = (0..8)
        .map(|step| through_fog(&mut renderer, &[wisps], step as f32 * 0.7, &[]))
        .collect();
    let least = shots.iter().map(|shot| shot[1]).min().unwrap_or(0);
    let most = shots.iter().map(|shot| shot[1]).max().unwrap_or(0);
    assert!(most > least + 25, "{shots:?}");
    let still = FogVolume {
        drift: Vec3::ZERO,
        ..wisps
    };
    let one = through_fog(&mut renderer, &[still], 0.0, &[]);
    let two = through_fog(&mut renderer, &[still], 5.0, &[]);
    assert_eq!(one, two, "what doesn't drift stays");
}

/// Lit fog is dark until something lights it.
#[test]
fn lit_fog_shows_what_goes_through_it() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let lit = FogVolume {
        lit: true,
        ..fog_box()
    };
    let dark = through_fog(&mut renderer, &[lit], 0.0, &[]);
    assert!(dark[1] < 40, "{dark:?}");
    let light = LiveLight {
        id: 1,
        origin: Vec3::new(64.0, 64.0, 30.0),
        radius: 200.0,
        color: [2.0, 2.0, 2.0],
        ..LiveLight::default()
    };
    let bright = through_fog(&mut renderer, &[lit], 0.0, &[light]);
    assert!(bright[1] > dark[1] + 80, "{bright:?} from {dark:?}");
    let far = LiveLight {
        origin: Vec3::new(4000.0, 64.0, 30.0),
        ..light
    };
    assert_eq!(
        through_fog(&mut renderer, &[lit], 0.0, &[far]),
        dark,
        "too far off"
    );
}

/// With more volumes than can be looked at, the nearest are.
#[test]
fn the_nearest_fog_is_what_shows() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let far = FogVolume {
        low: Vec3::new(5000.0, 0.0, 10.0),
        high: Vec3::new(5100.0, 128.0, 50.0),
        ..fog_box()
    };
    let mut volumes = vec![far; 12];
    volumes.push(fog_box());
    let shade = through_fog(&mut renderer, &volumes, 0.0, &[]);
    assert!(shade[1] > 200, "{shade:?}");
}

/// A look that's only a blue that gets thick with distance.
fn blue_look() -> PoolLook {
    PoolLook {
        color: [0.0, 0.0, 1.0],
        absorption: 0.02,
        caustics: 0.0,
        reflect: 0.0,
        waves: 0.0,
        refraction: 0.0,
        tint: [0.0; 4],
        deep: 0.0,
        view_effect: 0,
        overlays: [WaterOverlay::NONE; 2],
        foam: 0.0,
        foam_look: WaterOverlay::NONE,
        shore: 0.0,
        splash: 0,
        spray: 0,
        splash_sound: 0,
        trail: 0,
        trail_size: 28.0,
        rain_rings: 0.0,
    }
}

/// A sheet over the map's square, from `left` high along its low x to `right` along its high x.
fn sheet_over(left: f32, right: f32) -> WaterSheet {
    WaterSheet {
        origin: [-200.0, -200.0],
        cell: 400.0 / 4.0,
        size: [5, 5],
        heights: (0..25)
            .map(|at| left + (right - left) * ((at % 5) as f32 / 4.0))
            .collect(),
        depth: 400.0,
        contents: Contents::Water,
        look: blue_look(),
    }
}

/// The middle of the view, with `sheets` in the level.
fn over_sheets(renderer: &mut Renderer, sheets: Vec<WaterSheet>, view: &View) -> [u8; 4] {
    let mut built = map(1);
    built.water_sheets = sheets;
    renderer.load_map(Arc::new(built)).expect("load");
    centre(
        &renderer
            .capture(view, Drawables::default(), 64, 64)
            .expect("capture"),
    )
}

/// Looked down on, a sheet tints what's under it by how far the view goes in it.
#[test]
fn a_sheet_tints_what_is_under_it() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let view = view_from_above();
    let plain = over_sheets(&mut renderer, Vec::new(), &view);
    let shallow = over_sheets(&mut renderer, vec![sheet_over(10.0, 10.0)], &view);
    let deep = over_sheets(&mut renderer, vec![sheet_over(50.0, 50.0)], &view);
    assert!(shallow[2] > plain[2], "{shallow:?} from {plain:?}");
    assert!(deep[2] > shallow[2] + 20, "{deep:?} and {shallow:?}");
    let under = over_sheets(&mut renderer, vec![sheet_over(-10.0, -10.0)], &view);
    assert_eq!(under, plain, "under the floor it's not seen");
}

/// A sloping sheet is as deep as it is where the view meets it.
#[test]
fn a_sheet_goes_by_its_own_height_where_it_is_seen() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let view = view_from_above();
    let flat = over_sheets(&mut renderer, vec![sheet_over(30.0, 30.0)], &view);
    // It's at 30 at the middle of the map's square, by going from 62 down to -2 over 400 units
    // from -200, with the middle of the view at 64.
    let sloped = sheet_over(30.0 + 264.0 * 0.16, 30.0 - 136.0 * 0.16);
    let sloped = over_sheets(&mut renderer, vec![sloped], &view);
    assert!(flat[2].abs_diff(sloped[2]) < 12, "{flat:?} and {sloped:?}");
}

/// With the eye in one, what's seen goes through it from the eye.
#[test]
fn an_eye_in_a_sheet_sees_through_it() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let view = view_from_above();
    let above = over_sheets(&mut renderer, vec![sheet_over(50.0, 50.0)], &view);
    let inside = over_sheets(&mut renderer, vec![sheet_over(100.0, 100.0)], &view);
    // It's 64 units of water to the floor, against 50 from above.
    assert!(inside[0] + 15 < above[0], "{inside:?} and {above:?}");
    assert!(inside[2] > above[2], "{inside:?} and {above:?}");
}

/// Fog stops at a sheet, as it does at a pool.
#[test]
fn fog_stops_at_a_sheet() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let green = FogVolume {
        low: Vec3::new(0.0, 0.0, 1.0),
        high: Vec3::new(128.0, 128.0, 60.0),
        color: [0.0, 1.0, 0.0],
        density: 0.02,
        ..FogVolume::default()
    };
    let fog_over = |renderer: &mut Renderer, sheets: Vec<WaterSheet>| {
        let mut built = map(1);
        built.fog_volumes = vec![green];
        built.water_sheets = sheets;
        renderer.load_map(Arc::new(built)).expect("load");
        centre(
            &renderer
                .capture(&view_from_above(), Drawables::default(), 64, 64)
                .expect("capture"),
        )
    };
    let all = fog_over(&mut renderer, Vec::new());
    let cut = fog_over(&mut renderer, vec![sheet_over(50.0, 50.0)]);
    assert!(cut[1] + 25 < all[1], "{cut:?} and {all:?}");
}

/// More bodies than can be looked at are taken nearest first, sheets among them.
#[test]
fn sheets_take_their_turn_among_the_pools() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map(1);
    for at in 0..12 {
        built.pools.push(Pool {
            low: Vec3::new(9000.0 + at as f32 * 100.0, 0.0, 0.0),
            high: Vec3::new(9050.0 + at as f32 * 100.0, 50.0, 50.0),
            look: blue_look(),
        });
    }
    built.water_sheets = vec![sheet_over(50.0, 50.0)];
    renderer.load_map(Arc::new(built)).expect("load");
    let many = centre(
        &renderer
            .capture(&view_from_above(), Drawables::default(), 64, 64)
            .expect("capture"),
    );
    let one = over_sheets(
        &mut renderer,
        vec![sheet_over(50.0, 50.0)],
        &view_from_above(),
    );
    assert_eq!(many, one);
}

/// The middle of the view over a sheet `high` above the floor, of `look`, with `pictures`.
fn over_look(
    renderer: &mut Renderer,
    look: PoolLook,
    high: f32,
    pictures: Vec<Vec<MipLevel>>,
    time: f32,
) -> [u8; 4] {
    let mut built = map(1);
    built.water_sheets = vec![WaterSheet {
        look,
        ..sheet_over(high, high)
    }];
    built.water_pictures = pictures.into_iter().map(Arc::new).collect();
    renderer.load_map(Arc::new(built)).expect("load");
    let view = View {
        time,
        ..view_from_above()
    };
    centre(
        &renderer
            .capture(&view, Drawables::default(), 64, 64)
            .expect("capture"),
    )
}

/// A picture lies over a surface, as much of it as it says.
#[test]
fn a_surface_can_have_a_picture_over_it() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let green = image_of(8, |_, _| [0, 255, 0, 255]);
    let plain = blue_look();
    let bare = over_look(&mut renderer, plain, 30.0, vec![green.clone()], 0.0);
    let over = |strength: f32| PoolLook {
        overlays: [
            WaterOverlay {
                picture: 1,
                size: 64.0,
                flow: [0.0; 2],
                strength,
            },
            WaterOverlay::NONE,
        ],
        ..plain
    };
    let some = over_look(&mut renderer, over(0.5), 30.0, vec![green.clone()], 0.0);
    let all = over_look(&mut renderer, over(0.999), 30.0, vec![green], 0.0);
    assert!(some[1] > bare[1] + 40, "{some:?} from {bare:?}");
    assert!(all[1] > some[1] + 30, "{all:?} and {some:?}");
    assert!(all[0] < 40 && all[2] < 40, "{all:?}");
}

/// Both overlays show, and a picture goes the way it flows.
#[test]
fn what_is_on_a_surface_flows() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    // Green on the left half, and nothing on the right.
    let half = image_of(8, |x, _| {
        if x < 4 {
            [0, 255, 0, 255]
        } else {
            [0, 255, 0, 0]
        }
    });
    let second = |flow: [f32; 2]| PoolLook {
        overlays: [
            WaterOverlay::NONE,
            WaterOverlay {
                picture: 1,
                size: 64.0,
                flow,
                strength: 0.999,
            },
        ],
        ..blue_look()
    };
    let shades: Vec<u8> = (0..8)
        .map(|step| {
            let look = second([16.0, 0.0]);
            over_look(
                &mut renderer,
                look,
                30.0,
                vec![half.clone()],
                step as f32 * 0.5,
            )[1]
        })
        .collect();
    let least = shades.iter().copied().min().unwrap_or(0);
    let most = shades.iter().copied().max().unwrap_or(0);
    assert!(most > least + 80, "{shades:?}");
    let still = second([0.0, 0.0]);
    let one = over_look(&mut renderer, still, 30.0, vec![half.clone()], 0.0);
    let two = over_look(&mut renderer, still, 30.0, vec![half], 3.0);
    assert_eq!(one, two);
}

/// Foam's where it's shallow, and not where it's deep.
#[test]
fn foam_goes_by_how_shallow_it_is() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let solid = image_of(8, |_, _| [255, 255, 255, 255]);
    let foamy = PoolLook {
        foam: 20.0,
        foam_look: WaterOverlay {
            picture: 1,
            size: 64.0,
            flow: [0.0; 2],
            strength: 0.999,
        },
        ..blue_look()
    };
    let shallow = over_look(&mut renderer, foamy, 2.0, vec![solid.clone()], 0.0);
    let deep = over_look(&mut renderer, foamy, 50.0, vec![solid.clone()], 0.0);
    let plain = over_look(&mut renderer, blue_look(), 50.0, vec![solid], 0.0);
    assert!(shallow[1] > 150, "{shallow:?}");
    assert_eq!(deep, plain, "it doesn't reach");
}

/// With no picture foam has a shape of its own, that comes and goes.
#[test]
fn foam_needs_no_picture() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let foamy = PoolLook {
        foam: 20.0,
        ..blue_look()
    };
    let shades: Vec<u8> = (0..8)
        .map(|step| over_look(&mut renderer, foamy, 2.0, Vec::new(), step as f32 * 1.3)[1])
        .collect();
    let least = shades.iter().copied().min().unwrap_or(0);
    let most = shades.iter().copied().max().unwrap_or(0);
    assert!(most > least + 40, "{shades:?}");
}

/// A shore fades into what's under it, so the edge of water doesn't show as a line.
#[test]
fn water_fades_out_at_the_shore() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let dry = over_sheets(&mut renderer, Vec::new(), &view_from_above());
    let dark = PoolLook {
        absorption: 0.2,
        ..blue_look()
    };
    let hard = over_look(&mut renderer, dark, 5.0, Vec::new(), 0.0);
    let soft = PoolLook {
        shore: 100.0,
        ..dark
    };
    let soft = over_look(&mut renderer, soft, 5.0, Vec::new(), 0.0);
    let away = |shot: [u8; 4]| shot[2].abs_diff(dry[2]);
    assert!(
        away(soft) * 3 < away(hard),
        "{soft:?} and {hard:?} from {dry:?}"
    );
    let deep = PoolLook { shore: 2.0, ..dark };
    let deep = over_look(&mut renderer, deep, 5.0, Vec::new(), 0.0);
    assert_eq!(deep, hard, "and past that it's as it was");
}

/// Runs ripples on for `steps` steps, from `from`, with `pushes` at the first.
fn step_ripples(renderer: &mut Renderer, view: &View, from: u32, steps: u32, pushes: &[WaterPush]) {
    for step in from..from + steps {
        let now = View {
            // Half way through a step, so that rounding can't lose one.
            time: 1.0 + (step as f32 + 0.5) / 60.0,
            ..*view
        };
        let drawables = Drawables {
            water_pushes: if step == from { pushes } else { &[] },
            ..Drawables::default()
        };
        let _shot = renderer.capture(&now, drawables, 32, 32).expect("capture");
    }
}

/// How high the ripples are at a place, and the most anywhere.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a place inside the field, which is a few hundred cells"
)]
fn ripple_at(renderer: &mut Renderer, x: f32, y: f32) -> (f32, f32) {
    let (heights, [left, low, width]) = renderer.read_ripples().expect("reads").expect("there");
    let cells = heights.len().isqrt();
    let cell = width / cells as f32;
    let at = ((y - low) / cell) as usize * cells + ((x - left) / cell) as usize;
    let most = heights
        .iter()
        .copied()
        .fold(0.0, |most: f32, height| most.max(height.abs()));
    (heights[at], most)
}

/// A push goes out as a ring, and dies away.
#[test]
fn a_push_goes_out_as_a_ring_and_dies_away() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map(1);
    built.water_sheets = vec![sheet_over(30.0, 30.0)];
    renderer.load_map(Arc::new(built)).expect("load");
    let view = view_from_above();
    step_ripples(&mut renderer, &view, 0, 3, &[]);
    let (_, flat) = ripple_at(&mut renderer, 64.0, 64.0);
    assert_eq!(flat, 0.0, "nothing's touched it");
    let push = WaterPush {
        place: Vec3::new(64.0, 64.0, 30.0),
        reach: 12.0,
        strength: 2.0,
        ..WaterPush::default()
    };
    step_ripples(&mut renderer, &view, 3, 1, &[push]);
    let (middle, _) = ripple_at(&mut renderer, 64.0, 64.0);
    assert!(middle < -0.5, "it's pushed down: {middle}");
    let (aside, _) = ripple_at(&mut renderer, 120.0, 64.0);
    assert!(aside.abs() < 0.01, "and nothing's there yet: {aside}");
    step_ripples(&mut renderer, &view, 4, 40, &[]);
    let (aside, soon) = ripple_at(&mut renderer, 100.0, 64.0);
    assert!(soon > 0.0 && soon < 2.0, "{soon}");
    let _ = aside;
    let mut crossed = 0;
    for step in 0..30 {
        step_ripples(&mut renderer, &view, 44 + step, 1, &[]);
        if ripple_at(&mut renderer, 110.0, 64.0).0.abs() > 0.002 {
            crossed += 1;
        }
    }
    assert!(crossed > 0, "it reached out to here");
    step_ripples(&mut renderer, &view, 80, 400, &[]);
    let (_, late) = ripple_at(&mut renderer, 64.0, 64.0);
    assert!(late < soon * 0.3, "{late} from {soon}");
}

/// How much is going on in the ripples, all told.
fn ripple_energy(renderer: &mut Renderer) -> f32 {
    let (heights, _) = renderer.read_ripples().expect("reads").expect("there");
    heights.iter().map(|height| height * height).sum()
}

/// What's in the water dies away, and doesn't only spread out.
#[test]
fn ripples_lose_what_they_have() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map(1);
    built.water_sheets = vec![sheet_over(30.0, 30.0)];
    renderer.load_map(Arc::new(built)).expect("load");
    let view = view_from_above();
    step_ripples(&mut renderer, &view, 0, 3, &[]);
    let push = WaterPush {
        place: Vec3::new(64.0, 64.0, 30.0),
        reach: 12.0,
        strength: 2.0,
        ..WaterPush::default()
    };
    step_ripples(&mut renderer, &view, 3, 20, &[push]);
    let soon = ripple_energy(&mut renderer);
    step_ripples(&mut renderer, &view, 23, 150, &[]);
    let later = ripple_energy(&mut renderer);
    assert!(soon > 0.0 && later < soon * 0.2, "{later} from {soon}");
}

/// Frames come faster than steps, and what pushes on one with no step isn't lost.
#[test]
fn a_push_waits_for_a_step() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map(1);
    built.water_sheets = vec![sheet_over(30.0, 30.0)];
    renderer.load_map(Arc::new(built)).expect("load");
    let view = view_from_above();
    step_ripples(&mut renderer, &view, 0, 3, &[]);
    let push = [WaterPush {
        place: Vec3::new(64.0, 64.0, 30.0),
        reach: 12.0,
        strength: 2.0,
        ..WaterPush::default()
    }];
    // Still in the step the last frame was in, so there's none due.
    let same = View {
        time: 1.0 + 2.6 / 60.0,
        ..view
    };
    let drawables = Drawables {
        water_pushes: &push,
        ..Drawables::default()
    };
    let _shot = renderer.capture(&same, drawables, 32, 32).expect("capture");
    assert_eq!(ripple_at(&mut renderer, 64.0, 64.0).0, 0.0, "not yet");
    step_ripples(&mut renderer, &view, 3, 1, &[]);
    let (middle, _) = ripple_at(&mut renderer, 64.0, 64.0);
    assert!(middle < -0.5, "{middle}");
}

/// They're round the view, wherever that is.
#[test]
fn ripples_go_with_the_view() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map(1);
    built.water_sheets = vec![sheet_over(30.0, 30.0)];
    renderer.load_map(Arc::new(built)).expect("load");
    let far = View {
        origin: Vec3::new(5000.0, -3000.0, 64.0),
        ..view_from_above()
    };
    step_ripples(&mut renderer, &far, 0, 3, &[]);
    let (_, [left, low, width]) = renderer.read_ripples().expect("reads").expect("there");
    assert!((left + width * 0.5 - 5000.0).abs() < 4.0, "{left} {width}");
    assert!((low + width * 0.5 + 3000.0).abs() < 4.0, "{low} {width}");
}

/// Ripples stay where they are in the world as the view goes by.
#[test]
fn ripples_stay_put_as_the_view_moves() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map(1);
    built.water_sheets = vec![sheet_over(30.0, 30.0)];
    renderer.load_map(Arc::new(built)).expect("load");
    let view = view_from_above();
    step_ripples(&mut renderer, &view, 0, 3, &[]);
    let push = WaterPush {
        place: Vec3::new(64.0, 64.0, 30.0),
        reach: 12.0,
        strength: 2.0,
        ..WaterPush::default()
    };
    step_ripples(&mut renderer, &view, 3, 1, &[push]);
    let before = ripple_at(&mut renderer, 64.0, 64.0).0;
    let moved = View {
        origin: Vec3::new(164.0, 64.0, 64.0),
        ..view
    };
    step_ripples(&mut renderer, &moved, 4, 1, &[]);
    let after = ripple_at(&mut renderer, 64.0, 64.0).0;
    assert!(after < before * 0.5, "{after} from {before}");
}

/// A trail pushes as its picture, turned to how it's going.
#[test]
fn a_wake_goes_the_way_it_is_going() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    // Nothing on the left of the picture, and all on the right.
    let half = image_of(8, |x, _| {
        if x < 4 {
            [255, 255, 255, 0]
        } else {
            [255, 255, 255, 255]
        }
    });
    let mut built = map(1);
    built.water_sheets = vec![sheet_over(30.0, 30.0)];
    built.water_pictures = vec![Arc::new(half)];
    renderer.load_map(Arc::new(built)).expect("load");
    let view = view_from_above();
    step_ripples(&mut renderer, &view, 0, 3, &[]);
    let push = WaterPush {
        place: Vec3::new(64.0, 64.0, 30.0),
        reach: 30.0,
        strength: 2.0,
        turn: 0.0,
        picture: 1,
    };
    step_ripples(&mut renderer, &view, 3, 1, &[push]);
    let (left, _) = ripple_at(&mut renderer, 64.0, 44.0);
    let (right, _) = ripple_at(&mut renderer, 64.0, 84.0);
    assert!(
        left.abs() < 0.05 && right < -1.0 || right.abs() < 0.05 && left < -1.0,
        "{left} and {right}"
    );
    let turned = WaterPush {
        turn: std::f32::consts::FRAC_PI_2,
        ..push
    };
    let mut built = map(1);
    let solid = image_of(8, |x, _| {
        if x < 4 {
            [255, 255, 255, 0]
        } else {
            [255, 255, 255, 255]
        }
    });
    built.water_sheets = vec![sheet_over(30.0, 30.0)];
    built.water_pictures = vec![Arc::new(solid)];
    renderer.load_map(Arc::new(built)).expect("load");
    step_ripples(&mut renderer, &view, 0, 3, &[]);
    step_ripples(&mut renderer, &view, 3, 1, &[turned]);
    let (near, _) = ripple_at(&mut renderer, 44.0, 64.0);
    let (far, _) = ripple_at(&mut renderer, 84.0, 64.0);
    assert!(
        near.abs() < 0.05 && far < -1.0 || far.abs() < 0.05 && near < -1.0,
        "{near} and {far}"
    );
}

/// A surface shows what's going through it: here in what it mirrors, as nothing else is there.
#[test]
fn ripples_show_on_a_surface() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let still = wall_over_a_pool(&mut renderer, 1.0);
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
    let push = WaterPush {
        place: Vec3::new(0.0, 40.0, 1.0),
        reach: 30.0,
        strength: 4.0,
        ..WaterPush::default()
    };
    let mut shaken = None;
    for step in 0..8 {
        let now = View {
            time: 1.0 + (step as f32 + 0.5) / 60.0,
            ..view
        };
        let drawables = Drawables {
            water_pushes: if step == 1 { &[push] } else { &[] },
            ..with_instance(&instances)
        };
        shaken = Some(
            renderer
                .capture(&now, drawables, 128, 128)
                .expect("capture"),
        );
    }
    let shaken = shaken.expect("a picture");
    let different = (0..128)
        .flat_map(|y| (0..128).map(move |x| (x, y)))
        .filter(|(x, y)| pixel_at(&still, *x, *y) != pixel_at(&shaken, *x, *y))
        .count();
    assert!(different > 50, "{different}");
}

/// A level with no water has nothing to step.
#[test]
fn a_dry_level_has_no_ripples() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    assert!(renderer.read_ripples().expect("reads").is_none());
    let _shot = renderer
        .capture(&view_from_above(), Drawables::default(), 32, 32)
        .expect("capture");
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

/// A floor with a wall standing across it at x = 64, facing back towards x = 0, lit by `style`,
/// and with as much darkening in corners as `strength`.
fn creased_map(strength: f32, style: u8) -> Map {
    let mut built = map(1);
    built.look.ssao = strength;
    built.look.ssao_radius = 32.0;
    // Dim enough that, darkened, it doesn't stay at the top.
    built.light = vec![[100, 80, 30]; 81];
    let mut wall = built.faces[0].clone();
    wall.first_point = 4;
    wall.axes[0].axis = Vec3::new(0.0, 1.0, 0.0);
    wall.axes[1].axis = Vec3::new(0.0, 0.0, 1.0);
    built.points.extend([
        Vec3::new(64.0, 0.0, 0.0),
        Vec3::new(64.0, 0.0, 128.0),
        Vec3::new(64.0, 128.0, 128.0),
        Vec3::new(64.0, 128.0, 0.0),
    ]);
    built.faces.push(wall);
    for face in &mut built.faces {
        if let Some(light) = &mut face.light {
            light.styles = [style, 255, 255, 255];
        }
    }
    built.tree.leaves[1].face_count = 2;
    built.tree.leaf_faces.push(1);
    built.models[0].face_count = 2;
    built
}

/// Looking down at the crease from the floor's side, with it across the middle.
fn at_the_crease() -> View {
    View {
        origin: Vec3::new(16.0, 64.0, 48.0),
        pitch: 45.0,
        ..view_from_above()
    }
}

/// A picture of the crease in `built`, with `ssao` as the player's setting.
fn crease_picture(built: Map, ssao: u8, drawables: Drawables<'_>) -> Option<Capture> {
    let mut renderer = renderer()?;
    renderer.load_map(Arc::new(built)).expect("load");
    renderer.set_post(PostSettings {
        ssao,
        ..PostSettings::default()
    });
    Some(
        renderer
            .capture(&at_the_crease(), drawables, 64, 64)
            .expect("capture"),
    )
}

/// The floor just short of the crease.
const BY_THE_CREASE: (u32, u32) = (32, 34);

/// Where the floor is too far from the wall for it to shade.
const OPEN_FLOOR: (u32, u32) = (32, 63);

/// A crease darkens what's lit from all round, at either setting, and open floor stays as it is.
#[test]
fn a_crease_darkens_and_open_floor_does_not() {
    let (x, y) = BY_THE_CREASE;
    let (open_x, open_y) = OPEN_FLOOR;
    for setting in [1, 2] {
        let off = crease_picture(creased_map(1.0, 0), 0, Drawables::default());
        let on = crease_picture(creased_map(1.0, 0), setting, Drawables::default());
        if let (Some(off), Some(on)) = (off, on) {
            let (near_off, near_on) = (pixel(&off, x, y), pixel(&on, x, y));
            assert!(
                near_on[1] + 20 < near_off[1],
                "{setting}: {near_off:?} {near_on:?}"
            );
            assert_eq!(
                pixel(&on, open_x, open_y),
                pixel(&off, open_x, open_y),
                "{setting}"
            );
        }
    }
}

/// A level that has none has none, whatever the player's setting.
#[test]
fn a_level_with_no_ssao_has_none() {
    let (x, y) = BY_THE_CREASE;
    let off = crease_picture(creased_map(1.0, 0), 0, Drawables::default());
    let none = crease_picture(creased_map(0.0, 0), 2, Drawables::default());
    if let (Some(off), Some(none)) = (off, none) {
        assert_eq!(pixel(&none, x, y), pixel(&off, x, y));
    }
}

/// How much less a level's strength darkens than all of it.
#[test]
fn a_levels_strength_says_how_much() {
    let (x, y) = BY_THE_CREASE;
    let off = crease_picture(creased_map(1.0, 0), 0, Drawables::default());
    let some = crease_picture(creased_map(0.4, 0), 2, Drawables::default());
    let all = crease_picture(creased_map(1.0, 0), 2, Drawables::default());
    if let (Some(off), Some(some), Some(all)) = (off, some, all) {
        let green = |shot: &Capture| pixel(shot, x, y)[1];
        assert!(
            green(&some) < green(&off),
            "{} {}",
            green(&off),
            green(&some)
        );
        assert!(
            green(&all) + 10 < green(&some),
            "{} {}",
            green(&some),
            green(&all)
        );
    }
}

/// A sun's own light comes from one way and has its own shadows, so corners leave it alone,
/// baked or live.
#[test]
fn corners_leave_the_suns_own_light_alone() {
    let (x, y) = BY_THE_CREASE;
    let off = crease_picture(creased_map(1.0, SUN_STYLE), 0, Drawables::default());
    let on = crease_picture(creased_map(1.0, SUN_STYLE), 2, Drawables::default());
    if let (Some(off), Some(on)) = (off, on) {
        assert!(pixel(&off, x, y)[0] > 40, "lit by its baked sun");
        assert_eq!(pixel(&on, x, y), pixel(&off, x, y), "baked");
    }
    let unlit = || {
        let mut built = creased_map(1.0, 0);
        built.light = vec![[0, 0, 0]; 81];
        built
    };
    // Dim, so that darkened it wouldn't stay at the top.
    let sun = Some(SunDraw {
        toward: Vec3::new(-0.6, 0.0, 0.8),
        color: [0.0, 0.0, 0.3],
    });
    let drawables = Drawables {
        sun,
        ..Drawables::default()
    };
    let (off, on) = (
        crease_picture(unlit(), 0, drawables),
        crease_picture(unlit(), 2, drawables),
    );
    if let (Some(off), Some(on)) = (off, on) {
        assert!(pixel(&off, x, y)[2] > 40, "lit by a live sun");
        assert!(pixel(&off, x, y)[2] < 200, "not at the top");
        assert_eq!(pixel(&on, x, y), pixel(&off, x, y), "live");
    }
}

/// Light a day adds everywhere is from all round, so corners darken it too.
#[test]
fn corners_darken_a_days_light() {
    let (x, y) = BY_THE_CREASE;
    let unlit = || {
        let mut built = creased_map(1.0, 0);
        built.light = vec![[0, 0, 0]; 81];
        built
    };
    let drawables = Drawables {
        day_light: Some([0.0, 0.3, 0.0, 1.0]),
        ..Drawables::default()
    };
    let (off, on) = (
        crease_picture(unlit(), 0, drawables),
        crease_picture(unlit(), 2, drawables),
    );
    if let (Some(off), Some(on)) = (off, on) {
        let (off, on) = (pixel(&off, x, y), pixel(&on, x, y));
        assert!(on[1] + 20 < off[1], "{off:?} {on:?}");
    }
}

/// Turning it off takes it away, and leaves nothing of what was worked out before.
#[test]
fn turning_ssao_off_takes_it_away() {
    let fresh = crease_picture(creased_map(1.0, 0), 0, Drawables::default());
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer
        .load_map(Arc::new(creased_map(1.0, 0)))
        .expect("load");
    let mut shot = |ssao| {
        renderer.set_post(PostSettings {
            ssao,
            ..PostSettings::default()
        });
        renderer
            .capture(&at_the_crease(), Drawables::default(), 64, 64)
            .expect("capture")
    };
    let (on, off) = (shot(2), shot(0));
    let (x, y) = BY_THE_CREASE;
    if let Some(fresh) = fresh {
        assert_ne!(pixel(&on, x, y), pixel(&fresh, x, y), "it was on");
        assert_eq!(pixel(&off, x, y), pixel(&fresh, x, y));
    }
}

/// What's far in front of a point doesn't shade it, however much of what it looks round it
/// hides: a ledge high over a floor leaves the floor open.
#[test]
fn whats_far_in_front_does_not_shade() {
    let ledged = || {
        let mut built = creased_map(1.0, 0);
        built.look.ssao_radius = 8.0;
        let mut ledge = built.faces[0].clone();
        ledge.first_point = 8;
        built.points.extend([
            Vec3::new(17.0, 0.0, 40.0),
            Vec3::new(20.0, 0.0, 40.0),
            Vec3::new(20.0, 128.0, 40.0),
            Vec3::new(17.0, 128.0, 40.0),
        ]);
        built.faces.push(ledge);
        built.tree.leaves[1].face_count = 3;
        built.tree.leaf_faces.push(2);
        built.models[0].face_count = 3;
        built
    };
    let off = crease_picture(ledged(), 0, Drawables::default());
    let on = crease_picture(ledged(), 2, Drawables::default());
    if let (Some(off), Some(on)) = (off, on) {
        let rows = 44..64;
        let ledge_shown = rows
            .clone()
            .any(|y| pixel(&off, 32, y) != pixel(&off, 32, 63));
        assert!(ledge_shown, "the ledge is in view");
        let differ = rows
            .flat_map(|y| (0..64).map(move |x| (x, y)))
            .filter(|&(x, y)| pixel(&on, x, y) != pixel(&off, x, y))
            .count();
        assert_eq!(differ, 0);
    }
}

/// What's see-through wasn't seen when corners were worked out, so what's behind it doesn't
/// darken it.
#[test]
fn whats_see_through_is_not_darkened_by_whats_behind() {
    let (x, y) = BY_THE_CREASE;
    let glassy = || {
        let mut built = creased_map(1.0, 0);
        let mut glass = built.textures[0].clone();
        glass.material = Some(Arc::new(Material {
            alpha: AlphaMode::Blend,
            ..Material::default()
        }));
        built.textures.push(glass);
        let mut sheet = built.faces[0].clone();
        sheet.first_point = 8;
        sheet.texture = 1;
        built.points.extend([
            Vec3::new(0.0, 0.0, 8.0),
            Vec3::new(128.0, 0.0, 8.0),
            Vec3::new(128.0, 128.0, 8.0),
            Vec3::new(0.0, 128.0, 8.0),
        ]);
        built.faces.push(sheet);
        built.tree.leaves[1].face_count = 3;
        built.tree.leaf_faces.push(2);
        built.models[0].face_count = 3;
        built
    };
    let off = crease_picture(glassy(), 0, Drawables::default());
    let on = crease_picture(glassy(), 2, Drawables::default());
    if let (Some(off), Some(on)) = (off, on) {
        assert_eq!(pixel(&on, x, y), pixel(&off, x, y));
    }
}

/// The square with its sun's layer holding `bits`: the angles along an arc the sun reaches it at.
fn arc_square(models: u32, bits: u32) -> Map {
    let mut built = map(models);
    let [low, middle, high, top] = bits.to_le_bytes();
    built.light = vec![[low, middle, high]; 81];
    built.light_edges = vec![top; 81];
    for face in &mut built.faces {
        if let Some(light) = &mut face.light {
            light.styles = [SUN_STYLE, 255, 255, 255];
        }
    }
    built
}

/// A dim blue sun overhead, at `arc` along its arc if it's on one.
fn arc_drawables(color: [f32; 3], arc: Option<f32>) -> Drawables<'static> {
    Drawables {
        sun: Some(SunDraw {
            toward: Vec3::new(0.0, 0.0, 1.0),
            color,
        }),
        sun_arc: arc,
        ..Drawables::default()
    }
}

/// The middle of the square with `bits`, under a dim blue sun at `arc`.
fn under_arc(bits: u32, arc: Option<f32>) -> Option<[u8; 4]> {
    let drawables = arc_drawables([0.0, 0.0, 0.4], arc);
    centre_of(arc_square(1, bits), &view_from_above(), drawables)
}

/// A sun on an arc lights what its bits say it reaches where it is along the arc, all four bytes
/// of them, and not what they say it doesn't.
#[test]
fn a_sun_on_an_arc_lights_where_its_bits_say() {
    let bits = 0xff00_ffff;
    let (early, middle, late) = (
        under_arc(bits, Some(5.0)),
        under_arc(bits, Some(20.0)),
        under_arc(bits, Some(28.0)),
    );
    let none = under_arc(0, Some(5.0));
    if let (Some(early), Some(middle), Some(late), Some(none)) = (early, middle, late, none) {
        assert!(early[2] > none[2] + 30, "{early:?} {none:?}");
        assert!(late[2] > none[2] + 30, "{late:?} {none:?}");
        assert_eq!(middle, none);
    }
}

/// Between two angles, it's between what they say, so a shadow's edge moves smoothly.
#[test]
fn between_angles_an_arc_blends() {
    let bits = 0x0000_ffff;
    let (lit, half, dark) = (
        under_arc(bits, Some(15.0)),
        under_arc(bits, Some(15.5)),
        under_arc(bits, Some(16.0)),
    );
    if let (Some(lit), Some(half), Some(dark)) = (lit, half, dark) {
        assert!(half[2] > dark[2] + 10, "{dark:?} {half:?}");
        assert!(half[2] + 10 < lit[2], "{half:?} {lit:?}");
    }
}

/// On an arc, the sun's layer is where it reaches, not light, however bright its bytes are.
#[test]
fn an_arcs_layer_is_not_light() {
    let black = [0.0; 3];
    let on_arc = centre_of(
        arc_square(1, u32::MAX),
        &view_from_above(),
        arc_drawables(black, Some(5.0)),
    );
    let dark = centre_of(
        arc_square(1, 0),
        &view_from_above(),
        arc_drawables(black, Some(5.0)),
    );
    let baked = centre_of(
        arc_square(1, u32::MAX),
        &view_from_above(),
        Drawables::default(),
    );
    if let (Some(on_arc), Some(dark), Some(baked)) = (on_arc, dark, baked) {
        assert_eq!(on_arc, dark);
        assert!(
            baked[0] > dark[0] + 50,
            "as a baked sun, it's light: {baked:?}"
        );
    }
}

/// On an arc, shadows come from the bits, and none are drawn: what's over the square doesn't shade
/// it, as its bits say the sun reaches it. Off one, the sun's live and it does.
#[test]
fn on_an_arc_no_shadows_are_drawn() {
    // At a slant, so that a shadow would fall where the blocker doesn't hide it from above.
    let shot = |arc: Option<f32>, blocker: bool| -> Option<Capture> {
        // Off an arc the layer's drawn as light, so there it's dark, for the sun to show.
        let bits = if arc.is_some() { u32::MAX } else { 0 };
        let mut renderer = renderer()?;
        renderer
            .load_map(Arc::new(arc_square(2, bits)))
            .expect("load");
        let over = [ModelInstance {
            model: 1,
            origin: Vec3::new(64.0, 0.0, 30.0),
            visible: true,
            ..ModelInstance::default()
        }];
        let drawables = Drawables {
            instances: if blocker { &over } else { &[] },
            sun: Some(SunDraw {
                toward: Vec3::new(1.0, 0.0, 1.0),
                color: [0.0, 0.0, 0.4],
            }),
            sun_arc: arc,
            ..Drawables::default()
        };
        Some(
            renderer
                .capture(&view_from_above(), drawables, 64, 64)
                .expect("capture"),
        )
    };
    let darker = |open: &Capture, covered: &Capture| {
        (0..64)
            .flat_map(|y| (0..64).map(move |x| (x, y)))
            .filter(|(x, y)| pixel(covered, *x, *y)[2].saturating_add(30) < pixel(open, *x, *y)[2])
            .count()
    };
    let (arc_open, arc_covered) = (shot(Some(5.0), false), shot(Some(5.0), true));
    let (live_open, live_covered) = (shot(None, false), shot(None, true));
    if let (Some(arc_open), Some(arc_covered), Some(live_open), Some(live_covered)) =
        (arc_open, arc_covered, live_open, live_covered)
    {
        assert!(darker(&live_open, &live_covered) > 40, "live");
        assert_eq!(darker(&arc_open, &arc_covered), 0, "on an arc");
    }
}

/// The square, lit as the level says, with exposure going by itself as far as `range`.
fn exposed_square(range: [f32; 2]) -> Map {
    let mut built = map(1);
    built.look.exposure_range = range;
    built
}

/// A renderer with `built` loaded and exposure going by itself if `auto`.
fn exposing(built: Map, auto: bool) -> Option<Renderer> {
    let mut renderer = renderer()?;
    renderer.load_map(Arc::new(built)).expect("load");
    renderer.set_post(PostSettings {
        auto_exposure: auto,
        ..PostSettings::default()
    });
    Some(renderer)
}

/// A frame from above at `time`, with the level's light at `light` of what it is, and `drawables`.
fn exposed_frame(
    renderer: &mut Renderer,
    time: f32,
    light: f32,
    drawables: Drawables<'_>,
) -> Capture {
    let mut view = view_from_above();
    view.time = time;
    view.styles[0] = light;
    renderer.capture(&view, drawables, 32, 32).expect("capture")
}

/// How far exposure's gone by itself.
fn stops(renderer: &Renderer) -> f32 {
    renderer.exposure().1
}

/// In the dark, exposure goes up, and in bright light down, as far as the level lets it.
#[test]
fn exposure_goes_by_itself_as_far_as_a_level_lets_it() {
    let Some(mut renderer) = exposing(exposed_square([-1.0, 2.0]), true) else {
        return;
    };
    for frame in 0..2 {
        exposed_frame(
            &mut renderer,
            0.1 * frame as f32,
            0.02,
            Drawables::default(),
        );
    }
    assert!(
        (stops(&renderer) - 2.0).abs() < 1e-4,
        "up: {}",
        stops(&renderer)
    );
    let measured = renderer.exposure().0.expect("measured");
    assert!(measured < -4.47, "dark: {measured}");
    for frame in 2..40 {
        exposed_frame(&mut renderer, 0.1 * frame as f32, 4.0, Drawables::default());
    }
    assert!(
        (stops(&renderer) + 1.0).abs() < 1e-4,
        "down: {}",
        stops(&renderer)
    );
}

/// It goes by degrees, faster into light than into the dark, as eyes do.
#[test]
fn exposure_goes_by_degrees() {
    let Some(mut renderer) = exposing(exposed_square([-1.0, 2.0]), true) else {
        return;
    };
    let frame = |renderer: &mut Renderer, time: f32, light: f32| {
        exposed_frame(renderer, time, light, Drawables::default());
    };
    frame(&mut renderer, 0.0, 0.02);
    frame(&mut renderer, 0.1, 0.02);
    // The first bright frame goes by what was last measured, which was dark.
    frame(&mut renderer, 0.2, 4.0);
    frame(&mut renderer, 0.3, 4.0);
    let into_light = stops(&renderer);
    assert!(
        (into_light - 1.7).abs() < 0.01,
        "3 stops a second: {into_light}"
    );
    let mut time = 0.3;
    for _ in 0..20 {
        time += 0.1;
        frame(&mut renderer, time, 4.0);
    }
    frame(&mut renderer, time + 0.1, 0.02);
    frame(&mut renderer, time + 0.2, 0.02);
    let into_dark = stops(&renderer);
    assert!(
        (into_dark + 0.9).abs() < 0.01,
        "1 stop a second: {into_dark}"
    );
    // After a long gap, it's where it should be at once, which going by degrees wouldn't be.
    frame(&mut renderer, time + 1.7, 0.02);
    assert!(
        (stops(&renderer) - 2.0).abs() < 1e-4,
        "{}",
        stops(&renderer)
    );
}

/// What exposure's gone by itself shows: a dark square comes out brighter than with it off.
#[test]
fn exposure_that_goes_up_brightens_the_picture() {
    let on = exposing(exposed_square([0.0, 2.0]), true);
    let off = exposing(exposed_square([0.0, 2.0]), false);
    let (Some(mut on), Some(mut off)) = (on, off) else {
        return;
    };
    for frame in 0..3 {
        let time = 0.1 * frame as f32;
        exposed_frame(&mut on, time, 0.1, Drawables::default());
        exposed_frame(&mut off, time, 0.1, Drawables::default());
    }
    let lit = centre(&exposed_frame(&mut on, 0.3, 0.1, Drawables::default()));
    let dim = centre(&exposed_frame(&mut off, 0.3, 0.1, Drawables::default()));
    assert!(lit[1] > dim[1] + 20, "{dim:?} {lit:?}");
}

/// It doesn't go at all unless a level says how far, or if a player's turned it off, and then
/// the picture's as it was.
#[test]
fn exposure_stays_unless_a_level_and_a_player_want_it() {
    let pictures = |built: Map, auto: bool| -> Option<(Capture, (Option<f32>, f32))> {
        let mut renderer = exposing(built, auto)?;
        for frame in 0..3 {
            exposed_frame(
                &mut renderer,
                0.1 * frame as f32,
                0.05,
                Drawables::default(),
            );
        }
        let shot = exposed_frame(&mut renderer, 0.3, 0.05, Drawables::default());
        Some((shot, renderer.exposure()))
    };
    let still = pictures(exposed_square([0.0, 0.0]), true);
    let player_off = pictures(exposed_square([-1.0, 2.0]), false);
    let never = pictures(exposed_square([0.0, 0.0]), false);
    if let (Some(still), Some(player_off), Some(never)) = (still, player_off, never) {
        assert_eq!(still.1, (None, 0.0));
        assert_eq!(player_off.1, (None, 0.0));
        assert_eq!(centre(&still.0), centre(&never.0));
        assert_eq!(centre(&player_off.0), centre(&never.0));
    }
}

/// A zone's range goes in place of the level's.
#[test]
fn a_zone_says_how_far_exposure_goes_there() {
    let Some(mut renderer) = exposing(exposed_square([-1.0, 2.0]), true) else {
        return;
    };
    let drawables = Drawables {
        exposure_range: Some([0.0, 0.5]),
        ..Drawables::default()
    };
    for frame in 0..3 {
        exposed_frame(&mut renderer, 0.1 * frame as f32, 0.02, drawables);
    }
    assert!(
        (stops(&renderer) - 0.5).abs() < 1e-4,
        "{}",
        stops(&renderer)
    );
    // Where the level's off, a zone can still turn it on.
    let Some(mut renderer) = exposing(exposed_square([0.0, 0.0]), true) else {
        return;
    };
    for frame in 0..3 {
        exposed_frame(&mut renderer, 0.1 * frame as f32, 0.02, drawables);
    }
    assert!(
        (stops(&renderer) - 0.5).abs() < 1e-4,
        "{}",
        stops(&renderer)
    );
}

/// A new level starts where its own exposure should be, not where the last one left it.
#[test]
fn a_new_level_starts_again() {
    let Some(mut renderer) = exposing(exposed_square([-1.0, 2.0]), true) else {
        return;
    };
    for frame in 0..3 {
        exposed_frame(
            &mut renderer,
            0.1 * frame as f32,
            0.02,
            Drawables::default(),
        );
    }
    assert!(stops(&renderer) > 1.9);
    renderer
        .load_map(Arc::new(exposed_square([-1.0, 2.0])))
        .expect("load");
    assert_eq!(renderer.exposure(), (None, 0.0));
}

/// A small bright thing in view, like a lamp, doesn't sway how bright the scene's taken to be.
#[test]
fn a_bright_spot_does_not_sway_exposure() {
    let measured = |spot: bool| -> Option<f32> {
        let mut renderer = exposing(exposed_square([-8.0, 8.0]), true)?;
        let lamp = [LiveLight {
            spot: Vec3::new(0.0, 0.0, -1.0),
            spot_cosine: 0.975,
            color: [400.0, 400.0, 400.0],
            ..blue_light(Vec3::new(64.0, 64.0, 60.0), false)
        }];
        let drawables = Drawables {
            live_lights: if spot { &lamp } else { &[] },
            ..Drawables::default()
        };
        exposed_frame(&mut renderer, 0.0, 0.3, drawables);
        let shot = exposed_frame(&mut renderer, 0.1, 0.3, drawables);
        let bright = (0..32)
            .flat_map(|y| (0..32).map(move |x| (x, y)))
            .filter(|(x, y)| pixel(&shot, *x, *y)[0] == 255)
            .count();
        if spot {
            assert!((20..=80).contains(&bright), "a small spot: {bright}");
        }
        renderer.exposure().0
    };
    if let (Some(plain), Some(spotted)) = (measured(false), measured(true)) {
        assert!((plain - spotted).abs() < 0.1, "{plain} {spotted}");
    }
}

/// The square, dimly lit, with as much bloom as `strength` past `threshold`.
fn bloom_square(strength: f32, threshold: f32) -> Map {
    let mut built = map(1);
    // Light on a surface goes past 2, as on a level made for this engine.
    built.look.light_most = None;
    built.look.bloom = strength;
    built.look.bloom_threshold = threshold;
    built.light = vec![[20, 20, 20]; 81];
    built
}

/// A bright spot on the square, off to one side, `bright` times as bright as a dim one.
fn spot_of(bright: f32) -> LiveLight {
    LiveLight {
        spot: Vec3::new(0.0, 0.0, -1.0),
        spot_cosine: 0.995,
        color: [bright; 3],
        ..blue_light(Vec3::new(30.0, 100.0, 60.0), false)
    }
}

/// The square from above with `spot` on it, with bloom on if `on`.
fn bloomed(built: Map, spot: Option<LiveLight>, on: bool) -> Option<Capture> {
    let mut renderer = renderer()?;
    renderer.load_map(Arc::new(built)).expect("load");
    renderer.set_post(PostSettings {
        bloom: on,
        ..PostSettings::default()
    });
    let lamps: Vec<LiveLight> = spot.into_iter().collect();
    let drawables = Drawables {
        live_lights: &lamps,
        ..Drawables::default()
    };
    Some(
        renderer
            .capture(&view_from_above(), drawables, 64, 64)
            .expect("capture"),
    )
}

/// Where the spot is in the picture: the middle of what's at the top.
fn spot_place(shot: &Capture) -> (u32, u32) {
    let lit: Vec<(u32, u32)> = (0..64)
        .flat_map(|y| (0..64).map(move |x| (x, y)))
        .filter(|(x, y)| pixel(shot, *x, *y)[1] == 255)
        .collect();
    assert!(!lit.is_empty(), "the spot shows");
    #[expect(clippy::cast_possible_truncation, reason = "a small picture")]
    let count = lit.len() as u32;
    let x = lit.iter().map(|(x, _)| x).sum::<u32>() / count;
    let y = lit.iter().map(|(_, y)| y).sum::<u32>() / count;
    (x, y)
}

/// How much brighter `on` is than `off` round `at`, out to `reach` pixels, leaving out the spot.
fn glow_round(on: &Capture, off: &Capture, at: (u32, u32), reach: u32) -> u32 {
    let mut total = 0;
    for y in at.1.saturating_sub(reach)..(at.1 + reach).min(64) {
        for x in at.0.saturating_sub(reach)..(at.0 + reach).min(64) {
            if pixel(off, x, y)[1] < 250 {
                total += u32::from(pixel(on, x, y)[1].saturating_sub(pixel(off, x, y)[1]));
            }
        }
    }
    total
}

/// How much brighter `on` is than `off` between `inner` and `outer` pixels from `at`.
fn glow_ring(on: &Capture, off: &Capture, at: (u32, u32), (inner, outer): (u32, u32)) -> u32 {
    let mut total = 0;
    for y in 0..64_u32 {
        for x in 0..64_u32 {
            let away = x.abs_diff(at.0).max(y.abs_diff(at.1));
            if (inner..outer).contains(&away) {
                total += u32::from(pixel(on, x, y)[1].saturating_sub(pixel(off, x, y)[1]));
            }
        }
    }
    total
}

/// What's brighter than a screen shows glows round where it is, and not across the picture from
/// it.
#[test]
fn what_is_bright_glows_round_it() {
    let built = || bloom_square(1.0, 1.0);
    let off = bloomed(built(), Some(spot_of(400.0)), false);
    let on = bloomed(built(), Some(spot_of(400.0)), true);
    if let (Some(off), Some(on)) = (off, on) {
        let at = spot_place(&off);
        let near = glow_round(&on, &off, at, 8);
        assert!(near > 300, "{near}");
        let mirrored = (at.0, 63 - at.1);
        let across = glow_round(&on, &off, mirrored, 8);
        assert!(across * 4 < near, "{near} {across}");
        // It spreads wide, as each image on the way up adds what's below it.
        let wide = glow_ring(&on, &off, at, (10, 16));
        assert!(wide > 100, "{wide}");
    }
}

/// What a screen can show doesn't glow, so a level that has nothing brighter is as it was built.
#[test]
fn what_a_screen_can_show_does_not_glow() {
    let off = bloomed(bloom_square(1.0, 1.0), None, false);
    let on = bloomed(bloom_square(1.0, 1.0), None, true);
    if let (Some(off), Some(on)) = (off, on) {
        assert_eq!(off.rgba, on.rgba);
    }
}

/// Bloom is only there if a level has some and a player has it on.
#[test]
fn bloom_needs_a_level_and_a_player() {
    let plain = bloomed(bloom_square(0.0, 1.0), Some(spot_of(400.0)), false);
    let level_none = bloomed(bloom_square(0.0, 1.0), Some(spot_of(400.0)), true);
    let player_off = bloomed(bloom_square(1.0, 1.0), Some(spot_of(400.0)), false);
    if let (Some(plain), Some(level_none), Some(player_off)) = (plain, level_none, player_off) {
        assert_eq!(plain.rgba, level_none.rgba);
        assert_eq!(plain.rgba, player_off.rgba);
    }
}

/// A level says how much, and from how bright.
#[test]
fn a_level_says_how_much_glows() {
    // Dim enough that what glows doesn't reach the top of what a screen shows.
    let glow = |strength: f32, threshold: f32| -> Option<u32> {
        let built = || bloom_square(strength, threshold);
        let off = bloomed(built(), Some(spot_of(30.0)), false)?;
        let on = bloomed(built(), Some(spot_of(30.0)), true)?;
        Some(glow_round(&on, &off, spot_place(&off), 8))
    };
    let (some, more, higher) = (glow(0.5, 1.0), glow(1.0, 1.0), glow(1.0, 8.0));
    if let (Some(some), Some(more), Some(higher)) = (some, more, higher) {
        // Screens show light on a curve, so twice the light is less than twice the bytes.
        assert!(more * 10 > some * 13, "{some} {more}");
        assert!(
            higher * 2 < more,
            "a higher threshold glows less: {more} {higher}"
        );
    }
}

/// What's measured against the threshold is exposed first, so a level exposed down glows less.
#[test]
fn bloom_goes_by_exposed_light() {
    // Half the light at twice the exposure is the same light, once exposed, so it glows the same.
    // Dim, so that it's only a little past the threshold, and light on a surface doesn't stop.
    let shot = |bright: f32, exposure: f32| -> Option<(Capture, Capture)> {
        let mut built = bloom_square(1.0, 1.0);
        built.light = vec![[0, 0, 0]; 81];
        // So that half the light is half as bright.
        built.look.light_curve = 1.0;
        built.look.exposure = exposure;
        let off = bloomed(built.clone(), Some(spot_of(bright)), false)?;
        let on = bloomed(built, Some(spot_of(bright)), true)?;
        Some((off, on))
    };
    let at =
        bloomed(bloom_square(1.0, 1.0), Some(spot_of(400.0)), false).map(|shot| spot_place(&shot));
    if let (Some(plain), Some(bright), Some(at)) = (shot(30.0, 1.0), shot(15.0, 2.0), at) {
        let (glow, again) = (
            glow_ring(&plain.1, &plain.0, at, (0, 16)),
            glow_ring(&bright.1, &bright.0, at, (0, 16)),
        );
        assert!(glow > 50, "it glows: {glow}");
        assert!(glow.abs_diff(again) * 5 < glow, "{glow} {again}");
    }
}

/// One very bright spot is weighted down on the first step, so that it doesn't flicker as it
/// moves: ten times as bright doesn't glow ten times as much.
#[test]
fn a_very_bright_spot_is_held_down() {
    // Far out, where the glow is dim, so that what a screen shows doesn't top out.
    let glow = |bright: f32| -> Option<u32> {
        let off = bloomed(bloom_square(1.0, 1.0), Some(spot_of(bright)), false)?;
        let on = bloomed(bloom_square(1.0, 1.0), Some(spot_of(bright)), true)?;
        Some(glow_ring(&on, &off, spot_place(&off), (8, 16)))
    };
    if let (Some(bright), Some(brighter)) = (glow(400.0), glow(4000.0)) {
        assert!(brighter >= bright, "{bright} {brighter}");
        assert!(brighter * 2 < bright * 3, "{bright} {brighter}");
    }
}

/// The square lit at `style` of its light, exposed down so that bright doesn't top out, with
/// the most a surface takes as `most`.
fn exposed_down(style: f32, most: Option<f32>) -> Option<[u8; 4]> {
    let mut renderer = renderer()?;
    let mut built = map(1);
    // On a curve that keeps what's below its top as it is, and far below it.
    built.look.tone = ToneCurve::Neutral;
    built.look.exposure = 0.05;
    built.look.light_most = most;
    renderer.load_map(Arc::new(built)).expect("load");
    let mut view = view_from_above();
    view.styles[0] = style;
    let capture = renderer
        .capture(&view, Drawables::default(), 32, 32)
        .expect("capture");
    Some(centre(&capture))
}

/// Light on a surface goes past 2 on a level made for this engine, and stops there with the
/// fixture's look.
#[test]
fn light_on_a_surface_goes_past_two() {
    let (bright, brighter) = (exposed_down(2.0, None), exposed_down(4.0, None));
    let (held, still_held) = (exposed_down(2.0, Some(2.0)), exposed_down(4.0, Some(2.0)));
    if let (Some(bright), Some(brighter), Some(held), Some(still_held)) =
        (bright, brighter, held, still_held)
    {
        assert!(
            brighter[1] > bright[1].saturating_add(20),
            "{bright:?} {brighter:?}"
        );
        assert_eq!(held, still_held, "the fixture's look stops at 2");
    }
}

/// Baked light a bake brought down to fit a byte is multiplied back.
#[test]
fn baked_light_brought_down_is_multiplied_back() {
    let lit = |level: u8, shift: u8| -> Option<[u8; 4]> {
        let mut renderer = renderer()?;
        let mut built = map(1);
        built.light = vec![[level; 3]; 81];
        built.light_shift = shift;
        renderer.load_map(Arc::new(built)).expect("load");
        let capture = renderer
            .capture(&view_from_above(), Drawables::default(), 32, 32)
            .expect("capture");
        Some(centre(&capture))
    };
    if let (Some(half), Some(whole)) = (lit(50, 1), lit(100, 0)) {
        for part in 0..3 {
            assert!(half[part].abs_diff(whole[part]) <= 2, "{half:?} {whole:?}");
        }
    }
}

/// The fog grid's probes are multiplied back as baked light is.
#[test]
fn the_fog_grids_light_is_multiplied_back() {
    let grid = PostSettings {
        fog_grid: true,
        ..PostSettings::default()
    };
    let shifted = || {
        let mut built = fog_map(60);
        built.light_shift = 1;
        built
    };
    if let (Some(half), Some(whole)) = (
        post_centre(shifted(), grid),
        post_centre(fog_map(120), grid),
    ) {
        for part in 0..3 {
            assert!(half[part].abs_diff(whole[part]) <= 3, "{half:?} {whole:?}");
        }
    }
}

/// How much a lamp lighting a pixel or so of the square, `bright` times as bright as usual, glows
/// all round it, with a little bloom so that what's measured doesn't top out.
fn sparkle_glow(bright: f32) -> Option<u32> {
    let shot = |on: bool| -> Option<Capture> {
        let mut built = bloom_square(0.05, 1.0);
        built.light = vec![[0, 0, 0]; 81];
        let mut renderer = renderer()?;
        renderer.load_map(Arc::new(built)).expect("load");
        renderer.set_post(PostSettings {
            bloom: on,
            ..PostSettings::default()
        });
        let lamp = [LiveLight {
            spot_cosine: 0.9995,
            color: [bright; 3],
            ..spot_of(1.0)
        }];
        let drawables = Drawables {
            live_lights: &lamp,
            ..Drawables::default()
        };
        Some(
            renderer
                .capture(&view_from_above(), drawables, 64, 64)
                .expect("capture"),
        )
    };
    let (off, on) = (shot(false)?, shot(true)?);
    Some(glow_ring(&on, &off, (15, 14), (0, 30)))
}

/// A sparkle a pixel or so across is weighted down on the first step, so that it doesn't flicker
/// as it moves. With it, this one glows 3632; without, 6713, so it's held to between the two.
#[test]
fn a_sparkle_is_weighted_down() {
    if let Some(glow) = sparkle_glow(30_000.0) {
        assert!(glow > 1000, "it glows: {glow}");
        assert!(glow < 5200, "{glow}");
    }
}

/// The middle of a sky of one `color`, which a sky takes as sRGB, on the neutral curve, as
/// `output` would be sent with `post`.
fn sky_sent(color: f32, output: ScreenOutput, post: PostSettings) -> Option<[f32; 4]> {
    let mut renderer = renderer()?;
    let mut built = map(1);
    built.look.tone = ToneCurve::Neutral;
    built.look.light_most = None;
    built.sky = Some(Sky {
        zenith: [color; 3],
        horizon: [color; 3],
        sun_size: 0.0,
        sun_color: [0.0; 3],
        ..Sky::default()
    });
    renderer.load_map(Arc::new(built)).expect("load");
    renderer.set_post(post);
    let view = View {
        origin: UNDER,
        pitch: -89.0,
        ..view_from_above()
    };
    let sent = renderer
        .capture_as(&view, Drawables::default(), (32, 32), output)
        .expect("capture");
    Some(sent.values[16 * 32 + 16])
}

/// HDR on.
fn hdr(white: f32, peak: f32) -> PostSettings {
    PostSettings {
        hdr: true,
        white_nits: white,
        peak_nits: peak,
        ..PostSettings::default()
    }
}

/// Whether `value` is within `slack` of `wanted`.
fn near(value: f32, wanted: f32, slack: f32) -> bool {
    (value - wanted).abs() < slack
}

/// Sent as scRGB, what's below the curve is as it is, at white's nits, where 1 is 80 nits. A sky
/// of 0.5 is 0.214 of white in light.
#[test]
fn scrgb_is_linear_light_at_whites_nits() {
    let half = sky_sent(0.5, ScreenOutput::ScRgb, hdr(200.0, 1000.0));
    let dimmer_white = sky_sent(0.5, ScreenOutput::ScRgb, hdr(100.0, 1000.0));
    if let (Some(half), Some(dimmer_white)) = (half, dimmer_white) {
        assert!(near(half[1], 0.214 * 2.5, 0.01), "{half:?}");
        assert!(
            near(dimmer_white[1], 0.214 * 1.25, 0.01),
            "{dimmer_white:?}"
        );
    }
}

/// What's brighter than white goes past it in HDR, up to the brightest the screen shows, where on
/// a standard screen it stops at the top.
#[test]
fn hdr_shows_what_is_brighter_than_white() {
    // 1.613 as sRGB is three times white in light.
    let standard = sky_sent(1.613, ScreenOutput::Standard, PostSettings::default());
    let bright = sky_sent(1.613, ScreenOutput::ScRgb, hdr(200.0, 1000.0));
    let blinding = sky_sent(100.0, ScreenOutput::ScRgb, hdr(200.0, 1000.0));
    if let (Some(standard), Some(bright), Some(blinding)) = (standard, bright, blinding) {
        assert!(standard[1] <= 1.0);
        assert!(near(bright[1], 7.5, 0.1), "three times white: {bright:?}");
        assert!(
            blinding[1] <= 12.5 + 0.01,
            "no brighter than the peak: {blinding:?}"
        );
        assert!(blinding[1] > 11.0, "{blinding:?}");
    }
}

/// HDR10 is on the PQ curve: a sky of 0.5, 0.214 of white at 200 nits, is 43 nits, which is
/// 0.426 up it.
#[test]
fn hdr10_is_on_the_pq_curve() {
    if let Some(sent) = sky_sent(0.5, ScreenOutput::Hdr10, hdr(200.0, 1000.0)) {
        for part in 0..3 {
            assert!(near(sent[part], 0.426, 0.01), "{sent:?}");
        }
    }
}

/// What's drawn over the view, made for a standard screen, is white at white's nits in HDR.
#[test]
fn what_is_drawn_over_the_view_is_white_at_whites_nits() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    renderer.set_post(hdr(200.0, 1000.0));
    let quads = [Quad {
        picture: 0,
        place: [0.0, 0.0, 64.0, 32.0],
        source: [0.0, 0.0, 1.0, 1.0],
        color: [0.5, 0.5, 0.5, 1.0],
    }];
    let drawables = Drawables {
        quads: &quads,
        ..Drawables::default()
    };
    let sent = renderer
        .capture_as(&view_from_above(), drawables, (64, 32), ScreenOutput::ScRgb)
        .expect("capture");
    // sRGB's 0.5 is 0.214 of white in light.
    let middle = sent.values[16 * 64 + 32];
    assert!(near(middle[0], 0.214 * 2.5, 0.02), "{middle:?}");
}

/// A screen with nothing to say of HDR, as with no window, is sent what every screen shows, and
/// a capture as standard is the capture there's always been.
#[test]
fn a_screen_without_hdr_is_sent_standard() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    renderer.set_post(hdr(200.0, 1000.0));
    assert_eq!(renderer.screen_offers(), vec![ScreenOutput::Standard]);
    assert_eq!(renderer.screen_output(), ScreenOutput::Standard);
    let plain = renderer
        .capture(&view_from_above(), Drawables::default(), 32, 32)
        .expect("capture");
    let sent = renderer
        .capture_as(
            &view_from_above(),
            Drawables::default(),
            (32, 32),
            ScreenOutput::Standard,
        )
        .expect("capture");
    let first = plain.rgba.as_chunks::<4>().0[16 * 32 + 16].map(|part| f32::from(part) / 255.0);
    assert_eq!(sent.values[16 * 32 + 16], first);
}

/// HDR10 is in Rec. 2020's colours, where Rec. 709's pure red has some green and blue in it.
#[test]
fn hdr10_is_in_rec2020s_colours() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map(1);
    built.look.tone = ToneCurve::Neutral;
    built.sky = Some(Sky {
        zenith: [0.5, 0.0, 0.0],
        horizon: [0.5, 0.0, 0.0],
        sun_size: 0.0,
        sun_color: [0.0; 3],
        ..Sky::default()
    });
    renderer.load_map(Arc::new(built)).expect("load");
    renderer.set_post(hdr(200.0, 1000.0));
    let view = View {
        origin: UNDER,
        pitch: -89.0,
        ..view_from_above()
    };
    let sent = renderer
        .capture_as(&view, Drawables::default(), (32, 32), ScreenOutput::Hdr10)
        .expect("capture");
    let red = sent.values[16 * 32 + 16];
    assert!(red[0] > 0.3, "{red:?}");
    assert!(
        red[1] > 0.1 && red[2] > 0.05,
        "some green and blue: {red:?}"
    );
}

/// A picture after one taken for HDR is as one always was.
#[test]
fn a_picture_after_an_hdr_one_is_standard() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    renderer.set_post(hdr(200.0, 1000.0));
    let before = renderer
        .capture(&view_from_above(), Drawables::default(), 32, 32)
        .expect("capture");
    let _hdr = renderer
        .capture_as(
            &view_from_above(),
            Drawables::default(),
            (32, 32),
            ScreenOutput::ScRgb,
        )
        .expect("capture");
    let after = renderer
        .capture(&view_from_above(), Drawables::default(), 32, 32)
        .expect("capture");
    assert_eq!(before.rgba, after.rgba);
}

/// How many frames TAA's moves of the view take to come round again.
const TAA_ROUND: u32 = 8;

/// TAA on.
fn taa() -> PostSettings {
    PostSettings {
        edge_smoothing: 3,
        ..PostSettings::default()
    }
}

/// How far apart two captures are, over every pixel's colour.
fn difference(a: &Capture, b: &Capture) -> u64 {
    a.rgba
        .iter()
        .zip(&b.rgba)
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum()
}

/// How many pixels two captures differ at by more than a little.
fn pixels_apart(a: &Capture, b: &Capture) -> usize {
    let (a, _) = a.rgba.as_chunks::<4>();
    let (b, _) = b.rgba.as_chunks::<4>();
    a.iter()
        .zip(b)
        .filter(|(a, b)| a.iter().zip(*b).any(|(a, b)| a.abs_diff(*b) > 8))
        .count()
}

/// `frames` frames of `drawables` from `view`, and the last of them.
fn frames_of(
    renderer: &mut Renderer,
    view: &View,
    drawables: Drawables<'_>,
    frames: u32,
) -> Capture {
    let mut last = None;
    for _ in 0..frames {
        last = Some(renderer.capture(view, drawables, 64, 64).expect("capture"));
    }
    last.expect("a frame")
}

/// Black and white squares, `cell` texels a side, on a texture `size` texels a side.
fn checker(size: u32, cell: u32) -> Vec<MipLevel> {
    image_of(size, |x, y| {
        if (x / cell + y / cell).is_multiple_of(2) {
            [255, 255, 255, 255]
        } else {
            [0, 0, 0, 255]
        }
    })
}

/// TAA moves the view by less than a pixel each frame, round a pattern, so even a frame with
/// nothing kept has its edges a little off, and what's flat stays as it is.
#[test]
fn taa_moves_the_view_by_less_than_a_pixel() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let view = slanted_view();
    let plain = frames_of(&mut renderer, &view, Drawables::default(), 1);
    renderer.set_post(taa());
    let first = frames_of(&mut renderer, &view, Drawables::default(), 1);
    let second = frames_of(&mut renderer, &view, Drawables::default(), 1);
    // Each move of a round is less than a pixel, taken with nothing kept.
    for _ in 0..TAA_ROUND {
        renderer.set_post(taa());
        let one = frames_of(&mut renderer, &view, Drawables::default(), 1);
        let moved = pixels_apart(&one, &plain);
        assert!(moved < 400, "only along edges: {moved}");
        renderer.set_post(PostSettings::default());
        frames_of(&mut renderer, &view, Drawables::default(), 1);
    }
    // Light's read a little off too, which is a shade at most.
    let flat = centre(&first)
        .iter()
        .zip(centre(&plain))
        .all(|(a, b)| a.abs_diff(b) <= 1);
    assert!(flat, "what's flat stays");
    assert_ne!(first.rgba, plain.rgba, "edges move");
    assert_ne!(second.rgba, first.rgba, "and move again");
    assert!(pixels_apart(&first, &plain) > 0);
}

/// Over a few frames TAA puts shades between an edge's two sides, and then holds still, and
/// turning it off leaves nothing behind.
#[test]
fn taa_settles_an_edge_over_a_few_frames() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let view = slanted_view();
    let plain = frames_of(&mut renderer, &view, Drawables::default(), 1);
    renderer.set_post(taa());
    let first = frames_of(&mut renderer, &view, Drawables::default(), 1);
    let settled = frames_of(
        &mut renderer,
        &view,
        Drawables::default(),
        TAA_ROUND * 4 - 1,
    );
    let later = frames_of(&mut renderer, &view, Drawables::default(), TAA_ROUND);
    assert!(
        shades(&settled) > shades(&plain) + 4,
        "{} {}",
        shades(&plain),
        shades(&settled)
    );
    assert!(
        difference(&settled, &later) * 4 < difference(&first, &plain),
        "it settles"
    );
    let near = centre(&settled)
        .iter()
        .zip(centre(&plain))
        .all(|(a, b)| a.abs_diff(b) <= 1);
    assert!(near, "and keeps its colours: {:?}", centre(&settled));
    renderer.set_post(PostSettings::default());
    let again = frames_of(&mut renderer, &view, Drawables::default(), 1);
    assert_eq!(
        plain.rgba, again.rgba,
        "turning it off leaves nothing behind"
    );
}

/// How far one frame of `after` is from many, after many of `before`, and how far apart many of
/// each are: what TAA kept is found where it went, or the frame after a move is off.
fn taa_after_a_move(
    renderer: &mut Renderer,
    (view_before, before): (&View, Drawables<'_>),
    (view_after, after): (&View, Drawables<'_>),
) -> (u64, u64) {
    // Each frame takes a tenth of what's new, so it's long enough for what's kept to be there.
    renderer.set_post(taa());
    let settled_before = frames_of(renderer, view_before, before, TAA_ROUND * 8);
    let once = frames_of(renderer, view_after, after, 1);
    // Whole rounds later, so that the view's moved the same way as it was for that frame.
    let settled_after = frames_of(renderer, view_after, after, TAA_ROUND * 8);
    renderer.set_post(PostSettings::default());
    (
        difference(&once, &settled_after),
        difference(&settled_before, &settled_after),
    )
}

/// The floor in black and white squares, two pixels a side from above.
fn checkered_floor(models: u32) -> Map {
    let mut built = map(models);
    built.textures[0].mips = checker(64, 4);
    built
}

/// As the view moves, what TAA kept is found where it was from depth and where the view was.
#[test]
fn taa_finds_where_a_pixel_was_as_the_view_moves() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer
        .load_map(Arc::new(checkered_floor(1)))
        .expect("load");
    let before = view_from_above();
    let after = View {
        origin: before.origin + Vec3::new(6.0, 0.0, 0.0),
        ..before
    };
    let (off, apart) = taa_after_a_move(
        &mut renderer,
        (&before, Drawables::default()),
        (&after, Drawables::default()),
    );
    assert!(off * 4 < apart, "{off} {apart}");
}

/// A brush model that moves takes what TAA kept of it along.
#[test]
fn taa_follows_a_brush_model_that_moves() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut two = checkered_floor(2);
    two.tree.leaves[1].face_count = 0;
    two.tree.leaf_faces.clear();
    renderer.load_map(Arc::new(two)).expect("load");
    let at = |x: f32| ModelInstance {
        model: 1,
        origin: Vec3::new(x, 0.0, 0.0),
        visible: true,
        frame: 0,
        solid: true,
    };
    let (before, after) = ([at(0.0)], [at(6.0)]);
    let view = view_from_above();
    let (off, apart) = taa_after_a_move(
        &mut renderer,
        (&view, with_instance(&before)),
        (&view, with_instance(&after)),
    );
    assert!(off * 4 < apart, "{off} {apart}");
}

/// A model 40 units square in black and white squares, two units a side.
fn checkered_model() -> AliasModel {
    let vertex = |x, y| AliasVertex {
        position: [x, y, 0],
        normal: [127, 0, 0],
    };
    let corner = |vertex, u, v| AliasCorner { vertex, uv: [u, v] };
    let skin = checker(20, 1).remove(0).rgba;
    AliasModel {
        scale: Vec3::new(4.0, 4.0, 1.0),
        offset: Vec3::new(-20.0, -20.0, 0.0),
        skin_width: 20,
        skin_height: 20,
        skins: vec![skin],
        // Wound as Quake winds them, the other way from a mesh.
        corners: vec![
            corner(0, 0.0, 0.0),
            corner(2, 1.0, 1.0),
            corner(1, 1.0, 0.0),
            corner(0, 0.0, 0.0),
            corner(3, 0.0, 1.0),
            corner(2, 1.0, 1.0),
        ],
        poses: vec![vec![
            vertex(0, 0),
            vertex(10, 0),
            vertex(10, 10),
            vertex(0, 10),
        ]],
        frames: vec![AliasFrame {
            first_pose: 0,
            pose_count: 1,
            pose_ends: Vec::new(),
        }],
        spins: false,
        flags: 0,
    }
}

/// A model that moves takes what TAA kept of it along, as one with no id, which is no one in
/// particular, doesn't.
#[test]
fn taa_follows_a_model_that_moves() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let number = renderer
        .add_model(Arc::new(checkered_model()))
        .expect("add");
    let at = |x: f32, id: u64| EntityDraw {
        id,
        origin: Vec3::new(64.0 + x, 64.0, 32.0),
        ..draw_of(number)
    };
    let view = view_from_above();
    let (before, after) = ([at(0.0, 7)], [at(3.0, 7)]);
    let (off, apart) = taa_after_a_move(
        &mut renderer,
        (&view, with_entity(&before)),
        (&view, with_entity(&after)),
    );
    let (before, after) = ([at(0.0, 0)], [at(3.0, 0)]);
    let (unknown, _) = taa_after_a_move(
        &mut renderer,
        (&view, with_entity(&before)),
        (&view, with_entity(&after)),
    );
    assert!(off * 4 < apart, "{off} {apart}");
    assert!(off * 3 < unknown, "{off} {unknown}");
}

/// A square 32 units a side in black and white squares, as a moving mesh.
fn checkered_square() -> Map {
    let mut built = map(1);
    let mut texture = built.textures[0].clone();
    texture.mips = checker(16, 1);
    built.textures.push(texture);
    let corner = |x: f32, y: f32| MeshCorner {
        position: [x, y, 0.0],
        uv: [x / 32.0, y / 32.0],
        normal: [0, 0, 127],
    };
    built.moving_meshes = vec![MovingMesh {
        name: "checkered".to_owned(),
        mesh: SharedMesh {
            corners: vec![
                corner(0.0, 0.0),
                corner(32.0, 0.0),
                corner(32.0, 32.0),
                corner(0.0, 32.0),
            ],
            joints: vec![[0, 0, 0, 0]; 4],
            weights: vec![[255, 0, 0, 0]; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
            levels: vec![vec![SharedPart {
                texture: 1,
                first: 0,
                count: 6,
            }]],
            ..SharedMesh::default()
        },
        skeleton: Skeleton::default(),
    }];
    built
}

/// A moving mesh that moves takes what TAA kept of it along.
#[test]
fn taa_follows_a_moving_mesh_that_moves() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer
        .load_map(Arc::new(checkered_square()))
        .expect("load");
    let at = |x: f32| {
        let mut rows = NO_MOVE;
        rows[0][3] = 48.0 + x;
        rows[1][3] = 48.0;
        rows[2][3] = 32.0;
        MovingDraw {
            mesh: 0,
            rows,
            joint_count: 1,
            light: [1.0; 3],
            center: Vec3::new(64.0 + x, 64.0, 32.0),
            radius: 30.0,
            id: 7,
            ..MovingDraw::default()
        }
    };
    let palette = [NO_MOVE];
    let (before, after) = ([at(0.0)], [at(3.0)]);
    let drawn = |moving| Drawables {
        moving,
        palette: &palette,
        ..Drawables::default()
    };
    let view = view_from_above();
    let (off, apart) = taa_after_a_move(
        &mut renderer,
        (&view, drawn(&before)),
        (&view, drawn(&after)),
    );
    assert!(off * 4 < apart, "{off} {apart}");
}

/// What's kept in front and goes along with the view, as the player's weapon does, stays where
/// it is on the screen to TAA, though its depth is squeezed.
#[test]
fn taa_holds_what_is_kept_in_front_as_the_view_moves() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer
        .load_map(Arc::new(checkered_floor(1)))
        .expect("load");
    let number = renderer
        .add_model(Arc::new(checkered_model()))
        .expect("add");
    let before = view_from_above();
    let after = View {
        origin: before.origin + Vec3::new(6.0, 0.0, 0.0),
        ..before
    };
    let held = |view: &View, id: u64| EntityDraw {
        id,
        in_front: true,
        origin: view.origin - Vec3::new(0.0, 0.0, 32.0),
        ..draw_of(number)
    };
    let (first, second) = ([held(&before, u64::MAX)], [held(&after, u64::MAX)]);
    let (off, apart) = taa_after_a_move(
        &mut renderer,
        (&before, with_entity(&first)),
        (&after, with_entity(&second)),
    );
    let (first, second) = ([held(&before, 0)], [held(&after, 0)]);
    let (unknown, _) = taa_after_a_move(
        &mut renderer,
        (&before, with_entity(&first)),
        (&after, with_entity(&second)),
    );
    assert!(off < apart, "{off} {apart}");
    assert!(off * 4 < unknown, "{off} {unknown}");
}

/// At a new size TAA has nothing kept, so its first frame is that frame alone, as the first
/// frame of all is, the view moved the same way.
#[test]
fn taa_starts_afresh_at_a_new_size() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    renderer.set_post(taa());
    let view = slanted_view();
    let first = renderer
        .capture(&view, Drawables::default(), 48, 48)
        .expect("capture");
    frames_of(
        &mut renderer,
        &view,
        Drawables::default(),
        TAA_ROUND * 3 - 1,
    );
    let again = renderer
        .capture(&view, Drawables::default(), 48, 48)
        .expect("capture");
    assert_eq!(first.rgba, again.rgba);
}

/// What moves where it's hidden doesn't move what hides it to TAA.
#[test]
fn taa_leaves_what_hides_a_moving_model_where_it_is() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer
        .load_map(Arc::new(checkered_floor(1)))
        .expect("load");
    let number = renderer
        .add_model(Arc::new(checkered_model()))
        .expect("add");
    let at = |x: f32| EntityDraw {
        id: 7,
        origin: Vec3::new(64.0 + x, 64.0, -32.0),
        ..draw_of(number)
    };
    let view = view_from_above();
    let (before, after) = ([at(0.0)], [at(6.0)]);
    let (off, _) = taa_after_a_move(
        &mut renderer,
        (&view, with_entity(&before)),
        (&view, with_entity(&after)),
    );
    let (still, _) = taa_after_a_move(
        &mut renderer,
        (&view, Drawables::default()),
        (&view, Drawables::default()),
    );
    // What's left of the frames before is a few hundred at most, where taking the hidden model's
    // move is tens of thousands.
    assert!(off < still + 2000, "{off} {still}");
}

/// A moving mesh and a model of the same number and id are told apart, as is where each was.
#[test]
fn taa_tells_a_moving_mesh_from_a_model() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer
        .load_map(Arc::new(checkered_square()))
        .expect("load");
    let number = renderer
        .add_model(Arc::new(checkered_model()))
        .expect("add");
    assert_eq!(number, 0, "the same number as the mesh");
    let mesh_at = |x: f32| {
        let mut rows = NO_MOVE;
        rows[0][3] = 48.0 + x;
        rows[1][3] = 48.0;
        rows[2][3] = 32.0;
        MovingDraw {
            mesh: 0,
            rows,
            joint_count: 1,
            light: [1.0; 3],
            center: Vec3::new(64.0 + x, 64.0, 32.0),
            radius: 30.0,
            id: 7,
            ..MovingDraw::default()
        }
    };
    // Still, under the mesh, so that taking where it was for where the mesh was is seen.
    let away = [EntityDraw {
        id: 7,
        origin: Vec3::new(64.0, 64.0, 2.0),
        ..draw_of(number)
    }];
    let palette = [NO_MOVE];
    let (before, after) = ([mesh_at(0.0)], [mesh_at(3.0)]);
    let drawn = |moving| Drawables {
        moving,
        palette: &palette,
        entities: &away,
        ..Drawables::default()
    };
    let view = view_from_above();
    let (off, apart) = taa_after_a_move(
        &mut renderer,
        (&view, drawn(&before)),
        (&view, drawn(&after)),
    );
    assert!(off * 4 < apart, "{off} {apart}");
}

/// What's left where something was, as it goes, is let go of at once: what TAA kept there is held
/// to what's round it now.
#[test]
fn taa_lets_go_of_what_moved_away() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let number = renderer
        .add_model(Arc::new(checkered_model()))
        .expect("add");
    let there = [EntityDraw {
        id: 7,
        ..draw_of(number)
    }];
    let gone = [EntityDraw {
        id: 7,
        origin: Vec3::new(5000.0, 64.0, 32.0),
        ..draw_of(number)
    }];
    let view = view_from_above();
    let (off, apart) = taa_after_a_move(
        &mut renderer,
        (&view, with_entity(&there)),
        (&view, with_entity(&gone)),
    );
    assert!(off * 4 < apart, "{off} {apart}");
}

/// A black floor under a white light, shining sharply, with `normals` as its normal map, seen from
/// above, with highlights steadied or not.
fn shiny_floor(normals: Vec<MipLevel>, steady: bool) -> Option<Capture> {
    let mut renderer = renderer()?;
    let mut built = map(1);
    built.textures[0].mips = image_of(2, |_, _| [0, 0, 0, 255]);
    built.textures[0].material = Some(Arc::new(Material {
        specular_strength: 1.0,
        gloss: 200.0,
        slots: vec![
            Arc::new(normals),
            Arc::new(image_of(2, |_, _| [255, 255, 255, 255])),
        ],
        normal: Some(1),
        specular: Some(2),
        ..Material::default()
    }));
    renderer.load_map(Arc::new(built)).expect("load");
    renderer.set_post(PostSettings {
        steady_shine: steady,
        ..PostSettings::default()
    });
    let lights = [LightDraw {
        origin: Vec3::new(64.0, 64.0, 40.0),
        radius: 300.0,
        min_light: 0.0,
    }];
    let drawables = Drawables {
        lights: &lights,
        ..Drawables::default()
    };
    Some(
        renderer
            .capture(&view_from_above(), drawables, 64, 64)
            .expect("capture"),
    )
}

/// A normal map 64 texels a side whose texels lean `lean` across, one way and then the other, in
/// stripes `width` texels wide.
fn striped_normals(width: u32, lean: u8) -> Vec<u8> {
    (0..64 * 64)
        .flat_map(|at: u32| {
            if (at % 64 / width).is_multiple_of(2) {
                [128 + lean, 128, 255, 255]
            } else {
                [128 - lean, 128, 255, 255]
            }
        })
        .collect()
}

/// Bumps too small to see, two texels to a pixel, average to a flat surface further down a normal
/// map, but its mips keep that they spread, and a sharp highlight on them is dimmer and wider.
#[test]
fn bumps_too_small_to_see_dim_a_sharp_highlight() {
    let normals = || normal_chain(64, 64, &striped_normals(1, 64));
    let Some(sharp) = shiny_floor(normals(), false) else {
        return;
    };
    let steady = shiny_floor(normals(), true).expect("it drew");
    let (sharp, steady) = (centre(&sharp)[0], centre(&steady)[0]);
    assert!(sharp > 100, "a sharp highlight: {sharp}");
    assert!(u32::from(steady) * 2 < u32::from(sharp), "{sharp} {steady}");
}

/// The brightest of a black floor whose corners' normals lean apart across it, so that it turns,
/// under a white light, shining sharply, seen from `height` above and `aside` across, steadied or
/// not.
fn brightest_turning_floor(height: f32, aside: f32, steady: bool) -> Option<u8> {
    let mut renderer = renderer()?;
    let mut built = map(1);
    built.textures[0].mips = image_of(2, |_, _| [0, 0, 0, 255]);
    built.textures[0].material = Some(Arc::new(Material {
        specular_strength: 1.0,
        gloss: 200.0,
        slots: vec![Arc::new(image_of(2, |_, _| [255, 255, 255, 255]))],
        specular: Some(1),
        ..Material::default()
    }));
    built.normals = vec![[-90, 0, 90], [90, 0, 90], [90, 0, 90], [-90, 0, 90]];
    renderer.load_map(Arc::new(built)).expect("load");
    renderer.set_post(PostSettings {
        steady_shine: steady,
        ..PostSettings::default()
    });
    let lights = [LightDraw {
        origin: Vec3::new(64.0, 64.0, height),
        radius: height * 4.0,
        min_light: 0.0,
    }];
    let drawables = Drawables {
        lights: &lights,
        ..Drawables::default()
    };
    let view = View {
        origin: Vec3::new(64.0 + aside, 64.0, height),
        ..view_from_above()
    };
    let capture = renderer.capture(&view, drawables, 64, 64).expect("capture");
    let (pixels, _) = capture.rgba.as_chunks::<4>();
    pixels.iter().map(|pixel| pixel[0]).max()
}

/// How far the brightest of `brightest` swings as the view moves across a pixel from `height`
/// above, and the least of it.
fn swing(height: f32, brightest: impl Fn(f32) -> Option<u8>) -> Option<(u8, u8)> {
    // A pixel is this far across from there.
    let pixel = height * 2.0 / 64.0;
    let mut seen = Vec::new();
    for step in 0..8_u8 {
        seen.push(brightest(pixel * f32::from(step) / 8.0)?);
    }
    let (least, most) = (*seen.iter().min()?, *seen.iter().max()?);
    Some((most - least, least))
}

/// How far the brightest of the turning floor swings from `height` above, steadied or not.
fn turning_floor_swing(height: f32, steady: bool) -> Option<(u8, u8)> {
    swing(height, |aside| {
        brightest_turning_floor(height, aside, steady)
    })
}

/// Where the surface turns a long way across a pixel, as one far off does, a sharp highlight
/// falls between pixels and flashes as the view moves. Steadied, it's wider and dimmer, and
/// always there. Near, where it turns little across one, it's much as it was.
#[test]
fn a_surface_that_turns_across_a_pixel_shines_steadily() {
    let Some((sharp, sharp_least)) = turning_floor_swing(2000.0, false) else {
        return;
    };
    let (steady, steady_least) = turning_floor_swing(2000.0, true).expect("it drew");
    assert!(steady * 2 < sharp, "{sharp} {steady}");
    assert!(steady_least > sharp_least, "{sharp_least} {steady_least}");
    let near = brightest_turning_floor(64.0, 0.0, false).expect("it drew");
    let near_steady = brightest_turning_floor(64.0, 0.0, true).expect("it drew");
    assert!(near > 100, "{near}");
    assert!(near.abs_diff(near_steady) < 20, "{near} {near_steady}");
}

/// How fast land ripples, a turn every few grid points, which land kept to fewer points far off
/// still has.
const RIPPLE: f32 = 0.8;

/// All the light of black rippling land under a white light, shining sharply, seen from `height`
/// above and `aside` across, steadied or not.
fn rippling_land_light(height: f32, aside: f32, steady: bool) -> Option<u64> {
    let mut renderer = renderer()?;
    let mut built = painted_land([128, 128], |_| [255, 0]);
    let data = &mut built.terrain[0].data;
    let across = data.across();
    for (at, high) in (0_u32..).zip(&mut data.heights) {
        let (x, y) = (at % across, at / across);
        #[expect(clippy::cast_precision_loss, reason = "a small grid")]
        let (x, y) = (x as f32, y as f32);
        *high = ((x * RIPPLE).sin() + (y * RIPPLE).sin()) * 30.0;
    }
    let black = [0, 0, 0, 255].repeat(16);
    built.land_pictures[0].color = Arc::new(mip_chain(4, 4, black));
    built.land_pictures[0].surface = Arc::new(mip_chain(4, 4, [128, 128, 128, 255].repeat(16)));
    built.land_pictures[0].gloss = 200.0;
    renderer.load_map(Arc::new(built)).expect("load");
    renderer.set_post(PostSettings {
        steady_shine: steady,
        ..PostSettings::default()
    });
    // Away from the map's square, which is white.
    let over = Vec3::new(aside - 1000.0, -1000.0, height);
    let lights = [LightDraw {
        origin: over,
        radius: height * 4.0,
        min_light: 0.0,
    }];
    let drawables = Drawables {
        lights: &lights,
        ..Drawables::default()
    };
    let view = View {
        origin: over,
        ..view_from_above()
    };
    let capture = renderer.capture(&view, drawables, 64, 64).expect("capture");
    let (pixels, _) = capture.rgba.as_chunks::<4>();
    Some(pixels.iter().map(|pixel| u64::from(pixel[0])).sum())
}

/// How much all the light of the rippling land swings as the view moves across a pixel, steadied
/// or not, against how much there is.
fn rippling_land_flicker(height: f32, steady: bool) -> Option<(u64, u64)> {
    let pixel = height * 2.0 / 64.0;
    let mut seen = Vec::new();
    for step in 0..8_u8 {
        seen.push(rippling_land_light(
            height,
            pixel * f32::from(step) / 8.0,
            steady,
        )?);
    }
    let (least, most) = (*seen.iter().min()?, *seen.iter().max()?);
    Some((most - least, most))
}

/// Land shines as steadily, where it turns across a pixel: its sparkle comes and goes less as
/// the view moves.
#[test]
fn land_that_turns_across_a_pixel_shines_steadily() {
    let Some((sharp, sharp_all)) = rippling_land_flicker(1500.0, false) else {
        return;
    };
    let (steady, steady_all) = rippling_land_flicker(1500.0, true).expect("it drew");
    assert!(
        steady * sharp_all * 2 < sharp * steady_all,
        "{sharp} {steady}"
    );
}

/// The middle of white metal, smooth, with bumps too small to see, under a white light from above,
/// steadied or not.
fn bumpy_metal(steady: bool) -> Option<[u8; 4]> {
    let mut renderer = renderer()?;
    let mut built = map(1);
    built.textures[0].material = Some(Arc::new(Material {
        metal_rough: Some(MetalRough {
            metallic: 1.0,
            roughness: 0.2,
            map: None,
        }),
        slots: vec![Arc::new(normal_chain(64, 64, &striped_normals(1, 64)))],
        normal: Some(1),
        ..Material::default()
    }));
    renderer.load_map(Arc::new(built)).expect("load");
    renderer.set_post(PostSettings {
        steady_shine: steady,
        ..PostSettings::default()
    });
    let lights = [LightDraw {
        origin: Vec3::new(64.0, 64.0, 40.0),
        radius: 300.0,
        min_light: 0.0,
    }];
    let drawables = Drawables {
        lights: &lights,
        ..Drawables::default()
    };
    let capture = renderer
        .capture(&view_from_above(), drawables, 64, 64)
        .expect("capture");
    Some(centre(&capture))
}

/// Metal said to be smooth, with bumps too small to see, shines as the rougher thing it looks.
#[test]
fn bumps_too_small_to_see_roughen_metal() {
    let Some(sharp) = bumpy_metal(false) else {
        return;
    };
    let steady = bumpy_metal(true).expect("it drew");
    // Red is at the top with the floor's light, so green and blue tell.
    let bright = |pixel: [u8; 4]| u32::from(pixel[1]) + u32::from(pixel[2]);
    assert!(bright(steady) + 30 < bright(sharp), "{sharp:?} {steady:?}");
}

/// A flat surface shines just as it did, steadied or not.
#[test]
fn a_flat_surface_shines_as_it_did() {
    let flat = || normal_chain(64, 64, &[128, 128, 255, 255].repeat(64 * 64));
    let Some(sharp) = shiny_floor(flat(), false) else {
        return;
    };
    let steady = shiny_floor(flat(), true).expect("it drew");
    assert!(centre(&sharp)[0] > 100, "it shines");
    assert_eq!(sharp.rgba, steady.rgba);
}

/// Textures smoothed, with `samples` a pixel.
fn smooth_with_samples(renderer: &mut Renderer, samples: u32) {
    renderer
        .set_filter(TextureFilter {
            smooth: true,
            trilinear: true,
            anisotropy: 1.0,
        })
        .expect("filter");
    renderer.set_antialiasing(samples).expect("samples");
}

/// How many pixels inside a capture, away from its edge, are between `low` and `high` in red:
/// neither the surface nor what's behind it.
fn between(capture: &Capture, low: u8, high: u8) -> usize {
    let mut count = 0;
    for y in 4..capture.height - 4 {
        for x in 4..capture.width - 4 {
            let red = pixel(capture, x, y)[0];
            if red > low && red < high {
                count += 1;
            }
        }
    }
    count
}

/// The floor as one of Quake's pictures with holes and no material, holed past a slanted line,
/// from just above, with smoothed textures and `samples` a pixel.
fn quake_cut_out(samples: u32) -> Option<Capture> {
    let mut renderer = renderer()?;
    let mut built = map(1);
    built.textures[0].kind = TextureKind::Cutout;
    built.textures[0].mips = image_of(64, |x, y| {
        if x * 3 + y < 120 {
            [255, 255, 255, 255]
        } else {
            [0, 0, 0, 0]
        }
    });
    renderer.load_map(Arc::new(built)).expect("load");
    smooth_with_samples(&mut renderer, samples);
    // Near, so that a texel is a few pixels.
    let view = View {
        origin: Vec3::new(27.0, 40.0, 16.0),
        ..view_from_above()
    };
    Some(
        renderer
            .capture(&view, Drawables::default(), 64, 64)
            .expect("capture"),
    )
}

/// One of Quake's pictures with holes keeps them, and with several samples a pixel their edges
/// get shades between, as a masked material's do, where a hole's edge was all or nothing.
#[test]
fn a_quake_picture_with_holes_smooths_their_edges() {
    let Some(plain) = quake_cut_out(1) else {
        return;
    };
    let soft = quake_cut_out(4).expect("it drew");
    for shot in [&plain, &soft] {
        let (pixels, _) = shot.rgba.as_chunks::<4>();
        let solid = pixels.iter().filter(|pixel| pixel[0] > 150).count();
        let holes = pixels.iter().filter(|pixel| pixel[0] < 20).count();
        assert!(
            solid > 300 && holes > 300,
            "solid and holes: {solid} {holes}"
        );
    }
    let (hard, smooth) = (between(&plain, 20, 230), between(&soft, 20, 230));
    assert!(smooth > hard + 10, "{hard} {smooth}");
}

/// A sprite with a slanted hole, as `half_sprite`, but holed past a diagonal.
fn slanted_sprite() -> Sprite {
    let mut sprite = half_sprite();
    let image = &mut sprite.images[0];
    image.rgba = (0..32 * 32)
        .flat_map(|at: u32| {
            if at % 32 > at / 32 {
                [0, 0, 0, 0]
            } else {
                [255, 0, 255, 255]
            }
        })
        .collect();
    sprite
}

/// A sprite drawn close, with smoothed textures and `samples` a pixel.
fn sprite_with_samples(samples: u32) -> Option<Capture> {
    let mut renderer = renderer()?;
    renderer.load_map(Arc::new(map(1))).expect("load");
    smooth_with_samples(&mut renderer, samples);
    let number = renderer
        .add_sprite(Arc::new(slanted_sprite()))
        .expect("sprite");
    let sprites = [SpriteDraw {
        sprite: number,
        image: 0,
        origin: Vec3::new(40.0, 0.0, 0.0),
        angles: Vec3::ZERO,
    }];
    let drawables = Drawables {
        sprites: &sprites,
        ..Drawables::default()
    };
    Some(
        renderer
            .capture(&level_view(), drawables, 64, 64)
            .expect("capture"),
    )
}

/// A sprite's holes get shades between at their edges with several samples a pixel, as a masked
/// surface's do.
#[test]
fn a_sprite_smooths_its_holes_edges() {
    let Some(plain) = sprite_with_samples(1) else {
        return;
    };
    let soft = sprite_with_samples(4).expect("it drew");
    assert!(magenta(&soft, 0..64) > 100, "it shows");
    let (hard, smooth) = (between(&plain, 40, 200), between(&soft, 40, 200));
    assert!(smooth > hard + 10, "{hard} {smooth}");
}

/// A masked texture of one `alpha` all over: all hole at 0, and all solid at 255.
fn cut_out_texture(alpha: u8) -> Texture {
    let mut texture = map(1).textures[0].clone();
    texture.name = "cut".to_owned();
    texture.mips = image_of(4, |_, _| [255, 255, 255, alpha]);
    texture.material = Some(Arc::new(Material {
        alpha: AlphaMode::Mask,
        cutoff: 0.5,
        ..Material::default()
    }));
    texture
}

/// What casts a shadow, with holes in it.
#[derive(Clone, Copy, Debug)]
enum CutCaster {
    /// A brush model.
    Brush,
    /// A brush model in one of Quake's pictures with holes, with no material.
    Quake,
    /// A face of the world.
    World,
    /// A moving mesh.
    Moving,
    /// A moving mesh after another, whose places on the picture are its own.
    SecondMoving,
    /// A copy of a shared mesh.
    Copy,
}

/// How blue the floor is where `caster`, in a cut-out of `alpha`, would shade it from a light to
/// one side, without and with the light's shadows.
fn shade_of_a_cut_out(caster: CutCaster, alpha: u8) -> Option<[u8; 2]> {
    let mut renderer = renderer()?;
    let mut built = map(2);
    built.textures.push(cut_out_texture(alpha));
    let mut instances = Vec::new();
    let mut draws = Vec::new();
    let mut rows = NO_MOVE;
    rows[2][3] = 30.0;
    if matches!(caster, CutCaster::Quake) {
        let quake = &mut built.textures[1];
        quake.kind = TextureKind::Cutout;
        quake.material = None;
    }
    match caster {
        CutCaster::Brush | CutCaster::Quake => {
            built.faces[1].texture = 1;
            instances.push(ModelInstance {
                model: 1,
                origin: Vec3::new(64.0, 0.0, 30.0),
                visible: true,
                ..ModelInstance::default()
            });
        }
        CutCaster::World => {
            built.faces[1].texture = 1;
            for point in &mut built.points[4..8] {
                *point = *point + Vec3::new(64.0, 0.0, 30.0);
            }
            built.models.truncate(1);
            built.models[0].face_count = 2;
            built.tree.leaf_faces = vec![0, 1];
            built.tree.leaves[1].face_count = 2;
        }
        CutCaster::Moving | CutCaster::SecondMoving | CutCaster::Copy => {
            built.models.truncate(1);
            let mut square = square_of(1);
            // Where light from the lamp to the pixel looked at crosses its height.
            for corner in &mut square.mesh.corners {
                corner.position[0] += 16.0;
            }
            if matches!(caster, CutCaster::SecondMoving) {
                // The picture's left half is `alpha` and its right the other way, and the mesh
                // before is on its right, where this one's on its left.
                built.textures[1].mips = image_of(4, |x, _| {
                    let solid = if x < 2 { alpha } else { 255 - alpha };
                    [255, 255, 255, solid]
                });
                let mut before = square.clone();
                for corner in &mut before.mesh.corners {
                    corner.uv = [0.75, 0.5];
                }
                for corner in &mut square.mesh.corners {
                    corner.uv = [0.25, 0.5];
                }
                built.moving_meshes = vec![before, square];
                draws.push(MovingDraw {
                    mesh: 1,
                    rows,
                    joint_count: 1,
                    center: Vec3::new(80.0, 64.0, 30.0),
                    radius: 30.0,
                    ..MovingDraw::default()
                });
            } else if matches!(caster, CutCaster::Moving) {
                built.moving_meshes = vec![square];
                draws.push(MovingDraw {
                    rows,
                    joint_count: 1,
                    center: Vec3::new(80.0, 64.0, 30.0),
                    radius: 30.0,
                    ..MovingDraw::default()
                });
            } else {
                built.shared_meshes.push(SharedMesh {
                    center: Vec3::new(80.0, 64.0, 30.0),
                    radius: 30.0,
                    level_sizes: Vec::new(),
                    ..square.mesh
                });
                let stand_in = built.faces[0].clone();
                built.faces[1] = stand_in;
                built.models[0].face_count = 2;
                built.tree.leaf_faces = vec![0, 1];
                built.tree.leaves[1].face_count = 2;
                built.mesh_instances.push(MeshInstance {
                    mesh: 0,
                    rows,
                    first_face: 1,
                    face_count: 1,
                });
            }
        }
    }
    renderer.load_map(Arc::new(built)).expect("load");
    renderer.set_post(PostSettings {
        live_shadows: true,
        ..PostSettings::default()
    });
    let palette = [NO_MOVE];
    let shade = |renderer: &mut Renderer, shadows: bool| {
        let lights = [blue_light(Vec3::new(128.0, 64.0, 60.0), shadows)];
        let drawables = Drawables {
            instances: &instances,
            moving: &draws,
            palette: &palette,
            live_lights: &lights,
            ..Drawables::default()
        };
        let capture = renderer
            .capture(&view_from_above(), drawables, 64, 64)
            .expect("capture");
        pixel(&capture, 16, 32)[2]
    };
    Some([shade(&mut renderer, false), shade(&mut renderer, true)])
}

/// Light goes through a cut-out's holes to what's behind, and is stopped where it's solid, of a
/// brush model, one of Quake's pictures with holes, the world, a moving mesh and a copy of a
/// shared mesh alike.
#[test]
fn light_goes_through_a_cut_outs_holes_to_a_shadow() {
    for caster in [
        CutCaster::Brush,
        CutCaster::Quake,
        CutCaster::World,
        CutCaster::Moving,
        CutCaster::SecondMoving,
        CutCaster::Copy,
    ] {
        let Some([open, through]) = shade_of_a_cut_out(caster, 0) else {
            return;
        };
        let [_, blocked] = shade_of_a_cut_out(caster, 255).expect("it drew");
        assert!(open > 60, "{caster:?} is lit: {open}");
        assert!(
            through + 10 > open,
            "through {caster:?}'s holes: {open} {through}"
        );
        assert!(
            blocked + 20 < open,
            "not where {caster:?}'s solid: {open} {blocked}"
        );
    }
}

/// The middle of the floor, with probes over it, under a live light that shines straight up and
/// away from it, which bounces `bounce`.
fn floor_under_a_light_shining_up(bounce: f32, probes: bool) -> Option<[u8; 4]> {
    let mut renderer = renderer()?;
    let mut built = map(1);
    // None of the floor's own light, so what's there is what's bounced.
    built.light = vec![[0, 0, 0]; built.light.len()];
    if probes {
        let count = 5 * 5 * 3;
        built.probes = Some(ProbeGrid {
            origin: Vec3::ZERO,
            spacing: 32.0,
            size: [5, 5, 3],
            colors: vec![[[0; 3]; 6]; count],
            directions: vec![[0; 3]; count],
            sun_shares: vec![0; count],
        });
    }
    renderer.load_map(Arc::new(built)).expect("load");
    let lights = [LiveLight {
        origin: Vec3::new(64.0, 64.0, 40.0),
        radius: 300.0,
        color: [1.0; 3],
        spot: Vec3::new(0.0, 0.0, 1.0),
        spot_cosine: 0.9,
        bounce,
        ..LiveLight::default()
    }];
    let drawables = Drawables {
        live_lights: &lights,
        ..Drawables::default()
    };
    let capture = renderer
        .capture(&view_from_above(), drawables, 64, 64)
        .expect("capture");
    Some(centre(&capture))
}

/// What a live light bounces reaches what it doesn't light itself, from the probes round it, and
/// none of it without its bounce, or with no probes for it to be at.
#[test]
fn a_live_lights_bounce_reaches_what_it_does_not_light() {
    let Some(none) = floor_under_a_light_shining_up(0.0, true) else {
        return;
    };
    let bounced = floor_under_a_light_shining_up(1.0, true).expect("it drew");
    let no_probes = floor_under_a_light_shining_up(1.0, false).expect("it drew");
    assert!(none[0] < 5, "it doesn't light the floor itself: {none:?}");
    assert!(bounced[0] > none[0] + 20, "{none:?} {bounced:?}");
    assert_eq!(no_probes, none, "nowhere for it to be");
}

/// A live light that changes has what it bounces worked out again, in its new colour.
#[test]
fn a_live_lights_bounce_follows_it() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut built = map(1);
    built.light = vec![[0, 0, 0]; built.light.len()];
    let count = 5 * 5 * 3;
    built.probes = Some(ProbeGrid {
        origin: Vec3::ZERO,
        spacing: 32.0,
        size: [5, 5, 3],
        colors: vec![[[0; 3]; 6]; count],
        directions: vec![[0; 3]; count],
        sun_shares: vec![0; count],
    });
    renderer.load_map(Arc::new(built)).expect("load");
    let mut shot = |color: [f32; 3]| {
        let lights = [LiveLight {
            origin: Vec3::new(64.0, 64.0, 40.0),
            radius: 300.0,
            color,
            spot: Vec3::new(0.0, 0.0, 1.0),
            spot_cosine: 0.9,
            bounce: 1.0,
            ..LiveLight::default()
        }];
        let drawables = Drawables {
            live_lights: &lights,
            ..Drawables::default()
        };
        let capture = renderer
            .capture(&view_from_above(), drawables, 64, 64)
            .expect("capture");
        centre(&capture)
    };
    let white = shot([1.0; 3]);
    // Only its green changes, so a light kept by less than its whole colour would show.
    let magenta = shot([1.0, 0.0, 1.0]);
    assert!(white[1] > 20, "{white:?}");
    assert!(magenta[1] < white[1] / 4, "{white:?} {magenta:?}");
}

/// Post settings with the scene drawn at `scale` of the screen, and `edge_smoothing`.
fn scaled(scale: f32, edge_smoothing: u8) -> PostSettings {
    PostSettings {
        render_scale: scale,
        edge_smoothing,
        ..PostSettings::default()
    }
}

/// The floor in black and white squares, from above and turned, `frames` frames at `post`.
fn scaled_floor(renderer: &mut Renderer, post: PostSettings, frames: u32) -> Capture {
    renderer.set_post(post);
    let view = slanted_view();
    let mut last = None;
    for _ in 0..frames {
        last = Some(
            renderer
                .capture(&view, Drawables::default(), 64, 64)
                .expect("capture"),
        );
    }
    last.expect("a frame")
}

/// A scene drawn at another size is still the screen's size when it's shown, and as it was where
/// it's flat. Drawn at twice, its edges are smoothed, as there's more of each pixel.
#[test]
fn a_scene_drawn_at_another_size_fills_the_screen() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let plain = scaled_floor(&mut renderer, scaled(1.0, 0), 1);
    let half = scaled_floor(&mut renderer, scaled(0.5, 0), 1);
    let twice = scaled_floor(&mut renderer, scaled(2.0, 0), 1);
    for shot in [&half, &twice] {
        assert_eq!((shot.width, shot.height), (64, 64));
        let flat = centre(shot)
            .iter()
            .zip(centre(&plain))
            .all(|(a, b)| a.abs_diff(b) <= 2);
        assert!(flat, "{:?} {:?}", centre(shot), centre(&plain));
    }
    assert!(
        shades(&twice) > shades(&plain),
        "{} {}",
        shades(&plain),
        shades(&twice)
    );
    let again = scaled_floor(&mut renderer, scaled(0.0, 0), 1);
    assert_eq!(again.rgba, plain.rgba, "0 is the screen's size");
}

/// How much each pixel differs from the next across, over a capture: what's fine in it.
fn fineness(capture: &Capture) -> u64 {
    let mut total = 0;
    for y in 0..capture.height {
        for x in 1..capture.width {
            total += u64::from(pixel(capture, x, y)[0].abs_diff(pixel(capture, x - 1, y)[0]));
        }
    }
    total
}

/// Drawn at a half again each way, each of the screen's pixels takes all the scene's under it, so
/// a fine pattern's averaged, not picked at.
#[test]
fn a_bigger_scene_is_averaged_under_each_pixel() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer
        .load_map(Arc::new(checkered_floor(1)))
        .expect("load");
    let plain = scaled_floor(&mut renderer, scaled(1.0, 0), 1);
    let bigger = scaled_floor(&mut renderer, scaled(1.5, 0), 1);
    // Four blends across the pixel leave under a third of the fine pattern; one, more.
    assert!(
        fineness(&bigger) * 10 < fineness(&plain) * 3,
        "{} {}",
        fineness(&plain),
        fineness(&bigger)
    );
}

/// A grey floor with lighter stripes, 4 texels wide every 16.
fn striped_floor() -> Map {
    let mut built = map(1);
    built.textures[0].mips = image_of(64, |x, _| {
        if x % 16 < 4 {
            [150, 150, 150, 255]
        } else {
            [100, 100, 100, 255]
        }
    });
    built
}

/// Brought up to the screen, what's between pixels is never lighter or darker than the pixels
/// round it, so an edge doesn't ring.
#[test]
fn what_is_brought_up_does_not_ring() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(striped_floor())).expect("load");
    let plain = scaled_floor(&mut renderer, scaled(1.0, 0), 1);
    let half = scaled_floor(&mut renderer, scaled(0.5, 0), 1);
    // Inside the floor, away from its edges, which blend with what's past them.
    let range = |shot: &Capture| {
        let mut greens = Vec::new();
        for y in 20..44 {
            for x in 20..44 {
                greens.push(pixel(shot, x, y)[1]);
            }
        }
        greens.into_iter().fold((u8::MAX, 0), |(low, high), green| {
            (low.min(green), high.max(green))
        })
    };
    let (low, high) = range(&plain);
    let (half_low, half_high) = range(&half);
    assert!(
        half_low + 2 >= low && half_high <= high + 2,
        "{low}..{high} {half_low}..{half_high}"
    );
}

/// With TAA, a scene drawn at half the screen's size comes nearer to one drawn at its size, over
/// frames, than one brought up without it.
#[test]
fn taa_brings_a_smaller_scene_up_nearer() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer
        .load_map(Arc::new(checkered_floor(1)))
        .expect("load");
    let full = scaled_floor(&mut renderer, scaled(1.0, 3), 64);
    let brought_up = scaled_floor(&mut renderer, scaled(0.5, 0), 1);
    let taa_up = scaled_floor(&mut renderer, scaled(0.5, 3), 64);
    let (plain_off, taa_off) = (difference(&brought_up, &full), difference(&taa_up, &full));
    // Each of the frame's pixels where it truly is, by how near in the screen's pixels, at the
    // screen's size, with the moves going round four times as many frames as at the screen's
    // size: under half as far off as bringing up alone. Any of those undone is nearer two
    // thirds, and the shorter round half.
    assert!(taa_off * 100 < plain_off * 49, "{taa_off} {plain_off}");
    // Sharpened after, it has more shades than the screen's size has, where without it about as
    // many as it would have soft.
    assert!(
        shades(&taa_up) > shades(&full) * 2,
        "{} {}",
        shades(&taa_up),
        shades(&full)
    );
}

/// The middle of a floor whose texture's first level is red and every other green, seen from
/// above at `post`, `frames` frames.
fn mip_colour(post: PostSettings, frames: u32) -> Option<[u8; 4]> {
    let mut renderer = renderer()?;
    let mut built = map(1);
    let mut mips = mip_chain(64, 64, [255, 0, 0, 255].repeat(64 * 64));
    for level in &mut mips[1..] {
        for texel in level.rgba.as_chunks_mut::<4>().0 {
            *texel = [0, 255, 0, 255];
        }
    }
    built.textures[0].mips = mips;
    renderer.load_map(Arc::new(built)).expect("load");
    renderer
        .set_filter(TextureFilter {
            smooth: true,
            trilinear: false,
            anisotropy: 1.0,
        })
        .expect("filter");
    renderer.set_post(post);
    // Near enough that the screen's size reads the first level, and half of it the next.
    let view = View {
        origin: Vec3::new(64.0, 64.0, 40.0),
        ..view_from_above()
    };
    let mut last = None;
    for _ in 0..frames {
        last = Some(
            renderer
                .capture(&view, Drawables::default(), 64, 64)
                .expect("capture"),
        );
    }
    Some(centre(&last.expect("a frame")))
}

/// Drawn smaller with TAA, textures are read as finely as at the screen's size, for TAA to find
/// their detail; without TAA they're read as the scene's size says.
#[test]
fn a_smaller_scene_reads_fine_mips_only_with_taa() {
    let Some(full) = mip_colour(scaled(1.0, 0), 1) else {
        return;
    };
    let half = mip_colour(scaled(0.5, 0), 1).expect("it drew");
    let half_taa = mip_colour(scaled(0.5, 3), 8).expect("it drew");
    assert!(
        full[0] > full[1],
        "the first level at the screen's size: {full:?}"
    );
    assert!(half[1] > half[0], "a coarser one, drawn smaller: {half:?}");
    assert!(
        half_taa[0] > half_taa[1],
        "the first, with TAA: {half_taa:?}"
    );
}

/// SMAA's smoothing comes along when a smaller scene's brought up.
#[test]
fn smaa_comes_along_when_a_scene_is_brought_up() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let plain = scaled_floor(&mut renderer, scaled(0.5, 0), 1);
    let smoothed = scaled_floor(&mut renderer, scaled(0.5, 2), 1);
    assert_ne!(plain.rgba, smoothed.rgba);
    assert_eq!(centre(&plain), centre(&smoothed), "what's flat stays");
}
