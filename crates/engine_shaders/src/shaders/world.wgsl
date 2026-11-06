// The world: lit surfaces, cutouts, liquids and sky (proposal 0018).

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var lightmap: texture_2d<f32>;
@group(0) @binding(2) var lightmap_sampler: sampler;
@group(0) @binding(3) var light_direction: texture_2d<f32>;
@group(0) @binding(5) var shadow_atlas: texture_depth_2d;
@group(0) @binding(7) var paint_atlas: texture_2d<f32>;
@group(0) @binding(6) var shadow_sampler: sampler_comparison;
@group(1) @binding(0) var surface: texture_2d<f32>;
@group(1) @binding(1) var surface_sampler: sampler;

// Where the model being drawn has moved to.
var<immediate> model_offset: vec4<f32>;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) light_uv: vec2<f32>,
    @location(3) light_step: f32,
    @location(4) styles: vec4<u32>,
    @location(5) normal: vec4<f32>,
}

struct VertexOut {
    // The depth pass and what follows it have to agree to the last bit.
    @builtin(position) @invariant clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) light_uv: vec2<f32>,
    @location(3) light_step: f32,
    @location(4) @interpolate(flat) styles: vec4<u32>,
    // Which way a mesh faces here, or zeros on a flat face. w is 1 where its
    // light came from one place, as a probe's does, and has no shape yet.
    @location(5) normal: vec4<f32>,
}

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    var out: VertexOut;
    let world = shader_vertex(in.position + model_offset.xyz);
    out.clip = frame.view_projection * vec4<f32>(world, 1.0);
    out.world = world;
    out.uv = in.uv;
    out.light_uv = in.light_uv;
    out.light_step = in.light_step;
    out.styles = in.styles;
    out.normal = in.normal;
    return out;
}

// The light from one layer, or none for an unused one.
fn layer(in: VertexOut, slot: f32, style: u32) -> vec3<f32> {
    if style >= 64u {
        return vec3<f32>(0.0);
    }
    let uv = in.light_uv + vec2<f32>(in.light_step * slot, 0.0);
    let sample = textureSampleLevel(lightmap, lightmap_sampler, uv, 0.0);
    var color = sample.rgb;
    if style == 62u {
        // Alpha says how far the shadow's edge is, which blends between samples
        // far better than light does, so the edge stays sharp.
        color = color * smoothstep(0.46, 0.54, sample.a);
    }
    return color * frame.styles[style / 4u][style % 4u];
}

// Light from `R_AddDynamicLights`, on the same scale as a light map sample.
// Its plane is worked out from the pixel, since each face is flat.
fn dynamic_light(world: vec3<f32>) -> f32 {
    let normal = normalize(cross(dpdx(world), dpdy(world)));
    var total = 0.0;
    let count = u32(frame.params.z);
    for (var index = 0u; index < count; index = index + 1u) {
        let light = frame.lights[index];
        let cut = frame.light_cuts[index / 4u][index % 4u];
        let offset = world - light.xyz;
        let height = abs(dot(offset, normal));
        let reach = light.w - height;
        let across = sqrt(max(dot(offset, offset) - height * height, 0.0));
        // The original's edge is blurred by its 16 unit light map samples.
        let edge = clamp((reach - cut - across) / 16.0, 0.0, 1.0);
        total = total + max(reach - across, 0.0) * edge;
    }
    return total / 255.0;
}

struct LiveLight {
    // Where it is, and how far it reaches.
    place: vec4<f32>,
    // Its colour, and its first shadow tile, or -1.
    color: vec4<f32>,
    // Which way a spot points, and the cosine of its half angle.
    spot: vec4<f32>,
}

struct LiveFrame {
    // The view's width and height, how many lights, and slices per doubling.
    params: vec4<f32>,
    // Which way the view looks, and where slices start.
    forward: vec4<f32>,
    lights: array<LiveLight, 48>,
    shadows: array<mat4x4<f32>, 60>,
    // Two clusters to an entry, each a 64 bit mask.
    clusters: array<vec4<u32>, 1152>,
}

@group(0) @binding(4) var<uniform> live: LiveFrame;

// How many pixels a shadow tile is across, as in the backend.
const SHADOW_TILE_SIZE: f32 = 512.0;

// How many of a shadow's pixels a surface is looked at from above itself, facing
// its light and edge on to it. Edge on, one pixel covers far more of a surface.
const SHADOW_LIFT_FACING: f32 = 1.5;
const SHADOW_LIFT_EDGE_ON: f32 = 6.0;

// How much of a light reaches `point`, whose surface faces `normal`, by its
// shadow tile. A pixel of a shadow map covers a patch of surface, and on a
// surface at a shallow angle that patch dips below what was drawn, so it would
// shade itself in specks. Looking from just off the surface, by the size of a
// pixel there, clears that where a fixed bias can't.
fn live_shadow(
    first_tile: f32,
    from_light: vec3<f32>,
    point: vec3<f32>,
    normal: vec3<f32>,
) -> f32 {
    let size = abs(from_light);
    var face = 4;
    if size.x >= size.y && size.x >= size.z {
        face = 0;
        if from_light.x < 0.0 {
            face = 1;
        }
    } else if size.y >= size.z {
        face = 2;
        if from_light.y < 0.0 {
            face = 3;
        }
    } else if from_light.z < 0.0 {
        face = 5;
    }
    let tile = i32(first_tile) + face;
    // A tile spans 90 degrees, so it's twice its depth across.
    let pixel = 2.0 * max(size.x, max(size.y, size.z)) / SHADOW_TILE_SIZE;
    let facing = abs(dot(normal, normalize(from_light)));
    let edge_on = sqrt(max(1.0 - facing * facing, 0.0));
    let lift = mix(SHADOW_LIFT_FACING, SHADOW_LIFT_EDGE_ON, edge_on);
    let lifted = point + normal * pixel * lift;
    let clip = live.shadows[tile] * vec4<f32>(lifted, 1.0);
    let ndc = clip.xyz / clip.w;
    let inner = clamp(vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5), vec2<f32>(0.01), vec2<f32>(0.99));
    let cell = vec2<f32>(f32(tile % 8), f32(tile / 8));
    let uv = (cell + inner) / 8.0;
    // Two pixels apart, so that the blur is wider than a stair step on a surface
    // nearly edge on. The sampler's own blend leaves no gaps between them.
    let step_size = 2.0 / (SHADOW_TILE_SIZE * 8.0);
    // A tent over nine samples, each already blended by the sampler. It has no
    // noise in it, as noise on a surface reads as specks.
    var total = 0.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let offset = vec2<f32>(f32(x), f32(y)) * step_size;
            let weight = f32((2 - abs(x)) * (2 - abs(y)));
            total = total + weight * textureSampleCompareLevel(shadow_atlas, shadow_sampler, uv + offset, ndc.z);
        }
    }
    return total / 16.0;
}

// What reaches a pixel: a bit for each moving light, then each capsule.
fn cluster_at(world: vec3<f32>, screen: vec2<f32>) -> vec2<u32> {
    let depth = dot(world - frame.eye.xyz, live.forward.xyz);
    let slice = u32(clamp(log2(max(depth, live.forward.w) / live.forward.w) * live.params.w, 0.0, 15.0));
    let column = u32(clamp(screen.x / live.params.x * 16.0, 0.0, 15.0));
    let row = u32(clamp(screen.y / live.params.y * 9.0, 0.0, 8.0));
    let at = (slice * 9u + row) * 16u + column;
    let pair = live.clusters[at / 2u];
    if at % 2u == 1u {
        return pair.zw;
    }
    return pair.xy;
}

// Moving lights on a surface. A pixel's cluster says which to look at.
fn live_light(world: vec3<f32>, cluster: vec2<u32>) -> vec3<f32> {
    let masks = vec2<u32>(cluster.x, cluster.y & 0xffffu);
    if (masks.x | masks.y) == 0u {
        return vec3<f32>(0.0);
    }
    let normal = normalize(cross(dpdx(world), dpdy(world)));
    var total = vec3<f32>(0.0);
    for (var half_mask = 0u; half_mask < 2u; half_mask = half_mask + 1u) {
        var bits = masks[half_mask];
        while bits != 0u {
            let bit = firstTrailingBit(bits);
            bits = bits & (bits - 1u);
            let light = live.lights[half_mask * 32u + bit];
            let offset = light.place.xyz - world;
            let distance_to = length(offset);
            let reach = light.place.w;
            if distance_to >= reach {
                continue;
            }
            let toward = offset / max(distance_to, 0.001);
            // Depth derivatives can point either way, and light only lands on
            // the side that faces it.
            let facing = abs(dot(normal, toward));
            let fall = 1.0 - (distance_to * distance_to) / (reach * reach);
            var strength = facing * fall * fall;
            if dot(light.spot.xyz, light.spot.xyz) > 0.5 {
                let along = dot(-toward, light.spot.xyz);
                strength = strength * smoothstep(light.spot.w, light.spot.w + 0.05, along);
            }
            if strength > 0.001 && light.color.w >= 0.0 {
                let lit_side = normal * sign(dot(normal, toward));
                strength = strength * live_shadow(light.color.w, -offset, world, lit_side);
            }
            total = total + light.color.rgb * strength;
        }
    }
    return total;
}

// How much light characters leave on a point. Each is a soft ball, shading
// what's behind it from the direction its light comes from.
fn capsule_shade(world: vec3<f32>, cluster: vec2<u32>) -> f32 {
    var open = 1.0;
    var bits = cluster.y >> 16u;
    while bits != 0u {
        let index = firstTrailingBit(bits);
        bits = bits & (bits - 1u);
        let capsule = frame.capsules[index];
        let source = frame.capsule_light[index];
        let offset = world - capsule.xyz;
        let along = -dot(offset, source.xyz);
        if along <= 0.0 {
            continue;
        }
        let across = length(offset + source.xyz * along);
        let radius = capsule.w * (1.0 + along / 256.0);
        let shade = 1.0 - smoothstep(radius * 0.4, radius, across);
        let distance_fade = clamp(1.0 - along / 256.0, 0.0, 1.0);
        open = open * (1.0 - shade * distance_fade * source.w);
    }
    return open;
}

// All the light on a pixel. A sample of one half leaves a texture unchanged.
// `color`, brought down as a whole if any of it is past `most`. Cutting each part
// off alone would turn orange light green near its source.
fn keep_hue(color: vec3<f32>, most: f32) -> vec3<f32> {
    let brightest = max(color.r, max(color.g, color.b));
    return color * (most / max(brightest, most));
}

fn light(in: VertexOut) -> vec3<f32> {
    let baked = layer(in, 0.0, in.styles.x) + layer(in, 1.0, in.styles.y)
        + layer(in, 2.0, in.styles.z) + layer(in, 3.0, in.styles.w);
    let cluster = cluster_at(in.world, in.clip.xy);
    let shaded = baked * capsule_shade(in.world, cluster);
    let moving = vec3<f32>(dynamic_light(in.world)) + live_light(in.world, cluster);
    // A curve above 1 makes dim light dimmer, which is the murk of the old look:
    // 2.2 is what light did to a picture before scenes were linear.
    return pow(keep_hue(2.0 * (shaded + moving), 2.0), vec3<f32>(frame.color_params.x));
}

// A colour on its way into the scene, which holds linear light, and has room for
// more than a screen can show. The final pass brings it to the screen.
fn scene_color(color: vec3<f32>) -> vec3<f32> {
    return max(color, vec3<f32>(0.0));
}

// The depth pass only wants depth, but a pipeline has to have a fragment stage.
@fragment
fn fs_depth() {
}
