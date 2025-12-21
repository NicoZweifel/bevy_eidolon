use crate::components::GpuCullCompute;
use crate::cull::node::InstancedComputeNode;
use crate::cull::pipeline::InstancedComputePipeline;
use crate::cull::prepare::{
    prepare_global_cull_buffer, prepare_instanced_material_compute_resources,
};
use crate::cull::queue::queue_instanced_material_compute_pipeline;
use crate::prelude::InstancedMaterialComputeLabel;

use bevy_app::{App, Plugin};
use bevy_asset::embedded_asset;
use bevy_ecs::prelude::{FromWorld, IntoScheduleConfigs};
use bevy_render::extract_component::ExtractComponentPlugin;
use bevy_render::graph::CameraDriverLabel;
use bevy_render::render_graph::RenderGraph;
use bevy_render::{Render, RenderApp, RenderSystems};
use bevy_shader::load_shader_library;

pub struct GpuComputeCullPlugin;

impl Plugin for GpuComputeCullPlugin {
    fn build(&self, app: &mut App) {
        load_shader_library!(app, "types.wgsl");
        load_shader_library!(app, "bindings.wgsl");

        embedded_asset!(app, "compute.wgsl");

        app.add_plugins((ExtractComponentPlugin::<GpuCullCompute>::default(),));

        let render_app = app.sub_app_mut(RenderApp);

        render_app.add_systems(
            Render,
            (
                (queue_instanced_material_compute_pipeline,).in_set(RenderSystems::QueueMeshes),
                (
                    prepare_global_cull_buffer,
                    prepare_instanced_material_compute_resources.after(prepare_global_cull_buffer),
                )
                    .in_set(RenderSystems::PrepareResources),
            ),
        );

        let compute_node = InstancedComputeNode::from_world(render_app.world_mut());
        let mut render_graph = render_app.world_mut().resource_mut::<RenderGraph>();

        render_graph.add_node(InstancedMaterialComputeLabel, compute_node);
        render_graph.add_node_edge(InstancedMaterialComputeLabel, CameraDriverLabel);
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .init_resource::<InstancedComputePipeline>();
    }
}
