// Moves a surface from side to side, as cloth or leaves do.
// Numbers: how far, in world units, and how fast.
fn shader_vertex(world: vec3<f32>) -> vec3<f32> {
    let wave = sin(world.z * 0.02 + shader_time() * shader_param(1));
    return world + vec3<f32>(wave * shader_param(0), 0.0, 0.0);
}
