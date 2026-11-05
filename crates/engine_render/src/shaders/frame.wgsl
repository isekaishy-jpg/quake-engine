// The uniforms every frame shares.

struct Frame {
    view_projection: mat4x4<f32>,
    eye: vec4<f32>,
    // x is the time in seconds, y the width over the height, and z how many
    // dynamic lights there are.
    params: vec4<f32>,
    // 64 light style brightnesses, four to an entry.
    styles: array<vec4<f32>, 16>,
    // Each dynamic light's position, then its radius.
    lights: array<vec4<f32>, 32>,
    // Each dynamic light's least light, four to an entry.
    light_cuts: array<vec4<f32>, 8>,
    // The sky: its colours, then w of the first is the horizon's height.
    sky_zenith: vec4<f32>,
    sky_horizon: vec4<f32>,
    sky_ground: vec4<f32>,
    // xyz towards the sun, w its size.
    sky_sun: vec4<f32>,
    sky_sun_color: vec4<f32>,
    // Colour, then w is the coverage.
    sky_cloud: vec4<f32>,
    // xy speed, z scale, w 0 for none, 1 procedural, 2 from an image.
    sky_cloud_params: vec4<f32>,
    // Back from clip space to the world.
    inverse_view_projection: mat4x4<f32>,
    // Colour, then w is the density.
    fog_color: vec4<f32>,
    // x base height, y falloff.
    fog_params: vec4<f32>,
    // Colour, then w is 0 off, 1 full, 2 semi.
    outline: vec4<f32>,
    // x edge smoothing on, y outline width, z outline fade distance.
    post_params: vec4<f32>,
}
