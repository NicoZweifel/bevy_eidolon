use crate::allocator::{batch_buffer::BatchData, id_allocator::IdAllocator};
use crate::prelude::*;
use crate::render::prepare::core::BatchInput;

use bevy_ecs::entity::{Entity, EntityHashMap};

use bevy_render::{
    mesh::{RenderMesh, RenderMeshBufferInfo, allocator::MeshAllocator},
    render_asset::RenderAssets,
    render_resource::DrawIndexedIndirectArgs,
};

pub(super) struct Batcher<'a> {
    pub(super) page_id: usize,
    pub(super) mesh_allocator: &'a MeshAllocator,
    pub(super) meshes: &'a RenderAssets<RenderMesh>,
    pub(super) source_allocator: &'a InstanceAllocatorBackend,
    pub(super) batch_ranges: &'a mut BatchRanges,
}

impl Batcher<'_> {
    pub fn batch(
        &mut self,
        batcher: &mut crate::allocator::batch_buffer::BatchBuffer,
        page_entities: &Vec<Entity>,
        input_map: &EntityHashMap<BatchInput>,
        batch_allocator: &IdAllocator,
    ) {
        let max_batch = batch_allocator.watermark as usize;

        batcher.ensure_capacity(max_batch);

        for entity in page_entities {
            let Some(input) = input_map.get(entity) else {
                continue;
            };

            let Some(&index) = batch_allocator.allocations.get(&input.entity) else {
                continue;
            };

            let Some(mesh) = self.meshes.get(input.mesh_id) else {
                continue;
            };

            let RenderMeshBufferInfo::Indexed {
                count: index_count, ..
            } = mesh.buffer_info
            else {
                continue;
            };

            let offset = self
                .source_allocator
                .get_location(input.entity)
                .map(|l| l.offset)
                .unwrap_or(0);

            let first_index = self
                .mesh_allocator
                .mesh_index_slice(&input.mesh_id)
                .map(|s| s.range.start)
                .unwrap_or(0);

            let base_vertex = self
                .mesh_allocator
                .mesh_vertex_slice(&input.mesh_id)
                .map(|s| s.range.start)
                .unwrap_or(0) as i32;

            self.register(input, index);

            let instance_count = if input.gpu_cull {
                0
            } else {
                input.instances.len() as u32
            };

            let batch_data = BatchData {
                indirect: DrawIndexedIndirectArgs {
                    index_count,
                    instance_count,
                    first_index,
                    base_vertex,
                    first_instance: offset,
                },
                uniform: input.batch,
                metadata: BatchMetadata {
                    batch_id: index,
                    start_index: offset,
                    end_index: offset + input.instances.len() as u32,
                    lod_group_index: index,
                },
            };

            batcher.update(index as usize, batch_data);
        }
    }

    fn register(&mut self, input: &BatchInput, batch: u32) {
        let range = batch..(batch + 1);
        self.batch_ranges.batches.push(BatchInfo {
            page: self.page_id,
            range,
        });

        let index = (self.batch_ranges.batches.len() - 1) as u32;
        self.batch_ranges.batch_lookup.insert(input.entity, index);
        self.batch_ranges
            .representatives
            .push((input.entity, input.main_entity));
    }
}
