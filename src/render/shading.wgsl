#import bevy_eidolon::render::bindings::{material, instance_uniforms}
#import bevy_eidolon::render::io_types::{VertexOutput}


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
