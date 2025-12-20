use super::prepare::*;
use super::{
    components::GpuCull,
    draw::DrawInstancedMaterial,
    node::InstancedComputeNode,
    pipeline::{InstancedComputePipeline, InstancedMaterialPipeline},
    systems::*,
};
use crate::prelude::*;
use std::hash::Hash;
use std::marker::PhantomData;

use bevy_app::{App, Plugin};
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

pub struct InstancedMaterialCorePlugin;

impl Plugin for InstancedMaterialCorePlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "cull.wgsl");
        embedded_asset!(app, "render.wgsl");

        app.add_plugins((
            ExtractComponentPlugin::<InstanceMaterialData>::default(),
            ExtractComponentPlugin::<GpuCull>::default(),
        ));

        let render_app = app.sub_app_mut(RenderApp);

        render_app.add_systems(
            Render,
            (
                (queue_instanced_material_compute_pipeline,).in_set(RenderSystems::QueueMeshes),
                (
                    prepare_instance_buffer,
                    prepare_indirect_draw_buffer,
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

pub struct InstancedMaterialPlugin<M: InstancedMaterial>(PhantomData<M>);

impl<M: InstancedMaterial> Default for InstancedMaterialPlugin<M> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<M> Plugin for InstancedMaterialPlugin<M>
where
    M: InstancedMaterial,
    M::Data: PartialEq + Eq + Hash + Clone,
{
    fn build(&self, app: &mut App) {
        app.init_asset::<M>();

        app.add_plugins((
            ExtractComponentPlugin::<InstancedMeshMaterial<M>>::default(),
            RenderAssetPlugin::<PreparedInstancedMaterial<M>>::default(),
        ));

        let render_app = app.sub_app_mut(RenderApp);

        render_app
            .add_render_command::<AlphaMask3d, DrawInstancedMaterial<M>>()
            .init_resource::<SpecializedMeshPipelines<InstancedMaterialPipeline<M>>>()
            .add_systems(
                Render,
                (
                    queue_instanced_material::<M>.in_set(RenderSystems::QueueMeshes),
                    prepare_instanced_bind_group::<M>.in_set(RenderSystems::PrepareResources),
                ),
            );
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .init_resource::<InstancedMaterialPipeline<M>>();
    }
}
