#define_import_path bevy_eidolon::cull::types

struct InstanceData {
    pos_and_scale: vec4<f32>,
    rotation: f32,
    index: u32,
    batch_id: u32,
    seed: u32,
}

struct DrawIndexedIndirectArgs {
    index_count: u32,
    instance_count: atomic<u32>,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
}

struct BatchMetadata {
    batch_id: u32,
    start_index: u32,
    end_index: u32,
}

struct CameraCullData {
    view_pos: vec4<f32>,
    frustum: array<vec4<f32>, 6>,
}

struct InstanceUniforms {
    color: vec4<f32>,
    visibility_range: vec4<f32>,
    world_from_local: mat4x4<f32>,
    previous_world_from_local: mat4x4<f32>,
    aabb_center: vec4<f32>,
    aabb_half_extents: vec4<f32>,
}
