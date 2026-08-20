use crate::allocator::id_allocator::IdAllocator;
use crate::allocator::utils::ensure_buffer_capacity;
use crate::render::prepare::{core::Clear, utils::data_offset, *};

use bevy_ecs::entity::EntityHashMap;
use bevy_render::{
    render_resource::{Buffer, BufferUsages},
    renderer::{RenderDevice, RenderQueue},
};

/// Logic for resizing and updating raw Instance Data buffers.
pub(super) struct InstanceBufferUpdater<'a> {
    pub(super) device: &'a RenderDevice,
    pub(super) queue: &'a RenderQueue,
    pub(super) page_id: usize,
    pub(super) source_allocator: &'a mut InstanceAllocatorBackend,
    pub(super) material_name: &'static str,
}

impl<'a> InstanceBufferUpdater<'a> {
    pub fn update(
        &mut self,
        page: &mut InstancePage,
        writes: &[Write],
        entities: &Vec<Entity>,
        inputs: &EntityHashMap<BatchInput>,
    ) -> Vec<(u64, Vec<InstanceData>)> {
        let page_size = self.source_allocator.size(self.page_id) as u64;
        let capacity = page_size * size_of::<InstanceData>() as u64;

        self.ensure_capacity(page, capacity);

        let Some(source_buffer) = &page.source_buffer else {
            return Vec::new();
        };

        self.process_clears(source_buffer);
        self.process_writes(ProcessWriteContext {
            writes,
            source_buffer,
            inputs,
        });
        self.collect_output_writes(CollectOutputContext {
            entities,
            inputs,
            source_id_allocator: &page.id_allocator,
        })
    }

    fn ensure_capacity(&self, page: &mut InstancePage, capacity: u64) {
        let material_name = self.material_name;
        let page_id = self.page_id;

        self.ensure_buffer_capacity(
            &mut page.source_buffer,
            capacity,
            BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            &format!("material_{material_name}_page_{page_id}_source",),
            true,
        );

        self.ensure_buffer_capacity(
            &mut page.output_buffer,
            capacity,
            BufferUsages::STORAGE | BufferUsages::VERTEX | BufferUsages::COPY_DST,
            &format!("material_{material_name}_page_{page_id}_output"),
            false,
        );
    }
    fn ensure_buffer_capacity(
        &self,
        buffer: &mut Option<Buffer>,
        capacity: u64,
        usage: BufferUsages,
        label: &str,
        copy: bool,
    ) {
        ensure_buffer_capacity(
            self.device,
            self.queue,
            buffer,
            capacity,
            usage,
            label,
            copy,
        );
    }

    fn process_clears(&mut self, buffer: &Buffer) {
        for Clear { offset, clear } in self
            .source_allocator
            .drain(self.page_id)
            .into_iter()
            .map(Clear::from)
            .filter(|clear| buffer.check_bounds(clear.offset, &clear.clear))
        {
            self.queue
                .write_buffer(buffer, offset, bytemuck::cast_slice(&clear));
        }
    }

    /// Processes 'writes' to the source buffer.
    fn process_writes(&self, ctx: ProcessWriteContext<'_>) {
        for (offset, modified) in ctx.writes.iter().filter_map(|write| {
            ctx.inputs.get(&write.entity).map(|input| {
                (
                    write.offset,
                    input
                        .instances
                        .iter()
                        .map(|d| d.with_batch_id(write.batch_id))
                        .collect::<Vec<_>>(),
                )
            })
        }) {
            self.queue.write_buffer(
                ctx.source_buffer,
                data_offset::<InstanceData>(offset),
                bytemuck::cast_slice(&modified),
            );
        }
    }

    /// Collects writes to the output buffer for non-compute culling instances.
    fn collect_output_writes(
        &self,
        ctx: CollectOutputContext<'_>,
    ) -> Vec<(u64, Vec<InstanceData>)> {
        ctx.entities
            .iter()
            .filter_map(|e| ctx.inputs.get(e))
            .filter(|i| !i.gpu_cull)
            .filter_map(|input| {
                let alloc = self.source_allocator.get(input.entity)?;
                let batch_id = *ctx.source_id_allocator.allocations.get(&input.entity)?;
                Some((input, alloc, batch_id))
            })
            .map(|(input, alloc, batch_id)| {
                (
                    alloc.offset as u64 * size_of::<InstanceData>() as u64,
                    input
                        .instances
                        .iter()
                        .map(|d| d.with_batch_id(batch_id))
                        .collect::<Vec<_>>(),
                )
            })
            .collect()
    }
}

struct ProcessWriteContext<'a> {
    writes: &'a [Write],
    source_buffer: &'a Buffer,
    inputs: &'a EntityHashMap<BatchInput>,
}

struct CollectOutputContext<'a> {
    entities: &'a Vec<Entity>,
    inputs: &'a EntityHashMap<BatchInput>,
    source_id_allocator: &'a IdAllocator,
}
