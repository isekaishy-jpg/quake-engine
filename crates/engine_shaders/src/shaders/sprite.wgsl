// Sprites (proposal 0036).

struct Frame {
    view_projection: mat4x4<f32>,
    eye: vec4<f32>,
    // x is the time in seconds, and y the width over the height.
    params: vec4<f32>,
}

struct Draw {
    origin: vec4<f32>,
    // w is how far the left edge is from the origin.
    right: vec4<f32>,
    // w is how far the top edge is above the origin.
    up: vec4<f32>,
    // The size in pixels.
    size: vec4<f32>,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var picture: texture_2d<f32>;
@group(1) @binding(1) var picture_sampler: sampler;

var<immediate> draw: Draw;

// Two triangles, as corners from 0 to 1.
fn corner(index: u32) -> vec2<f32> {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    return corners[index];
}

struct SpriteOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_sprite(@builtin(vertex_index) index: u32) -> SpriteOut {
    let uv = corner(index);
    let across = draw.right.w + uv.x * draw.size.x;
    let above = draw.up.w - uv.y * draw.size.y;
    let world = draw.origin.xyz + draw.right.xyz * across + draw.up.xyz * above;
    var out: SpriteOut;
    out.clip = frame.view_projection * vec4<f32>(world, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs_sprite(in: SpriteOut) -> @location(0) vec4<f32> {
    let texel = textureSample(picture, picture_sampler, in.uv);
    if texel.a < 0.5 {
        discard;
    }
    return vec4<f32>(texel.rgb, 1.0);
}
