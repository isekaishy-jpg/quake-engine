//! Every shader the engine has, and surface code, which is a shader's own part in
//! how a surface is drawn (proposal 0107).
//!
//! A shader is a piece of WGSL that replaces some of the hooks below. What it
//! leaves alone goes on working, so a short one can change one thing about an
//! ordinary lit surface, and one that replaces `shader_whole` can do anything.
//!
//! This needs no GPU, so tools can check shaders.

/// The module surface code joins.
const WORLD_SHADER: &str = concat!(
    include_str!("shaders/frame.wgsl"),
    include_str!("shaders/world.wgsl"),
    include_str!("shaders/material.wgsl"),
    include_str!("shaders/sky.wgsl"),
);

/// The final pass, which post code joins.
const POST_SHADER: &str = concat!(
    include_str!("shaders/frame.wgsl"),
    include_str!("shaders/post.wgsl"),
);

/// The shader of models.
pub const ALIAS_SHADER: &str = concat!(
    include_str!("shaders/frame.wgsl"),
    include_str!("shaders/alias.wgsl"),
);

/// The shader of sprites and particles.
pub const SPRITE_SHADER: &str = include_str!("shaders/sprite.wgsl");

/// The canvas's shader.
pub const CANVAS_SHADER: &str = include_str!("shaders/canvas.wgsl");

/// The shader of moving lights' shadows.
pub const SHADOW_SHADER: &str = include_str!("shaders/shadow.wgsl");

/// The shader of edge smoothing.
pub const SMAA_SHADER: &str = include_str!("shaders/smaa.wgsl");

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

/// What shows when a shader is missing or broken: green, so it can't be missed.
pub const BROKEN: &str = "fn shader_whole(in: VertexOut) -> vec4<f32> {
    return vec4<f32>(scene_color(vec3<f32>(0.0, 1.0, 0.0)), 1.0);
}
";

/// The engine's own shaders, for pictures with no material. A container can hold
/// its own under the same names.
pub const BUILT_IN: [(&str, &str); 4] = [
    ("classic", include_str!("shaders/surfaces/classic.wgsl")),
    (
        "classic_cutout",
        include_str!("shaders/surfaces/classic_cutout.wgsl"),
    ),
    (
        "classic_liquid",
        include_str!("shaders/surfaces/classic_liquid.wgsl"),
    ),
    (
        "classic_sky",
        include_str!("shaders/surfaces/classic_sky.wgsl"),
    ),
];

/// The names of the engine's own shaders.
#[must_use]
pub fn built_in_shaders() -> [&'static str; 4] {
    BUILT_IN.map(|(name, _)| name)
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

/// Whether a surface with `code` can be drawn after the depth pass, which draws
/// depth without it. Moving vertices or making holes changes depth.
pub fn safe_after_depth(code: &str) -> bool {
    !code.contains("fn shader_vertex") && !code.contains("discard")
}

/// What's wrong with `source` as a whole module, if anything.
///
/// # Errors
///
/// Returns what the compiler said.
pub fn check_module(source: &str) -> Result<(), String> {
    use naga::valid::{Capabilities, ValidationFlags, Validator};
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

/// What's wrong with `code` as post code, if anything.
///
/// # Errors
///
/// Returns what the compiler said.
pub fn check_post_code(code: &str) -> Result<(), String> {
    check_module(&post_source(code))
}
