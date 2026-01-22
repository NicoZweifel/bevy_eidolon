use bevy_ecs::system::{SystemParamItem, lifetimeless::SRes};
use bevy_render::render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass};
use std::marker::PhantomData;

use crate::allocator::prelude::*;
use crate::prelude::*;
#[cfg(feature = "trace")]
use tracing::error;

pub struct SetInstanceBindGroup<M: InstancedMaterial, const I: usize>(PhantomData<M>);

impl<P: PhaseItem, M: InstancedMaterial, const I: usize> RenderCommand<P>
    for SetInstanceBindGroup<M, I>
{
    type Param = (
        SRes<GlobalInstanceAllocator<M>>,
        SRes<MaterialBatchRanges<M>>,
    );
    type ViewQuery = ();
    type ItemQuery = ();

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        _entity: Option<()>,
        (allocator, batch_ranges): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let allocator = allocator.into_inner();
        let batch_ranges = batch_ranges.into_inner();
        let entity = item.entity();

        let Some(&batch_index) = batch_ranges.batch_lookup.get(&entity) else {
            #[cfg(feature = "trace")]
            error!(
                "SetInstanceBindGroup: No batch index found for entity {:?}!",
                entity
            );
            return RenderCommandResult::Skip;
        };

        if batch_index as usize >= batch_ranges.batches.len() {
            #[cfg(feature = "trace")]
            error!(
                "SetInstanceBindGroup: Batch index {} out of bounds!",
                batch_index
            );
            return RenderCommandResult::Skip;
        }

        let page_id = batch_ranges.batches[batch_index as usize].page;

        if page_id >= allocator.pages.len() {
            #[cfg(feature = "trace")]
            error!(
                "SetInstanceBindGroup: Page ID {} out of bounds (Total: {})",
                page_id,
                allocator.pages.len()
            );
            return RenderCommandResult::Skip;
        }

        let page = &allocator.pages[page_id];

        if let Some(bind_group) = &page.common_bind_group {
            pass.set_bind_group(I, bind_group, &[]);
        } else {
            #[cfg(feature = "trace")]
            error!(
                "SetInstanceBindGroup: Page {} missing common_bind_group!",
                page_id
            );
            return RenderCommandResult::Skip;
        }

        if let Some(output_buffer) = &page.output_buffer {
            pass.set_vertex_buffer(1, output_buffer.slice(..));
        } else {
            #[cfg(feature = "trace")]
            error!(
                "SetInstanceBindGroup: Page {} missing output_buffer!",
                page_id
            );
            return RenderCommandResult::Skip;
        }

        RenderCommandResult::Success
    }
}
