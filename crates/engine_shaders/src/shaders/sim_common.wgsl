// What compute particles and those that draw them share (proposal 0146).

// One particle: `place` has its age in w, and `moving` how long it has.
struct SimParticle {
    place: vec4<f32>,
    moving: vec4<f32>,
}

// A force, as the map's are: x of `kind` is 0 for none, 1 attract, 2 vortex and 3 wind, then its
// strength, and its reach, how hard it draws in, or how much it comes and goes.
struct SimForce {
    kind: vec4<f32>,
    at: vec4<f32>,
    axis: vec4<f32>,
}

struct SimEmitter {
    view: mat4x4<f32>,
    back_from_view: mat4x4<f32>,
    // w is how long this step is.
    origin: vec4<f32>,
    // w is how far they can go from it, as a cosine.
    direction: vec4<f32>,
    // The slowest and fastest they start, and the shortest and longest they live.
    motion: vec4<f32>,
    // How far they start from it, gravity, drag and turbulence.
    pull: vec4<f32>,
    // Its first slot, how many it has, the next to use, and how many start this step.
    ring: vec4<u32>,
    // Its colour and size at eight even steps along a life.
    ramp: array<vec4<f32>, 8>,
    sizes: array<vec4<f32>, 2>,
    // Its picture, plus one, 1 if it adds, its frames a second, and how far it's drawn out along
    // how it's moving.
    look: vec4<f32>,
    // How far it fades into what's behind it, the time, a number that makes this step differ, and
    // how much speed it keeps when it bounces.
    extra: vec4<f32>,
    // 1 if its slots are new, 1 if there's a scene's depth to read, what it does when it hits
    // something (0 nothing, 1 bounce, 2 die, 3 stick), and its shape (0 a ball, 1 a ring, 2 a disc,
    // 3 a cone, 4 a line, 5 a box, 6 a mesh).
    flags: vec4<u32>,
    // Half of a line or box, a cone's height in z, or the first of a mesh's places and how many,
    // and w is 1 if they head away from the middle.
    extent: vec4<f32>,
    // The least and most it starts turned, and the least and most it turns a second, in radians.
    turn: vec4<f32>,
    // The columns and rows of its sheet.
    sheet: vec4<f32>,
    forces: array<SimForce, 4>,
}

// A number from 0 to 1 from two others.
fn hash_of(a: u32, b: u32) -> f32 {
    var value = a * 0x9e3779b1u + b * 0x85ebca77u;
    value = (value ^ (value >> 15u)) * 0x2c1b3c6du;
    value = (value ^ (value >> 12u)) * 0x297a2d39u;
    value = value ^ (value >> 15u);
    return f32(value & 0xffffffu) / 16777216.0;
}

// Two directions along a surface facing `normal`.
fn flat_axes(normal: vec3<f32>) -> array<vec3<f32>, 2> {
    var other = vec3<f32>(0.0, 0.0, 1.0);
    if abs(normal.z) > 0.9 {
        other = vec3<f32>(1.0, 0.0, 0.0);
    }
    let side = normalize(cross(other, normal));
    return array<vec3<f32>, 2>(side, cross(normal, side));
}

// A direction within `spread` of `axis`, from three random numbers.
fn scatter(axis: vec3<f32>, spread: f32, one: f32, two: f32) -> vec3<f32> {
    let z = 1.0 - one * (1.0 - spread);
    let angle = two * 6.2831853;
    let ring = sqrt(max(1.0 - z * z, 0.0));
    let axes = flat_axes(axis);
    return axes[0] * (ring * cos(angle)) + axes[1] * (ring * sin(angle)) + axis * z;
}
