use super::{draw::*, pipeline::*, queue::queue_instanced_material_prepass};
use crate::material::InstancedMaterial;

use bevy_app::prelude::*;
use bevy_asset::embedded_asset;
use bevy_core_pipeline::prepass::Opaque3dPrepass;
use bevy_ecs::prelude::*;
use bevy_pbr::init_prepass_pipeline;
use bevy_render::{
    Render, RenderApp, RenderStartup, RenderSystems, render_phase::AddRenderCommand,
    render_resource::SpecializedMeshPipelines,
};

use bevy_core_pipeline::deferred::Opaque3dDeferred;
use std::hash::Hash;
use std::marker::PhantomData;

pub struct InstancedPrepassPlugin<M>(PhantomData<M>);

impl<M> Default for InstancedPrepassPlugin<M> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<M> Plugin for InstancedPrepassPlugin<M>
where
    M::Data: PartialEq + Eq + Hash + Clone,
    M: InstancedMaterial,
{
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "prepass.wgsl");

        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .add_render_command::<Opaque3dPrepass, DrawInstancedPrepass<M>>()
            .add_render_command::<Opaque3dDeferred, DrawInstancedPrepass<M>>()
            .init_resource::<SpecializedMeshPipelines<InstancedPrepassPipeline<M>>>()
            .add_systems(
                RenderStartup,
                init_instanced_prepass_pipeline::<M>.after(init_prepass_pipeline),
            )
            .add_systems(
                Render,
                queue_instanced_material_prepass::<M>.in_set(RenderSystems::QueueMeshes),
            );
    }
}
