use crate::allocator::prelude::MaterialBatchRanges;
use crate::prelude::*;
use crate::render::{
    pipeline::{InstancedMaterialPipeline, InstancedMaterialPipelineKey},
    prepared_material::PreparedInstancedMaterial,
};

use bevy_core_pipeline::{
    core_3d::{Opaque3d, Opaque3dBatchSetKey, Opaque3dBinKey},
    prepass::{DeferredPrepass, DepthPrepass, MotionVectorPrepass, NormalPrepass},
    tonemapping::{DebandDither, Tonemapping},
};
use bevy_ecs::{prelude::*, system::SystemChangeTick};
use bevy_pbr::{
    ExtractedAtmosphere, MeshPipelineKey, RenderMeshInstances, ScreenSpaceAmbientOcclusion,
};
use bevy_render::{
    batching::gpu_preprocessing::GpuPreprocessingSupport,
    mesh::RenderMesh,
    mesh::allocator::MeshAllocator,
    render_asset::RenderAssets,
    render_phase::{BinnedRenderPhaseType, InputUniformIndex, ViewBinnedRenderPhases},
    render_resource::*,
    view::ExtractedView,
    view::Msaa,
};

use bevy_camera::visibility::RenderLayers;
use bevy_light::EnvironmentMapLight;
use std::hash::{Hash, Hasher};
#[cfg(feature = "trace")]
use tracing::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn queue_instanced_material<M>(
    custom_pipeline: Res<InstancedMaterialPipeline<M>>,
    mut pipelines: ResMut<SpecializedMeshPipelines<InstancedMaterialPipeline<M>>>,
    pipeline_cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<RenderMesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    render_material_instances: Res<RenderInstancedMaterialInstances>,
    render_materials: Res<RenderAssets<PreparedInstancedMaterial<M>>>,
    mesh_allocator: Res<MeshAllocator>,
    _gpu_preprocessing_support: Res<GpuPreprocessingSupport>,
    mut opaque_render_phases: ResMut<ViewBinnedRenderPhases<Opaque3d>>,
    ticks: SystemChangeTick,
    views: Query<(
        &ExtractedView,
        &Msaa,
        (
            (
                Has<DepthPrepass>,
                Has<NormalPrepass>,
                Has<MotionVectorPrepass>,
                Has<DeferredPrepass>,
            ),
            Has<EnvironmentMapLight>,
            Has<ScreenSpaceAmbientOcclusion>,
            Option<&Tonemapping>,
            Option<&DebandDither>,
            Option<&RenderLayers>,
            Has<ExtractedAtmosphere>,
        ),
    )>,
    batch_ranges: Res<MaterialBatchRanges<M>>,
) where
    M: InstancedMaterial,
    M::Data: PartialEq + Eq + Hash + Clone,
{
    for (
        view,
        msaa,
        (
            (depth, normal, motion, deferred),
            env_map,
            ssao,
            tonemapping,
            deband,
            view_layers,
            atmosphere,
        ),
    ) in &views
    {
        let Some(opaque_mask_phases) = opaque_render_phases.get_mut(&view.retained_view_entity)
        else {
            continue;
        };

        let mut view_key = MeshPipelineKey::from_msaa_samples(msaa.samples())
            | MeshPipelineKey::from_hdr(view.hdr);

        if env_map {
            view_key |= MeshPipelineKey::ENVIRONMENT_MAP;
        }

        if atmosphere {
            view_key |= MeshPipelineKey::ATMOSPHERE;
        }

        if ssao {
            view_key |= MeshPipelineKey::SCREEN_SPACE_AMBIENT_OCCLUSION;
        }
        if let Some(tonemapping) = tonemapping {
            if *tonemapping != Tonemapping::None {
                view_key |= MeshPipelineKey::TONEMAP_IN_SHADER;
            }
        }

        if let Some(deband) = deband {
            if *deband != DebandDither::Disabled {
                view_key |= MeshPipelineKey::DEBAND_DITHER;
            }
        }

        if depth {
            view_key |= MeshPipelineKey::DEPTH_PREPASS;
        }
        if normal {
            view_key |= MeshPipelineKey::NORMAL_PREPASS;
        }
        if motion {
            view_key |= MeshPipelineKey::MOTION_VECTOR_PREPASS;
        }
        if deferred {
            view_key |= MeshPipelineKey::DEFERRED_PREPASS;
        }

        #[cfg(feature = "trace")]
        if !batch_ranges.entities.is_empty() {
            trace!("queue: processing {} batches", batch_ranges.entities.len());
        }

        for (entity, main_entity, prepared_material, mesh, mesh_instance) in batch_ranges
            .entities
            .iter()
            .filter_map(|(entity, main_entity)| {
                #[cfg(feature = "trace")]
                trace!(
                    "queue_instanced_material: \n  - render: {entity:?}\n  - main: {main_entity:?}"
                );

                let material_instance = render_material_instances.instances.get(main_entity)?;
                let prepared_material = render_materials.get(material_instance.asset_id.typed())?;
                let mesh_instance = render_mesh_instances.render_mesh_queue_data(*main_entity)?;
                let mesh = meshes.get(mesh_instance.mesh_asset_id)?;

                view_layers
                    .unwrap_or_default()
                    .intersects(&mesh_instance.render_layers.clone().unwrap_or_default())
                    .then(|| (entity, main_entity, prepared_material, mesh, mesh_instance))
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
                    draw_function: prepared_material.draw_opaque,
                    material_bind_group_index: Some(material_index),
                    vertex_slab: vertex_slab.unwrap_or_default(),
                    index_slab,
                    lightmap_slab: None,
                },
                Opaque3dBinKey {
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
