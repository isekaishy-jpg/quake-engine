// Where the surfaces of shaped bodies of water are, for the final pass (proposal 0147). This
// follows frame.wgsl in one module.

struct WaterDraw {
    // Which of the frame's bodies it is, plus one.
    body: f32,
    spare: vec3<f32>,
}

var<immediate> water_draw: WaterDraw;

@group(0) @binding(0) var<uniform> frame: Frame;

struct WaterOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
}

@vertex
fn vs_water(@location(0) place: vec3<f32>) -> WaterOut {
    var out: WaterOut;
    out.clip = frame.view_projection * vec4<f32>(place, 1.0);
    out.world = place;
    return out;
}

// How far it is, which body it's of, and how it slopes, which is what's over what's flat. What's
// in front of it is for the final pass to find, as that's how far it has to go anyway.
@fragment
fn fs_water(in: WaterOut) -> @location(0) vec4<f32> {
    var facing = normalize(cross(dpdx(in.world), dpdy(in.world)));
    if facing.z < 0.0 {
        facing = -facing;
    }
    let slope = facing.xy / max(facing.z, 0.05);
    return vec4<f32>(length(in.world - frame.eye.xyz), water_draw.body, slope);
}
