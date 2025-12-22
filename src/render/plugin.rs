use crate::prelude::*;
use crate::render::{
    draw::DrawInstancedMaterial, pipeline::InstancedMaterialPipeline, prepare::*,
    prepared_material::PreparedInstancedMaterial, queue::*,
};

use std::hash::Hash;
use std::marker::PhantomData;

use bevy_app::{App, Plugin};
use bevy_asset::{AssetApp, embedded_asset};
use bevy_core_pipeline::core_3d::Opaque3d;
use bevy_ecs::prelude::*;
use bevy_render::{
    Render, RenderApp, RenderSystems, extract_component::ExtractComponentPlugin,
    render_asset::RenderAssetPlugin, render_graph::RenderLabel, render_phase::AddRenderCommand,
    render_resource::SpecializedMeshPipelines,
};
use bevy_shader::load_shader_library;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct InstancedMaterialComputeLabel;

pub struct InstancedMaterialCorePlugin;

impl Plugin for InstancedMaterialCorePlugin {
    fn build(&self, app: &mut App) {
        load_shader_library!(app, "types.wgsl");
        load_shader_library!(app, "io_types.wgsl");
        load_shader_library!(app, "bindings.wgsl");
        load_shader_library!(app, "utils.wgsl");

        embedded_asset!(app, "mesh.wgsl");
        embedded_asset!(app, "shading.wgsl");

        app.add_plugins((ExtractComponentPlugin::<InstanceMaterialData>::default(),));

        let render_app = app.sub_app_mut(RenderApp);

        render_app.add_systems(
            Render,
            ((prepare_instance_buffer, prepare_indirect_draw_buffer)
                .in_set(RenderSystems::PrepareResources),),
        );
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
            .add_render_command::<Opaque3d, DrawInstancedMaterial<M>>()
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
