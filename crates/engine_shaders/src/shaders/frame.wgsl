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
    // Per layer of cloud, lowest first: a colour with w its coverage.
    sky_cloud: array<vec4<f32>, 4>,
    // xy speed, z scale, w 0 for none, 1 procedural, 2 from an image.
    sky_cloud_params: array<vec4<f32>, 4>,
    // x how high in the world, y how curved, z how lit, w which picture.
    sky_cloud_shape: array<vec4<f32>, 4>,
    // w how large the moon looks, and its colour, which is black for none, with w how bright the
    // stars are.
    sky_moon: vec4<f32>,
    sky_moon_color: vec4<f32>,
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
    // waves look, y how far it bends what's seen through it, z how much thicker
    // it gets with depth, and w 1 if it has a shape, so that its box is only
    // where it might be.
    pool_low: array<vec4<f32>, 8>,
    pool_high: array<vec4<f32>, 8>,
    pool_color: array<vec4<f32>, 8>,
    pool_surface: array<vec4<f32>, 8>,
    // x how many pools, y how far details reach, z 1 under the open sky, and w
    // which of them with a shape the eye's in, plus one.
    pool_params: vec4<f32>,
    // The probes: their corner with w their spacing, and how many there are
    // along each axis, with w 1 if fog reads them.
    probe_origin: vec4<f32>,
    probe_size: vec4<f32>,
    // x what baked light is raised to, y how much light reaches the eye, and z how what's too
    // bright comes down: 0 it stops at the top, 1 it rolls off, 2 a film's curve.
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
    // Each body's two overlays, then its foam, three to a body: its picture, plus one, how wide it
    // is and how much shows, and the way it flows in zw of `pool_flow`.
    pool_overlay: array<vec4<f32>, 24>,
    // x how far from the shore foam reaches, and y how deep it has to be to stop fading.
    pool_edge: array<vec4<f32>, 8>,
    // Where the ripples are, and how wide, which is 0 with none.
    ripple_place: vec4<f32>,
    // How far light gets past each cell of each stencil, four cells to an entry.
    stencil_reach: array<vec4<f32>, 256>,
    // Rays from the sky: x how bright, y 0 for none, 1 by the clouds, 2 by a picture, z 1 if they
    // come into the level, and w how high they start, with 0 for wherever suits.
    sky_rays: vec4<f32>,
    // x how wide a repeat of the picture is, yz how it drifts, and w how much the sun flares.
    sky_ray_look: vec4<f32>,
    // The low corner of the cells of what the sun gets to, how wide each is, and their height.
    sky_ray_area: vec4<f32>,
    // How low the sun gets in each cell, four to an entry.
    sky_ray_floor: array<vec4<f32>, 64>,
    // What the sun's baked light is multiplied by, as a day goes.
    sun_tint: vec4<f32>,
    // Light a day adds everywhere, and w what it multiplies highlights by.
    day_light: vec4<f32>,
    // How much corners darken, how far that looks, and how many looks it takes.
    ssao: vec4<f32>,
    // What the final pass multiplies what glows by, which is none without it.
    bloom: vec4<f32>,
    // x what baked light and probes are multiplied by, and y the most light a surface takes,
    // which is 0 for none.
    baked: vec4<f32>,
    // What the final pass encodes for: x 0 for a standard screen, 1 scRGB and 2 HDR10, y how
    // bright white is in nits, and z how many times as bright the brightest is.
    screen: vec4<f32>,
    // x 1 with TAA, y 1 if there's a frame before to go on from, and z which frame of its round.
    taa: vec4<f32>,
    // Where the view was last frame, with nothing off, for TAA.
    last_view_projection: mat4x4<f32>,
    // Where the view is, with nothing off, for TAA.
    steady_view_projection: mat4x4<f32>,
    // x 1 if highlights widen where a surface turns or its bumps spread within a pixel.
    shine: vec4<f32>,
}

// How big the scene's drawn against the screen (proposal 0154).
fn render_scale() -> f32 {
    return select(frame.taa.w, 1.0, frame.taa.w <= 0.0);
}

// How many mips finer pictures are read than the scene's size says: drawn smaller than the
// screen with TAA, as fine as the screen's size would have them, so that what TAA brings up over
// frames has their detail to find. Without TAA, finer would only shimmer.
fn texture_bias() -> f32 {
    if frame.taa.x > 0.5 && render_scale() < 1.0 {
        return log2(render_scale());
    }
    return 0.0;
}

// How much of a pixel a cut-out covers at `alpha`, from how far that's past its `cutoff`, as
// far as alpha changes across a pixel: an edge is a pixel wide, however near or far.
fn cut_coverage(alpha: f32, cutoff: f32) -> f32 {
    return clamp((alpha - cutoff) / max(fwidth(alpha), 0.0001) + 0.5, 0.0, 1.0);
}

// How many of a pixel's `samples` a cut-out covering `coverage` of it keeps. With one, it's kept
// where it covers half: TAA's moving the view each frame then smooths the edge by itself.
fn cut_samples(coverage: f32, samples: f32) -> u32 {
    return u32(coverage * max(samples, 1.0) + 0.5);
}

// A colour as it's shown, in the linear light scenes are drawn in.
fn to_linear(color: vec3<f32>) -> vec3<f32> {
    let low = color / 12.92;
    let high = pow((max(color, vec3<f32>(0.0)) + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(high, low, color <= vec3<f32>(0.04045));
}
