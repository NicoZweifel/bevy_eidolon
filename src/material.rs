use bevy_asset::{Asset, AssetId, Handle};
use bevy_color::{Color, ColorToComponents};
use bevy_ecs::prelude::*;
use bevy_ecs::query::QueryItem;
use bevy_ecs::system::{SystemParamItem, lifetimeless::SRes};
use bevy_math::Vec4;
use bevy_reflect::TypePath;
use bevy_render::extract_component::ExtractComponent;
use bevy_render::render_asset::{PrepareAssetError, RenderAsset};
use bevy_render::render_resource::{AsBindGroup, AsBindGroupError, BindGroup, PolygonMode, ShaderType};
use bevy_render::renderer::RenderDevice;
use bytemuck::Zeroable;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Default)]
#[uniform(50, MaterialUniform)]
pub struct InstancedMaterial {
    pub debug: bool,
    pub gpu_cull: bool,
    pub debug_color: Color,
    pub polygon_mode: PolygonMode
}

#[repr(C)]
#[derive(ShaderType, Clone, Zeroable, Copy)]
pub struct MaterialUniform {
    pub debug_color: Vec4,
}

impl MaterialUniform {
    pub fn new(debug_color: Vec4) -> Self {
        Self { debug_color }
    }
}

#[derive(Component, Clone, Debug)]
pub struct InstancedMeshMaterial(pub Handle<InstancedMaterial>);

impl ExtractComponent for InstancedMeshMaterial {
    type QueryData = &'static InstancedMeshMaterial;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self> {
        Some(item.clone())
    }
}

pub struct PreparedInstancedMaterial {
    pub bind_group: BindGroup,
}

impl RenderAsset for PreparedInstancedMaterial {
    type SourceAsset = InstancedMaterial;
    type Param = (
        SRes<RenderDevice>,
        <InstancedMaterial as AsBindGroup>::Param,
    );

    fn prepare_asset(
        source_asset: Self::SourceAsset,
        _asset_id: AssetId<Self::SourceAsset>,
        (render_device, param): &mut SystemParamItem<Self::Param>,
        _previous_asset: Option<&Self>,
    ) -> Result<Self, PrepareAssetError<Self::SourceAsset>> {
        match source_asset.as_bind_group(
            &InstancedMaterial::bind_group_layout(render_device),
            render_device,
            param,
        ) {
            Ok(x) => Ok(PreparedInstancedMaterial {
                bind_group: x.bind_group,
            }),
            Err(AsBindGroupError::RetryNextUpdate) => {
                Err(PrepareAssetError::RetryNextUpdate(source_asset))
            }
            Err(other) => Err(PrepareAssetError::AsBindGroupError(other)),
        }
    }
}

impl<'a> From<&'a InstancedMaterial> for MaterialUniform {
    fn from(material: &'a InstancedMaterial) -> Self {
        MaterialUniform::new(material.debug_color.to_linear().to_vec4())
    }
}
