use super::pipeline::InstancedPrepassPipeline;

use crate::allocator::prelude::MaterialBatchRanges;
use crate::prelude::*;
use crate::render::{
    pipeline::InstancedMaterialPipelineKey, prepared_material::PreparedInstancedMaterial,
};

use bevy_core_pipeline::prepass::{
    DeferredPrepass, DepthPrepass, MotionVectorPrepass, NormalPrepass, Opaque3dPrepass,
    OpaqueNoLightmap3dBatchSetKey, OpaqueNoLightmap3dBinKey,
};
use bevy_ecs::{prelude::*, system::SystemChangeTick};
use bevy_pbr::{MeshPipelineKey, RenderMeshInstances};
use bevy_render::mesh::allocator::MeshSlabs;
use bevy_render::{
    batching::gpu_preprocessing::GpuPreprocessingSupport,
    mesh::RenderMesh,
    mesh::allocator::MeshAllocator,
    prelude::Msaa,
    render_asset::RenderAssets,
    render_phase::{BinnedRenderPhaseType, InputUniformIndex, ViewBinnedRenderPhases},
    render_resource::{PipelineCache, SpecializedMeshPipelines},
    view::ExtractedView,
};

use bevy_core_pipeline::deferred::Opaque3dDeferred;
use std::hash::Hash;
#[cfg(feature = "trace")]
use tracing::trace;

#[allow(clippy::too_many_arguments)]
pub fn queue_instanced_material_prepass<M>(
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
    mut opaque_deferred_phases: ResMut<ViewBinnedRenderPhases<Opaque3dDeferred>>,
    views: Query<(
        &ExtractedView,
        &Msaa,
        Option<&DepthPrepass>,
        Option<&NormalPrepass>,
        Option<&MotionVectorPrepass>,
        Option<&DeferredPrepass>,
    )>,
    batch_ranges: Res<MaterialBatchRanges<M>>,
    ticks: SystemChangeTick,
) where
    M: InstancedMaterial,
    M::Data: PartialEq + Eq + Hash + Clone,
{
    for (view, msaa, depth_prepass, normal_prepass, motion_vector_prepass, deferred_prepass) in
        &views
    {
        let base_key = MeshPipelineKey::from_msaa_samples(msaa.samples());

        let mut opaque_phase = opaque_render_phases.get_mut(&view.retained_view_entity);
        let mut deferred_phase = deferred_prepass
            .and_then(|_| opaque_deferred_phases.get_mut(&view.retained_view_entity));

        if opaque_phase.is_none() && deferred_phase.is_none() {
            continue;
        }

        for (batch_index, (entity, main_entity)) in batch_ranges.entities.iter().enumerate() {
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
            }
            let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(*main_entity)
            else {
                continue;
            };
            let Some(mesh) = render_meshes.get(mesh_instance.mesh_asset_id()) else {
                continue;
            };

            let Some(MeshSlabs {
                vertex_slab_id,
                index_slab_id,
                ..
            }) = mesh_allocator.mesh_slabs(&mesh_instance.mesh_asset_id())
            else {
                continue;
            };

            let primitive_topology_key = MeshPipelineKey::from_primitive_topology_and_strip_index(
                mesh.primitive_topology(),
                None,
            );

            #[cfg(feature = "trace")]
            trace!(
                "queue_prepass: adding batch {} for entity {:?}",
                batch_index, entity
            );

            if let Some(phase) = deferred_phase.as_mut() {
                let mut key = MeshPipelineKey::from_msaa_samples(1) | primitive_topology_key;
                key |= MeshPipelineKey::DEFERRED_PREPASS;

                if normal_prepass.is_some() {
                    key |= MeshPipelineKey::NORMAL_PREPASS;
                }

                if motion_vector_prepass.is_some() {
                    key |= MeshPipelineKey::MOTION_VECTOR_PREPASS;
                }

                let pipeline_key = InstancedMaterialPipelineKey {
                    mesh_key: key,
                    bind_group_data: prepared_material.key.clone(),
                };

                let pipeline = pipelines
                    .specialize(
                        &pipeline_cache,
                        &prepass_pipeline,
                        pipeline_key,
                        &mesh.layout,
                    )
                    .unwrap();

                phase.add(
                    OpaqueNoLightmap3dBatchSetKey {
                        pipeline,
                        draw_function: prepared_material.draw_deferred,
                        material_bind_group_index: None,
                        slabs: MeshSlabs {
                            vertex_slab_id,
                            index_slab_id,
                            ..Default::default()
                        },
                    },
                    OpaqueNoLightmap3dBinKey {
                        asset_id: mesh_instance.mesh_asset_id().into(),
                    },
                    (*entity, *main_entity),
                    InputUniformIndex(0),
                    BinnedRenderPhaseType::UnbatchableMesh,
                );
            }

            if let Some(phase) = opaque_phase.as_mut() {
                if depth_prepass.is_none()
                    && normal_prepass.is_none()
                    && motion_vector_prepass.is_none()
                {
                    continue;
                }

                let mut key = base_key | primitive_topology_key;
                if depth_prepass.is_some() {
                    key |= MeshPipelineKey::DEPTH_PREPASS;
                }
                if normal_prepass.is_some() {
                    key |= MeshPipelineKey::NORMAL_PREPASS;
                }
                if motion_vector_prepass.is_some() {
                    key |= MeshPipelineKey::MOTION_VECTOR_PREPASS;
                }

                let pipeline_key = InstancedMaterialPipelineKey {
                    mesh_key: key,
                    bind_group_data: prepared_material.key.clone(),
                };

                let pipeline = pipelines
                    .specialize(
                        &pipeline_cache,
                        &prepass_pipeline,
                        pipeline_key,
                        &mesh.layout,
                    )
                    .unwrap();

                phase.add(
                    OpaqueNoLightmap3dBatchSetKey {
                        pipeline,
                        draw_function: prepared_material.draw_prepass,
                        material_bind_group_index: None,
                        slabs: MeshSlabs {
                            vertex_slab_id,
                            index_slab_id,
                            ..Default::default()
                        },
                    },
                    OpaqueNoLightmap3dBinKey {
                        asset_id: mesh_instance.mesh_asset_id().into(),
                    },
                    (*entity, *main_entity),
                    InputUniformIndex(0),
                    BinnedRenderPhaseType::UnbatchableMesh,
                );
            }
        }
    }
}
