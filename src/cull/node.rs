use bevy_ecs::prelude::*;
use bevy_render::{
    render_graph::{Node, NodeRunError, RenderGraphContext},
    render_resource::{CachedPipelineState, ComputePassDescriptor, PipelineCache},
    renderer::RenderContext,
};

use super::resources::GlobalCullBuffer;
use crate::{
    allocator::resources::GlobalInstanceAllocator, cull::pipeline::InstancedComputePipeline,
    material::InstancedMaterial,
};

use std::marker::PhantomData;

#[cfg(feature = "trace")]
use tracing::*;

pub struct InstancedComputeNode<M: InstancedMaterial> {
    state: InstancedComputeNodeState,
    _marker: PhantomData<M>,
}

enum InstancedComputeNodeState {
    Loading,
    Ready,
}

impl<M: InstancedMaterial> FromWorld for InstancedComputeNode<M> {
    fn from_world(_world: &mut World) -> Self {
        Self {
            state: InstancedComputeNodeState::Loading,
            _marker: PhantomData,
        }
    }
}

impl<M: InstancedMaterial> Node for InstancedComputeNode<M> {
    fn update(&mut self, world: &mut World) {
        let pipeline = world.resource::<InstancedComputePipeline<M>>();
        let pipeline_cache = world.resource::<PipelineCache>();

        if let InstancedComputeNodeState::Loading = self.state
            && let Some(id) = pipeline.pipeline_id
            && let CachedPipelineState::Ok(_) = pipeline_cache.get_compute_pipeline_state(id)
        {
            self.state = InstancedComputeNodeState::Ready;
        }
    }

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        if let InstancedComputeNodeState::Loading = self.state {
            return Ok(());
        }

        let pipeline = world.resource::<InstancedComputePipeline<M>>();
        let cache = world.resource::<PipelineCache>();

        let Some(pipeline) = pipeline
            .pipeline_id
            .and_then(|id| cache.get_compute_pipeline(id))
        else {
            return Ok(());
        };

        let Some(allocator) = world.get_resource::<GlobalInstanceAllocator<M>>() else {
            return Ok(());
        };

        if allocator.pages.is_empty() {
            return Ok(());
        }

        let Some(cull_buffer) = world.get_resource::<GlobalCullBuffer>() else {
            return Ok(());
        };

        let mut pass =
            render_context
                .command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {
                    label: Some("instanced_gpu_cull_pass"),
                    timestamp_writes: None,
                });

        pass.set_pipeline(pipeline);
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

        Ok(())
    }
}
