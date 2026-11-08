// Flat pictures over the view (proposal 0039).

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var picture_sampler: sampler;

// What screen the canvas is sent to: x 0 for a standard one, 1 scRGB and 2 HDR10, and y how
// bright white is in nits.
struct Screen {
    told: vec4<f32>,
}

var<immediate> screen: Screen;

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
) -> VertexOut {
    var out: VertexOut;
    out.clip = vec4<f32>(position, 0.0, 1.0);
    out.uv = uv;
    out.color = color;
    return out;
}

// Rec. 709's colours as Rec. 2020's.
const TO_REC2020: mat3x3<f32> = mat3x3<f32>(
    vec3<f32>(0.6274, 0.0691, 0.0164),
    vec3<f32>(0.3293, 0.9195, 0.0880),
    vec3<f32>(0.0433, 0.0114, 0.8956),
);

// An sRGB colour as linear light.
fn linear(color: vec3<f32>) -> vec3<f32> {
    let low = color / 12.92;
    let high = pow((color + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(high, low, color <= vec3<f32>(0.04045));
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let color = textureSample(picture, picture_sampler, in.uv) * in.color;
    if screen.told.x < 0.5 {
        return color;
    }
    // What's drawn for a standard screen is sRGB, which on an HDR one is white at its white.
    let nits = linear(color.rgb) * screen.told.y;
    if screen.told.x < 1.5 {
        return vec4<f32>(nits / 80.0, color.a);
    }
    let share = pow(clamp(TO_REC2020 * nits / 10000.0, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(0.1593017578125));
    let curved = pow((0.8359375 + 18.8515625 * share) / (1.0 + 18.6875 * share), vec3<f32>(78.84375));
    return vec4<f32>(curved, color.a);
}
