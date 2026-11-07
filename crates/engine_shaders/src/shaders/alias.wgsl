// The fixture's models. Their corners are a mesh's by now, moved by the compute pass like any
// other (proposal 0142), and all that's left here is their look: lit per corner by Quake's rule,
// with what's fullbright in the skin's alpha (proposal 0033). This follows frame.wgsl in one
// module.

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var skin: texture_2d<f32>;
@group(1) @binding(1) var skin_sampler: sampler;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(5) normal: vec4<f32>,
    @location(6) row_x: vec4<f32>,
    @location(7) row_y: vec4<f32>,
    @location(8) row_z: vec4<f32>,
    // x is ambient light, y directional, and z above 0 keeps it in front.
    @location(9) light: vec3<f32>,
}

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) light: f32,
}

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    let corner = vec4<f32>(in.position, 1.0);
    let world = vec4<f32>(dot(in.row_x, corner), dot(in.row_y, corner), dot(in.row_z, corner), 1.0);

    var out: VertexOut;
    out.clip = frame.view_projection * world;
    if in.light.z > 0.0 {
        out.clip.z = out.clip.z * 0.3;
    }
    out.uv = in.uv;
    // Light comes along -x, as in R_DrawEntitiesOnList.
    let turned = dot(in.row_x.xyz, in.normal.xyz);
    out.light = in.light.x + in.light.y * max(turned, 0.0);
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let texel = textureSample(skin, skin_sampler, in.uv);
    let light = pow(max(in.light, 0.0), frame.color_params.x);
    return vec4<f32>(mix(texel.rgb, texel.rgb * light, texel.a), 1.0);
}
