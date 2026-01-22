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

        let pipeline_res = world.resource::<InstancedComputePipeline<M>>();
        let pipeline_cache = world.resource::<PipelineCache>();

        let Some(pipeline) = pipeline_res
            .pipeline_id
            .and_then(|id| pipeline_cache.get_compute_pipeline(id))
        else {
            return Ok(());
        };

        let Some(global_allocator) = world.get_resource::<GlobalInstanceAllocator<M>>() else {
            return Ok(());
        };

        if global_allocator.pages.is_empty() {
            return Ok(());
        }

        let Some(global_cull_buffer) = world.get_resource::<GlobalCullBuffer>() else {
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
        pass.set_bind_group(2, &global_cull_buffer.bind_group, &[]);

        let mut pages_dispatched = 0;

        for (page_index, page) in global_allocator.pages.iter().enumerate() {
            if page.compute_capacity == 0 {
                continue;
            }

            let Some(compute_bg) = &page.compute_bind_group else {
                #[cfg(feature = "trace")]
                warn!(
                    "Page {} has capacity {} but no compute_bind_group!",
                    page_index, page.compute_capacity
                );
                continue;
            };

            let Some(common_bg) = &page.common_bind_group else {
                #[cfg(feature = "trace")]
                warn!(
                    "Page {} has capacity {} but no common_bind_group!",
                    page_index, page.compute_capacity
                );
                continue;
            };

            pass.set_bind_group(0, compute_bg, &[]);
            pass.set_bind_group(1, common_bg, &[]);

            let total_instances = page.compute_capacity;
            let workgroup_size = 64;
            let total_workgroups = (total_instances as f32 / workgroup_size as f32).ceil() as u32;

            if total_workgroups > 0 {
                #[cfg(feature = "trace")]
                trace!(
                    "Dispatching Page {}: {} instances, {} workgroups",
                    page_index, total_instances, total_workgroups
                );

                let max_workgroups_per_dim = 65535;

                if total_workgroups > max_workgroups_per_dim {
                    let x = max_workgroups_per_dim;
                    let y = (total_workgroups as f32 / max_workgroups_per_dim as f32).ceil() as u32;
                    pass.dispatch_workgroups(x, y, 1);
                } else {
                    pass.dispatch_workgroups(total_workgroups, 1, 1);
                }
                pages_dispatched += 1;
            }
        }

        #[cfg(feature = "trace")]
        if pages_dispatched > 0 {
            trace!(
                "Finished Instanced Cull Pass: {} pages dispatched",
                pages_dispatched
            );
        }

        Ok(())
    }
}
