// What happens to a finished scene: edge smoothing, outlines and fog
// (proposal 0075). This follows frame.wgsl in one module.

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var scene: texture_2d<f32>;
@group(0) @binding(2) var scene_depth: texture_depth_2d;
@group(0) @binding(3) var scene_sampler: sampler;
@group(0) @binding(4) var stencil0: texture_2d<f32>;
@group(0) @binding(5) var stencil1: texture_2d<f32>;
@group(0) @binding(6) var stencil2: texture_2d<f32>;
@group(0) @binding(7) var stencil3: texture_2d<f32>;
@group(0) @binding(8) var probe_slices: texture_2d<f32>;

struct PostOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_post(@builtin(vertex_index) index: u32) -> PostOut {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: PostOut;
    out.uv = corner;
    out.clip = vec4<f32>(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0);
    return out;
}

fn luma(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.299, 0.587, 0.114));
}

fn load_color(at: vec2<i32>) -> vec3<f32> {
    let size = vec2<i32>(textureDimensions(scene));
    return textureLoad(scene, clamp(at, vec2<i32>(0), size - vec2<i32>(1)), 0).rgb;
}

fn load_depth(at: vec2<i32>) -> f32 {
    let size = vec2<i32>(textureDimensions(scene_depth));
    return textureLoad(scene_depth, clamp(at, vec2<i32>(0), size - vec2<i32>(1)), 0);
}

// The world position of a pixel.
fn world_at(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    let world = frame.inverse_view_projection * ndc;
    return world.xyz / world.w;
}

// Smooths hard edges by blending along them, keeping the rest sharp.
fn smoothed(at: vec2<i32>, uv: vec2<f32>) -> vec3<f32> {
    let centre = load_color(at);
    let middle = luma(centre);
    let north = luma(load_color(at + vec2<i32>(0, -1)));
    let south = luma(load_color(at + vec2<i32>(0, 1)));
    let east = luma(load_color(at + vec2<i32>(1, 0)));
    let west = luma(load_color(at + vec2<i32>(-1, 0)));
    let highest = max(middle, max(max(north, south), max(east, west)));
    let lowest = min(middle, min(min(north, south), min(east, west)));
    let contrast = highest - lowest;
    if contrast < max(0.05, highest * 0.12) {
        return centre;
    }
    let size = vec2<f32>(textureDimensions(scene));
    let horizontal = abs(north + south - 2.0 * middle);
    let vertical = abs(east + west - 2.0 * middle);
    var step = vec2<f32>(0.0, 1.0) / size;
    var first = north;
    var second = south;
    if vertical > horizontal {
        step = vec2<f32>(1.0, 0.0) / size;
        first = west;
        second = east;
    }
    var towards = step;
    if abs(first - middle) > abs(second - middle) {
        towards = -step;
    }
    // Only a share of the way across, which softens the stair without blurring.
    let share = clamp(contrast * 2.0, 0.0, 0.5);
    return textureSampleLevel(scene, scene_sampler, uv + towards * share, 0.0).rgb;
}

// Smooths edges by finding how far each runs, so a long shallow stair blends
// along its length and the texture beside it stays sharp.
fn searched(at: vec2<i32>, uv: vec2<f32>) -> vec3<f32> {
    let centre = load_color(at);
    let middle = luma(centre);
    let north = luma(load_color(at + vec2<i32>(0, -1)));
    let south = luma(load_color(at + vec2<i32>(0, 1)));
    let east = luma(load_color(at + vec2<i32>(1, 0)));
    let west = luma(load_color(at + vec2<i32>(-1, 0)));
    let highest = max(middle, max(max(north, south), max(east, west)));
    let lowest = min(middle, min(min(north, south), min(east, west)));
    if highest - lowest < max(0.05, highest * 0.12) {
        return centre;
    }
    let size = vec2<f32>(textureDimensions(scene));
    let across_rows = abs(north + south - 2.0 * middle) >= abs(east + west - 2.0 * middle);
    var along = vec2<i32>(1, 0);
    var towards = vec2<i32>(0, -1);
    var first = north;
    var second = south;
    if !across_rows {
        along = vec2<i32>(0, 1);
        towards = vec2<i32>(-1, 0);
        first = west;
        second = east;
    }
    var other = first;
    if abs(second - middle) > abs(first - middle) {
        towards = -towards;
        other = second;
    }
    let edge = (middle + other) * 0.5;
    let step_size = abs(other - middle) * 0.25;
    // Walks both ways along the edge until it stops being one.
    var ahead = 12;
    var behind = 12;
    for (var step = 1; step <= 12; step = step + 1) {
        let forward = at + along * step;
        let level = (luma(load_color(forward)) + luma(load_color(forward + towards))) * 0.5;
        if abs(level - edge) >= step_size {
            ahead = step;
            break;
        }
    }
    for (var step = 1; step <= 12; step = step + 1) {
        let backward = at - along * step;
        let level = (luma(load_color(backward)) + luma(load_color(backward + towards))) * 0.5;
        if abs(level - edge) >= step_size {
            behind = step;
            break;
        }
    }
    let nearest = f32(min(ahead, behind));
    let span = f32(ahead + behind);
    // Near an end of its run a pixel takes in more of what's across the edge.
    let share = 0.5 - nearest / span;
    if share <= 0.0 {
        return centre;
    }
    let offset = vec2<f32>(towards) * share / size;
    return textureSampleLevel(scene, scene_sampler, uv + offset, 0.0).rgb;
}

// How strongly a pixel is on an outline, from 0 to 1.
fn outline_strength(at: vec2<i32>, uv: vec2<f32>, depth: f32) -> f32 {
    let mode = frame.outline.w;
    if mode < 0.5 || depth >= 1.0 {
        return 0.0;
    }
    let reach = i32(max(frame.post_params.y, 1.0));
    let here = length(world_at(uv, depth) - frame.eye.xyz);
    let size = vec2<f32>(textureDimensions(scene));
    var strongest = 0.0;
    var crease = 0.0;
    let centre_normal = pixel_normal(at, uv, depth, size);
    for (var index = 0; index < 4; index = index + 1) {
        var offset = vec2<i32>(reach, 0);
        if index == 1 {
            offset = vec2<i32>(-reach, 0);
        } else if index == 2 {
            offset = vec2<i32>(0, reach);
        } else if index == 3 {
            offset = vec2<i32>(0, -reach);
        }
        let other_at = at + offset;
        let other_uv = (vec2<f32>(other_at) + vec2<f32>(0.5)) / size;
        let other_depth = load_depth(other_at);
        let there = length(world_at(other_uv, other_depth) - frame.eye.xyz);
        // A gap in depth is a silhouette.
        strongest = max(strongest, smoothstep(4.0, 24.0, (there - here)));
        let normal = pixel_normal(other_at, other_uv, other_depth, size);
        crease = max(crease, 1.0 - dot(centre_normal, normal));
    }
    var strength = max(strongest, smoothstep(0.15, 0.5, crease) * 0.7);
    if mode > 1.5 {
        // Semi-outlines: fade with distance, and lighter on creases.
        let fade = 1.0 - clamp(here / max(frame.post_params.z, 1.0), 0.0, 1.0);
        strength = max(strongest, smoothstep(0.15, 0.5, crease) * 0.35) * fade;
    }
    return strength;
}

// A pixel's normal, from the depth around it.
fn pixel_normal(at: vec2<i32>, uv: vec2<f32>, depth: f32, size: vec2<f32>) -> vec3<f32> {
    let right_uv = uv + vec2<f32>(1.0, 0.0) / size;
    let down_uv = uv + vec2<f32>(0.0, 1.0) / size;
    let here = world_at(uv, depth);
    let right = world_at(right_uv, load_depth(at + vec2<i32>(1, 0)));
    let down = world_at(down_uv, load_depth(at + vec2<i32>(0, 1)));
    return normalize(cross(right - here, down - here));
}

// The baked light at a point in the air.
fn probe_light(point: vec3<f32>) -> vec3<f32> {
    let size = frame.probe_size.xyz;
    let local = clamp(
        (point - frame.probe_origin.xyz) / frame.probe_origin.w,
        vec3<f32>(0.0),
        size - vec3<f32>(1.0),
    );
    let low = floor(local.z);
    let high = min(low + 1.0, size.z - 1.0);
    let across = (local.x + 0.5) / size.x;
    let rows = size.y * size.z;
    let first = vec2<f32>(across, (low * size.y + local.y + 0.5) / rows);
    let second = vec2<f32>(across, (high * size.y + local.y + 0.5) / rows);
    let below = textureSampleLevel(probe_slices, scene_sampler, first, 0.0).rgb;
    let above = textureSampleLevel(probe_slices, scene_sampler, second, 0.0).rgb;
    return mix(below, above, local.z - low);
}

// Fog lit by the bake, marched along the view. It's brightest by lights.
fn grid_fogged(color: vec3<f32>, ray: vec3<f32>, jitter: f32) -> vec3<f32> {
    let density = frame.fog_color.w;
    let falloff = max(frame.fog_params.y, 0.0001);
    let steps = 16;
    let length_of = length(ray);
    let step_length = length_of / f32(steps);
    var left = 1.0;
    var gathered = vec3<f32>(0.0);
    for (var step = 0; step < steps; step = step + 1) {
        let point = frame.eye.xyz + ray * ((f32(step) + jitter) / f32(steps));
        let thickness = density * exp(-(point.z - frame.fog_params.x) * falloff);
        let through = exp(-thickness * step_length);
        gathered = gathered + left * (1.0 - through) * probe_light(point) * 2.0 * frame.fog_color.rgb;
        left = left * through;
    }
    return color * left + gathered;
}

// Where a world point falls on the screen, with its depth.
fn on_screen(point: vec3<f32>) -> vec3<f32> {
    let clip = frame.view_projection * vec4<f32>(point, 1.0);
    let ndc = clip.xyz / clip.w;
    return vec3<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5, ndc.z);
}

// What's seen along `direction` from `start`, by walking it across the screen.
// Past the picture's edge it can only be sky, if there is one.
fn mirrored(start: vec3<f32>, direction: vec3<f32>, fallback: vec3<f32>, jitter: f32) -> vec3<f32> {
    let size = vec2<f32>(textureDimensions(scene));
    var reach = 24.0;
    var point = start + direction * reach * jitter;
    for (var step = 0; step < 20; step = step + 1) {
        point = point + direction * reach;
        reach = reach * 1.18;
        let place = on_screen(point);
        if place.x < 0.0 || place.x > 1.0 || place.y < 0.0 || place.y > 1.0 || place.z > 1.0 {
            break;
        }
        let at = vec2<i32>(place.xy * size);
        let surface = load_depth(at);
        if surface < place.z && surface < 1.0 {
            let found = world_at(place.xy, surface);
            if length(found - point) < reach * 2.0 {
                // Fading out by the picture's edges hides where it runs out.
                let margin = min(min(place.x, 1.0 - place.x), min(place.y, 1.0 - place.y));
                return mix(fallback, load_color(at), smoothstep(0.0, 0.12, margin));
            }
        }
    }
    return fallback;
}

// What a mirror sees past everything: the sky, or nothing indoors.
fn sky_towards(direction: vec3<f32>, indoors: vec3<f32>) -> vec3<f32> {
    if frame.sky_zenith.w <= 0.0 && dot(frame.sky_zenith.rgb, vec3<f32>(1.0)) <= 0.0 {
        return indoors;
    }
    let up = clamp(direction.z, 0.0, 1.0);
    return mix(frame.sky_horizon.rgb, frame.sky_zenith.rgb, sqrt(up));
}

// A surface mirroring what's around it, by `strength`, more at a glance.
fn reflected(
    color: vec3<f32>,
    point: vec3<f32>,
    normal: vec3<f32>,
    strength: f32,
    jitter: f32,
) -> vec3<f32> {
    let view = normalize(point - frame.eye.xyz);
    let bounce = reflect(view, normal);
    let glance = pow(1.0 - clamp(-dot(view, normal), 0.0, 1.0), 3.0);
    let amount = strength * (0.25 + 0.75 * glance);
    let seen = mirrored(point + normal * 0.5, bounce, sky_towards(bounce, color), jitter);
    return mix(color, seen, amount);
}

// Lightens outer edges and darkens inner ones, as if corners were rounded and
// caught the light.
fn rounded_edges(color: vec3<f32>, at: vec2<i32>, uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let reach = frame.post_params.w;
    if reach < 0.5 || depth >= 1.0 {
        return color;
    }
    let size = vec2<f32>(textureDimensions(scene));
    let span = i32(reach);
    let normal = pixel_normal(at, uv, depth, size);
    let here = world_at(uv, depth);
    var bend = 0.0;
    for (var index = 0; index < 4; index = index + 1) {
        var offset = vec2<i32>(span, 0);
        if index == 1 {
            offset = vec2<i32>(-span, 0);
        } else if index == 2 {
            offset = vec2<i32>(0, span);
        } else if index == 3 {
            offset = vec2<i32>(0, -span);
        }
        let other_at = at + offset;
        let other_uv = (vec2<f32>(other_at) + vec2<f32>(0.5)) / size;
        let other_depth = load_depth(other_at);
        if other_depth >= 1.0 {
            continue;
        }
        let other = pixel_normal(other_at, other_uv, other_depth, size);
        let gap = world_at(other_uv, other_depth) - here;
        let apart = length(gap);
        // Near surfaces only: a gap in depth is a silhouette, not a corner.
        if apart > 16.0 * reach || apart < 0.001 {
            continue;
        }
        // A neighbour turning away from us over the gap is an outer edge.
        bend = bend + dot(other - normal, gap / apart);
    }
    return color * (1.0 + clamp(-bend, -1.0, 1.0) * 0.7);
}

// Where a ray crosses a box, as shares of it, from its start at most.
fn box_span(ray: vec3<f32>, low: vec3<f32>, high: vec3<f32>) -> vec2<f32> {
    let eye = frame.eye.xyz;
    let safe = select(ray, vec3<f32>(0.0001), abs(ray) < vec3<f32>(0.0001));
    let first = (low - eye) / safe;
    let second = (high - eye) / safe;
    let near = min(first, second);
    let away = max(first, second);
    return vec2<f32>(
        max(max(near.x, near.y), max(near.z, 0.0)),
        min(min(away.x, away.y), min(away.z, 1.0)),
    );
}

// How much of a ray is in the air: fog stops where it enters a pool.
fn share_in_air(ray: vec3<f32>) -> f32 {
    var share = 1.0;
    let count = i32(frame.pool_params.x);
    for (var index = 0; index < count; index = index + 1) {
        let span = box_span(ray, frame.pool_low[index].xyz, frame.pool_high[index].xyz);
        if span.y > span.x {
            share = min(share, span.x);
        }
    }
    return share;
}

// Fog that thickens with distance and lies low.
fn fogged(color: vec3<f32>, uv: vec2<f32>, depth: f32, jitter: f32) -> vec3<f32> {
    let density = frame.fog_color.w;
    if density <= 0.0 {
        return color;
    }
    let far = world_at(uv, min(depth, 0.99999));
    let whole = far - frame.eye.xyz;
    let ray = whole * share_in_air(whole);
    if frame.probe_size.w > 0.5 {
        return grid_fogged(color, ray, jitter);
    }
    let distance_to = length(ray);
    let falloff = max(frame.fog_params.y, 0.0001);
    let start = frame.eye.z - frame.fog_params.x;
    let rise = ray.z * falloff;
    // The integral of density along the ray, for fog fading with height.
    var amount = exp(-start * falloff);
    if abs(rise) > 0.0001 {
        amount = amount * (1.0 - exp(-rise)) / rise;
    }
    let total = density * distance_to * amount;
    return mix(color, frame.fog_color.rgb, 1.0 - exp(-max(total, 0.0)));
}

// Moving bright lines, as light makes through a rippling surface.
fn caustic(place: vec2<f32>) -> f32 {
    let time = frame.params.x;
    var at = place / 40.0;
    for (var turn = 0; turn < 3; turn = turn + 1) {
        at = at + vec2<f32>(
            sin(at.y * 1.3 + time * 0.7) * 0.6,
            cos(at.x * 1.1 - time * 0.6) * 0.6,
        );
    }
    let lines = abs(sin(at.x) + sin(at.y));
    return pow(1.0 - clamp(lines * 0.5, 0.0, 1.0), 6.0);
}

// Where a pool's surface lets through the light that reaches `point`.
fn surface_above(point: vec3<f32>, top: f32) -> vec2<f32> {
    var sun = vec3<f32>(0.0, 0.0, 1.0);
    if frame.sky_sun.z > 0.05 {
        sun = normalize(frame.sky_sun.xyz);
    }
    return point.xy + sun.xy * (top - point.z) / sun.z;
}

// Light in the pools a view passes through: depth tints it, what's in a pool
// shows caustics, and a view through one sees them as shafts.
fn in_pools(start: vec3<f32>, uv: vec2<f32>, depth: f32, jitter: f32) -> vec3<f32> {
    var color = start;
    let eye = frame.eye.xyz;
    let far = world_at(uv, min(depth, 0.9999));
    let ray = far - eye;
    let count = i32(frame.pool_params.x);
    for (var index = 0; index < count; index = index + 1) {
        let low = frame.pool_low[index];
        let high = frame.pool_high[index];
        let span = box_span(ray, low.xyz, high.xyz);
        let enter = span.x;
        let leave = span.y;
        if leave <= enter {
            continue;
        }
        let path = (leave - enter) * length(ray);
        // A view from above meets the surface first, which mirrors the sky.
        let entry = eye + ray * enter;
        if eye.z > high.z && abs(entry.z - high.z) < 1.0 {
            let ripple = caustic(entry.xy * 0.5) * 0.04;
            let normal = normalize(vec3<f32>(ripple, ripple * 0.5, 1.0));
            color = reflected(color, entry, normal, 0.6, jitter);
        }
        let inside = all(far >= low.xyz - vec3<f32>(1.0)) && all(far <= high.xyz + vec3<f32>(1.0));
        if inside && depth < 1.0 && high.w > 0.0 {
            let below = high.z - far.z;
            let pattern = caustic(surface_above(far, high.z));
            let reach = exp(-below / 320.0);
            color = color * (1.0 + pattern * high.w * 3.0 * reach);
        }
        color = mix(color, frame.pool_color[index].rgb, 1.0 - exp(-path * low.w));
        if high.w > 0.0 {
            var shafts = 0.0;
            for (var step = 0; step < 8; step = step + 1) {
                let share = mix(enter, leave, (f32(step) + jitter) / 8.0);
                let point = eye + ray * share;
                let below = high.z - point.z;
                shafts = shafts + caustic(surface_above(point, high.z)) * exp(-below / 200.0);
            }
            let amount = shafts / 8.0 * min(path / 256.0, 1.0) * high.w;
            color = color + frame.pool_color[index].rgb * amount * 2.5;
        }
    }
    return color;
}

// One stencil's picture, blurred, so rays stay chunky.
fn read_stencil(index: i32, uv: vec2<f32>) -> vec4<f32> {
    let level = frame.stencil_params.z;
    switch index {
        case 0: { return textureSampleLevel(stencil0, scene_sampler, uv, level); }
        case 1: { return textureSampleLevel(stencil1, scene_sampler, uv, level); }
        case 2: { return textureSampleLevel(stencil2, scene_sampler, uv, level); }
        default: { return textureSampleLevel(stencil3, scene_sampler, uv, level); }
    }
}

// How open a stencil is at `uv`, from its occlusion grid.
fn stencil_open(index: i32, uv: vec2<f32>) -> f32 {
    let cell = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(0.999)) * 16.0);
    let at = cell.y * 16 + cell.x;
    return frame.stencil_mask[index * 64 + at / 4][at % 4];
}

// Narrows `span` of a ray to the side of a plane that `inside` is on.
fn clip_span(
    span: vec2<f32>,
    eye: vec3<f32>,
    ray: vec3<f32>,
    at: vec3<f32>,
    normal: vec3<f32>,
) -> vec2<f32> {
    let start = dot(normal, eye - at);
    let change = dot(normal, ray);
    var out = span;
    if abs(change) < 0.00001 {
        if start < 0.0 {
            out.y = -1.0;
        }
    } else if change > 0.0 {
        out.x = max(out.x, -start / change);
    } else {
        out.y = min(out.y, -start / change);
    }
    return out;
}

// Narrows `span` of a ray to one side of a beam, the edge from `from` to `to`.
fn clip_edge(
    span: vec2<f32>,
    eye: vec3<f32>,
    ray: vec3<f32>,
    from_corner: vec3<f32>,
    to_corner: vec3<f32>,
    middle: vec3<f32>,
    source: vec4<f32>,
) -> vec2<f32> {
    var towards = source.xyz - from_corner;
    if source.w > 0.5 {
        towards = source.xyz;
    }
    var normal = cross(to_corner - from_corner, towards);
    if dot(normal, middle - from_corner) < 0.0 {
        normal = -normal;
    }
    return clip_span(span, eye, ray, from_corner, normal);
}

// Light in the air, shaped by a stencil. The view ray is cut to the beam, and
// each step looks towards the light and reads where that crosses the stencil.
// Brightness follows how far the ray runs through the beam.
fn stencil_rays(index: i32, eye: vec3<f32>, stop: vec3<f32>, jitter: f32) -> vec3<f32> {
    let corner = frame.stencil_corner[index];
    let across = frame.stencil_across[index].xyz;
    let down = frame.stencil_down[index].xyz;
    let reach = frame.stencil_down[index].w;
    let source = frame.stencil_light[index];
    var normal = normalize(cross(across, down));
    let width = dot(across, across);
    let height = dot(down, down);
    let middle = corner.xyz + (across + down) * 0.5;
    var to_light = source.xyz - middle;
    if source.w > 0.5 {
        to_light = source.xyz;
    }
    // The beam runs away from the light.
    if dot(normal, to_light) > 0.0 {
        normal = -normal;
    }
    let ray = stop - eye;
    let first = corner.xyz;
    let second = first + across;
    let third = second + down;
    let fourth = first + down;
    var span = vec2<f32>(0.0, 1.0);
    span = clip_span(span, eye, ray, first, normal);
    span = clip_span(span, eye, ray, first + normal * reach, -normal);
    span = clip_edge(span, eye, ray, first, second, middle, source);
    span = clip_edge(span, eye, ray, second, third, middle, source);
    span = clip_edge(span, eye, ray, third, fourth, middle, source);
    span = clip_edge(span, eye, ray, fourth, first, middle, source);
    if span.y <= span.x {
        return vec3<f32>(0.0);
    }
    let steps = i32(frame.stencil_params.y);
    var total = vec3<f32>(0.0);
    for (var step = 0; step < steps; step = step + 1) {
        let share = mix(span.x, span.y, (f32(step) + jitter) / f32(steps));
        let point = eye + ray * share;
        var towards = source.xyz - point;
        if source.w > 0.5 {
            towards = source.xyz;
        }
        let facing = dot(towards, normal);
        if abs(facing) < 0.0001 {
            continue;
        }
        let along = dot(corner.xyz - point, normal) / facing;
        if along <= 0.0 || (source.w < 0.5 && along >= 1.0) {
            continue;
        }
        let hit = point + towards * along;
        let local = hit - corner.xyz;
        let uv = vec2<f32>(dot(local, across) / width, dot(local, down) / height);
        if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 {
            continue;
        }
        let distance_to = length(point - hit);
        let fade = clamp(1.0 - distance_to / max(reach, 1.0), 0.0, 1.0);
        // Soft at the rim, so the beam has no hard outline.
        let rim = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
        let soft = smoothstep(0.0, 0.06, rim);
        let texel = read_stencil(index, uv);
        total = total + texel.rgb * texel.a * fade * soft * stencil_open(index, uv);
    }
    let path = (span.y - span.x) * length(ray);
    return total * corner.w * path / (f32(steps) * 256.0);
}

@fragment
fn fs_post(in: PostOut) -> @location(0) vec4<f32> {
    let at = vec2<i32>(in.clip.xy);
    let depth = load_depth(at);
    var color = load_color(at);
    if frame.post_params.x > 1.5 {
        color = searched(at, in.uv);
    } else if frame.post_params.x > 0.5 {
        color = smoothed(at, in.uv);
    }
    color = rounded_edges(color, at, in.uv, depth);
    // Interleaved gradient noise: an even dither, not grain.
    let noise = fract(52.9829189 * fract(dot(in.clip.xy, vec2<f32>(0.06711056, 0.00583715))));
    let shine = 1.0 - textureLoad(scene, at, 0).a;
    if shine > 0.02 && depth < 1.0 {
        let size = vec2<f32>(textureDimensions(scene));
        let normal = pixel_normal(at, in.uv, depth, size);
        color = reflected(color, world_at(in.uv, depth), -normal, shine, noise);
    }
    let edge = outline_strength(at, in.uv, depth);
    color = mix(color, frame.outline.rgb, edge);
    color = in_pools(color, in.uv, depth, noise);
    color = fogged(color, in.uv, depth, noise);
    let count = i32(frame.stencil_params.x);
    if count > 0 {
        let stop = world_at(in.uv, min(depth, 0.9999));
        for (var index = 0; index < count; index = index + 1) {
            color = color + stencil_rays(index, frame.eye.xyz, stop, noise);
        }
    }
    return vec4<f32>(color, 1.0);
}
