// Moving meshes: each corner goes where its joints take it, into plain vertices, so that what
// draws them needs to know nothing about skeletons (proposal 0140).

struct Job {
    // Where this mesh's corners start, and how many it has.
    first_in: u32,
    count: u32,
    // Where the moved corners go.
    first_out: u32,
    // Where this mesh's joints start.
    first_joint: u32,
}

// A vertex is ten words, as the world's: three of place, two of uv, two of light place, one of
// light step, one of styles, and one of normal.
const VERTEX_WORDS: u32 = 10u;

@group(0) @binding(0) var<storage, read> source: array<u32>;
// Each corner's four joints as bytes, then its four weights as bytes.
@group(0) @binding(1) var<storage, read> skin: array<vec2<u32>>;
// Three rows to each joint.
@group(0) @binding(2) var<storage, read> palette: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> moved: array<u32>;

var<immediate> job: Job;

@compute @workgroup_size(64)
fn cs_deform(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= job.count {
        return;
    }
    let corner = job.first_in + id.x;
    let from_word = corner * VERTEX_WORDS;
    let to_word = (job.first_out + id.x) * VERTEX_WORDS;
    let place = vec4<f32>(
        bitcast<f32>(source[from_word]),
        bitcast<f32>(source[from_word + 1u]),
        bitcast<f32>(source[from_word + 2u]),
        1.0,
    );
    let packed = source[from_word + 9u];
    let normal = unpack4x8snorm(packed);
    let carried = skin[corner];
    let weights = unpack4x8unorm(carried.y);

    // Linear blending, as glTF defines skinning: a weighted sum of the joints' matrices.
    var row_x = vec4<f32>(0.0);
    var row_y = vec4<f32>(0.0);
    var row_z = vec4<f32>(0.0);
    for (var at = 0u; at < 4u; at += 1u) {
        let joint = (carried.x >> (at * 8u)) & 255u;
        let first = (job.first_joint + joint) * 3u;
        let weight = weights[at];
        row_x += palette[first] * weight;
        row_y += palette[first + 1u] * weight;
        row_z += palette[first + 2u] * weight;
    }

    let turned = vec3<f32>(
        dot(row_x.xyz, normal.xyz),
        dot(row_y.xyz, normal.xyz),
        dot(row_z.xyz, normal.xyz),
    );
    let facing = turned / max(length(turned), 0.000001);
    moved[to_word] = bitcast<u32>(dot(row_x, place));
    moved[to_word + 1u] = bitcast<u32>(dot(row_y, place));
    moved[to_word + 2u] = bitcast<u32>(dot(row_z, place));
    for (var word = 3u; word < 9u; word += 1u) {
        moved[to_word + word] = source[from_word + word];
    }
    moved[to_word + 9u] = pack4x8snorm(vec4<f32>(facing, normal.w));
}
