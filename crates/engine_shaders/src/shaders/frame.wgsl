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
    // x edge smoothing on, y outline width, z outline fade distance, w how
    // wide rounded edges are, in pixels.
    post_params: vec4<f32>,
    // Per stencil: a corner and strength, the two edges with reach in `down`,
    // and the light with w 1 for a sun.
    stencil_corner: array<vec4<f32>, 4>,
    stencil_across: array<vec4<f32>, 4>,
    stencil_down: array<vec4<f32>, 4>,
    stencil_light: array<vec4<f32>, 4>,
    // x how many stencils, y steps along a ray, z how blurred, w samples a pixel.
    stencil_params: vec4<f32>,
    // Each stencil's occlusion grid, four cells to an entry.
    stencil_mask: array<vec4<f32>, 256>,
    // Characters that shade the light: a centre and radius, and the direction
    // light comes from with w how strong.
    capsules: array<vec4<f32>, 16>,
    capsule_light: array<vec4<f32>, 16>,
    // x how many capsules.
    capsule_params: vec4<f32>,
    // Per pool: its low corner with w its absorption, its high corner with w
    // its caustics, its colour with w how much it mirrors, and x how tall its
    // waves look, y how far it bends what's seen through it, and z how much
    // thicker it gets with depth.
    pool_low: array<vec4<f32>, 8>,
    pool_high: array<vec4<f32>, 8>,
    pool_color: array<vec4<f32>, 8>,
    pool_surface: array<vec4<f32>, 8>,
    // x how many pools, y how far details reach, z 1 under the open sky.
    pool_params: vec4<f32>,
    // The probes: their corner with w their spacing, and how many there are
    // along each axis, with w 1 if fog reads them.
    probe_origin: vec4<f32>,
    probe_size: vec4<f32>,
    // x what baked light is raised to, y how much light reaches the eye.
    color_params: vec4<f32>,
    // How much of each colour of light a zone lets through to the eye.
    zone_light: vec4<f32>,
    // Per fog volume, nearest first: its low corner with w how soft its sides are, its high corner
    // with w how thick it is, its colour with w how much is wisps, and how it drifts with w how big
    // wisps are.
    fog_low: array<vec4<f32>, 8>,
    fog_high: array<vec4<f32>, 8>,
    fog_look: array<vec4<f32>, 8>,
    fog_drift: array<vec4<f32>, 8>,
    // x how many volumes, y how many lights in fog, z which of the volumes are lit, as bits, and w
    // 1 if any has to be marched.
    fog_counts: vec4<f32>,
    // The moving lights nearest the eye, for fog: where with w how far it reaches, and its colour.
    fog_light_place: array<vec4<f32>, 4>,
    fog_light_color: array<vec4<f32>, 4>,
}

// A colour as it's shown, in the linear light scenes are drawn in.
fn to_linear(color: vec3<f32>) -> vec3<f32> {
    let low = color / 12.92;
    let high = pow((max(color, vec3<f32>(0.0)) + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(high, low, color <= vec3<f32>(0.04045));
}
