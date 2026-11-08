// Which way what moves went since the last frame, for TAA (proposal 0151): drawn again with
// where it is and where it was, so that what TAA kept of it is found where it was. The world
// that doesn't move is found from depth and where the view was, so only what moves is drawn.
// This follows frame.wgsl in one module.

@group(0) @binding(0) var<uniform> frame: Frame;

// A model's or a moving mesh's corner, its rows now, and its rows last frame.
struct MeshIn {
    @location(0) position: vec3<f32>,
    @location(6) row_x: vec4<f32>,
    @location(7) row_y: vec4<f32>,
    @location(8) row_z: vec4<f32>,
    // On a model, z above 0 keeps it in front, as the player's weapon is.
    @location(9) light: vec3<f32>,
    @location(12) last_x: vec4<f32>,
    @location(13) last_y: vec4<f32>,
    @location(14) last_z: vec4<f32>,
}

// A corner of a brush model, which moves as a whole.
struct BrushIn {
    @location(0) position: vec3<f32>,
}

// Where a brush model is now and was last frame. For models and moving meshes, now.w is 1 for
// models, whose light can keep them in front, and 0 for moving meshes, whose light is a colour.
struct BrushPlaces {
    now: vec4<f32>,
    before: vec4<f32>,
}

var<immediate> places: BrushPlaces;

struct MotionOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) now: vec4<f32>,
    @location(1) before: vec4<f32>,
    // Where it's drawn, as clip is, read back as TAA reads depth.
    @location(2) seen: vec4<f32>,
}

// What's drawn at `world`, which was at `last` last frame.
fn moved(world: vec3<f32>, last: vec3<f32>) -> MotionOut {
    var out: MotionOut;
    out.clip = frame.view_projection * vec4<f32>(world, 1.0);
    out.now = frame.steady_view_projection * vec4<f32>(world, 1.0);
    out.before = frame.last_view_projection * vec4<f32>(last, 1.0);
    out.seen = out.clip;
    return out;
}

@vertex
fn vs_mesh(in: MeshIn) -> MotionOut {
    let corner = vec4<f32>(in.position, 1.0);
    let world = vec3<f32>(dot(in.row_x, corner), dot(in.row_y, corner), dot(in.row_z, corner));
    let last = vec3<f32>(dot(in.last_x, corner), dot(in.last_y, corner), dot(in.last_z, corner));
    var out = moved(world, last);
    if places.now.w > 0.5 && in.light.z > 0.0 {
        out.clip.z = out.clip.z * 0.3;
        out.seen = out.clip;
    }
    return out;
}

@vertex
fn vs_brush(in: BrushIn) -> MotionOut {
    return moved(in.position + places.now.xyz, in.position + places.before.xyz);
}

// Where on the screen a clip position is, as the final pass reads it.
fn screen_place(clip: vec4<f32>) -> vec2<f32> {
    let ndc = clip.xy / clip.w;
    return vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
}

// How far it went across the screen, how far it is from the eye, and that it's there. The
// distance is found from where it's drawn just as TAA finds it from depth, so that what's kept
// in front, whose depth is squeezed, is found to be what's there.
@fragment
fn fs_motion(in: MotionOut) -> @location(0) vec4<f32> {
    let ndc = in.seen.xyz / in.seen.w;
    let back = frame.inverse_view_projection * vec4<f32>(ndc, 1.0);
    let distance = length(back.xyz / back.w - frame.eye.xyz);
    return vec4<f32>(screen_place(in.now) - screen_place(in.before), distance, 1.0);
}
