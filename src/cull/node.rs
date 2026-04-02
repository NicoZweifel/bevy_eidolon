use bevy_ecs::prelude::*;
use bevy_render::{
    render_resource::{ComputePassDescriptor, PipelineCache},
    renderer::RenderContext,
};

use super::resources::GlobalCullBuffer;
use crate::{
    allocator::resources::GlobalInstanceAllocator, cull::pipeline::InstancedComputePipeline,
    material::InstancedMaterial,
};

#[cfg(feature = "trace")]
use tracing::*;

pub fn instanced_compute_node<M: InstancedMaterial>(
    mut render_context: RenderContext,
    pipeline: Option<Res<InstancedComputePipeline<M>>>,
    pipeline_cache: Option<Res<PipelineCache>>,
    allocator: Option<Res<GlobalInstanceAllocator<M>>>,
    cull_buffer: Option<Res<GlobalCullBuffer>>,
) {
    let Some(pipeline) = pipeline else {
        return;
    };

    let Some(pipeline_cache) = pipeline_cache else {
        return;
    };

    let Some(pipeline_id) = pipeline.pipeline_id else {
        return;
    };

    let Some(compute_pipeline) = pipeline_cache.get_compute_pipeline(pipeline_id) else {
        return;
    };

    let Some(allocator) = allocator else {
        return;
    };

    if allocator.pages.is_empty() {
        return;
    }

    let Some(cull_buffer) = cull_buffer else {
        return;
    };

    let mut pass = render_context
        .command_encoder()
        .begin_compute_pass(&ComputePassDescriptor {
            label: Some("instanced_gpu_cull_pass"),
            timestamp_writes: None,
        });

    pass.set_pipeline(compute_pipeline);
    pass.set_bind_group(2, &cull_buffer.bind_group, &[]);

    let mut dispatched = 0;

    for (i, page) in allocator.pages.iter().enumerate() {
        if page.compute_capacity == 0 {
            continue;
        }

        let Some(compute_bg) = &page.compute_bind_group else {
            #[cfg(feature = "trace")]
            warn!(
                "Page {} has capacity {} but no compute_bind_group!",
                i, page.compute_capacity
            );
            continue;
        };

        let Some(common_bg) = &page.common_bind_group else {
            #[cfg(feature = "trace")]
            warn!(
                "Page {} has capacity {} but no common_bind_group!",
                i, page.compute_capacity
            );
            continue;
        };

        pass.set_bind_group(0, compute_bg, &[]);
        pass.set_bind_group(1, common_bg, &[]);

        let instances = page.compute_capacity;
        let workgroup_size = 64;

        let workgroups = (instances as f32 / workgroup_size as f32).ceil() as u32;
        if workgroups > 0 {
            #[cfg(feature = "trace")]
            trace!(
                "Dispatching Page {}: {} instances, {} workgroups",
                i, instances, workgroups
            );

            let max_workgroups = 65535;
            if workgroups > max_workgroups {
                let x = max_workgroups;
                let y = (workgroups as f32 / max_workgroups as f32).ceil() as u32;
                pass.dispatch_workgroups(x, y, 1);
            } else {
                pass.dispatch_workgroups(workgroups, 1, 1);
            }
            dispatched += 1;
        }
    }

    #[cfg(feature = "trace")]
    if dispatched > 0 {
        trace!(
            "Finished Instanced Cull Pass: {} pages dispatched",
            dispatched
        );
    }
}
