// What happens to a finished scene: edge smoothing, outlines and fog
// (proposal 0075). This follows frame.wgsl in one module.

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var scene: texture_2d<f32>;
@group(0) @binding(2) var scene_depth: texture_depth_2d;
@group(0) @binding(3) var scene_sampler: sampler;

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

// Fog that thickens with distance and lies low.
fn fogged(color: vec3<f32>, uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let density = frame.fog_color.w;
    if density <= 0.0 {
        return color;
    }
    let far = world_at(uv, min(depth, 0.99999));
    let ray = far - frame.eye.xyz;
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

@fragment
fn fs_post(in: PostOut) -> @location(0) vec4<f32> {
    let at = vec2<i32>(in.clip.xy);
    let depth = load_depth(at);
    var color = load_color(at);
    if frame.post_params.x > 0.5 {
        color = smoothed(at, in.uv);
    }
    let edge = outline_strength(at, in.uv, depth);
    color = mix(color, frame.outline.rgb, edge);
    color = fogged(color, in.uv, depth);
    return vec4<f32>(color, 1.0);
}
