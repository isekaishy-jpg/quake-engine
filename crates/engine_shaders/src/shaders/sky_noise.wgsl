// What the sky and its rays make clouds from.

// A number from 0 to 1 for a cell, from its integer position. `sin` of a big
// number comes out differently on every device, and in blocks.
fn sky_hash(cell: vec2<f32>) -> f32 {
    let at = vec2<u32>(vec2<i32>(cell) + vec2<i32>(32768));
    var value = at.x * 1597334677u ^ at.y * 3812015801u;
    value = (value ^ (value >> 16u)) * 2246822519u;
    value = (value ^ (value >> 13u)) * 3266489917u;
    value = value ^ (value >> 16u);
    return f32(value >> 8u) / 16777216.0;
}

fn sky_noise(at: vec2<f32>) -> f32 {
    let cell = floor(at);
    let part = fract(at);
    let blend = part * part * (vec2<f32>(3.0) - 2.0 * part);
    let a = sky_hash(cell);
    let b = sky_hash(cell + vec2<f32>(1.0, 0.0));
    let c = sky_hash(cell + vec2<f32>(0.0, 1.0));
    let d = sky_hash(cell + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, blend.x), mix(c, d, blend.x), blend.y);
}

// Layers of noise, each turned so their grids don't line up. `detail` runs from
// 0 to 1, and low down the finest go, as they're too small there to be seen
// except as streaks. What they'd have added stays, so cover doesn't change.
fn sky_fractal(at_start: vec2<f32>, detail: f32) -> f32 {
    var at = at_start;
    var total = 0.0;
    var weight = 0.5;
    for (var index = 0; index < 6; index = index + 1) {
        let keep = clamp(detail * 6.0 - f32(index) + 1.0, 0.0, 1.0);
        total = total + mix(0.5, sky_noise(at), keep) * weight;
        at = vec2<f32>(at.x * 1.6 - at.y * 1.2, at.x * 1.2 + at.y * 1.6) + vec2<f32>(17.3, 9.1);
        weight = weight * 0.5;
    }
    return total;
}

// How much cloud shows of `density`, with `cover` of the sky covered.
fn cloud_cover(density: f32, cover: f32) -> f32 {
    return smoothstep(0.92 - cover * 0.84, 1.22 - cover * 0.84, density);
}
