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

// Where a view ray meets a layer of cloud, as a shell one unit up round a world. A big world makes
// it a flat plane, and one of no size makes it a dome.
fn cloud_plane(direction: vec3<f32>, curve: f32) -> vec2<f32> {
    let radius = 1.0 / max(curve, 0.0001) - 1.0;
    let along = radius * direction.z;
    return direction.xy * (2.0 * radius + 1.0) / (along + sqrt(along * along + 2.0 * radius + 1.0));
}

// How much of a layer's own picture is at `at`.
fn cloud_picture(picture: i32, at: vec2<f32>) -> f32 {
    switch picture {
        case 0: { return textureSampleLevel(surface, surface_sampler, at, 0.0).r; }
        case 1: { return textureSampleLevel(slot1, surface_sampler, at, 0.0).r; }
        case 2: { return textureSampleLevel(slot2, surface_sampler, at, 0.0).r; }
        default: { return textureSampleLevel(slot3, surface_sampler, at, 0.0).r; }
    }
}

// How much cloud a layer has along `direction`, once it's thinned out at the horizon.
fn cloud_amount(direction: vec3<f32>, layer: i32) -> f32 {
    let params = frame.sky_cloud_params[layer];
    let shape = frame.sky_cloud_shape[layer];
    var plane = cloud_plane(direction, shape.y);
    if shape.x > 0.0 {
        plane = plane + frame.eye.xy / shape.x;
    }
    let at = plane * params.z + params.xy * frame.params.x;
    var density = 0.0;
    if params.w > 1.5 {
        density = cloud_picture(i32(shape.w), at);
    } else {
        density = sky_fractal(at * 3.0, direction.z * 2.2);
    }
    let amount = cloud_cover(density, frame.sky_cloud[layer].w);
    return amount * clamp(direction.z * 6.0, 0.0, 1.0);
}

// A sky, with a layer of cloud over it.
fn with_clouds(color: vec3<f32>, direction: vec3<f32>, layer: i32, toward: f32) -> vec3<f32> {
    if frame.sky_cloud_params[layer].w < 0.5 {
        return color;
    }
    let amount = cloud_amount(direction, layer);
    let lit = frame.sky_cloud_shape[layer].z;
    let sun = frame.sky_sun_color.rgb;
    // Thick cloud lets less through, and thin cloud near the sun lights up.
    let thick = 1.0 - lit * 0.5 * amount * amount;
    let rim = sun * lit * (1.0 - amount) * pow(max(toward, 0.0), 8.0);
    let shade = frame.sky_cloud[layer].rgb * mix(vec3<f32>(1.0), sun, lit) * thick + rim;
    return mix(color, shade, amount);
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
    // The moon is on the far side of the sky.
    let moon_toward = -toward;
    let moon_size = frame.sky_moon.w;
    let moon = smoothstep(1.0 - moon_size * 1.2, 1.0 - moon_size, moon_toward);
    let moon_glow = pow(max(moon_toward, 0.0), 256.0) * 0.2;
    color = color + frame.sky_moon_color.rgb * (moon + moon_glow);
    let size = frame.sky_sun.w;
    let disc = smoothstep(1.0 - size * 1.2, 1.0 - size, toward);
    let glow = pow(max(toward, 0.0), 64.0) * 0.35;
    color = color + frame.sky_sun_color.rgb * (disc + glow);

    if height > 0.0 {
        // The highest first, so what's lower is in front of it.
        for (var layer = 3; layer >= 0; layer = layer - 1) {
            color = with_clouds(color, direction, layer, toward);
        }
    }
    return color;
}

@fragment
fn fs_sky_sphere(in: SkyOut) -> @location(0) vec4<f32> {
    return vec4<f32>(scene_color(to_linear(sky_color(in.direction))), 1.0);
}

// A face that's a window onto the sky.
@fragment
fn fs_sky_window(in: VertexOut) -> @location(0) vec4<f32> {
    return vec4<f32>(scene_color(to_linear(sky_color(in.world - frame.eye.xyz))), 1.0);
}
