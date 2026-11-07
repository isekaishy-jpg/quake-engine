// Particles (proposals 0036 and 0146). They're drawn in a pass of their own after the scene, with
// no depth attached: they read the scene's depth as a picture, and test against it themselves.
// Particle code joins this with `shader_particle`.

struct Frame {
    view_projection: mat4x4<f32>,
    eye: vec4<f32>,
    // x is the time in seconds, and y the width over the height.
    params: vec4<f32>,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(2) var scene_depth: texture_depth_2d;
@group(2) @binding(0) var pictures: texture_2d_array<f32>;
@group(2) @binding(1) var picture_sampler: sampler;

// What the view's depth is made from: its nearest and furthest.
const VIEW_NEAR: f32 = 4.0;
const VIEW_FAR: f32 = 65536.0;

// Two triangles, as corners from 0 to 1.
fn corner(index: u32) -> vec2<f32> {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    return corners[index];
}

struct ParticleIn {
    @builtin(vertex_index) index: u32,
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) size: f32,
    @location(3) end: vec3<f32>,
    // x is its shape (0 square, 1 soft, 2 line, 3 flat), y how far it glows, and z 1 if it adds.
    @location(4) look: vec3<f32>,
    // x is how it's turned, in radians, and y how far it fades into what's behind it.
    @location(5) motion: vec2<f32>,
    // x is its picture, plus one, or 0 for none, y its frame, and z and w the columns and rows of
    // its sheet.
    @location(6) picture: vec4<f32>,
    // How far through its life it is.
    @location(7) life: f32,
}

struct ParticleOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
    // From -1 to 1 across it.
    @location(1) across: vec2<f32>,
    // 0 for a hard square, 1 for what fades to its edge.
    @location(2) @interpolate(flat) soft: f32,
    // From 0 to 1 across it, for its picture.
    @location(3) uv: vec2<f32>,
    @location(4) @interpolate(flat) picture: vec4<f32>,
    // x is how far it fades into what's behind it, y 1 if it adds, z how far through its life.
    @location(5) @interpolate(flat) finish: vec3<f32>,
    // How far it is from the eye, along the view.
    @location(6) away: f32,
    @location(7) world: vec3<f32>,
}

// `offset` from `position`, on the plane facing the eye.
fn billboard(position: vec3<f32>, offset: vec2<f32>) -> vec3<f32> {
    let axes = flat_axes(normalize(frame.eye.xyz - position));
    return position + axes[0] * offset.x + axes[1] * offset.y;
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

// A square that shrinks with distance down to a limit, from `D_DrawParticle`,
// with sizes in pixels of a 240 line screen.
@vertex
fn vs_particle(in: ParticleIn) -> ParticleOut {
    let from_zero = corner(in.index);
    let square = from_zero * 2.0 - 1.0;
    var out: ParticleOut;
    out.across = square;
    out.uv = vec2<f32>(from_zero.x, 1.0 - from_zero.y);
    out.picture = in.picture;
    out.finish = vec3<f32>(in.motion.y, in.look.z, in.life);
    if in.look.x < 0.5 {
        var clip = frame.view_projection * vec4<f32>(in.position, 1.0);
        out.away = clip.w;
        out.world = in.position;
        let size = clamp(256.0 / max(clip.w, 1.0), 1.0, 4.0) / 240.0;
        clip = vec4<f32>(
            clip.xy + square * vec2<f32>(size / frame.params.y, size) * clip.w,
            clip.zw,
        );
        out.clip = clip;
        out.color = vec4<f32>(pow(in.color.rgb, vec3<f32>(2.2)), 1.0);
        out.soft = 0.0;
        return out;
    }
    let half_size = in.size * 0.5;
    var place = in.position;
    if in.look.x < 1.5 {
        let turn = vec2<f32>(cos(in.motion.x), sin(in.motion.x));
        let turned = vec2<f32>(
            square.x * turn.x - square.y * turn.y,
            square.x * turn.y + square.y * turn.x,
        );
        place = billboard(in.position, turned * half_size);
    } else if in.look.x < 2.5 {
        let along = in.end - in.position;
        let side = normalize(cross(along, frame.eye.xyz - in.position));
        place = in.position + along * (square.y * 0.5 + 0.5) + side * square.x * half_size;
        out.across = vec2<f32>(square.x, 0.0);
    } else {
        let axes = flat_axes(normalize(in.end));
        place = in.position + (axes[0] * square.x + axes[1] * square.y) * half_size;
    }
    out.clip = frame.view_projection * vec4<f32>(place, 1.0);
    out.away = out.clip.w;
    out.world = place;
    let color = pow(in.color.rgb, vec3<f32>(2.2)) * in.look.y;
    out.color = vec4<f32>(color, in.color.a);
    out.soft = 1.0;
    return out;
}

// What a particle's shader has to work with.
struct ParticleLook {
    // Its colour and alpha, with its picture.
    color: vec4<f32>,
    // Where it is across its picture.
    uv: vec2<f32>,
    // How far through its life it is, from 0 to 1.
    life: f32,
    // Where it is in the world.
    world: vec3<f32>,
}

// The seconds since the level began.
fn shader_time() -> f32 {
    return frame.params.x;
}

// One frame of a sheet.
fn read_frame(layer: i32, frame_number: f32, sheet: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let count = max(sheet.x * sheet.y, 1.0);
    let wrapped = frame_number % count;
    let cell = vec2<f32>(wrapped % sheet.x, floor(wrapped / sheet.x));
    let at = (cell + uv) / sheet;
    return textureSample(pictures, picture_sampler, at, layer);
}

// What its picture is here. Between frames it's a mix of the two, so that it doesn't step.
fn read_picture(picture: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let layer = i32(picture.x) - 1;
    let sheet = max(picture.zw, vec2<f32>(1.0));
    let first = floor(picture.y);
    let one = read_frame(layer, first, sheet, uv);
    let two = read_frame(layer, first + 1.0, sheet, uv);
    return mix(one, two, picture.y - first);
}

// How far along the view the scene is at this pixel.
fn scene_away(at: vec2<i32>) -> f32 {
    let depth = textureLoad(scene_depth, at, 0);
    let share = VIEW_FAR / (VIEW_FAR - VIEW_NEAR);
    return share * VIEW_NEAR / max(share - depth, 0.000001);
}

@fragment
fn fs_particle(in: ParticleOut) -> @location(0) vec4<f32> {
    // Every invocation reads it, as a picture's mips need its neighbours.
    var texel = vec4<f32>(1.0);
    if in.picture.x > 0.5 {
        texel = read_picture(in.picture, in.uv);
    }
    let gap = scene_away(vec2<i32>(in.clip.xy)) - in.away;
    if gap < 0.0 {
        discard;
    }
    var look: ParticleLook;
    look.uv = in.uv;
    look.life = in.finish.z;
    look.world = in.world;
    var fade = mix(1.0, 1.0 - smoothstep(0.3, 1.0, length(in.across)), in.soft);
    if in.picture.x > 0.5 {
        fade = 1.0;
    }
    look.color = vec4<f32>(in.color.rgb * texel.rgb, in.color.a * texel.a * fade);
    let shaded = shader_particle(look);
    var alpha = shaded.a;
    if in.finish.x > 0.0 {
        alpha = alpha * clamp(gap / in.finish.x, 0.0, 1.0);
    }
    // Premultiplied, so one blend does both: alpha covers, and with none it adds.
    return vec4<f32>(shaded.rgb * alpha, alpha * (1.0 - in.finish.y));
}

// What's the same for every one of a batch the GPU works out. Everything about
// each follows from its number, so nothing's kept from frame to frame.
struct GpuParticles {
    // w is how long each lives.
    origin: vec4<f32>,
    // w is how far from it they can go, as a cosine.
    direction: vec4<f32>,
    color: vec4<f32>,
    end_color: vec4<f32>,
    // How long it's been going, then how fast, gravity and turbulence.
    motion: vec4<f32>,
    // Size at the start and end, how far they start from it, and how many.
    shape: vec4<f32>,
    // x is its picture, plus one, y 1 if it adds, z how far it fades into what's behind it.
    look: vec4<f32>,
}

var<immediate> batch: GpuParticles;

// A number from 0 to 1 from two others.
fn hash_of(a: u32, b: u32) -> f32 {
    var value = a * 0x9e3779b1u + b * 0x85ebca77u;
    value = (value ^ (value >> 15u)) * 0x2c1b3c6du;
    value = (value ^ (value >> 12u)) * 0x297a2d39u;
    value = value ^ (value >> 15u);
    return f32(value & 0xffffffu) / 16777216.0;
}

// A direction within `spread` of `axis`, from three random numbers.
fn scatter(axis: vec3<f32>, spread: f32, one: f32, two: f32) -> vec3<f32> {
    let z = 1.0 - one * (1.0 - spread);
    let angle = two * 6.2831853;
    let ring = sqrt(max(1.0 - z * z, 0.0));
    let axes = flat_axes(axis);
    return axes[0] * (ring * cos(angle)) + axes[1] * (ring * sin(angle)) + axis * z;
}

@vertex
fn vs_gpu_particle(
    @builtin(vertex_index) index: u32,
    @builtin(instance_index) number: u32,
) -> ParticleOut {
    let life = batch.origin.w;
    let phase = hash_of(number, 1u) * life;
    let running = batch.motion.x + phase;
    let cycle = u32(floor(running / life));
    let age = running - f32(cycle) * life;
    let share = age / life;
    let start = scatter(vec3<f32>(0.0, 0.0, 1.0), -1.0, hash_of(number, cycle * 4u + 2u), hash_of(number, cycle * 4u + 3u));
    let heading = scatter(batch.direction.xyz, batch.direction.w, hash_of(number, cycle * 4u + 4u), hash_of(number, cycle * 4u + 5u));
    // The cube root spreads them evenly through a ball, not thickest at its middle.
    let out_from = pow(hash_of(number, cycle * 4u + 6u), 0.3333);
    var place = batch.origin.xyz + start * batch.shape.z * out_from;
    place = place + heading * batch.motion.y * age;
    place.z = place.z - 0.5 * batch.motion.z * age * age;
    let drift = vec3<f32>(
        sin(age * 3.1 + f32(number)),
        cos(age * 2.3 + f32(number) * 1.7),
        sin(age * 1.9 + f32(number) * 0.6),
    );
    place = place + drift * batch.motion.w * age;
    let from_zero = corner(index);
    let square = from_zero * 2.0 - 1.0;
    let size = mix(batch.shape.x, batch.shape.y, share) * 0.5;
    var out: ParticleOut;
    let world = billboard(place, square * size);
    out.clip = frame.view_projection * vec4<f32>(world, 1.0);
    out.away = out.clip.w;
    out.world = world;
    // What hasn't started yet isn't there.
    var color = mix(batch.color, batch.end_color, share);
    if batch.motion.x < phase - life {
        color = vec4<f32>(0.0);
    }
    out.color = color;
    out.across = square;
    out.soft = 1.0;
    out.uv = vec2<f32>(from_zero.x, 1.0 - from_zero.y);
    out.picture = vec4<f32>(batch.look.x, 0.0, 1.0, 1.0);
    out.finish = vec3<f32>(batch.look.z, batch.look.y, share);
    return out;
}
