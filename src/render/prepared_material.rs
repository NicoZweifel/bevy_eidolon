use crate::material::InstancedMaterial;
use crate::render::pipeline::InstancedMaterialPipeline;
use bevy_asset::AssetId;
use bevy_ecs::system::SystemParamItem;
use bevy_ecs::system::lifetimeless::SRes;
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
    _phantom: PhantomData<M>,
}

impl<M: InstancedMaterial> PreparedInstancedMaterial<M> {
    pub fn new(bind_group: BindGroup, key: M::Data) -> Self {
        Self {
            bind_group,
            key,
            disable_prepass: false,
            _phantom: PhantomData,
        }
    }
}

impl<M: InstancedMaterial> RenderAsset for PreparedInstancedMaterial<M> {
    type SourceAsset = M;
    type Param = (
        SRes<RenderDevice>,
        SRes<InstancedMaterialPipeline<M>>,
        SRes<PipelineCache>,
        <M as AsBindGroup>::Param,
    );

    fn prepare_asset(
        source_asset: Self::SourceAsset,
        _asset_id: AssetId<Self::SourceAsset>,
        (render_device, pipeline, pipeline_cache, material_params): &mut SystemParamItem<
            Self::Param,
        >,
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

                Ok(PreparedInstancedMaterial {
                    bind_group,
                    key: source_asset.bind_group_data(),
                    disable_prepass: source_asset.disable_prepass(),
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
