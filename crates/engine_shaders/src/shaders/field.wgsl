// A field: a picture worked out a step at a time, each cell from what the last step left
// (proposal 0145). Field code joins this with `shader_field`.

struct FieldStep {
    // Seconds since the level began.
    time: f32,
    // How wide and tall it is.
    width: u32,
    height: u32,
    spare: u32,
    // The numbers the level gave it.
    params: array<vec4<f32>, 2>,
}

var<immediate> field_step: FieldStep;

@group(0) @binding(0) var field_last: texture_2d<f32>;
@group(0) @binding(1) var field_next: texture_storage_2d<rgba16float, write>;

// One of the eight numbers the level gave this field.
fn shader_param(index: i32) -> f32 {
    return field_step.params[index / 4][index % 4];
}

// The seconds since the level began.
fn shader_time() -> f32 {
    return field_step.time;
}

// What a cell was after the last step. Past the edge, it's what's at the edge.
fn field_read(at: vec2<i32>) -> vec4<f32> {
    let last = vec2<i32>(i32(field_step.width) - 1, i32(field_step.height) - 1);
    return textureLoad(field_last, clamp(at, vec2<i32>(0), last), 0);
}

@compute @workgroup_size(8, 8, 1)
fn cs_field(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= field_step.width || id.y >= field_step.height {
        return;
    }
    let size = vec2<i32>(i32(field_step.width), i32(field_step.height));
    let at = vec2<i32>(id.xy);
    textureStore(field_next, at, shader_field(at, size));
}

// What's been worked out goes to where it's read. A surface reads one image, so its set of
// bindings doesn't have to change with each step.
@compute @workgroup_size(8, 8, 1)
fn cs_field_keep(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= field_step.width || id.y >= field_step.height {
        return;
    }
    let at = vec2<i32>(id.xy);
    textureStore(field_next, at, textureLoad(field_last, at, 0));
}
