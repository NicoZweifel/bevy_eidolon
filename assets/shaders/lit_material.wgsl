#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::mesh_bindings::mesh

#import bevy_pbr::pbr_types
#import bevy_pbr::pbr_functions

#import bevy_eidolon::render::utils
#import bevy_eidolon::render::bindings::instance_uniforms
#import bevy_eidolon::render::io_types::{VertexOutput, Vertex}

struct CustomMaterialUniform {
    color: vec4<f32>,
};

@group(3) @binding(0) var<uniform> material: CustomMaterialUniform;
@group(3) @binding(1) var base_color_texture: texture_2d<f32>;
@group(3) @binding(2) var base_color_sampler: sampler;

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    var scale = vertex.i_pos_scale.w;
    var translation = vertex.i_pos_scale.xyz;

    let final_matrix = utils::calculate_instance_world_matrix(vertex.i_pos_scale, vertex.i_rotation, instance_uniforms.world_from_local);
    let world_position = final_matrix * vec4<f32>(vertex.position, 1.0);

    out.clip_position = view.clip_from_world * world_position;
    out.uv = vertex.uv;

    out.world_position = world_position;
    out.world_normal = (final_matrix * vec4<f32>(vertex.normal, 0.0)).xyz;

    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(base_color_texture, base_color_sampler, in.uv);
    let base_color = material.color * tex_color * instance_uniforms.color;

    var pbr_input: pbr_types::PbrInput = pbr_types::pbr_input_new();

    pbr_input.material.base_color = base_color;
    pbr_input.material.perceptual_roughness = 0.5;
    pbr_input.material.metallic = 0.0;
    pbr_input.material.reflectance = vec3<f32>(0.5, 0.5, 0.5);

    pbr_input.material.emissive = vec4<f32>(0.0, 0.0, 0.0, 1.0);

    pbr_input.frag_coord = in.clip_position;
    pbr_input.world_position = in.world_position;

    pbr_input.flags = mesh[0].flags;
    pbr_input.is_orthographic = view.clip_from_view[3].w == 1.0;

    pbr_input.world_normal = bevy_pbr::pbr_functions::prepare_world_normal(
        in.world_normal,
        false,
        false
    );

    pbr_input.V = bevy_pbr::pbr_functions::calculate_view(
        in.world_position,
        pbr_input.is_orthographic
    );

    pbr_input.N = normalize(pbr_input.world_normal);

    var output_color = bevy_pbr::pbr_functions::apply_pbr_lighting(pbr_input);

    output_color = bevy_pbr::pbr_functions::main_pass_post_lighting_processing(
        pbr_input,
        output_color
    );

    return output_color;
}

