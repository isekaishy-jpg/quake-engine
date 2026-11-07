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
@group(0) @binding(9) var smaa_weights: texture_2d<f32>;
@group(0) @binding(10) var ray_light: texture_2d<f32>;
// Where shaped water's surfaces are: how far, which body, plus one, and how it slopes.
@group(0) @binding(11) var water_surface: texture_2d<f32>;
// What's over water: pictures that repeat.
@group(0) @binding(12) var water_pictures: texture_2d_array<f32>;
@group(0) @binding(13) var water_sampler: sampler;
// How high ripples near the view are.
@group(0) @binding(14) var ripples: texture_2d<f32>;
// A sky's layers of cloud, and what its rays go by, if it's a picture.
@group(0) @binding(15) var cloud_picture_0: texture_2d<f32>;
@group(0) @binding(16) var cloud_picture_1: texture_2d<f32>;
@group(0) @binding(17) var cloud_picture_2: texture_2d<f32>;
@group(0) @binding(18) var cloud_picture_3: texture_2d<f32>;
@group(0) @binding(19) var sky_ray_picture: texture_2d<f32>;

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

// How bright a colour looks. Scenes hold linear light.
fn luma(color: vec3<f32>) -> f32 {
    return sqrt(dot(color, vec3<f32>(0.299, 0.587, 0.114)));
}

// Linear light as a screen shows it. What's too bright comes down as a whole, so
// that it keeps its colour, and `gamma` is the player's brightness, as
// `BuildGammaTable` was for the palette. The noise, under a step of the screen's
// bytes, stops dark gradients banding.
fn to_screen(light: vec3<f32>, noise: f32) -> vec3<f32> {
    let bright = light * frame.color_params.y * frame.zone_light.rgb;
    let peak = max(bright.r, max(bright.g, bright.b));
    let held = max(bright / max(peak, 1.0), vec3<f32>(0.0));
    let raised = pow(held, vec3<f32>(frame.params.w));
    let low = raised * 12.92;
    let high = 1.055 * pow(raised, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(high, low, raised <= vec3<f32>(0.0031308)) + (noise - 0.5) / 255.0;
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

// What SMAA's weights say at `at`, which may be off the image's edge.
fn weights_at(at: vec2<i32>) -> vec4<f32> {
    let last = vec2<i32>(textureDimensions(smaa_weights)) - vec2<i32>(1);
    return textureLoad(smaa_weights, clamp(at, vec2<i32>(0), last), 0);
}

// SMAA's last step: a pixel takes what its edges' weights say from each side.
// The sampler does the mixing, by reading part way into the neighbour.
fn smaa_blended(at: vec2<i32>) -> vec3<f32> {
    let own = weights_at(at);
    let from_right = weights_at(at + vec2<i32>(1, 0)).a;
    let from_below = weights_at(at + vec2<i32>(0, 1)).g;
    let from_left = own.b;
    let from_above = own.r;
    if from_right + from_below + from_left + from_above < 0.00001 {
        return load_color(at);
    }
    let pixel = 1.0 / vec2<f32>(textureDimensions(scene));
    let middle = (vec2<f32>(at) + vec2<f32>(0.5)) * pixel;
    var first = vec2<f32>(0.0, from_below * pixel.y);
    var second = vec2<f32>(0.0, -from_above * pixel.y);
    var shares = vec2<f32>(from_below, from_above);
    if max(from_right, from_left) > max(from_below, from_above) {
        first = vec2<f32>(from_right * pixel.x, 0.0);
        second = vec2<f32>(-from_left * pixel.x, 0.0);
        shares = vec2<f32>(from_right, from_left);
    }
    shares = shares / (shares.x + shares.y);
    return textureSampleLevel(scene, scene_sampler, middle + first, 0.0).rgb * shares.x
        + textureSampleLevel(scene, scene_sampler, middle + second, 0.0).rgb * shares.y;
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
        // A gap in depth is a silhouette, unless it's only a crack between two
        // faces, with the same surface again just past it.
        let past = load_depth(other_at + offset);
        let crack = other_depth >= 1.0 && past < 1.0;
        if !crack {
            strongest = max(strongest, smoothstep(4.0, 24.0, (there - here)));
        }
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
    return pow(mix(below, above, local.z - low), vec3<f32>(frame.color_params.x));
}

// What moving lights add to fog at `point`.
fn lights_in_fog(point: vec3<f32>) -> vec3<f32> {
    var light = vec3<f32>(0.0);
    let count = i32(frame.fog_counts.y);
    for (var index = 0; index < count; index = index + 1) {
        let place = frame.fog_light_place[index];
        let share = 1.0 - length(place.xyz - point) / max(place.w, 1.0);
        light = light + frame.fog_light_color[index].rgb * max(share, 0.0) * max(share, 0.0);
    }
    return light;
}

// A number from 0 to 1 from a whole place in space.
fn fog_hash(at: vec3<i32>) -> f32 {
    var value = u32(at.x) * 0x9e3779b1u + u32(at.y) * 0x85ebca77u + u32(at.z) * 0xc2b2ae3du;
    value = (value ^ (value >> 15u)) * 0x2c1b3c6du;
    value = value ^ (value >> 12u);
    return f32(value & 0xffffu) / 65535.0;
}

// Smooth noise from 0 to 1.
fn fog_noise(place: vec3<f32>) -> f32 {
    let low = vec3<i32>(floor(place));
    let part = fract(place);
    let even = part * part * (vec3<f32>(3.0) - 2.0 * part);
    let near = mix(
        mix(fog_hash(low), fog_hash(low + vec3<i32>(1, 0, 0)), even.x),
        mix(fog_hash(low + vec3<i32>(0, 1, 0)), fog_hash(low + vec3<i32>(1, 1, 0)), even.x),
        even.y,
    );
    let far = mix(
        mix(fog_hash(low + vec3<i32>(0, 0, 1)), fog_hash(low + vec3<i32>(1, 0, 1)), even.x),
        mix(fog_hash(low + vec3<i32>(0, 1, 1)), fog_hash(low + vec3<i32>(1, 1, 1)), even.x),
        even.y,
    );
    return mix(near, far, even.z);
}

// How thick volume `index` is at `point`, out of all it can be.
fn fog_share(index: i32, point: vec3<f32>) -> f32 {
    let low = frame.fog_low[index];
    let high = frame.fog_high[index];
    let inside = min(point - low.xyz, high.xyz - point);
    var share = 1.0;
    if low.w > 0.0 {
        share = smoothstep(0.0, 1.0, clamp(min(inside.x, min(inside.y, inside.z)) / low.w, 0.0, 1.0));
    }
    let wisps = frame.fog_look[index].w;
    if wisps > 0.0 {
        let drift = frame.fog_drift[index];
        let at = (point - drift.xyz * frame.params.x) / drift.w;
        let body = fog_noise(at) * 0.65 + fog_noise(at * 2.3 + vec3<f32>(7.1)) * 0.35;
        share = share * mix(1.0, smoothstep(0.35, 0.75, body) * 1.6, wisps);
    }
    return share;
}

// Whether volume `index` has to be marched, as it changes along a ray.
fn fog_is_marched(index: i32) -> bool {
    let lit = (u32(frame.fog_counts.z) >> u32(index)) & 1u;
    return frame.fog_look[index].w > 0.0 || lit == 1u || frame.fog_low[index].w > 0.0;
}

// Even fog in places, along `ray` from the eye, furthest first, so that what's near goes over it.
fn volumes_fogged(start: vec3<f32>, ray: vec3<f32>) -> vec3<f32> {
    var color = start;
    let whole = length(ray);
    for (var index = i32(frame.fog_counts.x) - 1; index >= 0; index = index - 1) {
        if fog_is_marched(index) {
            continue;
        }
        let span = box_span(ray, frame.fog_low[index].xyz, frame.fog_high[index].xyz);
        if span.y > span.x {
            let through = exp(-frame.fog_high[index].w * (span.y - span.x) * whole);
            color = mix(frame.fog_look[index].rgb, color, through);
        }
    }
    return color;
}

// Fog in places that changes along `ray`: what it adds, and in w how much gets through. It's soft,
// so it's worked out with light in the air, for a quarter of the pixels.
fn volumes_marched(ray: vec3<f32>, jitter: f32) -> vec4<f32> {
    var left = 1.0;
    var gathered = vec3<f32>(0.0);
    let lit_bits = u32(frame.fog_counts.z);
    let whole = length(ray);
    for (var index = i32(frame.fog_counts.x) - 1; index >= 0; index = index - 1) {
        if !fog_is_marched(index) {
            continue;
        }
        let span = box_span(ray, frame.fog_low[index].xyz, frame.fog_high[index].xyz);
        if span.y <= span.x {
            continue;
        }
        let density = frame.fog_high[index].w;
        let tint = frame.fog_look[index].rgb;
        let lit = (lit_bits >> u32(index)) & 1u;
        let steps = 10;
        let step_length = (span.y - span.x) * whole / f32(steps);
        var own_left = 1.0;
        var own = vec3<f32>(0.0);
        for (var step = 0; step < steps; step = step + 1) {
            let share = mix(span.x, span.y, (f32(step) + jitter) / f32(steps));
            let point = frame.eye.xyz + ray * share;
            let through = exp(-density * fog_share(index, point) * step_length);
            var light = vec3<f32>(1.0);
            if lit == 1u {
                light = lights_in_fog(point);
                if frame.probe_size.w > 0.5 {
                    light = light + probe_light(point) * 2.0;
                }
            }
            own = own + own_left * (1.0 - through) * light * tint;
            own_left = own_left * through;
        }
        // Nearer goes over what's already been gathered from further off.
        gathered = gathered * own_left + own;
        left = left * own_left;
    }
    return vec4<f32>(gathered, left);
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
        let light = probe_light(point) * 2.0 + lights_in_fog(point);
        gathered = gathered + left * (1.0 - through) * light * frame.fog_color.rgb;
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
// Past the picture's edge it can only be sky, if there is one. Where a step goes
// past a surface it closes in on it, so what's mirrored is sharp, with no need of
// noise to hide the steps. That noise showed on a calm surface as a stipple.
fn mirrored(start: vec3<f32>, direction: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let size = vec2<f32>(textureDimensions(scene));
    var reach = 24.0;
    var point = start;
    for (var step = 0; step < 20; step = step + 1) {
        let before = point;
        point = point + direction * reach;
        reach = reach * 1.18;
        let place = on_screen(point);
        if place.x < 0.0 || place.x > 1.0 || place.y < 0.0 || place.y > 1.0 || place.z > 1.0 {
            break;
        }
        let surface = load_depth(vec2<i32>(place.xy * size));
        if surface < place.z && surface < 1.0 {
            var near = before;
            var far = point;
            for (var turn = 0; turn < 5; turn = turn + 1) {
                let middle = (near + far) * 0.5;
                let there = on_screen(middle);
                if load_depth(vec2<i32>(there.xy * size)) < there.z {
                    far = middle;
                } else {
                    near = middle;
                }
            }
            let hit = on_screen(far);
            let at = vec2<i32>(hit.xy * size);
            let found = world_at(hit.xy, load_depth(at));
            if length(found - far) < reach {
                // Fading out by the picture's edges hides where it runs out.
                let margin = min(min(hit.x, 1.0 - hit.x), min(hit.y, 1.0 - hit.y));
                return mix(fallback, load_color(at), smoothstep(0.0, 0.12, margin));
            }
        }
    }
    return fallback;
}

// What a mirror sees past everything: the sky, or nothing indoors. Having a sky
// isn't enough, as a cave under it can't see it.
fn sky_towards(direction: vec3<f32>, indoors: vec3<f32>) -> vec3<f32> {
    let no_sky = frame.sky_zenith.w <= 0.0 && dot(frame.sky_zenith.rgb, vec3<f32>(1.0)) <= 0.0;
    if no_sky || frame.pool_params.z < 0.5 {
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
) -> vec3<f32> {
    let view = normalize(point - frame.eye.xyz);
    let bounce = reflect(view, normal);
    let glance = pow(1.0 - clamp(-dot(view, normal), 0.0, 1.0), 5.0);
    let amount = strength * (0.08 + 0.92 * glance);
    let seen = mirrored(point + normal * 0.5, bounce, sky_towards(bounce, color));
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

// What's on the surface of shaped water at `uv`.
fn surface_at(uv: vec2<f32>) -> vec4<f32> {
    let size = vec2<f32>(textureDimensions(water_surface));
    let at = clamp(vec2<i32>(uv * size), vec2<i32>(0), vec2<i32>(size) - vec2<i32>(1));
    return textureLoad(water_surface, at, 0);
}

// How much of a ray at `uv` is in the air: fog stops where it enters a pool.
fn share_in_air(ray: vec3<f32>, uv: vec2<f32>) -> f32 {
    var share = 1.0;
    let count = i32(frame.pool_params.x);
    var shaped = false;
    for (var index = 0; index < count; index = index + 1) {
        if frame.pool_surface[index].w > 0.5 {
            shaped = true;
            continue;
        }
        let span = box_span(ray, frame.pool_low[index].xyz, frame.pool_high[index].xyz);
        if span.y > span.x {
            share = min(share, span.x);
        }
    }
    if frame.pool_params.w > 0.5 {
        return 0.0;
    }
    if shaped {
        let surface = surface_at(uv);
        if surface.y > 0.5 {
            share = min(share, surface.x / max(length(ray), 0.001));
        }
    }
    return share;
}

// Fog that thickens with distance and lies low, and then fog in places.
fn fogged(start: vec3<f32>, uv: vec2<f32>, depth: f32, jitter: f32) -> vec3<f32> {
    let density = frame.fog_color.w;
    let volumes = frame.fog_counts.x > 0.5;
    if density <= 0.0 && !volumes {
        return start;
    }
    let far = world_at(uv, min(depth, 0.99999));
    let whole = far - frame.eye.xyz;
    let ray = whole * share_in_air(whole, uv);
    var color = start;
    if density > 0.0 {
        color = level_fogged(color, ray, jitter);
    }
    if volumes {
        color = volumes_fogged(color, ray);
    }
    return color;
}

// How much of what's along `ray` gets through the level's fog, by its height.
fn level_through(ray: vec3<f32>) -> f32 {
    let distance_to = length(ray);
    let falloff = max(frame.fog_params.y, 0.0001);
    let start = frame.eye.z - frame.fog_params.x;
    let rise = ray.z * falloff;
    // The integral of density along the ray, for fog fading with height.
    var amount = exp(-start * falloff);
    if abs(rise) > 0.0001 {
        amount = amount * (1.0 - exp(-rise)) / rise;
    }
    return exp(-max(frame.fog_color.w * distance_to * amount, 0.0));
}

// The level's fog, along `ray`.
fn level_fogged(color: vec3<f32>, ray: vec3<f32>, jitter: f32) -> vec3<f32> {
    let density = frame.fog_color.w;
    if frame.probe_size.w > 0.5 {
        return grid_fogged(color, ray, jitter);
    }
    return mix(frame.fog_color.rgb, color, level_through(ray));
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

// Which way a pool's surface faces at `place`, with waves `height` tall. Sines at
// odd angles don't line up, so no pattern shows, and they're smooth, so nothing
// about them speckles.
fn water_normal(place: vec2<f32>, height: f32) -> vec3<f32> {
    let time = frame.params.x;
    var slope = vec2<f32>(0.0);
    let at = place * 0.03;
    slope = slope + vec2<f32>(0.8, 0.3) * cos(dot(at, vec2<f32>(0.8, 0.3)) * 1.0 + time * 0.9);
    slope = slope + vec2<f32>(-0.4, 0.9) * cos(dot(at, vec2<f32>(-0.4, 0.9)) * 2.3 + time * 1.3) * 0.6;
    slope = slope + vec2<f32>(0.6, -0.7) * cos(dot(at, vec2<f32>(0.6, -0.7)) * 5.1 + time * 1.9) * 0.3;
    return normalize(vec3<f32>(-slope * 0.04 * height, 1.0));
}

// What's seen through a surface at `uv`, bent by `normal`, or `color` if the place
// it lands on isn't under the surface, as that wouldn't be seen through it.
fn bent_through(
    color: vec3<f32>,
    uv: vec2<f32>,
    normal: vec3<f32>,
    amount: f32,
    top: f32,
) -> vec3<f32> {
    let size = vec2<f32>(textureDimensions(scene));
    let bent = clamp(uv + normal.xy * amount * 0.4, vec2<f32>(0.0), vec2<f32>(0.999));
    let at = vec2<i32>(bent * size);
    let depth = load_depth(at);
    if world_at(bent, min(depth, 0.9999)).z >= top {
        return color;
    }
    return load_color(at);
}

// How ripples slope at `place`, in the world.
fn ripple_slope(place: vec2<f32>) -> vec2<f32> {
    let width = frame.ripple_place.z;
    if width <= 0.0 {
        return vec2<f32>(0.0);
    }
    let cells = vec2<f32>(textureDimensions(ripples));
    let spot = (place - frame.ripple_place.xy) / width * cells;
    if any(spot < vec2<f32>(2.0)) || any(spot > cells - vec2<f32>(2.0)) {
        return vec2<f32>(0.0);
    }
    let at = vec2<i32>(spot);
    let across = textureLoad(ripples, at + vec2<i32>(1, 0), 0).x - textureLoad(ripples, at - vec2<i32>(1, 0), 0).x;
    let up = textureLoad(ripples, at + vec2<i32>(0, 1), 0).x - textureLoad(ripples, at - vec2<i32>(0, 1), 0).x;
    return vec2<f32>(across, up);
}

// What overlay `slot` shows at `place`, with its alpha how much of it to take.
fn overlay_at(slot: i32, place: vec2<f32>) -> vec4<f32> {
    let overlay = frame.pool_overlay[slot];
    if overlay.x < 0.5 {
        return vec4<f32>(0.0);
    }
    let size = max(floor(overlay.y), 1.0);
    let uv = (place - overlay.zw * frame.params.x) / size;
    let seen = textureSampleLevel(water_pictures, water_sampler, uv, i32(overlay.x) - 1, 0.0);
    return vec4<f32>(seen.rgb, seen.a * fract(overlay.y) / 0.999);
}

// Foam's shape at `place`, from a picture or a noise, from 0 to 1.
fn foam_at(slot: i32, place: vec2<f32>) -> f32 {
    if frame.pool_overlay[slot].x > 0.5 {
        let seen = overlay_at(slot, place);
        return seen.a;
    }
    let time = frame.params.x;
    let flow = vec3<f32>(place / 24.0, time * 0.3);
    return smoothstep(0.35, 0.7, fog_noise(flow) * 0.6 + fog_noise(flow * 2.7) * 0.4);
}

// Light in the pools a view passes through: depth tints it, what's in a pool
// shows caustics, and a view through one sees them as shafts.
fn in_pools(start: vec3<f32>, uv: vec2<f32>, depth: f32) -> vec3<f32> {
    var color = start;
    let eye = frame.eye.xyz;
    let far = world_at(uv, min(depth, 0.9999));
    let ray = far - eye;
    let count = i32(frame.pool_params.x);
    let reach = max(length(ray), 0.001);
    let surface = surface_at(uv);
    for (var index = 0; index < count; index = index + 1) {
        let low = frame.pool_low[index];
        let high = frame.pool_high[index];
        let surface_look = frame.pool_surface[index];
        let shaped = surface_look.w > 0.5;
        let on_it = shaped && abs(surface.y - f32(index + 1)) < 0.5;
        let eye_in = abs(frame.pool_params.w - f32(index + 1)) < 0.5;
        var enter = 0.0;
        var leave = 0.0;
        var top = high.z;
        if !shaped {
            let span = box_span(ray, low.xyz, high.xyz);
            enter = span.x;
            leave = span.y;
        } else if eye_in {
            // Out through the surface, if that comes first, and the far side of it isn't water.
            leave = 1.0;
            if on_it {
                leave = min(surface.x / reach, 1.0);
                top = (eye + ray * leave).z;
            }
        } else if on_it {
            enter = min(surface.x / reach, 1.0);
            leave = 1.0;
            top = (eye + ray * enter).z;
        }
        if leave <= enter {
            continue;
        }
        let dry = color;
        let path = (leave - enter) * reach;
        // A ray that stops on an opaque liquid's surface only touches its box,
        // and rounding says which side of it each pixel lands. Going by that
        // would flicker as the view moves, so a touch doesn't count.
        if path < 0.5 {
            continue;
        }
        // A view from above meets the surface first, which bends what's seen
        // through it, and mirrors what's over it.
        let entry = eye + ray * enter;
        let from_above = (eye.z > high.z && abs(entry.z - high.z) < 1.0) || (on_it && !eye_in);
        var normal = vec3<f32>(0.0, 0.0, 1.0);
        if from_above {
            normal = water_normal(entry.xy, surface_look.x);
            if on_it {
                normal = normalize(normal - vec3<f32>(surface.zw, 0.0));
            }
            normal = normalize(normal - vec3<f32>(ripple_slope(entry.xy) * 0.6, 0.0));
            color = bent_through(color, uv, normal, surface_look.y, top);
        }
        let inside = (all(far >= low.xyz - vec3<f32>(1.0)) && all(far <= high.xyz + vec3<f32>(1.0)))
            || on_it;
        if inside && depth < 1.0 && high.w > 0.0 {
            let below = top - far.z;
            let pattern = caustic(surface_above(far, top));
            let reach = exp(-below / 320.0);
            color = color * (1.0 + pattern * high.w * 3.0 * reach);
        }
        // Depth changes evenly along a ray, so its middle gives the whole of it exactly.
        let leaving = eye + ray * leave;
        let depth_in = max(top - (entry.z + leaving.z) * 0.5, 0.0);
        let thick = low.w * (1.0 + surface_look.z * depth_in);
        color = mix(color, frame.pool_color[index].rgb, 1.0 - exp(-path * thick));
        if high.w > 0.0 {
            var shafts = 0.0;
            for (var step = 0; step < 8; step = step + 1) {
                let share = mix(enter, leave, (f32(step) + 0.5) / 8.0);
                let point = eye + ray * share;
                let below = top - point.z;
                shafts = shafts + caustic(surface_above(point, top)) * exp(-below / 200.0);
            }
            let amount = shafts / 8.0 * min(path / 256.0, 1.0) * high.w;
            color = color + frame.pool_color[index].rgb * amount * 2.5;
        }
        if from_above {
            for (var slot = 0; slot < 2; slot = slot + 1) {
                let over = overlay_at(index * 3 + slot, entry.xy);
                color = mix(color, over.rgb, over.a);
            }
        }
        // What the surface mirrors isn't under it, so depth doesn't tint it.
        if from_above && frame.pool_color[index].w > 0.0 {
            color = reflected(color, entry, normal, frame.pool_color[index].w);
        }
        // The shore goes by how deep it is under the surface, here.
        let edge = frame.pool_edge[index];
        if from_above && depth < 1.0 && (edge.x > 0.0 || edge.y > 0.0) {
            let shallow = max(top - far.z, 0.0);
            if edge.x > 0.0 {
                let reach_of = 1.0 - smoothstep(0.0, edge.x, shallow);
                color = mix(color, vec3<f32>(1.0), reach_of * foam_at(index * 3 + 2, entry.xy));
            }
            if edge.y > 0.0 {
                color = mix(dry, color, smoothstep(0.0, edge.y, shallow));
            }
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

// How far light gets past one cell of a stencil.
fn stencil_cell_reach(index: i32, cell: vec2<i32>) -> f32 {
    let at = clamp(cell, vec2<i32>(0), vec2<i32>(15));
    let entry = at.y * 16 + at.x;
    return frame.stencil_reach[index * 64 + entry / 4][entry % 4];
}

// How far light gets past a stencil at `uv`. What's in the way is mostly flat, so that changes
// evenly across the stencil, and going smoothly between cells follows it closely.
fn stencil_reach_at(index: i32, uv: vec2<f32>) -> f32 {
    let at = uv * 16.0 - vec2<f32>(0.5);
    let cell = vec2<i32>(floor(at));
    let part = at - floor(at);
    let top = mix(
        stencil_cell_reach(index, cell),
        stencil_cell_reach(index, cell + vec2<i32>(1, 0)),
        part.x,
    );
    let bottom = mix(
        stencil_cell_reach(index, cell + vec2<i32>(0, 1)),
        stencil_cell_reach(index, cell + vec2<i32>(1, 1)),
        part.x,
    );
    return mix(top, bottom, part.y);
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

// How soft a beam's end is, in world units each way.
const STENCIL_STOP_SOFT: f32 = 6.0;

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
        // Whatever's in the way of the light stops it, and it's soft where it does.
        let gets_to = stencil_reach_at(index, uv);
        let stopped = 1.0 - smoothstep(gets_to - STENCIL_STOP_SOFT, gets_to + STENCIL_STOP_SOFT, distance_to);
        let fade = clamp(1.0 - distance_to / max(reach, 1.0), 0.0, 1.0) * stopped;
        // Soft at the rim, so the beam has no hard outline.
        let rim = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
        let soft = smoothstep(0.0, 0.06, rim);
        let texel = read_stencil(index, uv);
        total = total + texel.rgb * texel.a * fade * soft * stencil_open(index, uv);
    }
    let path = (span.y - span.x) * length(ray);
    return total * corner.w * path / (f32(steps) * 256.0);
}

// How high rays start, over the eye, where a sky has no layer of cloud with a height.
const SKY_RAY_TOP: f32 = 3000.0;

// How far along a view they're looked for.
const SKY_RAY_RANGE: f32 = 2400.0;

// How much a view has to look towards the sun for rays to show. Away from it, what's along a view
// is spread over a lot of sky, and it averages out to a haze, so there's nothing to work out.
const SKY_RAY_FACING: f32 = 0.3;

// How far through the air it takes for rays to be at their brightest.
const SKY_RAY_LENGTH: f32 = 6000.0;

// How tightly the sun's flare gathers round it.
const SKY_FLARE_GATHER: f32 = 40.0;

// How tightly rays gather round the sun.
const SKY_RAY_GATHER: f32 = 16.0;

// How little of the clouds' detail rays have, which are soft anyway.
const SKY_RAY_DETAIL: f32 = 0.34;

// How much of a layer of cloud's own picture is at `at`.
fn ray_cloud_picture(picture: i32, at: vec2<f32>) -> f32 {
    switch picture {
        case 0: { return textureSampleLevel(cloud_picture_0, water_sampler, at, 0.0).r; }
        case 1: { return textureSampleLevel(cloud_picture_1, water_sampler, at, 0.0).r; }
        case 2: { return textureSampleLevel(cloud_picture_2, water_sampler, at, 0.0).r; }
        default: { return textureSampleLevel(cloud_picture_3, water_sampler, at, 0.0).r; }
    }
}

// How much of the sun a layer of cloud lets down to `point`. It's the same cloud the sky shows
// there, as a layer with a height is fixed to the world, and one without goes with the eye.
fn cloud_lets_through(layer: i32, point: vec3<f32>, sun: vec3<f32>) -> f32 {
    let params = frame.sky_cloud_params[layer];
    if params.w < 0.5 {
        return 1.0;
    }
    let shape = frame.sky_cloud_shape[layer];
    var plane = vec2<f32>(0.0);
    if shape.x > 0.0 {
        let along = (shape.x - point.z) / sun.z;
        plane = (point.xy + sun.xy * along) / shape.x;
    } else {
        let along = (frame.eye.z + SKY_RAY_TOP - point.z) / sun.z;
        plane = (point.xy + sun.xy * along - frame.eye.xy) / SKY_RAY_TOP;
    }
    let at = plane * params.z + params.xy * frame.params.x;
    var density = 0.0;
    if params.w > 1.5 {
        density = ray_cloud_picture(i32(shape.w), at);
    } else {
        density = sky_fractal(at * 3.0, SKY_RAY_DETAIL);
    }
    return 1.0 - cloud_cover(density, frame.sky_cloud[layer].w);
}

// How much of the sun comes down to `point`, by the clouds or a picture.
fn sky_lets_through(point: vec3<f32>, sun: vec3<f32>, top: f32) -> f32 {
    if frame.sky_rays.y > 1.5 {
        let hit = point + sun * ((top - point.z) / sun.z);
        let look = frame.sky_ray_look;
        let at = hit.xy / look.x + look.yz * frame.params.x;
        return textureSampleLevel(sky_ray_picture, water_sampler, at, 0.0).r;
    }
    var open = 1.0;
    for (var layer = 0; layer < 4; layer = layer + 1) {
        open = open * cloud_lets_through(layer, point, sun);
    }
    return open;
}

// How low the sun gets in one cell.
fn sky_floor_cell(cell: vec2<i32>) -> f32 {
    let at = clamp(cell, vec2<i32>(0), vec2<i32>(15));
    let entry = at.y * 16 + at.x;
    return frame.sky_ray_floor[entry / 4][entry % 4];
}

// Whether the sun gets to `point`, from what's known near the view. Light comes along its
// direction, so it's the cell that line goes through.
fn sun_gets_here(point: vec3<f32>, sun: vec3<f32>) -> f32 {
    let area = frame.sky_ray_area;
    if area.z <= 0.0 {
        return 1.0;
    }
    let through = point.xy + sun.xy * ((area.w - point.z) / sun.z);
    let at = (through - area.xy) / area.z - vec2<f32>(0.5);
    let cell = vec2<i32>(floor(at));
    let part = at - floor(at);
    let top = mix(sky_floor_cell(cell), sky_floor_cell(cell + vec2<i32>(1, 0)), part.x);
    let bottom = mix(
        sky_floor_cell(cell + vec2<i32>(0, 1)),
        sky_floor_cell(cell + vec2<i32>(1, 1)),
        part.x,
    );
    return smoothstep(-STENCIL_STOP_SOFT, STENCIL_STOP_SOFT, point.z - mix(top, bottom, part.y));
}

// Light from the sky in the air. `whole` goes from the eye to what's seen, and rays stay in the
// sky unless they're told to come into the level.
fn sky_rays(whole: vec3<f32>, in_sky: bool, jitter: f32) -> vec3<f32> {
    let into_level = frame.sky_rays.z > 0.5;
    if frame.sky_rays.y < 0.5 || (!in_sky && !into_level) {
        return vec3<f32>(0.0);
    }
    let sun = normalize(frame.sky_sun.xyz);
    if sun.z < 0.05 {
        return vec3<f32>(0.0);
    }
    let facing = dot(normalize(whole), sun);
    if facing < SKY_RAY_FACING {
        return vec3<f32>(0.0);
    }
    let gathered = pow(facing * 0.5 + 0.5, SKY_RAY_GATHER);
    var top = frame.sky_rays.w;
    if top <= 0.0 {
        top = frame.eye.z + SKY_RAY_TOP;
    }
    var share = min(1.0, SKY_RAY_RANGE / max(length(whole), 1.0));
    if whole.z > 0.0 {
        share = min(share, max(top - frame.eye.z, 0.0) / whole.z);
    }
    let ray = whole * share;
    // Half of what a window's rays take, as there's a lot more sky than window.
    let steps = max(i32(frame.stencil_params.y) / 2, 4);
    var total = 0.0;
    for (var step = 0; step < steps; step = step + 1) {
        let point = frame.eye.xyz + ray * ((f32(step) + jitter) / f32(steps));
        var open = sky_lets_through(point, sun, top);
        if into_level {
            open = open * sun_gets_here(point, sun);
        }
        total = total + open;
    }
    let strength = frame.sky_rays.x * gathered * length(ray) / SKY_RAY_LENGTH;
    // Squared, so that gaps stand out from what's round them, or they'd all come to an even glow.
    let open = total / f32(steps);
    // Looking right at it, the sun flares, as far as what's in front of it lets it.
    let flare = frame.sky_ray_look.w * pow(facing * 0.5 + 0.5, SKY_FLARE_GATHER) * open;
    return frame.sky_sun_color.rgb * (open * open * strength + flare);
}

@fragment
fn fs_post(in: PostOut) -> @location(0) vec4<f32> {
    let at = vec2<i32>(in.uv * vec2<f32>(textureDimensions(scene)));
    let depth = load_depth(at);
    var color = load_color(at);
    if frame.post_params.x > 1.5 {
        color = smaa_blended(at);
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
        color = reflected(color, world_at(in.uv, depth), -normal, shine);
    }
    let edge = outline_strength(at, in.uv, depth);
    color = mix(color, frame.outline.rgb, edge);
    color = in_pools(color, in.uv, depth);
    color = fogged(color, in.uv, depth, noise);
    if frame.stencil_params.x > 0.5 || frame.fog_counts.w > 0.5 || frame.sky_rays.y > 0.5 {
        let air = textureSampleLevel(ray_light, scene_sampler, in.uv, 0.0);
        color = color * air.a + air.rgb;
    }
    return vec4<f32>(to_screen(shader_post(color, in.uv), noise), 1.0);
}

// Light in the air, for the final pass to add. It's soft, so a quarter of the
// pixels do, and a beam that fills the view would cost more than the scene
// otherwise.
@fragment
fn fs_rays(in: PostOut) -> @location(0) vec4<f32> {
    let at = vec2<i32>(in.uv * vec2<f32>(textureDimensions(scene)));
    let noise = fract(52.9829189 * fract(dot(in.clip.xy, vec2<f32>(0.06711056, 0.00583715))));
    let stop = world_at(in.uv, min(load_depth(at), 0.9999));
    var color = vec3<f32>(0.0);
    let count = i32(frame.stencil_params.x);
    for (var index = 0; index < count; index = index + 1) {
        color = color + stencil_rays(index, frame.eye.xyz, stop, noise);
    }
    let whole = stop - frame.eye.xyz;
    color = color + sky_rays(whole, load_depth(at) >= 1.0, noise);
    let ray = whole * share_in_air(whole, in.uv);
    let fog = volumes_marched(ray, noise);
    // Rays show up in what's in the air.
    var through = fog.w;
    if frame.fog_color.w > 0.0 {
        through = through * level_through(ray);
    }
    return vec4<f32>(fog.rgb + color * (3.0 - through * 2.0), fog.w);
}
