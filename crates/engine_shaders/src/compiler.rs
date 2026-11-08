//! Turning WGSL into SPIR-V, which a finished game has no need of (proposal 0133).

use naga::back::spv;
use naga::valid::{Capabilities, ValidationFlags, Validator};

use crate::Module;

/// The module surface code joins.
const WORLD_SHADER: &str = concat!(
    include_str!("shaders/frame.wgsl"),
    include_str!("shaders/world.wgsl"),
    include_str!("shaders/noise.wgsl"),
    include_str!("shaders/material.wgsl"),
    include_str!("shaders/sky_noise.wgsl"),
    include_str!("shaders/sky.wgsl"),
);

/// The final pass, which post code joins.
const POST_SHADER: &str = concat!(
    include_str!("shaders/frame.wgsl"),
    include_str!("shaders/sky_noise.wgsl"),
    include_str!("shaders/post.wgsl"),
);

/// What field code joins.
const FIELD_SHADER: &str = concat!(
    include_str!("shaders/noise.wgsl"),
    include_str!("shaders/field.wgsl"),
);

/// The shader of models.
const ALIAS_SHADER: &str = concat!(
    include_str!("shaders/frame.wgsl"),
    include_str!("shaders/alias.wgsl"),
);

/// The shader of sprites.
const SPRITE_SHADER: &str = concat!(
    include_str!("shaders/frame.wgsl"),
    include_str!("shaders/sprite.wgsl"),
);

/// Ripples on water.
const RIPPLE_SHADER: &str = include_str!("shaders/ripple.wgsl");

/// Where shaped water's surfaces are.
const WATER_SHADER: &str = concat!(
    include_str!("shaders/frame.wgsl"),
    include_str!("shaders/water.wgsl"),
);

/// What particle code joins.
const PARTICLE_SHADER: &str = concat!(
    include_str!("shaders/noise.wgsl"),
    include_str!("shaders/sim_common.wgsl"),
    include_str!("shaders/particle.wgsl"),
);

/// What motion code joins.
const MOTION_SHADER: &str = concat!(
    include_str!("shaders/noise.wgsl"),
    include_str!("shaders/sim_common.wgsl"),
    include_str!("shaders/motion.wgsl"),
);

/// The canvas's shader.
const CANVAS_SHADER: &str = include_str!("shaders/canvas.wgsl");

/// The shader of moving lights' shadows.
const SHADOW_SHADER: &str = include_str!("shaders/shadow.wgsl");

/// The shader of edge smoothing.
const DEFORM_SHADER: &str = include_str!("shaders/deform.wgsl");

/// Edge smoothing.
const SMAA_SHADER: &str = include_str!("shaders/smaa.wgsl");

/// What glows.
const BLOOM_SHADER: &str = include_str!("shaders/bloom.wgsl");

/// Which way what moves went, for TAA.
const MOTION_VECTORS_SHADER: &str = concat!(
    include_str!("shaders/frame.wgsl"),
    include_str!("shaders/motion_vectors.wgsl"),
);

/// The scene's samples brought to one a pixel.
const RESOLVE_SHADER: &str = concat!(
    include_str!("shaders/frame.wgsl"),
    include_str!("shaders/resolve.wgsl"),
);

/// The hooks of a surface, and what each does if a shader leaves it alone.
const SURFACE_HOOKS: [(&str, &str); 4] = [
    (
        "fn shader_vertex",
        "fn shader_vertex(world: vec3<f32>) -> vec3<f32> { return world; }\n",
    ),
    (
        "fn shader_uv",
        "fn shader_uv(uv: vec2<f32>, world: vec3<f32>) -> vec2<f32> { return uv; }\n",
    ),
    (
        "fn shader_surface",
        "fn shader_surface(surface: Surface) -> Surface { return surface; }\n",
    ),
    (
        "fn shader_whole",
        "fn shader_whole(in: VertexOut) -> vec4<f32> { return material_color(in); }\n",
    ),
];

/// The hook of the final pass, and what it does if it's left alone.
const POST_HOOK: (&str, &str) = (
    "fn shader_post",
    "fn shader_post(color: vec3<f32>, uv: vec2<f32>) -> vec3<f32> { return color; }\n",
);

/// The hook of a field, and what it does if it's left alone.
const FIELD_HOOK: (&str, &str) = (
    "fn shader_field",
    "fn shader_field(at: vec2<i32>, size: vec2<i32>) -> vec4<f32> { return field_read(at); }\n",
);

/// The hook of particles, and what it does if it's left alone.
const PARTICLE_HOOK: (&str, &str) = (
    "fn shader_particle",
    "fn shader_particle(look: ParticleLook) -> vec4<f32> { return look.color; }\n",
);

/// The hook of compute particles, and what it does if it's left alone.
const MOTION_HOOK: (&str, &str) = (
    "fn shader_motion",
    "fn shader_motion(particle: SimParticle, seconds: f32) -> SimParticle { return particle; }\n",
);

/// The whole module of compute particles with `code`.
pub fn motion_source(code: &str) -> String {
    let (name, standing_in) = MOTION_HOOK;
    let mut source = MOTION_SHADER.to_owned();
    if !code.contains(name) {
        source.push_str(standing_in);
    }
    source.push_str(code);
    source
}

/// The whole module of particles with `code`.
pub fn particle_source(code: &str) -> String {
    let (name, standing_in) = PARTICLE_HOOK;
    let mut source = PARTICLE_SHADER.to_owned();
    if !code.contains(name) {
        source.push_str(standing_in);
    }
    source.push_str(code);
    source
}

/// The whole module of a field with `code`.
pub fn field_source(code: &str) -> String {
    let (name, standing_in) = FIELD_HOOK;
    let mut source = FIELD_SHADER.to_owned();
    if !code.contains(name) {
        source.push_str(standing_in);
    }
    source.push_str(code);
    source
}

/// The whole module of a surface with `code`.
pub fn surface_source(code: &str) -> String {
    let mut source = WORLD_SHADER.to_owned();
    for (name, standing_in) in SURFACE_HOOKS {
        if !code.contains(name) {
            source.push_str(standing_in);
        }
    }
    source.push_str(code);
    source
}

/// The whole module of the final pass with `code`.
pub fn post_source(code: &str) -> String {
    let (name, standing_in) = POST_HOOK;
    let mut source = POST_SHADER.to_owned();
    if !code.contains(name) {
        source.push_str(standing_in);
    }
    source.push_str(code);
    source
}

/// The WGSL of `module`.
fn source_of(module: &Module<'_>) -> String {
    match module {
        Module::Canvas => CANVAS_SHADER.to_owned(),
        Module::Alias => ALIAS_SHADER.to_owned(),
        Module::Sprite => SPRITE_SHADER.to_owned(),
        Module::Shadow => SHADOW_SHADER.to_owned(),
        Module::Smaa => SMAA_SHADER.to_owned(),
        Module::Bloom => BLOOM_SHADER.to_owned(),
        Module::MotionVectors => MOTION_VECTORS_SHADER.to_owned(),
        Module::Resolve => RESOLVE_SHADER.to_owned(),
        Module::Deform => DEFORM_SHADER.to_owned(),
        Module::Surface(code) => surface_source(code),
        Module::Field(code) => field_source(code),
        Module::Water => WATER_SHADER.to_owned(),
        Module::Ripple => RIPPLE_SHADER.to_owned(),
        Module::Motion { code, samples } => {
            let source = motion_source(code);
            if *samples {
                source.replace("texture_depth_2d", "texture_depth_multisampled_2d")
            } else {
                source
            }
        }
        Module::Particle { code, samples } => {
            let source = particle_source(code);
            if *samples {
                source.replace("texture_depth_2d", "texture_depth_multisampled_2d")
            } else {
                source
            }
        }
        Module::PlainSurface => {
            surface_source("").replace("const PLAIN: bool = false;", "const PLAIN: bool = true;")
        }
        Module::Post { code, samples } => {
            let source = post_source(code);
            if *samples {
                source.replace("texture_depth_2d", "texture_depth_multisampled_2d")
            } else {
                source
            }
        }
    }
}

/// `module` as SPIR-V.
///
/// # Errors
///
/// Returns what the compiler said.
pub fn compile(module: &Module<'_>) -> Result<Vec<u32>, String> {
    let source = source_of(module);
    let parsed =
        naga::front::wgsl::parse_str(&source).map_err(|error| error.emit_to_string(&source))?;
    let info = Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&parsed)
        .map_err(|error| error.emit_to_string(&source))?;
    let options = spv::Options {
        lang_version: (1, 0),
        ..spv::Options::default()
    };
    spv::write_vec(&parsed, &info, &options, None).map_err(|error| error.to_string())
}

/// What's wrong with `source` as a whole module, if anything.
///
/// # Errors
///
/// Returns what the compiler said.
fn check_module(source: &str) -> Result<(), String> {
    let module =
        naga::front::wgsl::parse_str(source).map_err(|error| error.emit_to_string(source))?;
    Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .map(|_| ())
        .map_err(|error| error.emit_to_string(source))
}

/// What's wrong with `code` as surface code, if anything.
///
/// # Errors
///
/// Returns what the compiler said.
pub fn check_surface_code(code: &str) -> Result<(), String> {
    check_module(&surface_source(code))
}

/// What's wrong with `code` as compute particles', if anything.
///
/// # Errors
///
/// Returns what the compiler said.
pub fn check_motion_code(code: &str) -> Result<(), String> {
    check_module(&motion_source(code))
}

/// What's wrong with `code` as a particle's, if anything.
///
/// # Errors
///
/// Returns what the compiler said.
pub fn check_particle_code(code: &str) -> Result<(), String> {
    check_module(&particle_source(code))
}

/// What's wrong with `code` as a field's, if anything.
///
/// # Errors
///
/// Returns what the compiler said.
pub fn check_field_code(code: &str) -> Result<(), String> {
    check_module(&field_source(code))
}

/// What's wrong with `code` as post code, if anything.
///
/// # Errors
///
/// Returns what the compiler said.
pub fn check_post_code(code: &str) -> Result<(), String> {
    check_module(&post_source(code))
}
