use std::fmt::{Debug, Formatter};
use std::hash::Hash;
use std::marker::PhantomData;

use crate::cull::{
    node::InstancedComputeNode, pipeline::InstancedComputePipeline,
    prepare::prepare_global_cull_buffer, queue::queue_instanced_material_compute_pipeline,
};
use crate::prelude::*;

use bevy_app::prelude::*;
use bevy_asset::embedded_asset;
use bevy_ecs::prelude::*;
use bevy_render::{
    Render, RenderApp, RenderSystems,
    extract_component::ExtractComponentPlugin,
    graph::CameraDriverLabel,
    render_graph::{RenderGraph, RenderLabel},
};
use bevy_shader::load_shader_library;

#[derive(Clone, RenderLabel)]
pub struct InstancedMaterialComputeLabel<M: InstancedMaterial>(PhantomData<M>);

impl<M: InstancedMaterial> Hash for InstancedMaterialComputeLabel<M> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::any::TypeId::of::<Self>().hash(state);
    }
}

impl<M: InstancedMaterial> PartialEq for InstancedMaterialComputeLabel<M> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl<M: InstancedMaterial> Eq for InstancedMaterialComputeLabel<M> {}

impl<M: InstancedMaterial> Debug for InstancedMaterialComputeLabel<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "InstancedMaterialComputeLabel::<{}>",
            std::any::type_name::<M>()
        )
    }
}

impl<M: InstancedMaterial> Default for InstancedMaterialComputeLabel<M> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

pub struct GpuComputeCullCorePlugin;

impl Plugin for GpuComputeCullCorePlugin {
    fn build(&self, app: &mut App) {
        load_shader_library!(app, "types.wgsl");
        load_shader_library!(app, "bindings.wgsl");

        embedded_asset!(app, "compute.wgsl");

        app.add_plugins((ExtractComponentPlugin::<GpuCullCompute>::default(),));
    }
}

pub struct GpuCullComputePlugin<T: InstancedMaterial>(PhantomData<T>);

impl<T: InstancedMaterial> Default for GpuCullComputePlugin<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: InstancedMaterial> Plugin for GpuCullComputePlugin<T> {
    fn build(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);

        let label = InstancedMaterialComputeLabel::<T>::default();
        let compute_node = InstancedComputeNode::<T>::from_world(render_app.world_mut());

        let mut render_graph = render_app.world_mut().resource_mut::<RenderGraph>();
        render_graph.add_node(label.clone(), compute_node);
        render_graph.add_node_edge(label, CameraDriverLabel);
        render_app.add_systems(
            Render,
            (
                prepare_global_cull_buffer::<T>.in_set(RenderSystems::PrepareResources),
                queue_instanced_material_compute_pipeline::<T>.in_set(RenderSystems::QueueMeshes),
            ),
        );
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .init_resource::<InstancedComputePipeline<T>>();
    }
}
