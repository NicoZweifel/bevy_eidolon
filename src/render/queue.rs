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
    view::ExtractedView,
    view::Msaa,
};

use bevy_mesh::Mesh3d;
use bevy_render::view::RenderVisibleEntities;
use std::hash::{Hash, Hasher};

#[cfg(feature = "trace")]
use tracing::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn queue_instanced_material<M>(
    opaque_3d_draw_functions: Res<DrawFunctions<Opaque3d>>,
    custom_pipeline: Res<InstancedMaterialPipeline<M>>,
    mut pipelines: ResMut<SpecializedMeshPipelines<InstancedMaterialPipeline<M>>>,
    pipeline_cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<RenderMesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    render_material_instances: Res<RenderInstancedMaterialInstances>,
    render_materials: Res<RenderAssets<PreparedInstancedMaterial<M>>>,
    mesh_allocator: Res<MeshAllocator>,
    gpu_preprocessing_support: Res<GpuPreprocessingSupport>,
    mut opaque_render_phases: ResMut<ViewBinnedRenderPhases<Opaque3d>>,
    ticks: SystemChangeTick,
    views: Query<(
        &ExtractedView,
        &RenderVisibleEntities,
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

    for (view, visible_entities, msaa, depth_prepass, normal_prepass, motion_vector_prepass) in
        &views
    {
        let Some(opaque_mask_phases) = opaque_render_phases.get_mut(&view.retained_view_entity)
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

        for (entity, main_entity, prepared_material, mesh, mesh_instance) in visible_entities
            .iter::<Mesh3d>()
            .filter_map(|(entity, main_entity)| {
                #[cfg(feature = "trace")]
                trace!(
                    "queue_instanced_material: \n  - render: {entity:?}\n  - main: {main_entity:?}"
                );

                let material_instance = render_material_instances.instances.get(main_entity)?;
                let prepared_material = render_materials.get(material_instance.asset_id.typed())?;
                let mesh_instance = render_mesh_instances.render_mesh_queue_data(*main_entity)?;
                let mesh = meshes.get(mesh_instance.mesh_asset_id)?;

                Some((entity, main_entity, prepared_material, mesh, mesh_instance))
            })
        {
            let key = InstancedMaterialPipelineKey {
                mesh_key: view_key
                    | MeshPipelineKey::from_primitive_topology(mesh.primitive_topology()),
                bind_group_data: prepared_material.key.clone(),
            };

            let pipeline = pipelines
                .specialize(&pipeline_cache, &custom_pipeline, key, &mesh.layout)
                .unwrap();

            let (vertex_slab, index_slab) = mesh_allocator.mesh_slabs(&mesh_instance.mesh_asset_id);

            let material_instance = render_material_instances
                .instances
                .get(main_entity)
                .unwrap();
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            material_instance.asset_id.hash(&mut hasher);
            let material_index = hasher.finish() as u32;

            #[cfg(feature = "trace")]
            trace!(
                "queue_instanced_material: vertex_slab: {:?}, index_slab: {:?}",
                vertex_slab, index_slab
            );

            opaque_mask_phases.add(
                Opaque3dBatchSetKey {
                    pipeline,
                    draw_function: draw_custom,
                    material_bind_group_index: Some(material_index),
                    vertex_slab: vertex_slab.unwrap_or_default(),
                    index_slab,
                    lightmap_slab: None,
                },
                Opaque3dBinKey {
                    asset_id: mesh_instance.mesh_asset_id.into(),
                },
                (*entity, *main_entity),
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
