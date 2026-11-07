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
    // Each layer's base, mask and surface slots, the last -1 if it has none.
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
    // What its shader was given: eight numbers.
    shader_params: array<vec4<f32>, 2>,
    // The slots of the images its shader was given, or -1.
    shader_slots: vec4<i32>,
    // x how metal, y how rough, z 1 if it's described that way, w the slot of its
    // map, or -1: occlusion in red, roughness in green and metal in blue, as glTF
    // has them.
    metal_rough: vec4<f32>,
}

// What surface code sees of a surface, and can change, ahead of its light.
struct Surface {
    // Its colour and alpha.
    base: vec4<f32>,
    // Which way it faces, in the world, with bumps.
    normal: vec3<f32>,
    // Light of its own, which is added after everything else.
    emissive: vec3<f32>,
    // Where it is in its pictures.
    uv: vec2<f32>,
    // Where it is in the world.
    world: vec3<f32>,
}

// One of the eight numbers a material gave its shader.
fn shader_param(index: i32) -> f32 {
    return material.shader_params[index / 4][index % 4];
}

// The seconds since the level began.
fn shader_time() -> f32 {
    return frame.params.x;
}

@group(1) @binding(2) var slot1: texture_2d<f32>;
@group(1) @binding(3) var slot2: texture_2d<f32>;
@group(1) @binding(4) var slot3: texture_2d<f32>;
@group(1) @binding(5) var slot4: texture_2d<f32>;
@group(1) @binding(6) var slot5: texture_2d<f32>;
@group(1) @binding(7) var slot6: texture_2d<f32>;
@group(1) @binding(8) var slot7: texture_2d<f32>;
@group(1) @binding(9) var<uniform> material: MaterialParams;
@group(1) @binding(10) var slot8: texture_2d<f32>;
@group(1) @binding(11) var slot9: texture_2d<f32>;
@group(1) @binding(12) var slot10: texture_2d<f32>;
@group(1) @binding(13) var slot11: texture_2d<f32>;

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
        case 8: { return textureSampleGrad(slot8, surface_sampler, uv, dx, dy); }
        case 9: { return textureSampleGrad(slot9, surface_sampler, uv, dx, dy); }
        case 10: { return textureSampleGrad(slot10, surface_sampler, uv, dx, dy); }
        case 11: { return textureSampleGrad(slot11, surface_sampler, uv, dx, dy); }
        default: { return textureSampleGrad(surface, surface_sampler, uv, dx, dy); }
    }
}

// One of the images a material gave its shader, which has to be read outside
// anything that varies across a surface.
fn shader_image(index: i32, uv: vec2<f32>) -> vec4<f32> {
    return read_slot(material.shader_slots[index], uv, dpdx(uv), dpdy(uv));
}

// A small hash, for hexagon tiles.
fn hash2(cell: vec2<f32>) -> vec2<f32> {
    let p = vec2<f32>(dot(cell, vec2<f32>(127.1, 311.7)), dot(cell, vec2<f32>(269.5, 183.3)));
    return fract(sin(p) * 43758.5453);
}

// Where three overlapping hexagon tiles read from, and how much of each.
struct HexTaps {
    first: vec2<f32>,
    second: vec2<f32>,
    third: vec2<f32>,
    shares: vec3<f32>,
}

fn hex_taps(uv: vec2<f32>) -> HexTaps {
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
    let sharp = pow(max(weights, vec3<f32>(0.0)), vec3<f32>(5.0));
    let total = max(sharp.x + sharp.y + sharp.z, 0.0001);
    return HexTaps(uv + hash2(a), uv + hash2(b), uv + hash2(c), sharp / total);
}

// The slot's texel, broken up by three overlapping hexagon tiles with random
// offsets, so that large walls don't show their repeat.
fn read_hex(slot: i32, uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>) -> vec4<f32> {
    let taps = hex_taps(uv);
    let first = read_slot(slot, taps.first, dx, dy);
    let second = read_slot(slot, taps.second, dx, dy);
    let third = read_slot(slot, taps.third, dx, dy);
    return first * taps.shares.x + second * taps.shares.y + third * taps.shares.z;
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

fn surface_frame(world: vec3<f32>, uv: vec2<f32>, smooth_normal: vec3<f32>) -> SurfaceFrame {
    let dp1 = dpdx(world);
    let dp2 = dpdy(world);
    let duv1 = dpdx(uv);
    let duv2 = dpdy(uv);
    var out: SurfaceFrame;
    var normal = normalize(cross(dp2, dp1));
    if dot(normal, frame.eye.xyz - world) < 0.0 {
        normal = -normal;
    }
    // A mesh says which way it faces, and that's what makes it look round. Seen
    // from behind, it's the other way.
    if dot(smooth_normal, smooth_normal) > 0.01 {
        let given = normalize(smooth_normal);
        normal = select(given, -given, dot(given, normal) < 0.0);
    }
    out.normal = normal;
    let perp2 = cross(dp2, normal);
    let perp1 = cross(normal, dp1);
    let tangent = perp2 * duv1.x + perp1 * duv2.x;
    let bitangent = perp2 * duv1.y + perp1 * duv2.y;
    // Each its own length: a bake's frame is square, and a stretched texture isn't a shallower
    // bump. Scaling them together squashed one, however fine the mesh.
    out.tangent = tangent * inverseSqrt(max(dot(tangent, tangent), 1e-12));
    out.bitangent = bitangent * inverseSqrt(max(dot(bitangent, bitangent), 1e-12));
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

// How much highlight there is here, from moving lights and baked light.
fn shine_at(
    in: VertexOut,
    normal: vec3<f32>,
    to_eye: vec3<f32>,
    gloss: f32,
    baked: vec3<f32>,
    baked_from: vec3<f32>,
    has_direction: bool,
) -> f32 {
    var shine = specular_light(in.world, normal, to_eye, gloss);
    if has_direction {
        // A highlight from baked light, which slides as the eye moves.
        let half_vector = normalize(normalize(baked_from) + to_eye);
        let strength = dot(baked, vec3<f32>(0.333));
        shine = shine + pow(max(dot(normal, half_vector), 0.0), gloss) * strength;
    }
    return shine;
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

// What painted land has worked out for a pixel, for `material_color` to light. A surface that
// isn't land leaves it alone.
struct Ground {
    painted: bool,
    base: vec3<f32>,
    normal: vec3<f32>,
    shine: f32,
    gloss: f32,
    tint: vec3<f32>,
}

var<private> ground: Ground = Ground(
    false,
    vec3<f32>(0.0),
    vec3<f32>(0.0, 0.0, 1.0),
    0.0,
    0.0,
    vec3<f32>(1.0),
);

fn material_color(in: VertexOut) -> vec4<f32> {
    var uv0 = shader_uv(in.uv, in.world);
    if !PLAIN {
        uv0 = moved_uv(uv0);
    }
    let dx = dpdx(in.uv);
    let dy = dpdy(in.uv);
    let surface_at = surface_frame(in.world, in.uv, in.normal.xyz);
    let to_eye = normalize(frame.eye.xyz - in.world);
    var uv = uv0;
    var features = 0;
    if !PLAIN {
        uv = parallax_uv(uv0, surface_at, to_eye, dx, dy);
        features = material.flags.z;
    }

    var base: vec4<f32>;
    if ground.painted {
        base = vec4<f32>(ground.base, 1.0);
    } else if PLAIN {
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
    if !PLAIN && !ground.painted {
        height_slot = material.slots_a.w;
        layers = material.flags.w;
    }
    // With no height to join along, the picture's own light and dark do it. Paint is
    // held at light sample spacing, and this gives its edge the picture's detail, with
    // what's painted reaching the dark cracks first.
    var below = sqrt(dot(base.rgb, vec3<f32>(0.3, 0.6, 0.1)));
    if height_slot >= 0 {
        below = read_slot(height_slot, uv, dx, dy).r;
    }
    // What layers with bumps and shine of their own have put over the base, and how much of it.
    var layer_bump = vec2<f32>(0.0);
    var layer_shine = 0.0;
    var layer_cover = 0.0;
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
        if layer.z >= 0 {
            let own = read_slot(layer.z, uv, dx, dy);
            layer_bump = mix(layer_bump, own.xy * 2.0 - vec2<f32>(1.0), share);
            layer_shine = mix(layer_shine, own.w, share);
            layer_cover = mix(layer_cover, 1.0, share);
        }
    }

    if !PLAIN && !ground.painted && material.slots_b.w >= 0 {
        let scale = material.more.x;
        let detail = read_slot(material.slots_b.w, uv * scale, dx * scale, dy * scale);
        base = vec4<f32>(base.rgb * detail.rgb * 2.0, base.a);
    }

    // As glTF has it: a corner's colour multiplies the surface, and white leaves it alone.
    base = base * in.color;
    base = vec4<f32>(base.rgb * ground.tint, base.a);

    var normal = surface_at.normal;
    if ground.painted {
        normal = ground.normal;
    } else if material.slots_a.x >= 0 || layer_cover > 0.0 {
        // Compiled normal maps hold two channels, and the third follows from them.
        var flat_bump = vec2<f32>(0.0);
        if material.slots_a.x >= 0 {
            var coded_bump = vec4<f32>(0.0);
            if (features & 2) != 0 {
                // The bumps have to move with the picture they belong to.
                coded_bump = read_hex(material.slots_a.x, uv, dx, dy);
            } else {
                coded_bump = read_slot(material.slots_a.x, uv, dx, dy);
            }
            flat_bump = coded_bump.xy * 2.0 - vec2<f32>(1.0);
        }
        flat_bump = flat_bump * (1.0 - layer_cover) + layer_bump;
        let rise = sqrt(max(1.0 - dot(flat_bump, flat_bump), 0.0));
        let bump = vec3<f32>(flat_bump, rise);
        normal = normalize(
            surface_at.tangent * bump.x + surface_at.bitangent * bump.y + surface_at.normal * bump.z,
        );
    }

    var coded: Surface;
    coded.base = base;
    coded.normal = normal;
    coded.emissive = vec3<f32>(0.0);
    coded.uv = uv;
    coded.world = in.world;
    coded = shader_surface(coded);
    base = coded.base;
    normal = coded.normal;

    // Where baked light comes from, so bumps and shine answer it too.
    let stored = textureSampleLevel(light_direction, lightmap_sampler, in.light_uv, 0.0);
    var baked_from = vec3<f32>(0.0);
    if in.styles.x == OWN_LIGHT {
        // What moves has no light baked for it, so it's shaped by light from above.
        baked_from = vec3<f32>(0.3, 0.2, 0.93);
    } else if stored.a > 0.5 {
        baked_from = stored.xyz * 2.0 - vec3<f32>(1.0);
    }
    let has_direction = dot(baked_from, baked_from) > 0.01;

    // Metal and roughness come to a colour and a gloss, as a specular map does,
    // so that all four can be on one surface. What's metal has no colour of its
    // own under light, only what it reflects, tinted.
    var shine_tint = vec3<f32>(0.0);
    var shine_gloss = 0.0;
    var metal = 0.0;
    var rough = 1.0;
    var occlusion = 1.0;
    let described = !PLAIN && material.metal_rough.z > 0.5;
    if described {
        var packed = vec4<f32>(1.0);
        if material.metal_rough.w >= 0.0 {
            packed = read_slot(i32(material.metal_rough.w), uv, dx, dy);
        }
        metal = clamp(packed.b * material.metal_rough.x, 0.0, 1.0);
        rough = clamp(packed.g * material.metal_rough.y, 0.05, 1.0);
        occlusion = packed.r;
        shine_tint = mix(vec3<f32>(0.04), base.rgb, metal);
        // Roughness says the same thing as gloss, the other way round.
        let squared = rough * rough;
        shine_gloss = max(2.0 / (squared * squared) - 2.0, 1.0);
    }

    var color = base.rgb;
    var shading = 0;
    if !PLAIN {
        shading = material.flags.x;
    }
    var baked = vec3<f32>(0.0);
    if shading != 3 || material.slots_a.y >= 0 || described || ground.painted || layer_cover > 0.0 {
        baked = light(in);
    }
    if shading != 3 {
        var lit = baked;
        if has_direction {
            let toward = normalize(baked_from);
            // Light baked for a flat surface is shaped by how a bump turns from
            // it. Where it strikes at a shallow angle, a small turn is a huge
            // ratio, which only shows as glare, so it's held to what looks true.
            let flat_facing = dot(surface_at.normal, toward);
            let turned = dot(normal, toward) - flat_facing;
            let response = clamp(1.0 + turned / max(flat_facing, 0.25), 0.0, 1.5);
            lit = lit * mix(1.0, response, 0.8);
            // Light from a probe has no shape, so it gets it here.
            if in.normal.w > 0.5 {
                lit = lit * (0.5 + max(dot(normal, toward), 0.0));
            }
        }
        if shading == 1 {
            let amount = dot(lit, vec3<f32>(0.3, 0.59, 0.11));
            color = base.rgb * cel_bands(amount, dx, dy) * (lit / max(amount, 0.001)) * 0.5 + base.rgb * 0.0;
        } else {
            color = base.rgb * lit;
        }
        if described {
            // What metal reflects is mostly the room, which the light here stands
            // in for, and less of it comes back from what's rough.
            let room = shine_tint * lit * mix(1.0, 0.45, rough);
            color = mix(color, room, metal) * occlusion;
        }
    }

    if ground.painted {
        if ground.shine > 0.0 {
            let lit_shine = shine_at(in, normal, to_eye, ground.gloss, baked, baked_from, has_direction);
            color = color + vec3<f32>(ground.shine) * lit_shine;
        }
    } else if material.slots_a.y >= 0 || layer_cover > 0.0 {
        var specular = vec4<f32>(0.0);
        if material.slots_a.y >= 0 {
            specular = read_slot(material.slots_a.y, uv, dx, dy) * (1.0 - layer_cover);
        }
        specular = specular + vec4<f32>(vec3<f32>(layer_shine), layer_cover);
        let gloss = material.values.y * max(specular.a, 0.05);
        color = color + specular.rgb * shine_at(in, normal, to_eye, gloss, baked, baked_from, has_direction) * material.values.x;
    }
    if described {
        // Sharper highlights are brighter, as the same light is in a smaller spot.
        let focus = min((shine_gloss + 8.0) / 25.0, 8.0);
        color = color + shine_tint * shine_at(in, normal, to_eye, shine_gloss, baked, baked_from, has_direction) * focus;
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

    color = color + coded.emissive;

    // An opaque surface's alpha says how much the final pass mirrors it. Polished
    // metal mirrors without being told to.
    var alpha = 1.0 - max(material.extra.x, metal * (1.0 - rough) * (1.0 - rough) * 0.6);
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
    let shaded = shader_whole(in);
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
    return shader_whole(in);
}

// Past this, land's the plain colour of its layers, and reads next to nothing.
const LAND_FAR: f32 = 6000.0;
// Hexagon tiles stop here, as what they hide has gone by then, and they're three times the reading.
const LAND_HEX_REACH: f32 = 2400.0;
// How much more a layer shows for standing higher: a whole picture's height is this many doublings.
const LAND_HEIGHT_DOUBLINGS: f32 = 10.0;

// Soft noise from 0 to 1, for what doesn't want to show a repeat.
fn land_noise(at: vec2<f32>) -> f32 {
    let cell = floor(at);
    let inside = fract(at);
    let smooth_at = inside * inside * (vec2<f32>(3.0) - inside * 2.0);
    let low = mix(hash2(cell).x, hash2(cell + vec2<f32>(1.0, 0.0)).x, smooth_at.x);
    let high = mix(hash2(cell + vec2<f32>(0.0, 1.0)).x, hash2(cell + vec2<f32>(1.0, 1.0)).x, smooth_at.x);
    return mix(low, high, smooth_at.y);
}

// A large soft change of colour over the land, which is nearly free, and hides a repeat that a
// pixel would need many more reads to hide.
fn land_tint(world: vec3<f32>) -> vec3<f32> {
    let wide = land_noise(world.xy / 1900.0);
    let close = land_noise(world.xy / 430.0 + vec2<f32>(17.0, 5.0));
    let light = 0.84 + wide * 0.22 + close * 0.1;
    let warm = (land_noise(world.xy / 2700.0 + vec2<f32>(3.0, 11.0)) - 0.5) * 0.08;
    return vec3<f32>(light + warm, light, light - warm);
}

// How much of four layers is at sample `at`: the first four, or with `half` 1, the last.
fn land_paint_at(first: u32, across: u32, at: vec2<u32>, half: u32) -> vec4<f32> {
    return unpack4x8unorm(land_paint[first + (at.y * across + at.x) * 2u + half]);
}

// A picture read in one of a layer's places, with hexagon tiles if `hex` says so.
struct LandRead {
    color: vec4<f32>,
    surface: vec4<f32>,
}

fn land_read(uv: vec2<f32>, picture: u32, dx: vec2<f32>, dy: vec2<f32>, hex: bool) -> LandRead {
    var out: LandRead;
    if hex {
        let taps = hex_taps(uv);
        out.color = textureSampleGrad(land_colors, surface_sampler, taps.first, picture, dx, dy) * taps.shares.x
            + textureSampleGrad(land_colors, surface_sampler, taps.second, picture, dx, dy) * taps.shares.y
            + textureSampleGrad(land_colors, surface_sampler, taps.third, picture, dx, dy) * taps.shares.z;
        out.surface = textureSampleGrad(land_surfaces, surface_sampler, taps.first, picture, dx, dy) * taps.shares.x
            + textureSampleGrad(land_surfaces, surface_sampler, taps.second, picture, dx, dy) * taps.shares.y
            + textureSampleGrad(land_surfaces, surface_sampler, taps.third, picture, dx, dy) * taps.shares.z;
    } else {
        out.color = textureSampleGrad(land_colors, surface_sampler, uv, picture, dx, dy);
        out.surface = textureSampleGrad(land_surfaces, surface_sampler, uv, picture, dx, dy);
    }
    return out;
}

// Works out painted land for a pixel. It reads only the layers that are there: the top of a
// picture along the land, and where land's steep, along the side too, so a cliff isn't smeared.
fn paint_ground(in: VertexOut, sheet: LandSheet, along_x: vec3<f32>, along_y: vec3<f32>) {
    let cell = (in.world.xy - sheet.origin.xy) / sheet.origin.w;
    let across = sheet.shape.x;
    let low = vec2<u32>(max(cell, vec2<f32>(0.0)));
    let high = min(low + vec2<u32>(1u), vec2<u32>(across - 1u, sheet.paint.w));
    let inside = clamp(cell - vec2<f32>(low), vec2<f32>(0.0), vec2<f32>(1.0));
    let first = sheet.paint.z;
    var weights: array<vec4<f32>, 2>;
    for (var half = 0u; half < 2u; half = half + 1u) {
        let below = mix(
            land_paint_at(first, across, low, half),
            land_paint_at(first, across, vec2<u32>(high.x, low.y), half),
            inside.x,
        );
        let above = mix(
            land_paint_at(first, across, vec2<u32>(low.x, high.y), half),
            land_paint_at(first, across, high, half),
            inside.x,
        );
        weights[half] = mix(below, above, inside.y);
    }

    let facing = normalize(in.normal.xyz);
    let distance = length(frame.eye.xyz - in.world);
    let far = smoothstep(LAND_FAR * 0.6, LAND_FAR, distance);
    let near_hex = distance < LAND_HEX_REACH;
    // Up to here it's the top of a picture, and past it the side as well. Land further off has
    // fewer corners, so what they say of a cliff can be far from true, and the triangle's own
    // slope has the last word where it's steeper.
    let flat_facing = normalize(cross(along_y, along_x));
    let slope = select(facing, flat_facing, abs(flat_facing.z) < abs(facing.z));
    let side_share = 1.0 - smoothstep(0.5, 0.8, abs(slope.z));
    let on_x = abs(slope.x) > abs(slope.y);
    let side_uv = select(in.world.xz, in.world.yz, on_x);
    let side_dx = select(along_x.xz, along_x.yz, on_x);
    let side_dy = select(along_y.xz, along_y.yz, on_x);

    var near_color = vec3<f32>(0.0);
    var plain_color = vec3<f32>(0.0);
    var bump_top = vec2<f32>(0.0);
    var bump_side = vec2<f32>(0.0);
    var shine = 0.0;
    var gloss = 0.0;
    var near_total = 0.0;
    var plain_total = 0.0;
    for (var number = 0u; number < sheet.paint.y; number = number + 1u) {
        let weight = weights[number / 4u][number % 4u];
        if weight < 0.004 {
            continue;
        }
        let layer = land_layers[sheet.paint.x + number];
        if far > 0.0 {
            // The last of its mips is the mean of the whole picture.
            let mean = textureSampleLevel(land_colors, surface_sampler, vec2<f32>(0.5), layer.picture, 16.0);
            plain_color = plain_color + mean.rgb * weight;
            plain_total = plain_total + weight;
        }
        if far >= 1.0 {
            continue;
        }
        let scale = layer.per_unit;
        var read: LandRead;
        var side_read: LandRead;
        if side_share < 1.0 {
            read = land_read(
                in.world.xy * scale,
                layer.picture,
                along_x.xy * scale,
                along_y.xy * scale,
                near_hex && layer.hex != 0u,
            );
        }
        if side_share > 0.0 {
            side_read = land_read(side_uv * scale, layer.picture, side_dx * scale, side_dy * scale, false);
        }
        let color = mix(read.color, side_read.color, side_share);
        let surface = mix(read.surface, side_read.surface, side_share);
        let share = weight * exp2(surface.z * LAND_HEIGHT_DOUBLINGS);
        near_color = near_color + color.rgb * share;
        bump_top = bump_top + (read.surface.xy * 2.0 - vec2<f32>(1.0)) * share;
        bump_side = bump_side + (side_read.surface.xy * 2.0 - vec2<f32>(1.0)) * share;
        shine = shine + surface.w * share;
        gloss = gloss + layer.gloss * share;
        near_total = near_total + share;
    }

    let near_scale = 1.0 / max(near_total, 0.0001);
    let top = bump_top * near_scale * (1.0 - side_share);
    let side = bump_side * near_scale * side_share;
    // Bumps go on the way a picture lies on the land.
    var turned = vec3<f32>(top, 0.0);
    if on_x {
        turned = turned + vec3<f32>(0.0, side.x, side.y);
    } else {
        turned = turned + vec3<f32>(side.x, 0.0, side.y);
    }
    let plain = plain_color / max(plain_total, 0.0001);
    ground.painted = true;
    ground.base = mix(near_color * near_scale, plain, far);
    ground.normal = normalize(facing + turned * (1.0 - far));
    ground.shine = shine * near_scale * (1.0 - far);
    ground.gloss = gloss * near_scale;
}

// Land, with its holes left out.
@fragment
fn fs_land(in: VertexOut) -> @location(0) vec4<f32> {
    // Ahead of the hole, as a pixel gone from its neighbours would leave them nothing to compare.
    let along_x = dpdx(in.world);
    let along_y = dpdy(in.world);
    if in_land_hole(in.land - 1u, in.world) {
        discard;
    }
    let sheet = land_sheets[in.land - 1u];
    ground.tint = land_tint(in.world);
    if sheet.paint.y > 0u {
        paint_ground(in, sheet, along_x, along_y);
    }
    return shader_whole(in);
}
