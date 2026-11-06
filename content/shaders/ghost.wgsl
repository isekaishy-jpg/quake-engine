// A whole shader: lines of light that scroll up a surface, with no picture.
// Use it with `alpha = blend`. Numbers: red, green, blue, and how fast.
fn shader_whole(in: VertexOut) -> vec4<f32> {
    let color = vec3<f32>(shader_param(0), shader_param(1), shader_param(2));
    let lines = 0.5 + 0.5 * sin(in.world.z * 0.5 + shader_time() * shader_param(3));
    return vec4<f32>(scene_color(color), 0.2 + 0.5 * lines);
}
