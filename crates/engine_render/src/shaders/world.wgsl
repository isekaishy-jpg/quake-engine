// The world: lit surfaces, cutouts, liquids and sky (proposal 0018).

struct Frame {
    view_projection: mat4x4<f32>,
    eye: vec4<f32>,
    // x is the time in seconds.
    params: vec4<f32>,
    // 64 light style brightnesses, four to an entry.
    styles: array<vec4<f32>, 16>,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var lightmap: texture_2d<f32>;
@group(0) @binding(2) var lightmap_sampler: sampler;
@group(1) @binding(0) var surface: texture_2d<f32>;
@group(1) @binding(1) var surface_sampler: sampler;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) light_uv: vec2<f32>,
    @location(3) light_step: f32,
    @location(4) styles: vec4<u32>,
}

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) light_uv: vec2<f32>,
    @location(3) light_step: f32,
    @location(4) @interpolate(flat) styles: vec4<u32>,
}

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip = frame.view_projection * vec4<f32>(in.position, 1.0);
    out.world = in.position;
    out.uv = in.uv;
    out.light_uv = in.light_uv;
    out.light_step = in.light_step;
    out.styles = in.styles;
    return out;
}

// The light from one layer, or none for an unused one.
fn layer(in: VertexOut, slot: f32, style: u32) -> vec3<f32> {
    if style >= 64u {
        return vec3<f32>(0.0);
    }
    let uv = in.light_uv + vec2<f32>(in.light_step * slot, 0.0);
    let sample = textureSampleLevel(lightmap, lightmap_sampler, uv, 0.0).rgb;
    return sample * frame.styles[style / 4u][style % 4u];
}

// All the light on a pixel. A sample of one half leaves a texture unchanged.
fn light(in: VertexOut) -> vec3<f32> {
    return 2.0 * (layer(in, 0.0, in.styles.x) + layer(in, 1.0, in.styles.y)
        + layer(in, 2.0, in.styles.z) + layer(in, 3.0, in.styles.w));
}

@fragment
fn fs_solid(in: VertexOut) -> @location(0) vec4<f32> {
    let texel = textureSample(surface, surface_sampler, in.uv);
    let lit = texel.rgb * light(in);
    return vec4<f32>(mix(texel.rgb, lit, texel.a), 1.0);
}

@fragment
fn fs_cutout(in: VertexOut) -> @location(0) vec4<f32> {
    let texel = textureSample(surface, surface_sampler, in.uv);
    if texel.a < 0.5 {
        discard;
    }
    return vec4<f32>(texel.rgb * light(in), 1.0);
}

@fragment
fn fs_liquid(in: VertexOut) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(surface, 0));
    let texels = in.uv * size;
    let time = frame.params.x;
    let warped = texels + 8.0 * sin(texels.yx * 0.125 + time);
    let texel = textureSample(surface, surface_sampler, warped / size);
    return vec4<f32>(texel.rgb, 1.0);
}

@fragment
fn fs_sky(in: VertexOut) -> @location(0) vec4<f32> {
    var direction = in.world - frame.eye.xyz;
    direction.z = direction.z * 3.0;
    let flat_direction = normalize(direction).xy * (6.0 * 63.0);
    let time = frame.params.x;
    let back_uv = fract((flat_direction + time * 8.0) / 128.0);
    let front_uv = fract((flat_direction + time * 16.0) / 128.0);
    let back = textureSampleLevel(
        surface, surface_sampler, vec2<f32>(0.5 + back_uv.x * 0.5, back_uv.y), 0.0);
    let front = textureSampleLevel(
        surface, surface_sampler, vec2<f32>(front_uv.x * 0.5, front_uv.y), 0.0);
    return vec4<f32>(mix(back.rgb, front.rgb, front.a), 1.0);
}
