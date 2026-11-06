// A sky as one picture: two layers side by side, scrolling at their own speeds.
fn shader_whole(in: VertexOut) -> vec4<f32> {
    var direction = in.world - frame.eye.xyz;
    direction.z = direction.z * 3.0;
    let flat_direction = normalize(direction).xy * (6.0 * 63.0);
    let time = frame.params.x;
    let back_uv = fract((flat_direction + time * 8.0) / 128.0);
    let front_uv = fract((flat_direction + time * 16.0) / 128.0);
    let back = textureSampleLevel(
        surface, surface_sampler, vec2<f32>(0.5 + back_uv.x * 0.5, back_uv.y), 0.0);
    let front = textureSampleLevel(
        surface, surface_sampler, vec2<f32>(front_uv.x * 0.5, front_uv.y), 0.0);
    return vec4<f32>(scene_color(mix(back.rgb, front.rgb, front.a)), 1.0);
}
