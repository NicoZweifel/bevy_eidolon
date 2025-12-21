#import bevy_pbr::utils::rand_f
#import bevy_eidolon::cull::bindings::{source_buffer, instance_buffer, indirect_args, lod_data, camera}

fn hash_noise(index: u32) -> f32 {
    var state = index;
    return rand_f(&state);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let i = global_id.x;
    if (i >= arrayLength(&source_buffer)) { return; }

    let instance = source_buffer[i];
    let local_pos = vec4<f32>(instance.pos_and_scale.xyz, 1.0);
    let world_pos = lod_data.world_from_local * local_pos;

    let dist = distance(world_pos.xyz, camera.view_pos.xyz);

    if (dist < lod_data.visibility_range.x || dist > lod_data.visibility_range.w) {
        return;
    }

    let write_index = atomicAdd(&indirect_args.instance_count, 1u);

    instance_buffer[write_index] = instance;
}
