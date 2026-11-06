// Alias models: two poses blended, lit per vertex (proposal 0033). This follows
// frame.wgsl in one module.

struct Draw {
    model: mat4x4<f32>,
    // w is how far to blend from the first pose to the second.
    scale: vec4<f32>,
    offset: vec4<f32>,
    // x is ambient light, y directional, and z above 0 keeps it in front.
    light: vec4<f32>,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var skin: texture_2d<f32>;
@group(1) @binding(1) var skin_sampler: sampler;

var<immediate> draw: Draw;

struct VertexIn {
    @location(0) position_a: vec4<u32>,
    @location(1) normal_a: vec4<f32>,
    @location(2) position_b: vec4<u32>,
    @location(3) normal_b: vec4<f32>,
    @location(4) uv: vec2<f32>,
}

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) light: f32,
}

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    let blend = draw.scale.w;
    let pose = mix(vec3<f32>(in.position_a.xyz), vec3<f32>(in.position_b.xyz), blend);
    let local = pose * draw.scale.xyz + draw.offset.xyz;
    let normal = mix(in.normal_a.xyz, in.normal_b.xyz, blend);
    let turned = (draw.model * vec4<f32>(normal, 0.0)).xyz;

    var out: VertexOut;
    out.clip = frame.view_projection * draw.model * vec4<f32>(local, 1.0);
    if draw.light.z > 0.0 {
        out.clip.z = out.clip.z * 0.3;
    }
    out.uv = in.uv;
    // Light comes along -x, as in R_DrawEntitiesOnList.
    out.light = draw.light.x + draw.light.y * max(turned.x, 0.0);
    return out;
}

// The same model pushed out along its normals, with its front faces culled:
// what shows round its edge is an outline (proposal 0081).
@vertex
fn vs_hull(in: VertexIn) -> VertexOut {
    let blend = draw.scale.w;
    let pose = mix(vec3<f32>(in.position_a.xyz), vec3<f32>(in.position_b.xyz), blend);
    let local = pose * draw.scale.xyz + draw.offset.xyz;
    let normal = mix(in.normal_a.xyz, in.normal_b.xyz, blend);
    let world = draw.model * vec4<f32>(local, 1.0);
    // Wider with distance, so a line keeps its width on screen.
    let away = length(world.xyz - frame.eye.xyz);
    let width = max(frame.post_params.y, 1.0) * away * 0.0016;
    let pushed = world.xyz + normalize((draw.model * vec4<f32>(normal, 0.0)).xyz) * width;

    var out: VertexOut;
    out.clip = frame.view_projection * vec4<f32>(pushed, 1.0);
    out.uv = in.uv;
    out.light = 1.0;
    return out;
}

@fragment
fn fs_hull(in: VertexOut) -> @location(0) vec4<f32> {
    return vec4<f32>(frame.outline.rgb, 1.0);
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let texel = textureSample(skin, skin_sampler, in.uv);
    let light = pow(max(in.light, 0.0), frame.color_params.x);
    return vec4<f32>(mix(texel.rgb, texel.rgb * light, texel.a), 1.0);
}
