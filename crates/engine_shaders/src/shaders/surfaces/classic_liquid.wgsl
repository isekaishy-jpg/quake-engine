// A liquid with no material, which swims and takes no light.
fn shader_whole(in: VertexOut) -> vec4<f32> {
    let size = vec2<f32>(textureDimensions(surface, 0));
    let texels = in.uv * size;
    let time = frame.params.x;
    let warped = texels + 8.0 * sin(texels.yx * 0.125 + time);
    let texel = textureSampleBias(surface, surface_sampler, warped / size, texture_bias());
    return vec4<f32>(scene_color(texel.rgb), 1.0);
}
