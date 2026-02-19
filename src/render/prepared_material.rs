use crate::material::InstancedMaterial;
use crate::prepass::draw::DrawInstancedPrepass;
use crate::render::draw::DrawInstancedMaterial;
use crate::render::pipeline::InstancedMaterialPipeline;
use bevy_asset::AssetId;
use bevy_core_pipeline::core_3d::Opaque3d;
use bevy_core_pipeline::deferred::Opaque3dDeferred;
use bevy_core_pipeline::prepass::Opaque3dPrepass;
use bevy_ecs::system::SystemParamItem;
use bevy_ecs::system::lifetimeless::SRes;
use bevy_render::render_phase::{DrawFunctionId, DrawFunctions};
use bevy_render::render_resource::{BindGroup, BindGroupEntry, PipelineCache};
use bevy_render::{
    render_asset::{PrepareAssetError, RenderAsset},
    render_resource::{AsBindGroup, AsBindGroupError},
    renderer::RenderDevice,
};
use std::marker::PhantomData;

pub struct PreparedInstancedMaterial<M: InstancedMaterial> {
    pub bind_group: BindGroup,
    pub key: M::Data,
    pub disable_prepass: bool,
    pub draw_opaque: DrawFunctionId,
    pub draw_deferred: DrawFunctionId,
    pub draw_prepass: DrawFunctionId,
    _phantom: PhantomData<M>,
}

impl<M: InstancedMaterial> RenderAsset for PreparedInstancedMaterial<M> {
    type SourceAsset = M;
    type Param = (
        SRes<RenderDevice>,
        SRes<InstancedMaterialPipeline<M>>,
        SRes<PipelineCache>,
        <M as AsBindGroup>::Param,
        SRes<DrawFunctions<Opaque3d>>,
        SRes<DrawFunctions<Opaque3dDeferred>>,
        SRes<DrawFunctions<Opaque3dPrepass>>,
    );

    fn prepare_asset(
        source_asset: Self::SourceAsset,
        _asset_id: AssetId<Self::SourceAsset>,
        (
            render_device,
            pipeline,
            pipeline_cache,
            material_params,
            draw_opaque_functions,
            draw_deferred_functions,
            draw_prepass_functions,
        ): &mut SystemParamItem<Self::Param>,
        _previous_asset: Option<&Self>,
    ) -> bevy_ecs::error::Result<Self, PrepareAssetError<Self::SourceAsset>> {
        match source_asset.unprepared_bind_group(
            &pipeline_cache.get_bind_group_layout(&pipeline.material_layout),
            render_device,
            material_params,
            false,
        ) {
            Ok(unprepared) => {
                let entries: Vec<BindGroupEntry> = unprepared
                    .bindings
                    .iter()
                    .map(|(index, resource)| BindGroupEntry {
                        binding: *index,
                        resource: resource.get_binding(),
                    })
                    .collect();

                let bind_group = render_device.create_bind_group(
                    Some("instanced_material_user_bind_group"),
                    &pipeline_cache.get_bind_group_layout(&pipeline.material_layout),
                    &entries,
                );

                let draw_opaque = draw_opaque_functions
                    .read()
                    .get_id::<DrawInstancedMaterial<M>>()
                    .unwrap();

                let draw_deferred = draw_deferred_functions
                    .read()
                    .get_id::<DrawInstancedPrepass<M>>()
                    .unwrap();

                let draw_prepass = draw_prepass_functions
                    .read()
                    .get_id::<DrawInstancedPrepass<M>>()
                    .unwrap();

                Ok(PreparedInstancedMaterial {
                    bind_group,
                    key: source_asset.bind_group_data(),
                    disable_prepass: source_asset.disable_prepass(),
                    draw_opaque,
                    draw_deferred,
                    draw_prepass,
                    _phantom: PhantomData,
                })
            }
            Err(AsBindGroupError::RetryNextUpdate) => {
                Err(PrepareAssetError::RetryNextUpdate(source_asset))
            }
            Err(other) => Err(PrepareAssetError::AsBindGroupError(other)),
        }
    }
}
