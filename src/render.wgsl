#import bevy_pbr::mesh_view_bindings::{view, lights, globals, clusterable_objects}
#import bevy_pbr::shadows::fetch_directional_shadow
#import bevy_pbr::shadows::fetch_point_shadow
#import bevy_pbr::mesh_view_types::POINT_LIGHT_FLAGS_SHADOWS_ENABLED_BIT
#import bevy_pbr::clustered_forward::{fragment_cluster_index, unpack_clusterable_object_index_ranges, get_clusterable_object_id}

#import bevy_pbr::mesh_functions::mesh_normal_local_to_world
#import bevy_pbr::utils::rand_f
#import bevy_pbr::mesh_bindings::mesh

struct MaterialUniforms {
    debug_color: vec4<f32>
};

struct InstanceUniforms {
    color: vec4<f32>,
    visibility_range: vec4<f32>,
    world_from_local: mat4x4<f32>,
};

@group(3) @binding(0) var<uniform> material: MaterialUniforms;
@group(3) @binding(100) var<uniform> instance_uniforms: InstanceUniforms;

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
    @location(0) ao: f32,

#ifdef VISIBILITY_RANGE_DITHER
    @location(1) @interpolate(flat) visibility_range_dither: i32,
#endif

    @location(2) world_position: vec3<f32>,
    @location(3) world_normal: vec3<f32>,
    @location(4) uv: vec2<f32>,
    @location(5) local_pos: vec3<f32>,
    @location(6) world_tangent: vec4<f32>,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    var scale = vertex.i_pos_scale.w;
    var translation = vertex.i_pos_scale.xyz;

    let final_matrix = calculate_instance_world_matrix(vertex.i_pos_scale, vertex.i_rotation, instance_uniforms.world_from_local);

    let world_position = final_matrix * vec4<f32>(vertex.position, 1.0);

    out.world_position = world_position.xyz;
    out.clip_position = view.clip_from_world * world_position;

#ifdef VERTEX_NORMALS
    out.world_normal = normalize((final_matrix * vec4<f32>(vertex.normal, 0.0)).xyz);
#else
    out.world_normal = vec3<f32>(0.0, 1.0, 0.0);
#endif

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#else
    out.uv = vec2<f32>(0.0);
#endif

#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = get_visibility_range_dither_level(
        instance_uniforms.visibility_range,
        final_matrix[3]
    );
#endif

    return out;
}

fn calculate_instance_world_matrix(
    i_pos_scale: vec4<f32>,
    i_rotation: f32,
    parent_transform: mat4x4<f32>
) -> mat4x4<f32> {
    let scale = i_pos_scale.w;
    let translation = i_pos_scale.xyz;

    let c = cos(i_rotation) * scale;
    let s = sin(i_rotation) * scale;

    let instance_local = mat4x4<f32>(
        vec4<f32>(c, 0.0, s, 0.0),
        vec4<f32>(0.0, scale, 0.0, 0.0),
        vec4<f32>(-s, 0.0, c, 0.0),
        vec4<f32>(translation, 1.0)
    );

    return parent_transform * instance_local;
}

#ifdef VISIBILITY_RANGE_DITHER
// taken/adapted from https://github.com/bevyengine/bevy/blob/main/crates/bevy_pbr/src/render/mesh_functions.wgsl
fn get_visibility_range_dither_level(lod_range: vec4<f32>, world_position: vec4<f32>) -> i32 {
    let camera_distance = length(view.world_position.xyz - world_position.xyz);

    // This encodes the following mapping:
    //
    //     `lod_range.`          x        y        z        w           camera distance
    //                   ←───────┼────────┼────────┼────────┼────────→
    //     Dither Level  -16    -16       0        0        16      16  Dither Level

    let offset = select(-16, 0, camera_distance >= lod_range.z);
    let bounds = select(lod_range.xy, lod_range.zw, camera_distance >= lod_range.z);
    let level = i32(round((camera_distance - bounds.x) / (bounds.y - bounds.x) * 16.));
    return offset + clamp(level, 0, 16);
}
#endif

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> @location(0) vec4<f32> {

#ifdef MATERIAL_DEBUG
    final_color = material.debug_color;
#endif

#ifdef VISIBILITY_RANGE_DITHER
    bevy_pbr::pbr_functions::visibility_range_dither(in.clip_position, in.visibility_range_dither);
#endif

    return instance_uniforms.color;
}