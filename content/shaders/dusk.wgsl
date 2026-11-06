// For a level's final pass: warms what's bright and cools what's dark.
fn shader_post(color: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let bright = dot(color, vec3<f32>(0.3, 0.59, 0.11));
    let tint = mix(vec3<f32>(0.8, 0.85, 1.0), vec3<f32>(1.1, 0.95, 0.8), clamp(bright, 0.0, 1.0));
    return color * tint;
}
