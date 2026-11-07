// The world: lit surfaces, cutouts, liquids and sky (proposal 0018).

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var lightmap: texture_2d<f32>;
@group(0) @binding(2) var lightmap_sampler: sampler;
@group(0) @binding(3) var light_direction: texture_2d<f32>;
@group(0) @binding(5) var shadow_atlas: texture_depth_2d;
@group(0) @binding(7) var paint_atlas: texture_2d<f32>;
@group(0) @binding(6) var shadow_sampler: sampler_comparison;
// The sun's three maps, side by side, nearest first.
@group(0) @binding(8) var sun_shadows: texture_depth_2d;
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
    @location(11) color: vec4<f32>,
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
    // What a mesh's corners' colours multiply the surface by. White on what has none.
    @location(6) color: vec4<f32>,
    // Which sheet of land this is, plus one, or 0 for anything else.
    @location(7) @interpolate(flat) land: u32,
}

// A style saying that what moves has no chart in the atlas, and its light fields are a colour.
const OWN_LIGHT: u32 = 254u;

// A copy of a shared mesh: its corners, and what the copy says (proposal 0140).
struct MeshIn {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(5) normal: vec4<f32>,
    @location(6) row_x: vec4<f32>,
    @location(7) row_y: vec4<f32>,
    @location(8) row_z: vec4<f32>,
    @location(9) light: vec3<f32>,
    @location(10) styles: vec4<u32>,
    @location(11) color: vec4<f32>,
}

@vertex
fn vs_mesh(in: MeshIn) -> VertexOut {
    return placed_mesh(in);
}

// What a copy of a mesh is, wherever it's going.
fn placed_mesh(in: MeshIn) -> VertexOut {
    var out: VertexOut;
    let corner = vec4<f32>(in.position, 1.0);
    let placed = vec3<f32>(dot(in.row_x, corner), dot(in.row_y, corner), dot(in.row_z, corner));
    let world = shader_vertex(placed);
    out.clip = frame.view_projection * vec4<f32>(world, 1.0);
    out.world = world;
    out.uv = in.uv;
    out.color = in.color;
    out.land = 0u;
    out.light_uv = in.light.xy;
    out.light_step = in.light.z;
    out.styles = in.styles;
    // Cofactors turn a normal rightly under any stretch, and a mirror flips them.
    let across = vec3<f32>(in.row_x.x, in.row_y.x, in.row_z.x);
    let along = vec3<f32>(in.row_x.y, in.row_y.y, in.row_z.y);
    let up = vec3<f32>(in.row_x.z, in.row_y.z, in.row_z.z);
    let turned = cross(along, up) * in.normal.x + cross(up, across) * in.normal.y
        + cross(across, along) * in.normal.z;
    let mirrored = select(1.0, -1.0, dot(across, cross(along, up)) < 0.0);
    // As a probe's face has: one light for all of it, given its shape here. Something
    // flattened to nothing has no way to face, and mustn't make one up.
    let facing = turned / max(length(turned), 0.000001);
    out.normal = vec4<f32>(facing * mirrored, 1.0);
    return out;
}

// What's told of a sheet of land.
struct LandSheet {
    // Its low corner, and how far apart its heights are.
    origin: vec4<f32>,
    // How many heights across, where its first is, whether it faces down, and where its holes
    // are, plus one, or 0 if it has none.
    shape: vec4<u32>,
    // What makes a place a place on its texture.
    uv: vec4<f32>,
    // Its first layer, how many it has, where its paint starts, in words, and its last row.
    paint: vec4<u32>,
}

// A layer land's painted with.
struct LandLayer {
    // One over how wide a repeat is.
    per_unit: f32,
    // How tight its shine is.
    gloss: f32,
    // Which of the pictures it is.
    picture: u32,
    // Whether its repeats are broken up by hexagon tiles.
    hex: u32,
}

@group(2) @binding(0) var<storage, read> land_sheets: array<LandSheet>;
@group(2) @binding(1) var<storage, read> land_heights: array<f32>;
@group(2) @binding(2) var<storage, read> land_normals: array<u32>;
@group(2) @binding(3) var<storage, read> land_holes: array<u32>;
@group(2) @binding(4) var<storage, read> land_paint: array<u32>;
@group(2) @binding(5) var<storage, read> land_layers: array<LandLayer>;
@group(2) @binding(6) var land_colors: texture_2d_array<f32>;
@group(2) @binding(7) var land_surfaces: texture_2d_array<f32>;

// Whether `world` is over a hole in sheet `sheet`, which is a bit for each cell.
fn in_land_hole(number: u32, world: vec3<f32>) -> bool {
    let sheet = land_sheets[number];
    if sheet.shape.w == 0u {
        return false;
    }
    let cell = vec2<u32>((world.xy - sheet.origin.xy) / sheet.origin.w);
    let at = cell.y * (sheet.shape.x - 1u) + cell.x;
    return (land_holes[sheet.shape.w - 1u + at / 32u] >> (at % 32u) & 1u) != 0u;
}

struct LandIn {
    @builtin(vertex_index) vertex: u32,
    // Its low corner's sample, how many cells between vertices, and its grid's size.
    @location(0) node: vec4<u32>,
    // Where vertices begin to slide and where they've arrived, then its light's low corner.
    @location(1) slide: vec4<f32>,
    // Its light's high corner, and the step to the next style.
    @location(2) light: vec4<f32>,
    // Which sheet, its styles, and its chunk.
    @location(3) sheet: vec4<u32>,
}

// The sample `x`, `y` of the sheet whose first height is `first`, `across` to a row.
fn land_at(first: u32, across: u32, x: u32, y: u32) -> u32 {
    return first + y * across + x;
}

// Land is a grid drawn again and again, and each vertex finds its height. Near the end of its
// reach, a vertex that isn't on the next coarser lattice slides onto one that is, so that where a
// part of the land meets a coarser part they already agree, and nothing has to be stitched.
@vertex
fn vs_terrain(in: LandIn) -> VertexOut {
    let sheet = land_sheets[in.sheet.x];
    let side = in.node.w + 1u;
    let grid = vec2<u32>(in.vertex % side, in.vertex / side);
    let stride = in.node.z;
    let cell = sheet.origin.w;
    let first = sheet.shape.y;
    let across = sheet.shape.x;
    let at = in.node.xy + grid * stride;
    let here = land_at(first, across, at.x, at.y);
    let height = land_heights[here];
    let flat_place = vec2<f32>(at) * cell + sheet.origin.xy;
    let place = vec3<f32>(flat_place, height + sheet.origin.z);
    let away = length(place - frame.eye.xyz);
    let share = clamp((away - in.slide.x) / max(in.slide.y - in.slide.x, 1.0), 0.0, 1.0);
    let odd = grid % vec2<u32>(2u);
    let goes = in.node.xy + (grid - odd) * stride;
    let there = land_at(first, across, goes.x, goes.y);
    let moved_flat = vec2<f32>(goes) * cell + sheet.origin.xy;
    let flat_now = mix(flat_place, moved_flat, share);
    let height_now = mix(height, land_heights[there], share);
    let normal_here = unpack4x8snorm(land_normals[here]).xyz;
    let normal_there = unpack4x8snorm(land_normals[there]).xyz;

    var out: VertexOut;
    let world = shader_vertex(vec3<f32>(flat_now, height_now + sheet.origin.z));
    out.clip = frame.view_projection * vec4<f32>(world, 1.0);
    out.world = world;
    out.uv = flat_now * sheet.uv.xy;
    out.color = vec4<f32>(1.0);
    // Light's laid over the chunk, which a part may be a quarter of.
    let chunk_first = (in.node.xy / 64u) * 64u;
    let through = (vec2<f32>(at) - vec2<f32>(chunk_first)) / 64.0;
    out.light_uv = mix(in.slide.zw, in.light.xy, through);
    out.light_step = in.light.z;
    let styles = in.sheet.y;
    out.styles = vec4<u32>(styles & 255u, (styles >> 8u) & 255u, (styles >> 16u) & 255u, styles >> 24u);
    out.normal = vec4<f32>(normalize(mix(normal_here, normal_there, share)), 0.0);
    out.land = in.sheet.x + 1u;
    return out;
}

// Depth for land, which has holes in it.
@fragment
fn fs_land_depth(in: VertexOut) {
    if in_land_hole(in.land - 1u, in.world) {
        discard;
    }
}

// The same mesh pushed out along its normals, with its front faces culled: what shows round its
// edge is an outline, as an alias model has (proposal 0081).
@vertex
fn vs_mesh_hull(in: MeshIn) -> VertexOut {
    var out = placed_mesh(in);
    // Wider with distance, so a line keeps its width on screen.
    let away = length(out.world - frame.eye.xyz);
    let width = max(frame.post_params.y, 1.0) * away * 0.0016;
    let pushed = out.world + out.normal.xyz * width;
    out.clip = frame.view_projection * vec4<f32>(pushed, 1.0);
    return out;
}

@fragment
fn fs_mesh_hull() -> @location(0) vec4<f32> {
    return vec4<f32>(frame.outline.rgb, 1.0);
}

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    var out: VertexOut;
    let world = shader_vertex(in.position + model_offset.xyz);
    out.clip = frame.view_projection * vec4<f32>(world, 1.0);
    out.world = world;
    out.uv = in.uv;
    out.color = in.color;
    out.land = 0u;
    out.light_uv = in.light_uv;
    out.light_step = in.light_step;
    out.styles = in.styles;
    out.normal = in.normal;
    return out;
}

// The layers of the sun's own light and of what it bounces, as in the map.
const SUN_BOUNCE_STYLE: u32 = 58u;
const SUN_STYLE: u32 = 59u;

// The light from one layer, or none for an unused one.
fn layer(in: VertexOut, slot: f32, style: u32) -> vec3<f32> {
    if style >= 64u {
        return vec3<f32>(0.0);
    }
    let uv = in.light_uv + vec2<f32>(in.light_step * slot, 0.0);
    let sample = textureSampleLevel(lightmap, lightmap_sampler, uv, 0.0);
    var color = sample.rgb;
    if style >= 60u && style <= 62u {
        // Alpha says how far the shadow's edge is, which blends between samples
        // far better than light does, so the edge stays sharp.
        color = color * smoothstep(0.46, 0.54, sample.a);
    }
    // A day changes the sun's colour, and a style's only a number.
    if style == SUN_BOUNCE_STYLE || style == SUN_STYLE {
        color = color * frame.sun_tint.rgb;
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
    // A sun that isn't baked: its maps, then towards it with w 1 if it's out, its light, and how
    // far each map reaches.
    sun_matrices: array<mat4x4<f32>, 3>,
    sun_toward: vec4<f32>,
    sun_light: vec4<f32>,
    sun_reach: vec4<f32>,
}

@group(0) @binding(4) var<uniform> live: LiveFrame;

// How many pixels a shadow tile is across, as in the backend.
const SHADOW_TILE_SIZE: f32 = 512.0;

// How many of a shadow's pixels a surface is looked at from above itself, facing
// its light and edge on to it. Edge on, one pixel covers far more of a surface.
const SHADOW_LIFT_FACING: f32 = 1.5;
const SHADOW_LIFT_EDGE_ON: f32 = 6.0;

// How many of a shadow's pixels apart its samples are: close to its light, far
// from it, and how many world units it takes to add one.
const SHADOW_SOFT_NEAR: f32 = 1.5;
const SHADOW_SOFT_FAR: f32 = 5.0;
const SHADOW_SOFT_RATE: f32 = 96.0;

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
    // A shadow softens the further it falls, as a real light has some size. The
    // wider it looks, the further off the surface it has to look from, or its
    // far samples fall below a slanted surface and shade it in stripes.
    let spread = clamp(SHADOW_SOFT_NEAR + length(from_light) / SHADOW_SOFT_RATE, SHADOW_SOFT_NEAR, SHADOW_SOFT_FAR);
    let lift = mix(SHADOW_LIFT_FACING, SHADOW_LIFT_EDGE_ON, edge_on) * spread * 0.75;
    let lifted = point + normal * pixel * lift;
    let clip = live.shadows[tile] * vec4<f32>(lifted, 1.0);
    let ndc = clip.xyz / clip.w;
    let margin = (spread * 1.5 + 1.0) / SHADOW_TILE_SIZE;
    let inner = clamp(vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5), vec2<f32>(margin), vec2<f32>(1.0 - margin));
    let cell = vec2<f32>(f32(tile % 8), f32(tile / 8));
    let uv = (cell + inner) / 8.0;
    let step_size = spread / (SHADOW_TILE_SIZE * 8.0);
    // A tent over sixteen samples, each already blended by the sampler. It's
    // wider than a stair step on a surface nearly edge on, and has no noise in
    // it, as noise on a surface reads as specks.
    let weights = vec4<f32>(1.0, 3.0, 3.0, 1.0);
    var total = 0.0;
    for (var y = 0; y < 4; y = y + 1) {
        for (var x = 0; x < 4; x = x + 1) {
            let offset = (vec2<f32>(f32(x), f32(y)) - vec2<f32>(1.5)) * step_size;
            let weight = weights[x] * weights[y];
            total = total + weight * textureSampleCompareLevel(shadow_atlas, shadow_sampler, uv + offset, ndc.z);
        }
    }
    return total / 64.0;
}

// How many pixels across one of the sun's maps is, as in the backend.
const SUN_TILE: f32 = 2048.0;

// How much of the sun gets to `point`, which faces `normal`, from the nearest of its maps it's in.
// Past the last there's no shadow, as far off it's too coarse to see.
fn sun_shadow(point: vec3<f32>, normal: vec3<f32>) -> f32 {
    let depth = dot(point - frame.eye.xyz, live.forward.xyz);
    var cascade = 0;
    if depth > live.sun_reach.x * 0.9 {
        cascade = 1;
    }
    if depth > live.sun_reach.y * 0.9 {
        cascade = 2;
    }
    if depth > live.sun_reach.z * 0.9 {
        return 1.0;
    }
    let reach = live.sun_reach[cascade];
    let pixel = 2.0 * reach / SUN_TILE;
    let facing = abs(dot(normal, live.sun_toward.xyz));
    let lifted = point + normal * pixel * mix(3.0, 1.5, facing);
    let clip = live.sun_matrices[cascade] * vec4<f32>(lifted, 1.0);
    let margin = 2.0 / SUN_TILE;
    let inner = clamp(vec2<f32>(clip.x * 0.5 + 0.5, 0.5 - clip.y * 0.5), vec2<f32>(margin), vec2<f32>(1.0 - margin));
    let uv = vec2<f32>((f32(cascade) + inner.x) / 3.0, inner.y);
    let step_size = vec2<f32>(1.0 / (SUN_TILE * 3.0), 1.0 / SUN_TILE);
    var total = 0.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let offset = vec2<f32>(f32(x), f32(y)) * step_size * 1.5;
            total = total + textureSampleCompareLevel(sun_shadows, shadow_sampler, uv + offset, clip.z);
        }
    }
    return total / 9.0;
}

// The sun's light on a pixel, and how much of it gets there. It's kept for the highlight.
var<private> sun_seen: f32 = 0.0;

// A sun that isn't baked, on a surface. Its plane comes from the pixel, facing the eye.
fn sun_on(world: vec3<f32>, smooth_normal: vec3<f32>) -> vec3<f32> {
    sun_seen = 0.0;
    if live.sun_toward.w < 0.5 {
        return vec3<f32>(0.0);
    }
    var normal = normalize(cross(dpdx(world), dpdy(world)));
    if dot(normal, frame.eye.xyz - world) < 0.0 {
        normal = -normal;
    }
    if dot(smooth_normal, smooth_normal) > 0.01 {
        normal = normalize(smooth_normal);
    }
    let facing = dot(normal, live.sun_toward.xyz);
    if facing <= 0.0 {
        return vec3<f32>(0.0);
    }
    sun_seen = sun_shadow(world, normal);
    return live.sun_light.rgb * facing * sun_seen;
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
fn live_light(world: vec3<f32>, cluster: vec2<u32>, smooth_normal: vec3<f32>) -> vec3<f32> {
    let masks = vec2<u32>(cluster.x, cluster.y & 0xffffu);
    if (masks.x | masks.y) == 0u {
        return vec3<f32>(0.0);
    }
    let normal = normalize(cross(dpdx(world), dpdy(world)));
    // A mesh's light follows its own normal, or each triangle would be a band.
    let rounded = dot(smooth_normal, smooth_normal) > 0.01;
    var round_normal = vec3<f32>(0.0);
    if rounded {
        round_normal = normalize(smooth_normal);
    }
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
            var facing = abs(dot(normal, toward));
            if rounded {
                facing = max(dot(round_normal, toward), 0.0);
            }
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
    var baked = vec3<f32>(in.light_uv, in.light_step);
    if in.styles.x != OWN_LIGHT {
        baked = layer(in, 0.0, in.styles.x) + layer(in, 1.0, in.styles.y)
            + layer(in, 2.0, in.styles.z) + layer(in, 3.0, in.styles.w);
    }
    let cluster = cluster_at(in.world, in.clip.xy);
    let shaded = baked * capsule_shade(in.world, cluster);
    let moving = vec3<f32>(dynamic_light(in.world)) + live_light(in.world, cluster, in.normal.xyz)
        + sun_on(in.world, in.normal.xyz);
    // A curve above 1 makes dim light dimmer, which is the murk of the old look:
    // 2.2 is what light did to a picture before scenes were linear.
    let lit = shaded + moving + frame.day_light.rgb;
    return pow(keep_hue(2.0 * lit, 2.0), vec3<f32>(frame.color_params.x));
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
