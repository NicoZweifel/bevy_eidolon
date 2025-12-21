#import bevy_pbr::mesh_view_bindings::view

#import bevy_eidolon::render::bindings::{material, instance_uniforms}
#import bevy_eidolon::render::utils
#import bevy_eidolon::render::io_types::{VertexOutput, Vertex}


@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    var scale = vertex.i_pos_scale.w;
    var translation = vertex.i_pos_scale.xyz;

    let final_matrix = utils::calculate_instance_world_matrix(vertex.i_pos_scale, vertex.i_rotation, instance_uniforms.world_from_local);

    let world_position = final_matrix * vec4<f32>(vertex.position, 1.0);

    out.world_position = world_position;
    out.clip_position = view.clip_from_world * world_position;

#ifdef VERTEX_NORMALS
    out.world_normal = normalize(final_matrix * vec4<f32>(vertex.normal, 0.0));
#else
    out.world_normal = vec3<f32>(0.0, 1.0, 0.0);
#endif

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#else
    out.uv = vec2<f32>(0.0);
#endif

#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = utils::get_visibility_range_dither_level(
        instance_uniforms.visibility_range,
        final_matrix[3]
    );
#endif

    return out;
}



