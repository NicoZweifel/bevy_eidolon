#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::mesh_bindings::mesh
#import bevy_pbr::mesh_view_bindings::globals

#import bevy_eidolon::render::utils
#import bevy_eidolon::render::bindings::instance_uniforms
#import bevy_eidolon::render::io_types::{VertexOutput, Vertex}

struct CustomMaterialUniform {
    color: vec4<f32>,
    speed: f32,
    amplitude: f32,
    frequency: f32
};

@group(3) @binding(0) var<uniform> material: CustomMaterialUniform;
@group(3) @binding(1) var base_color_texture: texture_2d<f32>;
@group(3) @binding(2) var base_color_sampler: sampler;

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    var local_position = vertex.position;

    local_position.y += sin((vertex.position.x + vertex.position.z) * material.frequency + globals.time
                                                                                            * material.speed)
                                                                                            * material.amplitude;

    let final_matrix = utils::calculate_instance_world_matrix(
        vertex.i_pos_scale,
        vertex.i_rotation,
        instance_uniforms.world_from_local
    );

    let world_position = final_matrix * vec4<f32>(local_position, 1.0);

    out.clip_position = view.clip_from_world * world_position;
    out.uv = vertex.uv;

    return out;
}


@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(base_color_texture, base_color_sampler, in.uv);

#ifdef IS_RED
    return vec4(1., 0., 0., 0.);
#endif

    return material.color * tex_color * instance_uniforms.color;
}

