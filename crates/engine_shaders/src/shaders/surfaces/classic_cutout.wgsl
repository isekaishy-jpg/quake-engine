// A picture with no material, with holes, where alpha's below half. It's drawn as a masked
// material is, which smooths their edges.
fn shader_whole(in: VertexOut) -> vec4<f32> {
    let texel = textureSampleBias(surface, surface_sampler, in.uv, texture_bias());
    let lit = texel.rgb * in.color.rgb * light(in);
    return vec4<f32>(scene_color(lit), texel.a);
}
