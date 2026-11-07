// A picture with no material: lit, and bright where its alpha says so.
fn shader_whole(in: VertexOut) -> vec4<f32> {
    let texel = textureSample(surface, surface_sampler, in.uv);
    let colored = texel.rgb * in.color.rgb;
    let lit = colored * light(in);
    return vec4<f32>(scene_color(mix(colored, lit, texel.a)), 1.0);
}
