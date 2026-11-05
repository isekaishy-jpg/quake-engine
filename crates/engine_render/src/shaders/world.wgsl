// The world: lit surfaces, cutouts, liquids and sky (proposal 0018).

struct Frame {
    view_projection: mat4x4<f32>,
    eye: vec4<f32>,
    // x is the time in seconds, y the width over the height, and z how many
    // dynamic lights there are.
    params: vec4<f32>,
    // 64 light style brightnesses, four to an entry.
    styles: array<vec4<f32>, 16>,
    // Each dynamic light's position, then its radius.
    lights: array<vec4<f32>, 32>,
    // Each dynamic light's least light, four to an entry.
    light_cuts: array<vec4<f32>, 8>,
    // The sky: its colours, then w of the first is the horizon's height.
    sky_zenith: vec4<f32>,
    sky_horizon: vec4<f32>,
    sky_ground: vec4<f32>,
    // xyz towards the sun, w its size.
    sky_sun: vec4<f32>,
    sky_sun_color: vec4<f32>,
    // Colour, then w is the coverage.
    sky_cloud: vec4<f32>,
    // xy speed, z scale, w 0 for none, 1 procedural, 2 from an image.
    sky_cloud_params: vec4<f32>,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var lightmap: texture_2d<f32>;
@group(0) @binding(2) var lightmap_sampler: sampler;
@group(1) @binding(0) var surface: texture_2d<f32>;
@group(1) @binding(1) var surface_sampler: sampler;

// Where the model being drawn has moved to.
var<immediate> model_offset: vec4<f32>;

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
    let world = in.position + model_offset.xyz;
    out.clip = frame.view_projection * vec4<f32>(world, 1.0);
    out.world = world;
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

// Light from `R_AddDynamicLights`, on the same scale as a light map sample.
// Its plane is worked out from the pixel, since each face is flat.
fn dynamic_light(world: vec3<f32>) -> f32 {
    let normal = normalize(cross(dpdx(world), dpdy(world)));
    var total = 0.0;
    let count = u32(frame.params.z);
    for (var index = 0u; index < count; index = index + 1u) {
        let light = frame.lights[index];
        let cut = frame.light_cuts[index / 4u][index % 4u];
        let offset = world - light.xyz;
        let height = abs(dot(offset, normal));
        let reach = light.w - height;
        let across = sqrt(max(dot(offset, offset) - height * height, 0.0));
        // The original's edge is blurred by its 16 unit light map samples.
        let edge = clamp((reach - cut - across) / 16.0, 0.0, 1.0);
        total = total + max(reach - across, 0.0) * edge;
    }
    return total / 255.0;
}

// All the light on a pixel. A sample of one half leaves a texture unchanged.
fn light(in: VertexOut) -> vec3<f32> {
    let baked = layer(in, 0.0, in.styles.x) + layer(in, 1.0, in.styles.y)
        + layer(in, 2.0, in.styles.z) + layer(in, 3.0, in.styles.w);
    return min(2.0 * (baked + vec3<f32>(dynamic_light(in.world))), vec3<f32>(2.0));
}

// The brightness setting, as `BuildGammaTable` does for the palette.
fn raised(color: vec3<f32>) -> vec3<f32> {
    return pow(max(color, vec3<f32>(0.0)), vec3<f32>(frame.params.w));
}

@fragment
fn fs_solid(in: VertexOut) -> @location(0) vec4<f32> {
    let texel = textureSample(surface, surface_sampler, in.uv);
    let lit = texel.rgb * light(in);
    return vec4<f32>(raised(mix(texel.rgb, lit, texel.a)), 1.0);
}

@fragment
fn fs_cutout(in: VertexOut) -> @location(0) vec4<f32> {
    let texel = textureSample(surface, surface_sampler, in.uv);
    let lit = texel.rgb * light(in);
    if texel.a < 0.5 {
        discard;
    }
    return vec4<f32>(raised(lit), 1.0);
}

@fragment
fn fs_liquid(in: VertexOut) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(surface, 0));
    let texels = in.uv * size;
    let time = frame.params.x;
    let warped = texels + 8.0 * sin(texels.yx * 0.125 + time);
    let texel = textureSample(surface, surface_sampler, warped / size);
    return vec4<f32>(raised(texel.rgb), 1.0);
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
    return vec4<f32>(raised(mix(back.rgb, front.rgb, front.a)), 1.0);
}
