#define_import_path bevy_eidolon::render::io_types

struct Vertex {
    @location(0) position: vec3<f32>,

#ifdef VERTEX_NORMALS
    @location(1) normal: vec3<f32>,
#endif
#ifdef VERTEX_UVS_A
    @location(2) uv: vec2<f32>,
#endif
#ifdef VERTEX_UVS_B
    @location(3) uv_b: vec2<f32>,
#endif
#ifdef VERTEX_TANGENTS
    @location(4) tangent: vec4<f32>,
#endif
#ifdef VERTEX_COLORS
    @location(5) color: vec4<f32>,
#endif

// TODO
#ifdef SKINNED
    @location(6) joint_indices: vec4<u32>,
    @location(7) joint_weights: vec4<f32>,
#endif

    @location(8) i_pos_scale: vec4<f32>,
    @location(9) i_rotation: f32,
    @location(10) i_index: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,

#ifdef VISIBILITY_RANGE_DITHER
    @location(0) @interpolate(flat) visibility_range_dither: i32,
#endif

    @location(1) world_position: vec4<f32>,
    @location(2) world_normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) world_tangent: vec4<f32>,
    @location(5) local_pos: vec3<f32>,
};
