use crate::allocator::batch_buffer::BatchBuffer;
use crate::allocator::id_allocator::IdAllocator;
use crate::material::InstancedMaterial;

use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::mem::size_of;
use std::ops::Range;

use bevy_asset::{Asset, AssetId};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::entity::EntityHashMap;
use bevy_ecs::prelude::*;
use bevy_mesh::Mesh;
use bevy_render::{
    render_resource::{
        BindGroup, BindGroupEntry, BindGroupLayout, Buffer, BufferDescriptor, BufferUsages,
        ShaderType,
    },
    renderer::{RenderDevice, RenderQueue},
    sync_world::MainEntity,
};
use bevy_utils::default;
use bytemuck::{Pod, Zeroable};

use crate::prelude::InstanceAllocatorBackend;
#[cfg(feature = "trace")]
use tracing::trace;

/// Ensures a GPU buffer has sufficient capacity, resizing and copying data if necessary.
pub fn ensure_buffer_capacity(
    device: &RenderDevice,
    queue: &RenderQueue,
    buffer_opt: &mut Option<Buffer>,
    capacity: u64,
    usage: BufferUsages,
    label: &str,
    copy: bool,
) {
    let aligned_size = (capacity + 3) & !3; // 4-byte align
    let current_size = buffer_opt.as_ref().map(|b| b.size()).unwrap_or(0);
    if aligned_size <= current_size {
        return;
    }

    let size = aligned_size.max(1024);

    #[cfg(feature = "trace")]
    trace!(
        "Resizing Buffer [{}]: {} bytes -> {} bytes (Copy: {})",
        label, current_size, size, copy
    );

    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size,
        usage,
        mapped_at_creation: false,
    });

    if copy && let Some(old) = buffer_opt {
        let mut encoder = device.create_command_encoder(&default());
        let copy_size = old.size().min(size);

        encoder.copy_buffer_to_buffer(old, 0, &buffer, 0, copy_size);

        queue.submit(Some(encoder.finish()));
    }

    *buffer_opt = Some(buffer);
}

#[derive(Clone, Copy)]
pub struct BatchKey<M: Asset> {
    pub material: AssetId<M>,
    pub mesh: AssetId<Mesh>,
    pub gpu_cull: bool,
}

impl<M: InstancedMaterial> Hash for BatchKey<M> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.material.hash(state);
        self.mesh.hash(state);
        self.gpu_cull.hash(state);
    }
}

impl<M: InstancedMaterial> PartialEq<Self> for BatchKey<M> {
    fn eq(&self, other: &Self) -> bool {
        self.material == other.material
            && self.mesh == other.mesh
            && self.gpu_cull == other.gpu_cull
    }
}

impl<M: InstancedMaterial> Eq for BatchKey<M> {}

#[derive(Clone, Debug)]
pub struct BatchInfo {
    pub page: usize,
    pub range: Range<u32>,
}

#[derive(Default)]
pub struct BatchRanges {
    pub batches: Vec<BatchInfo>,
    pub representatives: Vec<(Entity, MainEntity)>,
    pub batch_lookup: EntityHashMap<u32>,
}

impl BatchRanges {
    pub fn clear(&mut self) {
        self.batches.clear();
        self.representatives.clear();
        self.batch_lookup.clear();
    }
}

#[derive(Resource, Deref, DerefMut)]
pub struct MaterialBatchRanges<M>(#[deref] pub BatchRanges, PhantomData<M>);

impl<M: InstancedMaterial> Default for MaterialBatchRanges<M> {
    fn default() -> Self {
        Self(BatchRanges::default(), PhantomData)
    }
}

impl<T: InstancedMaterial> MaterialBatchRanges<T> {
    pub fn clear(&mut self) {
        (**self).clear();
    }
}

#[derive(Clone, Copy, Pod, Zeroable, Default, ShaderType, Debug)]
#[repr(C)]
pub struct BatchMetadata {
    pub batch_id: u32,
    pub start_index: u32,
    pub end_index: u32,
    pub lod_group_index: u32,
}

#[derive(Default)]
pub struct InstancePage {
    pub source_buffer: Option<Buffer>,
    pub output_buffer: Option<Buffer>,
    pub indirect_buffer: Option<Buffer>,
    pub metadata_buffer: Option<Buffer>,
    pub batch_buffer: Option<Buffer>,

    pub compute_bind_group: Option<BindGroup>,
    pub common_bind_group: Option<BindGroup>,

    pub compute_capacity: u32,

    pub batcher: BatchBuffer,
    pub id_allocator: IdAllocator,
}

impl InstancePage {
    /// Allocates a batch ID for the given entity.
    pub fn alloc(&mut self, entity: Entity) -> u32 {
        self.id_allocator.alloc(entity)
    }

    /// Frees the batch ID for the given entity and clears the associated batch data.
    pub fn free(&mut self, entity: Entity) {
        if let Some(id) = self.id_allocator.allocations.get(&entity).copied() {
            self.id_allocator.free(entity);
            self.batcher.clear(id as usize);
        }
    }

    /// Flushes the internal batcher to the GPU buffers, resizing if necessary.
    pub fn flush(&mut self, device: &RenderDevice, queue: &RenderQueue, page_id: usize) {
        let batch_capacity = self.batcher.capacity() as u64;
        let mut resized = false;

        let mut check_resize = |buffer: &mut Option<Buffer>, size: u64, usage, label: String| {
            let old_size = buffer.as_ref().map(|b| b.size()).unwrap_or(0);
            let aligned = (size + 3) & !3;
            if aligned > old_size {
                ensure_buffer_capacity(device, queue, buffer, size, usage, &label, false);
                resized = true;
            }
        };

        check_resize(
            &mut self.batch_buffer,
            batch_capacity * size_of::<crate::components::InstanceUniforms>() as u64,
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            format!("page_{}_batch", page_id),
        );

        check_resize(
            &mut self.indirect_buffer,
            batch_capacity
                * size_of::<bevy_render::render_resource::DrawIndexedIndirectArgs>() as u64,
            BufferUsages::STORAGE | BufferUsages::INDIRECT | BufferUsages::COPY_DST,
            format!("page_{}_indirect", page_id),
        );

        check_resize(
            &mut self.metadata_buffer,
            batch_capacity * size_of::<BatchMetadata>() as u64,
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            format!("page_{}_metadata", page_id),
        );

        if resized {
            self.batcher.mark_dirty();
        }

        if let (Some(ind), Some(batch), Some(meta)) = (
            &self.indirect_buffer,
            &self.batch_buffer,
            &self.metadata_buffer,
        ) {
            self.batcher.flush(queue, ind, batch, meta);
        }
    }

    /// Recreates bind groups if capacity or buffers have changed.
    pub fn update(
        &mut self,
        device: &RenderDevice,
        page_id: usize,
        common_layout: &BindGroupLayout,
        compute_layout: Option<&BindGroupLayout>,
        capacity: u32,
    ) {
        self.compute_capacity = capacity;

        if let Some(layout) = compute_layout
            && self.source_buffer.is_some()
            && self.metadata_buffer.is_some()
        {
            self.compute_bind_group = Some(device.create_bind_group(
                Some(format!("page_{}_compute_bind_group", page_id).as_str()),
                layout,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: self.source_buffer.as_ref().unwrap().as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: self.output_buffer.as_ref().unwrap().as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: self.indirect_buffer.as_ref().unwrap().as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: self.metadata_buffer.as_ref().unwrap().as_entire_binding(),
                    },
                ],
            ));
        }

        if let Some(batch_buffer) = &self.batch_buffer {
            self.common_bind_group = Some(device.create_bind_group(
                Some(format!("page_{}_common_bind_group", page_id).as_str()),
                common_layout,
                &[BindGroupEntry {
                    binding: 0,
                    resource: batch_buffer.as_entire_binding(),
                }],
            ));
        }
    }
}

#[derive(Resource)]
pub struct GlobalInstanceAllocator<M> {
    pub pages: Vec<InstancePage>,
    pub backend: InstanceAllocatorBackend,
    pub _marker: PhantomData<M>,
}

impl<M: InstancedMaterial> Default for GlobalInstanceAllocator<M> {
    fn default() -> Self {
        Self {
            pages: Vec::new(),
            backend: InstanceAllocatorBackend::default(),
            _marker: PhantomData,
        }
    }
}
