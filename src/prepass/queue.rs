use super::draw::*;
use super::pipeline::InstancedPrepassPipeline;

use crate::allocator::prelude::MaterialBatchRanges;
use crate::prelude::*;
use crate::render::{
    pipeline::InstancedMaterialPipelineKey, prepared_material::PreparedInstancedMaterial,
};

use bevy_core_pipeline::prepass::{
    DepthPrepass, MotionVectorPrepass, NormalPrepass, Opaque3dPrepass,
    OpaqueNoLightmap3dBatchSetKey, OpaqueNoLightmap3dBinKey,
};
use bevy_ecs::{prelude::*, system::SystemChangeTick};
use bevy_pbr::{MeshPipelineKey, RenderMeshInstances};
use bevy_render::{
    batching::gpu_preprocessing::GpuPreprocessingSupport,
    mesh::RenderMesh,
    mesh::allocator::MeshAllocator,
    prelude::Msaa,
    render_asset::RenderAssets,
    render_phase::{
        BinnedRenderPhaseType, DrawFunctions, InputUniformIndex, ViewBinnedRenderPhases,
    },
    render_resource::{PipelineCache, SpecializedMeshPipelines},
    view::ExtractedView,
};

use std::hash::Hash;

#[cfg(feature = "trace")]
use tracing::trace;

#[allow(clippy::too_many_arguments)]
pub fn queue_instanced_material_prepass<M>(
    opaque_draw_functions: Res<DrawFunctions<Opaque3dPrepass>>,
    prepass_pipeline: Res<InstancedPrepassPipeline<M>>,
    mut pipelines: ResMut<SpecializedMeshPipelines<InstancedPrepassPipeline<M>>>,
    pipeline_cache: Res<PipelineCache>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    render_materials: Res<RenderAssets<PreparedInstancedMaterial<M>>>,
    render_material_instances: Res<RenderInstancedMaterialInstances>,
    mesh_allocator: Res<MeshAllocator>,
    _gpu_preprocessing_support: Res<GpuPreprocessingSupport>,
    mut opaque_render_phases: ResMut<ViewBinnedRenderPhases<Opaque3dPrepass>>,
    views: Query<(
        &ExtractedView,
        &Msaa,
        Option<&DepthPrepass>,
        Option<&NormalPrepass>,
        Option<&MotionVectorPrepass>,
    )>,
    batch_ranges: Res<MaterialBatchRanges<M>>,
    ticks: SystemChangeTick,
) where
    M: InstancedMaterial,
    M::Data: PartialEq + Eq + Hash + Clone,
{
    let draw_opaque = opaque_draw_functions.read().id::<DrawInstancedPrepass<M>>();

    for (view, msaa, depth_prepass, normal_prepass, motion_vector_prepass) in &views {
        if depth_prepass.is_none() && normal_prepass.is_none() && motion_vector_prepass.is_none() {
            continue;
        }

        let mut view_key = MeshPipelineKey::from_msaa_samples(msaa.samples());
        if depth_prepass.is_some() {
            view_key |= MeshPipelineKey::DEPTH_PREPASS;
        }
        if normal_prepass.is_some() {
            view_key |= MeshPipelineKey::NORMAL_PREPASS;
        }
        if motion_vector_prepass.is_some() {
            view_key |= MeshPipelineKey::MOTION_VECTOR_PREPASS;
        }

        let mut opaque_phase = opaque_render_phases.get_mut(&view.retained_view_entity);

        for (batch_index, (entity, main_entity)) in batch_ranges.representatives.iter().enumerate()
        {
            let batch_index = batch_index as u32;

            let Some(material_instance) = render_material_instances.instances.get(main_entity)
            else {
                continue;
            };
            let Some(prepared_material) = render_materials.get(material_instance.asset_id.typed())
            else {
                continue;
            };

            if prepared_material.disable_prepass {
                continue;
            };

            let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(*main_entity)
            else {
                continue;
            };
            let Some(mesh) = render_meshes.get(mesh_instance.mesh_asset_id) else {
                continue;
            };

            let mesh_key =
                view_key | MeshPipelineKey::from_primitive_topology(mesh.primitive_topology());

            let key = InstancedMaterialPipelineKey {
                mesh_key,
                bind_group_data: prepared_material.key.clone(),
            };

            let pipeline = pipelines
                .specialize(&pipeline_cache, &prepass_pipeline, key, &mesh.layout)
                .unwrap();

            let (vertex_slab, index_slab) = mesh_allocator.mesh_slabs(&mesh_instance.mesh_asset_id);

            let Some(phase) = opaque_phase.as_mut() else {
                continue;
            };

            #[cfg(feature = "trace")]
            trace!(
                "queue_prepass: adding batch {} for entity {:?}",
                batch_index, entity
            );

            phase.add(
                OpaqueNoLightmap3dBatchSetKey {
                    pipeline,
                    draw_function: draw_opaque,
                    material_bind_group_index: None,
                    vertex_slab: vertex_slab.unwrap_or_default(),
                    index_slab,
                },
                OpaqueNoLightmap3dBinKey {
                    asset_id: mesh_instance.mesh_asset_id.into(),
                },
                (*entity, *main_entity),
                InputUniformIndex(0),
                BinnedRenderPhaseType::UnbatchableMesh,
                ticks.this_run(),
            );
        }
    }
}
