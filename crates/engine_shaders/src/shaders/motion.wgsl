// Compute particles: a ring of slots, stepped each frame (proposal 0146). When what's spawned has a
// life, the oldest slot is the next free one, so there's no list of what's alive to keep. Motion
// code joins this with `shader_motion`.

struct SimStep {
    emitter: u32,
    spare: vec3<u32>,
}

var<immediate> sim_step: SimStep;

@group(0) @binding(0) var<storage, read_write> sim_particles: array<SimParticle>;
@group(0) @binding(1) var<storage, read> sim_emitters: array<SimEmitter>;
@group(1) @binding(2) var scene_depth: texture_depth_2d;
@group(2) @binding(0) var motion_field: texture_2d<f32>;

// What the view's depth is made from: its nearest and furthest.
const VIEW_NEAR: f32 = 4.0;
const VIEW_FAR: f32 = 65536.0;

// How deep behind the scene something can be and still be hitting it, and not behind it.
const HIT_DEPTH: f32 = 48.0;

// The seconds since the level began, for what a shader wants.
var<private> motion_time: f32;

fn shader_time() -> f32 {
    return motion_time;
}

// What the field this emitter was given has at `uv`, from 0 to 1 across it.
fn field_at(uv: vec2<f32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(motion_field));
    let at = clamp(vec2<i32>(fract(uv) * vec2<f32>(size)), vec2<i32>(0), size - vec2<i32>(1));
    return textureLoad(motion_field, at, 0);
}

// How `force` pushes something at `place`. It's `ParticleForce::push`, which tests hold it to.
fn force_on(force: SimForce, place: vec3<f32>, middle: vec3<f32>, time: f32) -> vec3<f32> {
    let kind = force.kind.x;
    let strength = force.kind.y;
    if kind < 0.5 {
        return vec3<f32>(0.0);
    }
    if kind < 1.5 {
        let toward = middle + force.at.xyz - place;
        let distance = length(toward);
        let reach = force.kind.z;
        if distance < 1.0 || (reach > 0.0 && distance > reach) {
            return vec3<f32>(0.0);
        }
        var share = 1.0;
        if reach > 0.0 {
            share = 1.0 - distance / reach;
        }
        return toward * (strength * share / distance);
    }
    if kind < 2.5 {
        let line = normalize(force.axis.xyz);
        let from_it = place - (middle + force.at.xyz);
        let out = from_it - line * dot(from_it, line);
        let distance = length(out);
        if distance < 1.0 {
            return vec3<f32>(0.0);
        }
        return cross(line, out) * (strength / distance) - out * (force.kind.z / distance);
    }
    let wave = sin(time * 0.7) * 0.6 + sin(time * 1.9 + 1.3) * 0.4;
    return force.at.xyz * (1.0 + wave * force.kind.z);
}

// How far along the view the scene is at pixel `at`.
fn scene_away(at: vec2<i32>) -> f32 {
    let depth = textureLoad(scene_depth, at, 0);
    let share = VIEW_FAR / (VIEW_FAR - VIEW_NEAR);
    return share * VIEW_NEAR / max(share - depth, 0.000001);
}

// Where the scene is in the world at pixel `at`.
fn scene_place(at: vec2<i32>, size: vec2<f32>, emitter: SimEmitter) -> vec3<f32> {
    let depth = textureLoad(scene_depth, at, 0);
    let flat_at = (vec2<f32>(at) + vec2<f32>(0.5)) / size * 2.0 - vec2<f32>(1.0);
    let clip = vec4<f32>(flat_at, depth, 1.0);
    let world = emitter.back_from_view * clip;
    return world.xyz / world.w;
}

// What a particle at `place` hits of what the view last saw, with `xyz` the way it faces and `w` 1
// if it hit.
fn depth_hit(place: vec3<f32>, emitter: SimEmitter) -> vec4<f32> {
    let clip = emitter.view * vec4<f32>(place, 1.0);
    if clip.w <= 0.0 {
        return vec4<f32>(0.0);
    }
    let flat_at = clip.xy / clip.w;
    if any(abs(flat_at) >= vec2<f32>(0.99)) {
        return vec4<f32>(0.0);
    }
    let size = vec2<f32>(textureDimensions(scene_depth));
    let at = vec2<i32>((flat_at * 0.5 + vec2<f32>(0.5)) * size);
    let behind = clip.w - scene_away(at);
    if behind < 0.0 || behind > HIT_DEPTH {
        return vec4<f32>(0.0);
    }
    let here = scene_place(at, size, emitter);
    let across = scene_place(at + vec2<i32>(1, 0), size, emitter) - here;
    let down = scene_place(at + vec2<i32>(0, 1), size, emitter) - here;
    // Across and down the view, which comes out towards the eye whichever way the surface faces.
    return vec4<f32>(normalize(cross(across, down)), 1.0);
}

// A particle starting in slot `number`.
fn born(number: u32, emitter: SimEmitter) -> SimParticle {
    let salt = bitcast<u32>(emitter.extra.z);
    let pick = ((number + 1u) * 0x9e3779b1u) ^ salt;
    let up = vec3<f32>(0.0, 0.0, 1.0);
    var offset = vec3<f32>(0.0);
    let radius = emitter.pull.x;
    let angle = hash_of(pick, 2u) * 6.2831853;
    let round = vec3<f32>(cos(angle), sin(angle), 0.0);
    switch emitter.flags.w {
        case 1u: {
            offset = round * radius;
        }
        case 2u: {
            offset = round * radius * sqrt(hash_of(pick, 3u));
        }
        case 3u: {
            // Evenly by volume, which there's more of towards the top.
            let along = pow(hash_of(pick, 3u), 0.3333);
            offset = round * radius * along * sqrt(hash_of(pick, 4u));
            offset.z = emitter.extent.z * along;
        }
        case 4u: {
            offset = emitter.extent.xyz * (hash_of(pick, 3u) * 2.0 - 1.0);
        }
        case 5u: {
            let spot = vec3<f32>(hash_of(pick, 3u), hash_of(pick, 4u), hash_of(pick, 5u));
            offset = emitter.extent.xyz * (spot * 2.0 - vec3<f32>(1.0));
        }
        default: {
            let way = scatter(up, -1.0, hash_of(pick, 4u), hash_of(pick, 5u));
            offset = way * radius * pow(hash_of(pick, 3u), 0.3333);
        }
    }
    var heading = scatter(emitter.direction.xyz, emitter.direction.w, hash_of(pick, 6u), hash_of(pick, 7u));
    if emitter.extent.w > 0.5 && length(offset) > 0.001 {
        heading = normalize(offset);
    }
    let speed = mix(emitter.motion.x, emitter.motion.y, hash_of(pick, 8u));
    let life = mix(emitter.motion.z, emitter.motion.w, hash_of(pick, 9u));
    var out: SimParticle;
    out.place = vec4<f32>(emitter.origin.xyz + offset, 0.0);
    out.moving = vec4<f32>(heading * speed, max(life, 0.01));
    return out;
}

// A particle one step on.
fn stepped(particle: SimParticle, number: u32, emitter: SimEmitter) -> SimParticle {
    let seconds = emitter.origin.w;
    var place = particle.place.xyz;
    var moving = particle.moving.xyz;
    for (var at = 0; at < 4; at = at + 1) {
        moving = moving + force_on(emitter.forces[at], place, emitter.origin.xyz, emitter.extra.y) * seconds;
    }
    moving.z = moving.z - emitter.pull.y * seconds;
    moving = moving * max(1.0 - emitter.pull.z * seconds, 0.0);
    if emitter.pull.w > 0.0 {
        let salt = bitcast<u32>(emitter.extra.z);
        let push = vec3<f32>(hash_of(number, salt), hash_of(number, salt + 1u), hash_of(number, salt + 2u)) - vec3<f32>(0.5);
        moving = moving + push * (emitter.pull.w * seconds * 8.0);
    }
    place = place + moving * seconds;
    var age = particle.place.w + seconds;
    if emitter.flags.y == 1u && emitter.flags.z > 0u {
        let hit = depth_hit(place, emitter);
        if hit.w > 0.5 {
            if emitter.flags.z == 2u {
                age = particle.moving.w;
            } else if emitter.flags.z == 3u {
                moving = vec3<f32>(0.0);
                place = particle.place.xyz;
            } else {
                let into = dot(moving, hit.xyz);
                if into < 0.0 {
                    moving = (moving - hit.xyz * (into * 2.0)) * emitter.extra.w;
                    place = particle.place.xyz + moving * seconds;
                }
            }
        }
    }
    var out: SimParticle;
    out.place = vec4<f32>(place, age);
    out.moving = vec4<f32>(moving, particle.moving.w);
    return out;
}

@compute @workgroup_size(64, 1, 1)
fn cs_motion(@builtin(global_invocation_id) id: vec3<u32>) {
    let emitter = sim_emitters[sim_step.emitter];
    let count = emitter.ring.y;
    if id.x >= count {
        return;
    }
    motion_time = emitter.extra.y;
    let slot = emitter.ring.x + id.x;
    let from_head = (id.x + count - emitter.ring.z) % count;
    if from_head < emitter.ring.w {
        sim_particles[slot] = born(id.x, emitter);
        return;
    }
    if emitter.flags.x == 1u {
        // A new slot that nothing's started in yet.
        var nothing: SimParticle;
        nothing.place = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        nothing.moving = vec4<f32>(0.0);
        sim_particles[slot] = nothing;
        return;
    }
    let particle = sim_particles[slot];
    if particle.place.w < particle.moving.w {
        sim_particles[slot] = shader_motion(stepped(particle, id.x, emitter), emitter.origin.w);
    }
}
