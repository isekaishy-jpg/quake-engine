// Slides a surface's pictures in small waves, as water does.
// Numbers: how far, in repeats, and how fast.
fn shader_uv(uv: vec2<f32>, world: vec3<f32>) -> vec2<f32> {
    let wave = sin(world.xy * 0.05 + shader_time() * shader_param(1));
    return uv + wave.yx * shader_param(0);
}
