// Materials: up to eight images, shading models, layers and surface detail
// (proposal 0069). This follows world.wgsl in one module, and slot 0 is its
// `surface`.

struct MaterialParams {
    // x shading (0 lit, 1 cel, 2 matcap, 3 unlit), y alpha (0 opaque, 1 mask,
    // 2 blend), z feature bits (1 triplanar, 2 hex tiles), w layer count.
    flags: vec4<i32>,
    // Slots of the normal, specular, emissive and height maps, or -1.
    slots_a: vec4<i32>,
    // Slots of the tint, matcap, ramp and detail maps, or -1.
    slots_b: vec4<i32>,
    // Each layer's base and mask slots.
    layers: array<vec4<i32>, 3>,
    // xy scroll in repeats a second, zw flipbook columns and rows.
    motion: vec4<f32>,
    // x specular strength, y gloss, z parallax depth, w flipbook rate.
    values: vec4<f32>,
    // x detail scale, y emissive strength, z alpha cutoff.
    more: vec4<f32>,
    // x how much it mirrors, y how far flow moves it, z flow cycles a second,
    // w the slot of its flow map, or -1.
    extra: vec4<f32>,
}

@group(1) @binding(2) var slot1: texture_2d<f32>;
@group(1) @binding(3) var slot2: texture_2d<f32>;
@group(1) @binding(4) var slot3: texture_2d<f32>;
@group(1) @binding(5) var slot4: texture_2d<f32>;
@group(1) @binding(6) var slot5: texture_2d<f32>;
@group(1) @binding(7) var slot6: texture_2d<f32>;
@group(1) @binding(8) var slot7: texture_2d<f32>;
@group(1) @binding(9) var<uniform> material: MaterialParams;

// One slot's texel, with derivatives worked out once up front, so that it can
// be read inside branches.
fn read_slot(slot: i32, uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>) -> vec4<f32> {
    switch slot {
        case 1: { return textureSampleGrad(slot1, surface_sampler, uv, dx, dy); }
        case 2: { return textureSampleGrad(slot2, surface_sampler, uv, dx, dy); }
        case 3: { return textureSampleGrad(slot3, surface_sampler, uv, dx, dy); }
        case 4: { return textureSampleGrad(slot4, surface_sampler, uv, dx, dy); }
        case 5: { return textureSampleGrad(slot5, surface_sampler, uv, dx, dy); }
        case 6: { return textureSampleGrad(slot6, surface_sampler, uv, dx, dy); }
        case 7: { return textureSampleGrad(slot7, surface_sampler, uv, dx, dy); }
        default: { return textureSampleGrad(surface, surface_sampler, uv, dx, dy); }
    }
}

// A small hash, for hexagon tiles.
fn hash2(cell: vec2<f32>) -> vec2<f32> {
    let p = vec2<f32>(dot(cell, vec2<f32>(127.1, 311.7)), dot(cell, vec2<f32>(269.5, 183.3)));
    return fract(sin(p) * 43758.5453);
}

// The slot's texel, broken up by three overlapping hexagon tiles with random
// offsets, so that large walls don't show their repeat.
fn read_hex(slot: i32, uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>) -> vec4<f32> {
    let skewed = vec2<f32>(uv.x + uv.y * 0.57735, uv.y * 1.1547) * 3.0;
    let base = floor(skewed);
    let local = fract(skewed);
    var a = base;
    var b = base + vec2<f32>(1.0, 0.0);
    var c = base + vec2<f32>(0.0, 1.0);
    var weights = vec3<f32>(1.0 - local.x - local.y, local.x, local.y);
    if weights.x < 0.0 {
        a = base + vec2<f32>(1.0, 1.0);
        weights = vec3<f32>(-weights.x, 1.0 - local.y, 1.0 - local.x);
    }
    let first = read_slot(slot, uv + hash2(a), dx, dy);
    let second = read_slot(slot, uv + hash2(b), dx, dy);
    let third = read_slot(slot, uv + hash2(c), dx, dy);
    let sharp = pow(max(weights, vec3<f32>(0.0)), vec3<f32>(5.0));
    let total = max(sharp.x + sharp.y + sharp.z, 0.0001);
    return (first * sharp.x + second * sharp.y + third * sharp.z) / total;
}

// Texture coordinates after scrolling and flipbook frames.
fn moved_uv(uv: vec2<f32>) -> vec2<f32> {
    let time = frame.params.x;
    var out = uv + material.motion.xy * time;
    let columns = material.motion.z;
    let rows = material.motion.w;
    if columns > 0.0 && rows > 0.0 {
        let count = columns * rows;
        let at = floor(time * material.values.w) % count;
        let cell = vec2<f32>(at % columns, floor(at / columns));
        out = (fract(out) + cell) / vec2<f32>(columns, rows);
    }
    return out;
}

// A surface's frame from screen derivatives, which needs no extra vertex data.
struct SurfaceFrame {
    normal: vec3<f32>,
    tangent: vec3<f32>,
    bitangent: vec3<f32>,
}

fn surface_frame(world: vec3<f32>, uv: vec2<f32>) -> SurfaceFrame {
    let dp1 = dpdx(world);
    let dp2 = dpdy(world);
    let duv1 = dpdx(uv);
    let duv2 = dpdy(uv);
    var out: SurfaceFrame;
    var normal = normalize(cross(dp2, dp1));
    if dot(normal, frame.eye.xyz - world) < 0.0 {
        normal = -normal;
    }
    out.normal = normal;
    let perp2 = cross(dp2, normal);
    let perp1 = cross(normal, dp1);
    let tangent = perp2 * duv1.x + perp1 * duv2.x;
    let bitangent = perp2 * duv1.y + perp1 * duv2.y;
    let scale = inverseSqrt(max(max(dot(tangent, tangent), dot(bitangent, bitangent)), 1e-12));
    out.tangent = tangent * scale;
    out.bitangent = bitangent * scale;
    return out;
}

// Steps through the height map along the view, for depth on a flat surface.
fn parallax_uv(
    uv: vec2<f32>,
    surface_at: SurfaceFrame,
    to_eye: vec3<f32>,
    dx: vec2<f32>,
    dy: vec2<f32>,
) -> vec2<f32> {
    let slot = material.slots_a.w;
    let depth = material.values.z;
    if slot < 0 || depth <= 0.0 {
        return uv;
    }
    let view = vec3<f32>(
        dot(to_eye, surface_at.tangent),
        dot(to_eye, surface_at.bitangent),
        max(dot(to_eye, surface_at.normal), 0.05),
    );
    let step_uv = (view.xy / view.z) * depth / 16.0;
    var at = uv;
    var layer_depth = 0.0;
    var above = 0.0;
    for (var index = 0; index < 16; index = index + 1) {
        let below = 1.0 - read_slot(slot, at, dx, dy).r - layer_depth;
        if below <= 0.0 {
            // Between the last two steps the view crossed the surface, and this
            // finds where, which takes the terraces out of it.
            let share = above / max(above - below, 0.0001);
            return at + step_uv * (1.0 - share);
        }
        above = below;
        at = at - step_uv;
        layer_depth = layer_depth + 1.0 / 16.0;
    }
    return at;
}

// Whether this is the shader for surfaces with nothing unusual about them, which
// most are. Everything else a surface can do makes the shader bigger and slower
// for all of them, even where it's off, so `plain` leaves it out.
const PLAIN: bool = false;

// Specular light from the dynamic lights, on a bumped surface.
fn specular_light(world: vec3<f32>, normal: vec3<f32>, to_eye: vec3<f32>, gloss: f32) -> f32 {
    var total = 0.0;
    let count = u32(frame.params.z);
    for (var index = 0u; index < count; index = index + 1u) {
        let light = frame.lights[index];
        let offset = light.xyz - world;
        let distance_to = length(offset);
        let reach = clamp(1.0 - distance_to / max(light.w, 1.0), 0.0, 1.0);
        let half_vector = normalize(normalize(offset) + to_eye);
        total = total + pow(max(dot(normal, half_vector), 0.0), gloss) * reach;
    }
    return total;
}

// Hard bands from smooth light, from the ramp if there is one.
fn cel_bands(amount: f32, dx: vec2<f32>, dy: vec2<f32>) -> vec3<f32> {
    let slot = material.slots_b.z;
    let level = clamp(amount * 0.5, 0.0, 1.0);
    if slot >= 0 {
        return read_slot(slot, vec2<f32>(level, 0.5), vec2<f32>(0.0), vec2<f32>(0.0)).rgb * 2.0;
    }
    return vec3<f32>(floor(level * 3.0 + 0.5) / 3.0 * 2.0);
}

// The base colour moved along a flow map. Two phases half a cycle apart hide
// each other's restart.
fn flowing(uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>) -> vec4<f32> {
    let flow = read_slot(i32(material.extra.w), uv, dx, dy).xy * 2.0 - vec2<f32>(1.0);
    let cycle = frame.params.x * material.extra.z;
    let first = fract(cycle);
    let second = fract(cycle + 0.5);
    let strength = material.extra.y;
    let one = read_slot(0, uv + flow * first * strength, dx, dy);
    let two = read_slot(0, uv + flow * second * strength, dx, dy);
    return mix(one, two, abs(first * 2.0 - 1.0));
}

fn material_color(in: VertexOut) -> vec4<f32> {
    var uv0 = in.uv;
    if !PLAIN {
        uv0 = moved_uv(in.uv);
    }
    let dx = dpdx(in.uv);
    let dy = dpdy(in.uv);
    let surface_at = surface_frame(in.world, in.uv);
    let to_eye = normalize(frame.eye.xyz - in.world);
    var uv = uv0;
    var features = 0;
    if !PLAIN {
        uv = parallax_uv(uv0, surface_at, to_eye, dx, dy);
        features = material.flags.z;
    }

    var base: vec4<f32>;
    if PLAIN {
        base = read_slot(0, uv, dx, dy);
    } else if (features & 1) != 0 {
        // Project from the world along each axis, and blend by the normal.
        let scale = 1.0 / 128.0;
        let weights = pow(abs(surface_at.normal), vec3<f32>(4.0));
        let total = weights.x + weights.y + weights.z;
        let along_x = read_slot(0, in.world.yz * scale, dx, dy);
        let along_y = read_slot(0, in.world.xz * scale, dx, dy);
        let along_z = read_slot(0, in.world.xy * scale, dx, dy);
        base = (along_x * weights.x + along_y * weights.y + along_z * weights.z) / total;
    } else if (features & 2) != 0 {
        base = read_hex(0, uv, dx, dy);
    } else if material.extra.w >= 0.0 {
        base = flowing(uv, dx, dy);
    } else {
        base = read_slot(0, uv, dx, dy);
    }

    // Layers over the base, each joining along the height of what's below.
    var height_slot = -1;
    var layers = 0;
    if !PLAIN {
        height_slot = material.slots_a.w;
        layers = material.flags.w;
    }
    var below = 0.5;
    if height_slot >= 0 {
        below = read_slot(height_slot, uv, dx, dy).r;
    }
    for (var index = 0; index < layers; index = index + 1) {
        let layer = material.layers[index];
        let color = read_slot(layer.x, uv, dx, dy);
        var mask = read_slot(layer.y, uv, dx, dy).r;
        // Mask 15 is paint, which belongs to the surface, not the material.
        if layer.y == 15 {
            mask = textureSampleLevel(paint_atlas, lightmap_sampler, in.light_uv, 0.0)[index];
        }
        let share = smoothstep(below - 0.1, below + 0.1, mask * 1.2 - 0.1);
        base = vec4<f32>(mix(base.rgb, color.rgb, share), base.a);
    }

    if !PLAIN && material.slots_b.w >= 0 {
        let scale = material.more.x;
        let detail = read_slot(material.slots_b.w, uv * scale, dx * scale, dy * scale);
        base = vec4<f32>(base.rgb * detail.rgb * 2.0, base.a);
    }

    var normal = surface_at.normal;
    if material.slots_a.x >= 0 {
        let bump = read_slot(material.slots_a.x, uv, dx, dy).xyz * 2.0 - vec3<f32>(1.0);
        normal = normalize(
            surface_at.tangent * bump.x + surface_at.bitangent * bump.y + surface_at.normal * bump.z,
        );
    }

    // Where baked light comes from, so bumps and shine answer it too.
    let stored = textureSampleLevel(light_direction, lightmap_sampler, in.light_uv, 0.0);
    var baked_from = vec3<f32>(0.0);
    if stored.a > 0.5 {
        baked_from = stored.xyz * 2.0 - vec3<f32>(1.0);
    }
    let has_direction = dot(baked_from, baked_from) > 0.01;

    var color = base.rgb;
    var shading = 0;
    if !PLAIN {
        shading = material.flags.x;
    }
    var baked = vec3<f32>(0.0);
    if shading != 3 || material.slots_a.y >= 0 {
        baked = light(in);
    }
    if shading != 3 {
        var lit = baked;
        if has_direction {
            let toward = normalize(baked_from);
            // Light baked for a flat surface is shaped by how a bump turns from
            // it. Where it strikes at a shallow angle, a small turn is a huge
            // ratio, which only shows as glare, so it's held to what looks true.
            let flat_response = max(dot(surface_at.normal, toward), 0.25);
            let bumped = max(dot(normal, toward), 0.0);
            lit = lit * mix(1.0, min(bumped / flat_response, 1.5), 0.8);
        }
        if shading == 1 {
            let amount = dot(lit, vec3<f32>(0.3, 0.59, 0.11));
            color = base.rgb * cel_bands(amount, dx, dy) * (lit / max(amount, 0.001)) * 0.5 + base.rgb * 0.0;
        } else {
            color = base.rgb * lit;
        }
    }

    if material.slots_a.y >= 0 {
        let specular = read_slot(material.slots_a.y, uv, dx, dy);
        let gloss = material.values.y * max(specular.a, 0.05);
        var shine = specular_light(in.world, normal, to_eye, gloss);
        if has_direction {
            // A highlight from baked light, which slides as the eye moves.
            let half_vector = normalize(normalize(baked_from) + to_eye);
            let strength = dot(baked, vec3<f32>(0.333));
            shine = shine + pow(max(dot(normal, half_vector), 0.0), gloss) * strength;
        }
        color = color + specular.rgb * shine * material.values.x;
    }

    if shading == 2 && material.slots_b.y >= 0 {
        // A matcap held to the view's direction, so it doesn't swim as the
        // camera turns.
        let side = normalize(cross(to_eye, vec3<f32>(0.0, 0.0, 1.0)));
        let up = cross(side, to_eye);
        let at = vec2<f32>(dot(normal, side), -dot(normal, up)) * 0.5 + vec2<f32>(0.5);
        let shine = read_slot(material.slots_b.y, at, vec2<f32>(0.0), vec2<f32>(0.0));
        color = color * shine.rgb * 2.0;
    }

    if !PLAIN && material.slots_a.z >= 0 {
        color = color + read_slot(material.slots_a.z, uv, dx, dy).rgb * material.more.y;
    }

    // An opaque surface's alpha says how much the final pass mirrors it.
    var alpha = 1.0 - material.extra.x;
    if material.flags.y != 0 {
        alpha = base.a;
    }
    // A detail fades away with distance: by alpha if it can, or by a dither.
    if (features & 4) != 0 {
        let reach = frame.pool_params.y;
        let fade = 1.0 - smoothstep(reach * 0.75, reach, length(frame.eye.xyz - in.world));
        if material.flags.y == 2 {
            alpha = alpha * fade;
        } else {
            let at = in.clip.xy;
            let noise = fract(52.9829189 * fract(dot(at, vec2<f32>(0.06711056, 0.00583715))));
            if fade < noise {
                discard;
            }
        }
    }
    return vec4<f32>(scene_color(color), alpha);
}

struct MaskedOut {
    @location(0) color: vec4<f32>,
    @builtin(sample_mask) mask: u32,
}

// A masked surface covers some of a pixel's samples, by how far alpha is past
// its cutoff here, which keeps thin edges from crawling.
@fragment
fn fs_masked(in: VertexOut) -> MaskedOut {
    let shaded = material_color(in);
    let coverage = clamp(
        (shaded.a - material.more.z) / max(fwidth(shaded.a), 0.0001) + 0.5,
        0.0,
        1.0,
    );
    let count = u32(coverage * frame.stencil_params.w + 0.5);
    if count == 0u {
        discard;
    }
    var out: MaskedOut;
    out.color = vec4<f32>(shaded.rgb, 1.0 - material.extra.x);
    out.mask = (1u << count) - 1u;
    return out;
}

@fragment
fn fs_material(in: VertexOut) -> @location(0) vec4<f32> {
    return material_color(in);
}
