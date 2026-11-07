// Depth for shadow maps (proposal 0084). Nothing here has a colour.

struct Tile {
    matrix: mat4x4<f32>,
    // Where the model being drawn has moved to.
    offset: vec4<f32>,
}

var<immediate> tile: Tile;

@vertex
fn vs_shadow(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return tile.matrix * vec4<f32>(position + tile.offset.xyz, 1.0);
}

// A triangle over a whole tile, at the far end of depth.
@vertex
fn vs_clear(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - vec2<f32>(1.0), 1.0, 1.0);
}

// Depth needs nothing from here, but a pipeline has to have one.
@fragment
fn fs_shadow() {
}
