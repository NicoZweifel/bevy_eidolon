use std::marker::PhantomData;

use crate::cull::{
    node::instanced_compute_node, pipeline::InstancedComputePipeline,
    prepare::prepare_global_cull_buffer, queue::queue_instanced_material_compute_pipeline,
};
use crate::prelude::*;

use bevy_app::prelude::*;
use bevy_asset::embedded_asset;
use bevy_core_pipeline::schedule::camera_driver;
use bevy_ecs::prelude::*;
use bevy_render::{
    Render, RenderApp, RenderSystems, extract_component::ExtractComponentPlugin,
    renderer::RenderGraph,
};
use bevy_shader::load_shader_library;

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

        render_app.add_systems(
            Render,
            (
                prepare_global_cull_buffer::<T>.in_set(RenderSystems::PrepareResources),
                queue_instanced_material_compute_pipeline::<T>.in_set(RenderSystems::QueueMeshes),
            ),
        );

        render_app.add_systems(
            RenderGraph,
            instanced_compute_node::<T>.before(camera_driver),
        );
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .init_resource::<InstancedComputePipeline<T>>();
    }
}
