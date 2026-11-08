// Bloom (proposal 0150): what's brighter than a screen can show, blurred wide by going down a
// chain of images each half the last, and back up, each adding what's below it. The way down
// takes 13 looks a pixel, and the way up a tent of nine, as Jimenez showed for Call of Duty.

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var smooth_sampler: sampler;

struct BloomStep {
    // How bright light has to be, once exposed, before it glows.
    threshold: f32,
    // How soft that is: light this far below it glows a little.
    knee: f32,
    // What exposes it, to measure it against the threshold.
    exposure: f32,
    // How far the way up spreads, in pixels of what it reads.
    spread: f32,
}

var<immediate> step: BloomStep;

struct Full {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_full(@builtin(vertex_index) index: u32) -> Full {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: Full;
    out.clip = vec4<f32>(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0);
    out.uv = corner;
    return out;
}

fn look(uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(source, smooth_sampler, uv, 0.0).rgb;
}

// How bright a colour looks.
fn luma(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// The 13 looks round `uv`, as the five overlapping boxes of four they make: the middle one, then
// the four round it.
fn boxes(uv: vec2<f32>) -> array<vec3<f32>, 5> {
    let texel = 1.0 / vec2<f32>(textureDimensions(source));
    let a = look(uv + texel * vec2<f32>(-2.0, -2.0));
    let b = look(uv + texel * vec2<f32>(0.0, -2.0));
    let c = look(uv + texel * vec2<f32>(2.0, -2.0));
    let d = look(uv + texel * vec2<f32>(-1.0, -1.0));
    let e = look(uv + texel * vec2<f32>(1.0, -1.0));
    let f = look(uv + texel * vec2<f32>(-2.0, 0.0));
    let g = look(uv);
    let h = look(uv + texel * vec2<f32>(2.0, 0.0));
    let i = look(uv + texel * vec2<f32>(-1.0, 1.0));
    let j = look(uv + texel * vec2<f32>(1.0, 1.0));
    let k = look(uv + texel * vec2<f32>(-2.0, 2.0));
    let l = look(uv + texel * vec2<f32>(0.0, 2.0));
    let m = look(uv + texel * vec2<f32>(2.0, 2.0));
    return array<vec3<f32>, 5>(
        (d + e + i + j) * 0.25,
        (a + b + f + g) * 0.25,
        (b + c + g + h) * 0.25,
        (f + g + k + l) * 0.25,
        (g + h + l + m) * 0.25,
    );
}

// The middle box counts for half, and the four round it an eighth each.
const BOX_WEIGHTS: array<f32, 5> = array<f32, 5>(0.5, 0.125, 0.125, 0.125, 0.125);

// What of `color` glows: what's past the threshold once exposed, coming in softly over the knee,
// and kept in the scene's own light.
fn glowing(color: vec3<f32>) -> vec3<f32> {
    let bright = max(color.r, max(color.g, color.b)) * step.exposure;
    let knee = max(step.knee, 0.0001);
    let soft = clamp(bright - step.threshold + knee, 0.0, 2.0 * knee);
    let over = max(soft * soft / (4.0 * knee), bright - step.threshold);
    return color * (over / max(bright, 0.0001));
}

// The first step down, from the scene: only what glows, with each box weighted down by how bright
// it is, so that one bright pixel doesn't flicker as it moves.
@fragment
fn fs_down_first(in: Full) -> @location(0) vec4<f32> {
    let found = boxes(in.uv);
    var total = vec3<f32>(0.0);
    var weight = 0.0;
    for (var index = 0; index < 5; index = index + 1) {
        let glow = glowing(found[index]);
        let share = BOX_WEIGHTS[index] / (1.0 + luma(glow) * step.exposure);
        total = total + glow * share;
        weight = weight + share;
    }
    return vec4<f32>(total / max(weight, 0.0001), 1.0);
}

// A step down.
@fragment
fn fs_down(in: Full) -> @location(0) vec4<f32> {
    let found = boxes(in.uv);
    var total = vec3<f32>(0.0);
    for (var index = 0; index < 5; index = index + 1) {
        total = total + found[index] * BOX_WEIGHTS[index];
    }
    return vec4<f32>(total, 1.0);
}

// A step up: what's below, spread by a tent of nine, added to what's here.
@fragment
fn fs_up(in: Full) -> @location(0) vec4<f32> {
    let texel = step.spread / vec2<f32>(textureDimensions(source));
    var total = look(in.uv) * 4.0;
    total = total + (look(in.uv + texel * vec2<f32>(0.0, -1.0)) + look(in.uv + texel * vec2<f32>(-1.0, 0.0))
        + look(in.uv + texel * vec2<f32>(1.0, 0.0)) + look(in.uv + texel * vec2<f32>(0.0, 1.0))) * 2.0;
    total = total + look(in.uv + texel * vec2<f32>(-1.0, -1.0)) + look(in.uv + texel * vec2<f32>(1.0, -1.0))
        + look(in.uv + texel * vec2<f32>(-1.0, 1.0)) + look(in.uv + texel * vec2<f32>(1.0, 1.0));
    return vec4<f32>(total / 16.0, 1.0);
}
