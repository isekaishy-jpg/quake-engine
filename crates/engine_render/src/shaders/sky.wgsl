// The sky sphere: a gradient to the horizon, a sun, and a cloud layer
// (proposal 0071). It follows world.wgsl in one module.

struct SkyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) direction: vec3<f32>,
}

// A cube around the eye, built from the vertex number, so no buffer is needed.
fn cube_corner(index: u32) -> vec3<f32> {
    let face = index / 6u;
    let corner = index % 6u;
    var uv = vec2<f32>(-1.0, -1.0);
    if corner == 1u {
        uv = vec2<f32>(1.0, -1.0);
    } else if corner == 2u || corner == 3u {
        uv = vec2<f32>(1.0, 1.0);
    } else if corner == 4u {
        uv = vec2<f32>(-1.0, 1.0);
    }
    let axis = face / 2u;
    let side = f32(face % 2u) * 2.0 - 1.0;
    if axis == 0u {
        return vec3<f32>(side, uv.x, uv.y);
    }
    if axis == 1u {
        return vec3<f32>(uv.x, side, uv.y);
    }
    return vec3<f32>(uv.x, uv.y, side);
}

@vertex
fn vs_sky_sphere(@builtin(vertex_index) index: u32) -> SkyOut {
    let direction = cube_corner(index);
    var out: SkyOut;
    let clip = frame.view_projection * vec4<f32>(frame.eye.xyz + direction * 64.0, 1.0);
    // At the far plane, so it's behind everything at any distance.
    out.clip = vec4<f32>(clip.xy, clip.w * 0.99999, clip.w);
    out.direction = direction;
    return out;
}

// A number from 0 to 1 for a cell, from its integer position. `sin` of a big
// number comes out differently on every device, and in blocks.
fn sky_hash(cell: vec2<f32>) -> f32 {
    let at = vec2<u32>(vec2<i32>(cell) + vec2<i32>(32768));
    var value = at.x * 1597334677u ^ at.y * 3812015801u;
    value = (value ^ (value >> 16u)) * 2246822519u;
    value = (value ^ (value >> 13u)) * 3266489917u;
    value = value ^ (value >> 16u);
    return f32(value >> 8u) / 16777216.0;
}

fn sky_noise(at: vec2<f32>) -> f32 {
    let cell = floor(at);
    let part = fract(at);
    let blend = part * part * (vec2<f32>(3.0) - 2.0 * part);
    let a = sky_hash(cell);
    let b = sky_hash(cell + vec2<f32>(1.0, 0.0));
    let c = sky_hash(cell + vec2<f32>(0.0, 1.0));
    let d = sky_hash(cell + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, blend.x), mix(c, d, blend.x), blend.y);
}

// Layers of noise, each turned so their grids don't line up. `detail` runs from
// 0 to 1, and low down the finest go, as they're too small there to be seen
// except as streaks. What they'd have added stays, so cover doesn't change.
fn sky_fractal(at_start: vec2<f32>, detail: f32) -> f32 {
    var at = at_start;
    var total = 0.0;
    var weight = 0.5;
    for (var index = 0; index < 6; index = index + 1) {
        let keep = clamp(detail * 6.0 - f32(index) + 1.0, 0.0, 1.0);
        total = total + mix(0.5, sky_noise(at), keep) * weight;
        at = vec2<f32>(at.x * 1.6 - at.y * 1.2, at.x * 1.2 + at.y * 1.6) + vec2<f32>(17.3, 9.1);
        weight = weight * 0.5;
    }
    return total;
}

fn sky_color(direction_in: vec3<f32>) -> vec3<f32> {
    let direction = normalize(direction_in);
    let height = direction.z;
    let band = frame.sky_zenith.w;
    var color = mix(
        frame.sky_horizon.rgb,
        frame.sky_zenith.rgb,
        pow(clamp(height / max(band, 0.01), 0.0, 1.0), 0.6),
    );
    if height < 0.0 {
        color = mix(frame.sky_horizon.rgb, frame.sky_ground.rgb, clamp(-height * 8.0, 0.0, 1.0));
    }
    let toward = dot(direction, frame.sky_sun.xyz);
    let size = frame.sky_sun.w;
    let disc = smoothstep(1.0 - size * 1.2, 1.0 - size, toward);
    let glow = pow(max(toward, 0.0), 64.0) * 0.35;
    color = color + frame.sky_sun_color.rgb * (disc + glow);

    let mode = frame.sky_cloud_params.w;
    if mode > 0.5 && height > 0.0 {
        // A flat layer overhead, seen in perspective, thinning at the horizon.
        let plane = direction.xy / (height + 0.12);
        let drift = frame.sky_cloud_params.xy * frame.params.x;
        let at = plane * frame.sky_cloud_params.z + drift;
        var density = 0.0;
        if mode > 1.5 {
            density = textureSampleLevel(surface, surface_sampler, at, 0.0).r;
        } else {
            density = sky_fractal(at * 3.0, height * 2.2);
        }
        let cover = frame.sky_cloud.w;
        let amount = smoothstep(0.92 - cover * 0.84, 1.22 - cover * 0.84, density);
        let fade = clamp(height * 6.0, 0.0, 1.0);
        color = mix(color, frame.sky_cloud.rgb, amount * fade);
    }
    return color;
}

@fragment
fn fs_sky_sphere(in: SkyOut) -> @location(0) vec4<f32> {
    return vec4<f32>(raised(sky_color(in.direction)), 1.0);
}

// A face that's a window onto the sky.
@fragment
fn fs_sky_window(in: VertexOut) -> @location(0) vec4<f32> {
    return vec4<f32>(raised(sky_color(in.world - frame.eye.xyz)), 1.0);
}
