// Noise, which surfaces and fields both have.

// A small hash, for hexagon tiles.
fn hash2(cell: vec2<f32>) -> vec2<f32> {
    let p = vec2<f32>(dot(cell, vec2<f32>(127.1, 311.7)), dot(cell, vec2<f32>(269.5, 183.3)));
    return fract(sin(p) * 43758.5453);
}

// Soft noise from 0 to 1, for what doesn't want to show a repeat.
fn shader_noise(at: vec2<f32>) -> f32 {
    let cell = floor(at);
    let inside = fract(at);
    let smooth_at = inside * inside * (vec2<f32>(3.0) - inside * 2.0);
    let low = mix(hash2(cell).x, hash2(cell + vec2<f32>(1.0, 0.0)).x, smooth_at.x);
    let high = mix(hash2(cell + vec2<f32>(0.0, 1.0)).x, hash2(cell + vec2<f32>(1.0, 1.0)).x, smooth_at.x);
    return mix(low, high, smooth_at.y);
}

