use bevy_asset::AssetId;
use bevy_ecs::system::SystemParamItem;
use bevy_ecs::system::lifetimeless::SRes;
use bevy_render::{
    render_asset::{PrepareAssetError, RenderAsset},
    render_resource::{AsBindGroup, AsBindGroupError, OwnedBindingResource},
    renderer::RenderDevice,
};
use std::marker::PhantomData;

use crate::material::InstancedMaterial;
use crate::render::pipeline::InstancedMaterialPipeline;

pub struct PreparedInstancedMaterial<M: InstancedMaterial> {
    pub bindings: Vec<(u32, OwnedBindingResource)>,
    pub key: M::Data,
    _phantom: PhantomData<M>,
}

impl<M: InstancedMaterial> PreparedInstancedMaterial<M> {
    pub fn new(bindings: Vec<(u32, OwnedBindingResource)>, key: M::Data) -> Self {
        Self {
            bindings,
            key,
            _phantom: PhantomData,
        }
    }
}

impl<M: InstancedMaterial> RenderAsset for PreparedInstancedMaterial<M> {
    type SourceAsset = M;
    type Param = (
        SRes<RenderDevice>,
        SRes<InstancedMaterialPipeline<M>>,
        <M as AsBindGroup>::Param,
    );

    fn prepare_asset(
        source_asset: Self::SourceAsset,
        _asset_id: AssetId<Self::SourceAsset>,
        (render_device, pipeline, material_params): &mut SystemParamItem<Self::Param>,
        _previous_asset: Option<&Self>,
    ) -> bevy_ecs::error::Result<Self, PrepareAssetError<Self::SourceAsset>> {
        match source_asset.unprepared_bind_group(
            &pipeline.combined_layout,
            render_device,
            material_params,
            false,
        ) {
            Ok(unprepared) => Ok(PreparedInstancedMaterial {
                key: source_asset.bind_group_data(),
                bindings: unprepared.bindings.0,
                _phantom: PhantomData,
            }),
            Err(AsBindGroupError::RetryNextUpdate) => {
                Err(PrepareAssetError::RetryNextUpdate(source_asset))
            }
            Err(other) => Err(PrepareAssetError::AsBindGroupError(other)),
        }
    }
}
