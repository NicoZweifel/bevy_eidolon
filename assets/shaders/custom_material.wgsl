#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::mesh_bindings::mesh
#import bevy_eidolon::render::utils

struct InstanceUniforms {
    color: vec4<f32>,
    visibility_range: vec4<f32>,
    world_from_local: mat4x4<f32>
};

@group(3) @binding(100) var<uniform> instance_uniforms: InstanceUniforms;

struct CustomMaterialUniform {
    color: vec4<f32>,
};

@group(3) @binding(0) var<uniform> material: CustomMaterialUniform;
@group(3) @binding(1) var base_color_texture: texture_2d<f32>;
@group(3) @binding(2) var base_color_sampler: sampler;

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(8) i_pos_scale: vec4<f32>,
    @location(9) i_rotation: f32,
    @location(10) i_index: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    var scale = vertex.i_pos_scale.w;
    var translation = vertex.i_pos_scale.xyz;

    let final_matrix = utils::calculate_instance_world_matrix(vertex.i_pos_scale, vertex.i_rotation, instance_uniforms.world_from_local);

    let world_position = final_matrix * vec4<f32>(vertex.position, 1.0);
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

