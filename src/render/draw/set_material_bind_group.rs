use bevy_ecs::system::{SystemParamItem, lifetimeless::SRes};
use bevy_render::{
    render_asset::RenderAssets,
    render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass},
};
use std::marker::PhantomData;

#[cfg(feature = "trace")]
use tracing::error;

use crate::prelude::*;
use crate::render::prepared_material::PreparedInstancedMaterial;

pub struct SetMaterialBindGroup<M: InstancedMaterial, const I: usize>(PhantomData<M>);

impl<P: PhaseItem, M: InstancedMaterial, const I: usize> RenderCommand<P>
    for SetMaterialBindGroup<M, I>
{
    type Param = (
        SRes<RenderInstancedMaterialInstances>,
        SRes<RenderAssets<PreparedInstancedMaterial<M>>>,
    );
    type ViewQuery = ();
    type ItemQuery = ();

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        _entity: Option<()>,
        (material_instances, materials): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let main_entity = item.main_entity();
        let Some(instance) = material_instances.into_inner().instances.get(&main_entity) else {
            #[cfg(feature = "trace")]
            error!("No material instance for entity {:?}", main_entity);
            return RenderCommandResult::Skip;
        };
        let Some(prepared) = materials.into_inner().get(instance.asset_id.typed()) else {
            #[cfg(feature = "trace")]
            error!(
                "Prepared material missing for asset {:?}",
                instance.asset_id
            );
            return RenderCommandResult::Skip;
        };

        pass.set_bind_group(I, &prepared.bind_group, &[]);
        RenderCommandResult::Success
    }
}
