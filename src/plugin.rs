use super::prepare::*;
use super::{
    components::GpuCull,
    draw::DrawInstancedMaterial,
    node::InstancedComputeNode,
    pipeline::{InstancedComputePipeline, InstancedMaterialPipeline},
    systems::*,
};
use crate::prelude::*;

use bevy_app::{App, Plugin, PostUpdate};
use bevy_asset::{AssetApp, embedded_asset};
use bevy_core_pipeline::core_3d::AlphaMask3d;
use bevy_ecs::prelude::*;
use bevy_render::graph::CameraDriverLabel;
use bevy_render::{
    Render, RenderApp, RenderSystems,
    extract_component::ExtractComponentPlugin,
    render_asset::RenderAssetPlugin,
    render_graph::{RenderGraph, RenderLabel},
    render_phase::AddRenderCommand,
    render_resource::SpecializedMeshPipelines,
};

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct InstancedMaterialComputeLabel;

pub struct InstancedMaterialPlugin;

impl Plugin for InstancedMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "render.wgsl");
        embedded_asset!(app, "cull.wgsl");

        app.init_asset::<InstancedMaterial>();

        app.add_plugins((
            ExtractComponentPlugin::<InstancePipelineKey>::default(),
            ExtractComponentPlugin::<InstanceMaterialData>::default(),
            ExtractComponentPlugin::<InstancedMeshMaterial>::default(),
            ExtractComponentPlugin::<GpuCull>::default(),
            RenderAssetPlugin::<PreparedInstancedMaterial>::default(),
        ))
        .add_systems(PostUpdate, add_instance_key_component);

        let render_app = app.sub_app_mut(RenderApp);

        render_app
            .add_render_command::<AlphaMask3d, DrawInstancedMaterial>()
            .init_resource::<SpecializedMeshPipelines<InstancedMaterialPipeline>>()
            .add_systems(
                Render,
                (
                    (queue_instanced_material, queue_instanced_material_compute_pipeline)
                        .in_set(RenderSystems::QueueMeshes),
                    (
                        prepare_indirect_draw_buffer,
                        prepare_instance_buffer,
                        prepare_global_cull_buffer,
                        prepare_instanced_bind_group,
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
            .init_resource::<InstancedMaterialPipeline>()
            .init_resource::<InstancedComputePipeline>();
    }
}
