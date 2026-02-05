use bevy_ecs::system::{SystemParamItem, lifetimeless::SRes};
use bevy_pbr::RenderMeshInstances;
use bevy_render::{
    mesh::allocator::MeshAllocator,
    mesh::{RenderMesh, RenderMeshBufferInfo},
    render_asset::RenderAssets,
    render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass},
    render_resource::DrawIndexedIndirectArgs,
};

use std::marker::PhantomData;

#[cfg(feature = "trace")]
use tracing::error;

use crate::allocator::prelude::*;
use crate::prelude::*;

pub struct DrawInstancedMaterialMesh<M: InstancedMaterial>(PhantomData<M>);

impl<P, M> RenderCommand<P> for DrawInstancedMaterialMesh<M>
where
    P: PhaseItem,
    M: InstancedMaterial,
{
    type Param = (
        SRes<RenderAssets<RenderMesh>>,
        SRes<RenderMeshInstances>,
        SRes<MeshAllocator>,
        SRes<MaterialBatchRanges<M>>,
        SRes<GlobalInstanceAllocator<M>>,
    );

    type ViewQuery = ();
    type ItemQuery = ();

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        _item_query: Option<()>,
        (meshes, render_mesh_instances, mesh_allocator, batch_ranges, allocator): SystemParamItem<
            'w,
            '_,
            Self::Param,
        >,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let main_entity = item.main_entity();
        let entity = item.entity();

        let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(main_entity) else {
            #[cfg(feature = "trace")]
            error!("No mesh_instance for entity {:?}", main_entity);
            return RenderCommandResult::Skip;
        };

        let Some(gpu_mesh) = meshes.into_inner().get(mesh_instance.mesh_asset_id) else {
            #[cfg(feature = "trace")]
            error!("No gpu_mesh for asset {:?}", mesh_instance.mesh_asset_id);
            return RenderCommandResult::Skip;
        };

        let mesh_allocator = mesh_allocator.into_inner();
        let Some(vertex_buffer_slice) =
            mesh_allocator.mesh_vertex_slice(&mesh_instance.mesh_asset_id)
        else {
            #[cfg(feature = "trace")]
            error!(
                "No vertex_buffer_slice for asset {:?}",
                mesh_instance.mesh_asset_id
            );
            return RenderCommandResult::Skip;
        };

        pass.set_vertex_buffer(0, vertex_buffer_slice.buffer.slice(..));

        let batch_ranges = batch_ranges.into_inner();
        let allocator = allocator.into_inner();

        let Some(&batch_index) = batch_ranges.batch_lookup.get(&entity) else {
            #[cfg(feature = "trace")]
            error!("No batch index found for entity {:?}", entity);
            return RenderCommandResult::Skip;
        };

        if batch_index as usize >= batch_ranges.batches.len() {
            #[cfg(feature = "trace")]
            error!("Batch index {} out of bounds!", batch_index);
            return RenderCommandResult::Skip;
        }

        let batch_info = &batch_ranges.batches[batch_index as usize];
        let page_id = batch_info.page;

        if page_id >= allocator.pages.len() {
            #[cfg(feature = "trace")]
            error!("Page ID {} out of bounds!", page_id);
            return RenderCommandResult::Skip;
        }
        let page = &allocator.pages[page_id];

        let Some(indirect_buffer) = &page.indirect_buffer else {
            #[cfg(feature = "trace")]
            error!("Page {} missing indirect_buffer!", page_id);
            return RenderCommandResult::Skip;
        };

        let range = &batch_info.range;

        match &gpu_mesh.buffer_info {
            RenderMeshBufferInfo::Indexed { index_format, .. } => {
                let Some(index_buffer_slice) =
                    mesh_allocator.mesh_index_slice(&mesh_instance.mesh_asset_id)
                else {
                    #[cfg(feature = "trace")]
                    error!(
                        "No index_buffer_slice for asset {:?}",
                        mesh_instance.mesh_asset_id
                    );
                    return RenderCommandResult::Skip;
                };

                pass.set_index_buffer(index_buffer_slice.buffer.slice(..), *index_format);

                let count = range.end - range.start;
                let offset = (range.start as u64) * size_of::<DrawIndexedIndirectArgs>() as u64;

                pass.multi_draw_indexed_indirect(indirect_buffer, offset, count);
            }
            RenderMeshBufferInfo::NonIndexed => {
                #[cfg(feature = "trace")]
                error!("NonIndexed meshes not supported");
            }
        }
        RenderCommandResult::Success
    }
}
