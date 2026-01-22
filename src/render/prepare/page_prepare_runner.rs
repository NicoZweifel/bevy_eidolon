use crate::components::InstanceData;
use crate::prelude::{
    BatchRanges, BufferBoundsCheck, InstanceAllocator, InstanceAllocatorBackend, InstancePage,
};
use crate::render::prepare::{
    batch_builder::Batcher, core::BatchInput, core::Write,
    instance_buffer_updater::InstanceBufferUpdater,
};

use bevy_ecs::entity::{Entity, EntityHashMap};
use bevy_render::{
    mesh::RenderMesh,
    mesh::allocator::MeshAllocator,
    render_asset::RenderAssets,
    render_resource::BindGroupLayout,
    renderer::{RenderDevice, RenderQueue},
};

pub(super) struct PagePrepareRunner<'a> {
    pub(super) id: usize,
    pub(super) page: &'a mut InstancePage,
    pub(super) source_allocator: &'a mut InstanceAllocatorBackend,
    pub(super) batch_ranges: &'a mut BatchRanges,

    pub(super) device: &'a RenderDevice,
    pub(super) queue: &'a RenderQueue,
    pub(super) meshes: &'a RenderAssets<RenderMesh>,
    pub(super) mesh_allocator: &'a MeshAllocator,

    pub(super) common_layout: &'a BindGroupLayout,
    pub(super) compute_layout: Option<&'a BindGroupLayout>,

    pub(super) material_name: &'static str,
}

impl<'a> PagePrepareRunner<'a> {
    pub fn prepare(
        &mut self,
        page_entities: &Vec<Entity>,
        writes: &[Write],
        all_inputs: &EntityHashMap<BatchInput>,
    ) {
        let output_writes = self.update(page_entities, writes, all_inputs);

        self.batch(page_entities, all_inputs);
        self.write_output(output_writes);
        self.flush();
        self.update_page();
    }

    /// Write non-compute culled instances to the output buffer.
    fn write_output(&mut self, writes: Vec<(u64, Vec<InstanceData>)>) {
        let Some(buffer) = &self.page.output_buffer else {
            return;
        };

        for (offset, data) in writes
            .into_iter()
            .filter(|(offset, data)| buffer.check_bounds(*offset, data))
        {
            self.queue
                .write_buffer(buffer, offset, bytemuck::cast_slice(&data));
        }
    }

    fn update(
        &mut self,
        entities: &Vec<Entity>,
        writes: &[Write],
        inputs: &EntityHashMap<BatchInput>,
    ) -> Vec<(u64, Vec<InstanceData>)> {
        let material_name = self.material_name;
        let mut instance_updater = InstanceBufferUpdater {
            device: self.device,
            queue: self.queue,
            page_id: self.id,
            source_allocator: self.source_allocator,
            material_name,
        };

        instance_updater.update(self.page, writes, entities, inputs)
    }

    fn batch(&mut self, entities: &Vec<Entity>, inputs: &EntityHashMap<BatchInput>) {
        if entities.is_empty() {
            return;
        }

        let mut batcher = Batcher {
            page_id: self.id,
            mesh_allocator: self.mesh_allocator,
            meshes: self.meshes,
            source_allocator: self.source_allocator,
            batch_ranges: self.batch_ranges,
        };

        batcher.batch(
            &mut self.page.batcher,
            entities,
            inputs,
            &self.page.id_allocator,
        );
    }

    fn flush(&mut self) {
        self.page.flush(self.device, self.queue, self.id);
    }

    fn update_page(&mut self) {
        self.page.update(
            self.device,
            self.id,
            self.common_layout,
            self.compute_layout,
            self.source_allocator.size(self.id),
        );
    }
}
