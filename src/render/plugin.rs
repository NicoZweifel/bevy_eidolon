use crate::prelude::*;
use crate::render::{
    draw::DrawInstancedMaterial, pipeline::InstancedMaterialPipeline, prepare::*,
    prepared_material::PreparedInstancedMaterial, queue::*,
};

use bevy_app::{App, Plugin, PreUpdate};
use bevy_asset::{AssetApp, UntypedAssetId, embedded_asset};
use bevy_camera::prelude::ViewVisibility;
use bevy_core_pipeline::core_3d::Opaque3d;
use bevy_ecs::{component::Tick, prelude::*};
use bevy_mesh::Mesh3d;
use bevy_platform::collections::hash_map::Entry;
use bevy_render::sync_world::{MainEntity, MainEntityHashMap};
use bevy_render::{
    Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
    extract_component::ExtractComponentPlugin, render_asset::RenderAssetPlugin,
    render_phase::AddRenderCommand, render_resource::SpecializedMeshPipelines,
};
use bevy_shader::load_shader_library;
use bevy_transform::prelude::GlobalTransform;

use crate::prepass::InstancedPrepassPlugin;

use std::hash::Hash;
use std::marker::PhantomData;

/// A SystemSet for ordering instanced material extraction.
#[derive(SystemSet, Clone, PartialEq, Eq, Debug, Hash)]
pub struct InstancedMaterialExtractionSystems;

pub struct InstancedMaterialCorePlugin;

impl Plugin for InstancedMaterialCorePlugin {
    fn build(&self, app: &mut App) {
        load_shader_library!(app, "types.wgsl");
        load_shader_library!(app, "constants.wgsl");
        load_shader_library!(app, "io_types.wgsl");
        load_shader_library!(app, "bindings.wgsl");
        load_shader_library!(app, "utils.wgsl");

        embedded_asset!(app, "mesh.wgsl");
        embedded_asset!(app, "shading.wgsl");

        app.add_plugins((
            ExtractComponentPlugin::<InstanceMaterialData>::default(),
            ExtractComponentPlugin::<InstanceHistory>::default(),
        ))
        .add_systems(PreUpdate, update_instance_history);

        let render_app = app.sub_app_mut(RenderApp);

        render_app
            .init_resource::<RenderInstancedMaterialInstances>()
            .configure_sets(ExtractSchedule, InstancedMaterialExtractionSystems)
            .add_systems(
                ExtractSchedule,
                late_sweep_instanced_material_instances.after(InstancedMaterialExtractionSystems),
            )
            .add_systems(
                Render,
                ((prepare_instance_buffer, prepare_indirect_draw_buffer)
                    .in_set(RenderSystems::PrepareResources),),
            );
    }
}

fn update_instance_history(
    mut commands: Commands,
    mut query: Query<
        (Entity, &GlobalTransform, Option<&mut InstanceHistory>),
        (With<InstanceMaterialData>, With<Mesh3d>),
    >,
) {
    for (entity, global_transform, history) in &mut query {
        let current_matrix = global_transform.to_matrix();
        if let Some(mut history) = history {
            history.0 = current_matrix;
        } else {
            commands
                .entity(entity)
                .insert(InstanceHistory(current_matrix));
        }
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
            InstancedPrepassPlugin::<M>::default(),
        ));

        let render_app = app.sub_app_mut(RenderApp);

        render_app
            .add_render_command::<Opaque3d, DrawInstancedMaterial<M>>()
            .init_resource::<SpecializedMeshPipelines<InstancedMaterialPipeline<M>>>()
            .add_systems(
                ExtractSchedule,
                (
                    extract_instanced_mesh_materials::<M>
                        .in_set(InstancedMaterialExtractionSystems),
                    early_sweep_instanced_material_instances::<M>
                        .after(InstancedMaterialExtractionSystems)
                        .before(late_sweep_instanced_material_instances),
                ),
            )
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

fn extract_instanced_mesh_materials<M: InstancedMaterial>(
    mut material_instances: ResMut<RenderInstancedMaterialInstances>,
    changed_meshes_query: Extract<
        Query<
            (Entity, &ViewVisibility, &InstancedMeshMaterial<M>),
            (
                Or<(Changed<ViewVisibility>, Changed<InstancedMeshMaterial<M>>)>,
                With<Mesh3d>,
            ),
        >,
    >,
) {
    let last_change_tick = material_instances.current_change_tick;

    for (entity, view_visibility, material) in &changed_meshes_query {
        if view_visibility.get() {
            material_instances.instances.insert(
                entity.into(),
                RenderInstancedMaterialInstance {
                    asset_id: material.id().untyped(),
                    last_change_tick,
                },
            );
        } else {
            material_instances
                .instances
                .remove(&MainEntity::from(entity));
        }
    }
}

fn early_sweep_instanced_material_instances<M: InstancedMaterial>(
    mut material_instances: ResMut<RenderInstancedMaterialInstances>,
    mut removed_materials_query: Extract<RemovedComponents<InstancedMeshMaterial<M>>>,
) {
    let last_change_tick = material_instances.current_change_tick;

    for entity in removed_materials_query.read() {
        let Entry::Occupied(occupied_entry) = material_instances.instances.entry(entity.into())
        else {
            continue;
        };

        if occupied_entry.get().last_change_tick != last_change_tick {
            occupied_entry.remove();
        }
    }
}

fn late_sweep_instanced_material_instances(
    mut material_instances: ResMut<RenderInstancedMaterialInstances>,
    mut removed_visibility_query: Extract<RemovedComponents<ViewVisibility>>,
    mut removed_mesh_query: Extract<RemovedComponents<Mesh3d>>,
) {
    let last_change_tick = material_instances.current_change_tick;

    let mut remove = |entity: Entity| {
        if let Entry::Occupied(occupied_entry) = material_instances.instances.entry(entity.into())
            && occupied_entry.get().last_change_tick != last_change_tick
        {
            occupied_entry.remove();
        }
    };

    for entity in removed_visibility_query.read() {
        remove(entity);
    }

    for entity in removed_mesh_query.read() {
        remove(entity);
    }

    material_instances
        .current_change_tick
        .set(last_change_tick.get() + 1);
}

#[derive(Resource, Default)]
pub struct RenderInstancedMaterialInstances {
    /// Maps from each entity in the main world to the
    /// [`RenderInstancedMaterialInstance`] associated with it.
    pub instances: MainEntityHashMap<RenderInstancedMaterialInstance>,
    /// A monotonically increasing counter, which we use to sweep
    /// [`RenderInstancedMaterialInstances::instances`] when the entities and/or required
    /// components are removed.
    pub current_change_tick: Tick,
}

pub struct RenderInstancedMaterialInstance {
    pub asset_id: UntypedAssetId,
    pub last_change_tick: Tick,
}
