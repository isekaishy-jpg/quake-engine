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

// What's told of a sheet of land, as world.wgsl has it.
struct LandSheet {
    origin: vec4<f32>,
    shape: vec4<u32>,
    uv: vec4<f32>,
    paint: vec4<u32>,
}

@group(0) @binding(0) var<storage, read> land_sheets: array<LandSheet>;
@group(0) @binding(1) var<storage, read> land_heights: array<f32>;
@group(0) @binding(3) var<storage, read> land_holes: array<u32>;

struct LandOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) place: vec2<f32>,
    @location(1) @interpolate(flat) sheet: u32,
}

// Land, at one level, as light doesn't care how far off the view is.
@vertex
fn vs_land(
    @builtin(vertex_index) vertex: u32,
    @location(0) node: vec4<u32>,
    @location(3) sheet_at: vec4<u32>,
) -> LandOut {
    let sheet = land_sheets[sheet_at.x];
    let side = node.w + 1u;
    let at = node.xy + vec2<u32>(vertex % side, vertex / side) * node.z;
    let height = land_heights[sheet.shape.y + at.y * sheet.shape.x + at.x];
    let place = vec2<f32>(at) * sheet.origin.w + sheet.origin.xy;
    var out: LandOut;
    out.clip = tile.matrix * vec4<f32>(place, height + sheet.origin.z, 1.0);
    out.place = place;
    out.sheet = sheet_at.x;
    return out;
}

// Light goes through a hole in the land.
@fragment
fn fs_land(in: LandOut) {
    let sheet = land_sheets[in.sheet];
    if sheet.shape.w == 0u {
        return;
    }
    let cell = vec2<u32>((in.place - sheet.origin.xy) / sheet.origin.w);
    let at = cell.y * (sheet.shape.x - 1u) + cell.x;
    if (land_holes[sheet.shape.w - 1u + at / 32u] >> (at % 32u) & 1u) != 0u {
        discard;
    }
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
