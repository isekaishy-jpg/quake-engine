// A picture with no material, with holes.
fn shader_whole(in: VertexOut) -> vec4<f32> {
    let texel = textureSample(surface, surface_sampler, in.uv);
    let lit = texel.rgb * light(in);
    if texel.a < 0.5 {
        discard;
    }
    return vec4<f32>(scene_color(lit), 1.0);
}
