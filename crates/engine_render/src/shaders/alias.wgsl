// Alias models: two poses blended, lit per vertex (proposal 0033).

struct Frame {
    view_projection: mat4x4<f32>,
    eye: vec4<f32>,
    params: vec4<f32>,
    styles: array<vec4<f32>, 16>,
}

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

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let texel = textureSample(skin, skin_sampler, in.uv);
    let color = mix(texel.rgb, texel.rgb * in.light, texel.a);
    return vec4<f32>(pow(max(color, vec3<f32>(0.0)), vec3<f32>(frame.params.w)), 1.0);
}
