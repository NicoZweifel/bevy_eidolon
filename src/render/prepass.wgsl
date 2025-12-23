#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::prepass_bindings
#import bevy_eidolon::render::bindings::instance_uniforms
#import bevy_eidolon::render::utils
#import bevy_eidolon::render::io_types::Vertex

struct PrepassVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) previous_world_position: vec4<f32>,

#ifdef NORMAL_PREPASS
    @location(2) world_normal: vec3<f32>,
    #ifdef VERTEX_TANGENTS
    @location(3) world_tangent: vec4<f32>,
    #endif
#endif

#ifdef VISIBILITY_RANGE_DITHER
    @location(4) @interpolate(flat) visibility_range_dither: i32,
#endif
};

@vertex
fn vertex(vertex: Vertex) -> PrepassVertexOutput {
    var out: PrepassVertexOutput;

    let final_matrix = utils::calculate_instance_world_matrix(
        vertex.i_pos_scale,
        vertex.i_rotation,
        instance_uniforms.world_from_local
    );
    let world_position = final_matrix * vec4<f32>(vertex.position, 1.0);

    let prev_final_matrix = utils::calculate_instance_world_matrix(
        vertex.i_pos_scale,
        vertex.i_rotation,
        instance_uniforms.previous_world_from_local
    );
    let previous_world_position = prev_final_matrix * vec4<f32>(vertex.position, 1.0);

    out.world_position = world_position;
    out.previous_world_position = previous_world_position;

    out.clip_position = view.clip_from_world * world_position;

#ifdef NORMAL_PREPASS
    #ifdef VERTEX_NORMALS
        out.world_normal = normalize((final_matrix * vec4<f32>(vertex.normal, 0.0)).xyz);
    #else
        out.world_normal = vec3<f32>(0.0, 1.0, 0.0);
    #endif
    #ifdef VERTEX_TANGENTS
        out.world_tangent = vec4<f32>(
            normalize((final_matrix * vec4<f32>(vertex.tangent.xyz, 0.0)).xyz),
            vertex.tangent.w
        );
    #endif
#endif

#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = utils::get_visibility_range_dither_level(
        instance_uniforms.visibility_range,
        final_matrix[3]
    );
#endif

    return out;
}

#ifdef PREPASS_FRAGMENT
struct PrepassFragmentOutput {
#ifdef NORMAL_PREPASS
    @location(0) normal_depth: vec4<f32>,
#endif
#ifdef MOTION_VECTOR_PREPASS
    @location(1) motion_vector: vec2<f32>,
#endif
}

@fragment
fn fragment(
    in: PrepassVertexOutput,
    @builtin(front_facing) is_front: bool,
) -> PrepassFragmentOutput {
    #ifdef VISIBILITY_RANGE_DITHER
        bevy_pbr::pbr_functions::visibility_range_dither(
            in.clip_position,
            in.visibility_range_dither
        );
    #endif

    var out: PrepassFragmentOutput;

    #ifdef NORMAL_PREPASS
        var normal = normalize(in.world_normal);
        if !is_front {
            normal = -normal;
        }

        out.normal_depth = vec4<f32>(normal * 0.5 + 0.5, 1.0);
    #endif

    #ifdef MOTION_VECTOR_PREPASS
        let clip_position_t = view.unjittered_clip_from_world * in.world_position;
        let clip_position = clip_position_t.xy / clip_position_t.w;

        let previous_clip_position_t = prepass_bindings::previous_view_uniforms.clip_from_world * in.previous_world_position;
        let previous_clip_position = previous_clip_position_t.xy / previous_clip_position_t.w;

        out.motion_vector = (clip_position - previous_clip_position) * vec2(0.5, -0.5);
    #endif

    return out;
}

#else

@fragment
fn fragment(in: PrepassVertexOutput) {
    #ifdef VISIBILITY_RANGE_DITHER
        bevy_pbr::pbr_functions::visibility_range_dither(
            in.clip_position,
            in.visibility_range_dither
        );
    #endif
}

#endif // PREPASS_FRAGMENT
