// Ripples on water near the view: a damped wave equation on a height field that goes with the view
// (proposal 0147). `x` of a cell is its height, and `y` what it was a step before.

struct RippleStep {
    // How far the field has moved since the last step, in cells.
    moved: vec2<i32>,
    // How many pushes there are, and 1 for the step that only keeps what the last one made.
    pushes: u32,
    keeping: u32,
}

// Something pushing the water down.
struct RipplePush {
    // Where it is, in cells, and how far it reaches.
    place: vec2<f32>,
    reach: f32,
    strength: f32,
    // Which way it's going, as a cosine and a sine, and its picture, plus one, or 0 for a ring.
    way: vec2<f32>,
    picture: f32,
    spare: f32,
}

var<immediate> ripple_step: RippleStep;

@group(0) @binding(0) var ripple_from: texture_2d<f32>;
@group(0) @binding(1) var ripple_to: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<storage, read> ripple_pushes: array<RipplePush>;
@group(0) @binding(3) var ripple_pictures: texture_2d_array<f32>;
@group(0) @binding(4) var ripple_sampler: sampler;

// How much of a cell's height goes on to the next step.
const RIPPLE_KEEPS: f32 = 0.985;

// How fast ripples go, as a share of a cell a step, which has to stay under a half.
const RIPPLE_SPEED: f32 = 0.45;

// What's in a cell, which is flat off the field.
fn ripple_cell(at: vec2<i32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(ripple_from));
    if any(at < vec2<i32>(0)) || any(at >= size) {
        return vec4<f32>(0.0);
    }
    return textureLoad(ripple_from, at, 0);
}

// What `push` does to the cell at `at`.
fn pushed(push: RipplePush, at: vec2<f32>) -> f32 {
    let from_it = at - push.place;
    if push.picture < 0.5 {
        let share = 1.0 - length(from_it) / max(push.reach, 1.0);
        return push.strength * max(share, 0.0) * max(share, 0.0);
    }
    // Along how it's going is up the picture, with what's behind it at the bottom.
    let along = dot(from_it, push.way);
    let across = dot(from_it, vec2<f32>(-push.way.y, push.way.x));
    let uv = vec2<f32>(across, -along) / (push.reach * 2.0) + vec2<f32>(0.5);
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) {
        return 0.0;
    }
    let seen = textureSampleLevel(ripple_pictures, ripple_sampler, uv, i32(push.picture) - 1, 0.0);
    return push.strength * seen.a;
}

@compute @workgroup_size(8, 8, 1)
fn cs_ripple(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(ripple_from);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let at = vec2<i32>(id.xy);
    if ripple_step.keeping == 1u {
        textureStore(ripple_to, at, textureLoad(ripple_from, at, 0));
        return;
    }
    let from_at = at + ripple_step.moved;
    let here = ripple_cell(from_at);
    let round = ripple_cell(from_at + vec2<i32>(1, 0)).x + ripple_cell(from_at - vec2<i32>(1, 0)).x
        + ripple_cell(from_at + vec2<i32>(0, 1)).x + ripple_cell(from_at - vec2<i32>(0, 1)).x;
    var next = (here.x * 2.0 - here.y + (round - here.x * 4.0) * RIPPLE_SPEED) * RIPPLE_KEEPS;
    for (var index = 0u; index < ripple_step.pushes; index = index + 1u) {
        next = next - pushed(ripple_pushes[index], vec2<f32>(at));
    }
    // Nothing goes on for ever at the field's edge, or it'd come back.
    let edge = min(min(at.x, at.y), min(i32(size.x) - 1 - at.x, i32(size.y) - 1 - at.y));
    next = next * clamp(f32(edge) / 16.0, 0.0, 1.0);
    textureStore(ripple_to, at, vec4<f32>(clamp(next, -8.0, 8.0), here.x, 0.0, 0.0));
}
