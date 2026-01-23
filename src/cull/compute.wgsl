#import bevy_pbr::utils::rand_f
#import bevy_eidolon::cull::bindings::{source_buffer, instance_buffer, indirect_args, batch_offsets, instance_uniforms, camera}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let threads_per_row = 65535u * 64u;
    let i = global_id.x + global_id.y * threads_per_row;

    // Bounds check
    if (i >= arrayLength(&source_buffer)) { return; }

    let instance = source_buffer[i];

    // Skip freed slots
    if (instance.batch_id == 0xFFFFFFFFu) { return; }

    let batch_id = instance.batch_id;

    // Bounds check
    if (batch_id >= arrayLength(&indirect_args)) { return; }

    let batch = instance_uniforms[batch_id];

    // Chunk AABB Frustum Culling
    let aabb_center = batch.aabb_center.xyz;
    let aabb_extents = batch.aabb_half_extents.xyz;

    for (var k = 0u; k < 6u; k++) {
        let plane = camera.frustum[k];
        let dist = dot(plane.xyz, aabb_center) + plane.w;
        let radius = dot(abs(plane.xyz), aabb_extents);
        if (dist < -radius) {
            return;
        }
    }

    let local_pos = vec4<f32>(instance.pos_and_scale.xyz, 1.0);
    let world_pos = batch.world_from_local * local_pos;
    let dist = distance(world_pos.xyz, camera.view_pos.xyz);

    // Distance Culling
    if (dist < batch.visibility_range.x || dist > batch.visibility_range.w) {
        return;
    }

    // Atomic Append to the batch's draw command
    let write_index_in_batch = atomicAdd(&indirect_args[batch_id].instance_count, 1u);

    let batch_start_offset = batch_offsets[batch_id].start_index;
    let final_write_index = batch_start_offset + write_index_in_batch;

    // Write the instance to the output buffer
    instance_buffer[final_write_index] = instance;
}

