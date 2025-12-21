#define_import_path bevy_eidolon::render::utils

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

#import bevy_pbr::mesh_view_bindings::view

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
