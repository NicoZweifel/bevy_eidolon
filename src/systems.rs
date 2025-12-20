use super::{
    components::InstanceMaterialData,
    draw::DrawInstancedMaterial,
    material::InstancedMeshMaterial,
    pipeline::{InstancedMaterialPipeline, InstancedMaterialPipelineKey},
};
use crate::pipeline::InstancedComputePipeline;
use bevy_core_pipeline::{
    core_3d::AlphaMask3d,
    prepass::{
        DepthPrepass, MotionVectorPrepass, NormalPrepass, OpaqueNoLightmap3dBatchSetKey,
        OpaqueNoLightmap3dBinKey,
    },
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
use bevy_utils::default;
use std::hash::Hash;

use crate::material::InstancedMaterial;
use crate::prelude::PreparedInstancedMaterial;

#[allow(clippy::too_many_arguments)]
pub(super) fn queue_instanced_material<M>(
    alpha_mask_3d_draw_functions: Res<DrawFunctions<AlphaMask3d>>,
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
    mut alpha_mask_render_phases: ResMut<ViewBinnedRenderPhases<AlphaMask3d>>,
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
    let draw_custom = alpha_mask_3d_draw_functions
        .read()
        .id::<DrawInstancedMaterial<M>>();

    for (view, msaa, depth_prepass, normal_prepass, motion_vector_prepass) in &views {
        let Some(alpha_mask_phase) = alpha_mask_render_phases.get_mut(&view.retained_view_entity)
        else {
            continue;
        };

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

        for (entity, e_main, h_material) in &material_meshes {
            let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(*e_main) else {
                continue;
            };
            let Some(mesh) = meshes.get(mesh_instance.mesh_asset_id) else {
                continue;
            };
            let Some(prepared_material) = render_materials.get(&h_material.0) else {
                continue;
            };

            let mut key = InstancedMaterialPipelineKey {
                mesh_key: view_key
                    | MeshPipelineKey::from_primitive_topology(mesh.primitive_topology()),
                bind_group_data: prepared_material.key.clone(),
            };

            key.mesh_key |= MeshPipelineKey::MAY_DISCARD;

            let pipeline = pipelines
                .specialize(&pipeline_cache, &custom_pipeline, key, &mesh.layout)
                .unwrap();

            let (vertex_slab, index_slab) = mesh_allocator.mesh_slabs(&mesh_instance.mesh_asset_id);

            alpha_mask_phase.add(
                OpaqueNoLightmap3dBatchSetKey {
                    pipeline,
                    draw_function: draw_custom,
                    material_bind_group_index: None,
                    vertex_slab: vertex_slab.unwrap_or_default(),
                    index_slab,
                },
                OpaqueNoLightmap3dBinKey {
                    asset_id: mesh_instance.mesh_asset_id.into(),
                },
                (entity, *e_main),
                mesh_instance.current_uniform_index,
                BinnedRenderPhaseType::mesh(
                    mesh_instance.should_batch(),
                    &gpu_preprocessing_support,
                ),
                ticks.this_run(),
            );
        }
    }
}

pub(super) fn queue_instanced_material_compute_pipeline(
    pipeline_cache: Res<PipelineCache>,
    mut compute_pipeline: ResMut<InstancedComputePipeline>,
) {
    if compute_pipeline.pipeline_id.is_some() {
        return;
    }

    let id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("instanced_material_compute_pipeline".into()),
        layout: vec![
            compute_pipeline.entity_layout.clone(),
            compute_pipeline.global_layout.clone(),
        ],
        push_constant_ranges: vec![],
        shader: compute_pipeline.shader.clone(),
        shader_defs: vec![],
        entry_point: Some("main".into()),
        ..default()
    });

    compute_pipeline.pipeline_id = Some(id);
}
