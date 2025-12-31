#define_import_path bevy_eidolon::render::types

struct MaterialUniforms {
    debug_color: vec4<f32>
};

struct InstanceUniforms {
    color: vec4<f32>,
    visibility_range: vec4<f32>,
    world_from_local: mat4x4<f32>,
    previous_world_from_local: mat4x4<f32>,
};

