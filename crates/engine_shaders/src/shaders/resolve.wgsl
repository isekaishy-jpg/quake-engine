// The scene's samples brought to one a pixel (proposal 0151): each tamed, as the screen will
// show it, near enough, then averaged, and taken back to light. Light averaged as it is would
// let one sample brighter than the screen shows outweigh three dark ones, once the final pass
// brought it down to fit, and something thin and dark before a bright wall would go.
// This follows frame.wgsl in one module.

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var drawn: texture_multisampled_2d<f32>;

@vertex
fn vs_resolve(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0);
}

@fragment
fn fs_resolve(@builtin(position) place: vec4<f32>) -> @location(0) vec4<f32> {
    let at = vec2<i32>(place.xy);
    let count = textureNumSamples(drawn);
    var sum = vec4<f32>(0.0);
    for (var sample = 0u; sample < count; sample = sample + 1u) {
        let color = textureLoad(drawn, at, i32(sample));
        sum = sum + vec4<f32>(tamed(color.rgb), color.a);
    }
    let mean = sum / f32(max(count, 1u));
    return vec4<f32>(untamed(mean.rgb), mean.a);
}
