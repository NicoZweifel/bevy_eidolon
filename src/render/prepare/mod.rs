mod batcher;
mod core;
mod instance_buffer_updater;
mod page_prepare_runner;
mod utils;

use crate::allocator::prelude::*;
use crate::cull::pipeline::InstancedComputePipeline;
use crate::prelude::*;
use crate::render::pipeline::InstancedMaterialPipeline;

use core::{BatchInput, CachedInputState, Write};
use page_prepare_runner::PagePrepareRunner;

use bevy_camera::primitives::Aabb;
use bevy_ecs::{entity::EntityHashMap, prelude::*};
use bevy_math::{Vec3, Vec3A};
use bevy_pbr::RenderMeshInstances;
use bevy_render::{
    mesh::RenderMesh,
    mesh::allocator::MeshAllocator,
    render_asset::RenderAssets,
    renderer::{RenderDevice, RenderQueue},
    sync_world::MainEntity,
};
use bevy_transform::components::GlobalTransform;
use bevy_utils::default;

use bevy_render::render_resource::PipelineCache;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::mem::size_of;
use std::sync::Arc;

pub(crate) fn prepare_instanced_material_buffers<M: InstancedMaterial>(
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut allocator: ResMut<GlobalInstanceAllocator<M>>,
    mut batch_ranges: ResMut<MaterialBatchRanges<M>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    meshes: Res<RenderAssets<RenderMesh>>,
    mesh_allocator: Res<MeshAllocator>,
    render_pipeline: Res<InstancedMaterialPipeline<M>>,
    compute_pipeline: Option<Res<InstancedComputePipeline<M>>>,
    query: Query<(
        Entity,
        &MainEntity,
        &InstanceMaterialData,
        &GlobalTransform,
        Option<&GpuCullCompute>,
        Option<&Aabb>,
    )>,
    instanced_materials: Query<&InstancedMeshMaterial<M>>,
    mut cached_state: Local<EntityHashMap<CachedInputState>>,
    pipeline_cache: Res<PipelineCache>,
) {
    let CollectInputResult {
        inputs,
        dirty_entities,
        active_entities,
    } = collect_inputs(
        &query,
        &instanced_materials,
        &render_mesh_instances,
        &mut cached_state,
    );

    let allocator = &mut *allocator;
    let source_allocator = &mut allocator.backend;
    let pages = &mut allocator.pages;

    process_removals(&active_entities, &mut cached_state, source_allocator, pages);

    let ProcessedAllocations { pending, entities } =
        process_allocations(&dirty_entities, &inputs, source_allocator, pages);

    batch_ranges.clear();

    let count = source_allocator.page_count();
    let pipeline_cache = pipeline_cache.into_inner();

    for (id, entities) in (0..count).filter_map(|id| entities.get(&id).map(|e| (id, e))) {
        let empty = Vec::new();
        let writes = pending.get(&id).unwrap_or(&empty);

        let page = &mut allocator.pages[id];
        let mut runner = PagePrepareRunner {
            id,
            page,
            source_allocator,
            pipeline_cache,
            batch_ranges: &mut batch_ranges,
            device: &render_device,
            queue: &render_queue,
            meshes: &meshes,
            mesh_allocator: &mesh_allocator,
            compute_layout: compute_pipeline.as_ref().map(|p| &p.compute_layout),
            common_layout: &render_pipeline.common_layout,
            material_name: std::any::type_name::<M>(),
        };

        runner.prepare(entities, writes, &inputs);
    }
}

struct CollectInputResult {
    inputs: EntityHashMap<BatchInput>,
    dirty_entities: Vec<Entity>,
    active_entities: BTreeSet<Entity>,
}

fn collect_inputs<M: InstancedMaterial>(
    query: &Query<(
        Entity,
        &MainEntity,
        &InstanceMaterialData,
        &GlobalTransform,
        Option<&GpuCullCompute>,
        Option<&Aabb>,
    )>,
    instanced_materials: &Query<&InstancedMeshMaterial<M>>,
    render_mesh_instances: &RenderMeshInstances,
    cached_state: &mut Local<EntityHashMap<CachedInputState>>,
) -> CollectInputResult {
    let mut dirty_entities = Vec::new();
    let mut active_entities = BTreeSet::new();

    let default_aabb = Aabb {
        center: Vec3A::ZERO,
        half_extents: Vec3A::splat(f32::MAX),
    };

    let inputs = query
        .iter()
        .filter(|(_, _, instance_data, ..)| !instance_data.instances.is_empty())
        .filter_map(|data| {
            let (entity, main_entity, ..) = data;
            let mesh_instance = render_mesh_instances.render_mesh_queue_data(*main_entity)?;
            let material = instanced_materials.get(entity).ok()?;

            Some((data, mesh_instance, material))
        })
        .fold(
            EntityHashMap::default(),
            |mut acc, (data, mesh_instance, material)| {
                let (entity, main_entity, instance_data, gtf, gpu_cull, aabb) = data;

                active_entities.insert(entity);

                let batch_key = BatchKey {
                    material: material.id(),
                    mesh: mesh_instance.mesh_asset_id,
                    gpu_cull: gpu_cull.is_some(),
                };

                let mut hasher = DefaultHasher::new();
                batch_key.hash(&mut hasher);

                let key_hash = hasher.finish();
                let dirty = cached_state
                    .get(&entity)
                    .map(|cached| {
                        cached.key_hash != key_hash
                            || !Arc::ptr_eq(&cached.instances, &instance_data.instances)
                    })
                    .unwrap_or(true);

                if dirty {
                    cached_state.insert(
                        entity,
                        CachedInputState {
                            instances: instance_data.instances.clone(),
                            key_hash,
                        },
                    );
                    dirty_entities.push(entity);
                }

                let Aabb {
                    center,
                    half_extents,
                } = aabb
                    .map(|aabb| {
                        let transform = gtf.to_matrix();
                        let center = transform
                            .transform_point3(Vec3::from(aabb.center))
                            .to_vec3a();

                        let x = transform.transform_vector3(Vec3::X * aabb.half_extents.x);
                        let y = transform.transform_vector3(Vec3::Y * aabb.half_extents.y);
                        let z = transform.transform_vector3(Vec3::Z * aabb.half_extents.z);
                        let half_extents = (x.abs() + y.abs() + z.abs()).to_vec3a();

                        Aabb {
                            center,
                            half_extents,
                        }
                    })
                    .unwrap_or(default_aabb);

                let input = BatchInput {
                    entity,
                    main_entity: *main_entity,
                    mesh_id: mesh_instance.mesh_asset_id,
                    instances: instance_data.instances.clone(),
                    gpu_cull: gpu_cull.is_some(),
                    batch: InstanceUniforms {
                        world_from_local: gtf.to_matrix(),
                        previous_world_from_local: gtf.to_matrix(),
                        aabb_center: center.extend(0.0),
                        aabb_half_extents: half_extents.extend(0.0),
                        ..instance_data.into()
                    },
                };
                acc.insert(entity, input);
                acc
            },
        );

    CollectInputResult {
        inputs,
        dirty_entities,
        active_entities,
    }
}

fn process_removals(
    active_entities: &BTreeSet<Entity>,
    cached_state: &mut Local<EntityHashMap<CachedInputState>>,
    source_allocator: &mut InstanceAllocatorBackend,
    pages: &mut [InstancePage],
) {
    for entity in cached_state
        .keys()
        .filter(|e| !active_entities.contains(e))
        .cloned()
        .collect::<Vec<_>>()
    {
        free(source_allocator, pages, entity);
        cached_state.remove(&entity);
    }
}

fn free(
    source_allocator: &mut InstanceAllocatorBackend,
    pages: &mut [InstancePage],
    entity: Entity,
) {
    let Some(InstanceAllocation { page, .. }) = source_allocator.get(entity) else {
        return;
    };

    if page < pages.len() {
        pages[page].free(entity);
    }

    source_allocator.free(entity);
}

fn process_allocations(
    dirty_entities: &[Entity],
    inputs: &EntityHashMap<BatchInput>,
    source_allocator: &mut InstanceAllocatorBackend,
    pages: &mut Vec<InstancePage>,
) -> ProcessedAllocations {
    let pending = process_dirty_entities(dirty_entities, inputs, source_allocator, pages);

    sync_pages(pages, source_allocator);

    let entities: BTreeMap<usize, Vec<Entity>> = inputs
        .keys()
        .map(|entity| {
            (
                source_allocator
                    .get(*entity)
                    .unwrap_or_else(|| {
                        source_allocator
                            .alloc(
                                *entity,
                                inputs
                                    .get(entity)
                                    .expect("Entity should exist!")
                                    .instances
                                    .len() as u32,
                            )
                            .expect("Instance Allocator out of nodes!")
                    })
                    .page,
                entity,
            )
        })
        .fold(BTreeMap::new(), |mut acc, (page, entity)| {
            acc.entry(page).or_default().push(*entity);
            acc
        });

    ProcessedAllocations { pending, entities }
}

struct ProcessedAllocations {
    pending: BTreeMap<usize, Vec<Write>>,
    entities: BTreeMap<usize, Vec<Entity>>,
}

fn process_dirty_entities(
    dirty_entities: &[Entity],
    inputs: &EntityHashMap<BatchInput>,
    source_allocator: &mut InstanceAllocatorBackend,
    pages: &mut Vec<InstancePage>,
) -> BTreeMap<usize, Vec<Write>> {
    dirty_entities
        .iter()
        .filter_map(|entity| inputs.get(entity))
        .fold(BTreeMap::new(), |mut acc, input| {
            let entity = input.entity;

            free(source_allocator, pages, entity);

            let InstanceAllocation { page, offset } = source_allocator
                .alloc(entity, input.instances.len() as u32)
                .expect("Instance Allocator out of nodes!");

            sync_pages(pages, source_allocator);

            let batch_id = pages[page].alloc(entity);

            acc.entry(page).or_default().push(Write {
                entity,
                offset,
                batch_id,
            });
            acc
        })
}

fn sync_pages(pages: &mut Vec<InstancePage>, source_allocator: &InstanceAllocatorBackend) {
    if pages.len() < source_allocator.page_count() {
        pages.resize_with(source_allocator.page_count(), default);
    }
}
