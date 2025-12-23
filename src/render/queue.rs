use crate::prelude::*;
use crate::render::{
    draw::DrawInstancedMaterial,
    pipeline::{InstancedMaterialPipeline, InstancedMaterialPipelineKey},
    prepared_material::PreparedInstancedMaterial,
};

use bevy_core_pipeline::{
    core_3d::{Opaque3d, Opaque3dBatchSetKey, Opaque3dBinKey},
    prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass},
};
use bevy_ecs::{prelude::*, system::SystemChangeTick};
use bevy_pbr::{MeshPipelineKey, RenderMeshInstances};
use bevy_render::{
    batching::gpu_preprocessing::GpuPreprocessingSupport,
    mesh::RenderMesh,
    mesh::allocator::MeshAllocator,
    render_asset::RenderAssets,
    render_phase::DrawFunctions,
    render_phase::{BinnedRenderPhaseType, ViewBinnedRenderPhases},
    render_resource::*,
    sync_world::MainEntity,
    view::ExtractedView,
    view::Msaa,
};

use bevy_core_pipeline::prepass::{
    Opaque3dPrepass, OpaqueNoLightmap3dBatchSetKey, OpaqueNoLightmap3dBinKey,
};
use std::hash::Hash;

#[allow(clippy::too_many_arguments)]
pub(crate) fn queue_instanced_material<M>(
    opaque_3d_draw_functions: Res<DrawFunctions<Opaque3d>>,
    opaque_3d_prepass_draw_functions: Res<DrawFunctions<Opaque3dPrepass>>,
    custom_pipeline: Res<InstancedMaterialPipeline<M>>,
    mut pipelines: ResMut<SpecializedMeshPipelines<InstancedMaterialPipeline<M>>>,
    pipeline_cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<RenderMesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    render_materials: Res<RenderAssets<PreparedInstancedMaterial<M>>>,
    material_meshes: Query<
        (Entity, &MainEntity, &InstancedMeshMaterial<M>),
        With<InstanceMaterialData>,
    >,
    mesh_allocator: Res<MeshAllocator>,
    gpu_preprocessing_support: Res<GpuPreprocessingSupport>,
    mut opaque_render_phases: ResMut<ViewBinnedRenderPhases<Opaque3d>>,
    mut opaque_prepass_phases: ResMut<ViewBinnedRenderPhases<Opaque3dPrepass>>,
    ticks: SystemChangeTick,
    views: Query<(
        &ExtractedView,
        &Msaa,
        Option<&DepthPrepass>,
        Option<&NormalPrepass>,
        Option<&MotionVectorPrepass>,
    )>,
) where
    M: InstancedMaterial,
    M::Data: PartialEq + Eq + Hash + Clone,
{
    let draw_custom = opaque_3d_draw_functions
        .read()
        .id::<DrawInstancedMaterial<M>>();

    let draw_prepass = opaque_3d_prepass_draw_functions
        .read()
        .id::<DrawInstancedMaterial<M>>();

    for (view, msaa, depth_prepass, normal_prepass, motion_vector_prepass) in &views {
        let Some(opaque_mask_phases) = opaque_render_phases.get_mut(&view.retained_view_entity)
        else {
            continue;
        };

        let mut prepass_phase = opaque_prepass_phases.get_mut(&view.retained_view_entity);

        let mut view_key = MeshPipelineKey::from_msaa_samples(msaa.samples())
            | MeshPipelineKey::from_hdr(view.hdr);

        if depth_prepass.is_some() {
            view_key |= MeshPipelineKey::DEPTH_PREPASS;
        }
        if normal_prepass.is_some() {
            view_key |= MeshPipelineKey::NORMAL_PREPASS;
        }
        if motion_vector_prepass.is_some() {
            view_key |= MeshPipelineKey::MOTION_VECTOR_PREPASS;
        }

        for (entity, main_entity, h_material) in &material_meshes {
            let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(*main_entity)
            else {
                continue;
            };
            let Some(mesh) = meshes.get(mesh_instance.mesh_asset_id) else {
                continue;
            };
            let Some(prepared_material) = render_materials.get(&h_material.0) else {
                continue;
            };

            let (vertex_slab, index_slab) = mesh_allocator.mesh_slabs(&mesh_instance.mesh_asset_id);
            let binned_phase_type = BinnedRenderPhaseType::mesh(
                mesh_instance.should_batch(),
                &gpu_preprocessing_support,
            );

            let main_pipeline_key = InstancedMaterialPipelineKey {
                mesh_key: view_key
                    | MeshPipelineKey::from_primitive_topology(mesh.primitive_topology()),
                bind_group_data: prepared_material.key.clone(),
                is_prepass: false,
            };

            let pipeline = pipelines
                .specialize(
                    &pipeline_cache,
                    &custom_pipeline,
                    main_pipeline_key,
                    &mesh.layout,
                )
                .unwrap();

            opaque_mask_phases.add(
                Opaque3dBatchSetKey {
                    pipeline,
                    draw_function: draw_custom,
                    material_bind_group_index: None,
                    vertex_slab: vertex_slab.unwrap_or_default(),
                    index_slab: index_slab.clone(),
                    lightmap_slab: None,
                },
                Opaque3dBinKey {
                    asset_id: mesh_instance.mesh_asset_id.into(),
                },
                (entity, *main_entity),
                mesh_instance.current_uniform_index,
                binned_phase_type,
                ticks.this_run(),
            );

            let Some(prepass_phase) = &mut prepass_phase else {
                continue;
            };

            let prepass_pipeline_key = InstancedMaterialPipelineKey {
                mesh_key: view_key
                    | MeshPipelineKey::from_primitive_topology(mesh.primitive_topology()),
                bind_group_data: prepared_material.key.clone(),
                is_prepass: true,
            };

            let prepass_pipeline = pipelines
                .specialize(
                    &pipeline_cache,
                    &custom_pipeline,
                    prepass_pipeline_key,
                    &mesh.layout,
                )
                .unwrap();

            prepass_phase.add(
                OpaqueNoLightmap3dBatchSetKey {
                    pipeline: prepass_pipeline,
                    draw_function: draw_prepass,
                    material_bind_group_index: None,
                    vertex_slab: vertex_slab.unwrap_or_default(),
                    index_slab: index_slab.clone(),
                },
                OpaqueNoLightmap3dBinKey {
                    asset_id: mesh_instance.mesh_asset_id.into(),
                },
                (entity, *main_entity),
                mesh_instance.current_uniform_index,
                binned_phase_type,
                ticks.this_run(),
            );
        }
    }
}
