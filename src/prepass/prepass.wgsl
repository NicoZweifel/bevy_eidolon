#import bevy_pbr::prepass_bindings
#import bevy_pbr::pbr_types::{PbrInput, pbr_input_new}
#import bevy_pbr::pbr_functions
#import bevy_pbr::mesh_view_bindings::view
#import bevy_render::view::View
#import bevy_render::{
    globals::Globals,
}

#import bevy_eidolon::render::bindings::instance_uniforms
#import bevy_eidolon::render::utils
#import bevy_eidolon::render::io_types::Vertex

#ifdef DEFERRED_PREPASS
    #import bevy_pbr::pbr_deferred_functions::deferred_gbuffer_from_pbr_input
#endif

#ifdef MOTION_VECTOR_PREPASS
    #import bevy_pbr::pbr_prepass_functions::calculate_motion_vector
#endif

@group(0) @binding(1) var<uniform> globals: Globals;

struct PrepassVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) previous_world_position: vec4<f32>,

#ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
    @location(2) world_normal: vec3<f32>,
    #ifdef VERTEX_TANGENTS
    @location(3) world_tangent: vec4<f32>,
    #endif
#endif

#ifdef VISIBILITY_RANGE_DITHER
    @location(4) @interpolate(flat) visibility_range_dither: i32,
#endif

#ifdef DEFERRED_PREPASS
    @location(5) @interpolate(flat) i_batch_id: u32,
    #ifdef VERTEX_UVS_A
        @location(6) uv: vec2<f32>,
    #endif
#endif
};

@vertex
fn vertex(vertex: Vertex) -> PrepassVertexOutput {
    var out: PrepassVertexOutput;
    let batch = instance_uniforms[vertex.i_batch_id];

    let final_matrix = utils::calc_instance_world_matrix(
        vertex.i_pos_scale,
        vertex.i_rotation,
        batch.world_from_local
    );
    let world_position = final_matrix * vec4<f32>(vertex.position, 1.0);

    out.world_position = world_position;

#ifdef MOTION_VECTOR_PREPASS
    let prev_final_matrix = utils::calc_instance_world_matrix(
        vertex.i_pos_scale,
        vertex.i_rotation,
        batch.previous_world_from_local
    );
    let previous_world_position = prev_final_matrix * vec4<f32>(vertex.position, 1.0);
    out.previous_world_position = previous_world_position;
#endif

    out.clip_position = view.clip_from_world * world_position;

#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.unclipped_depth = out.clip_position.z / out.clip_position.w;
#endif

#ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
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
        batch.visibility_range,
        final_matrix[3]
    );
#endif

#ifdef DEFERRED_PREPASS
    out.i_batch_id = vertex.i_batch_id;
    #ifdef VERTEX_UVS_A
        out.uv = vertex.uv;
    #endif
#endif

    return out;
}

#ifdef PREPASS_FRAGMENT

#import bevy_pbr::prepass_io::FragmentOutput

@fragment
fn fragment(in: PrepassVertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

#ifdef VISIBILITY_RANGE_DITHER
    bevy_pbr::pbr_functions::visibility_range_dither(
        in.clip_position,
        in.visibility_range_dither
    );
#endif

#ifdef NORMAL_PREPASS
    out.normal = vec4(in.world_normal * 0.5 + vec3(0.5), 1.0);
#endif

#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.frag_depth = in.unclipped_depth;
#endif

#ifdef MOTION_VECTOR_PREPASS
    out.motion_vector = calculate_motion_vector(in.world_position, in.previous_world_position);
#endif

#ifdef DEFERRED_PREPASS
    let batch = instance_uniforms[in.i_batch_id];

    var pbr_input = pbr_input_new();
    pbr_input.material.base_color = batch.color;
    pbr_input.material.metallic = 0.0;
    pbr_input.material.perceptual_roughness = 0.5;
    pbr_input.world_normal = normalize(in.world_normal);
    pbr_input.world_position = in.world_position;
    pbr_input.N = normalize(in.world_normal);
    pbr_input.frag_coord = in.clip_position;

    out.deferred = deferred_gbuffer_from_pbr_input(pbr_input);

    out.deferred_lighting_pass_id = 1u;
#endif

    return out;
}
#endif
