use bevy_asset::{Asset, AssetId, Handle};
use bevy_color::{Color, ColorToComponents};
use bevy_ecs::{
    prelude::*,
    query::QueryItem,
    system::{SystemParamItem, lifetimeless::SRes},
};
use bevy_math::Vec4;
use bevy_reflect::TypePath;
use bevy_render::{
    extract_component::ExtractComponent,
    render_asset::{PrepareAssetError, RenderAsset},
    render_resource::{Buffer, BufferInitDescriptor, BufferUsages, PolygonMode, ShaderType},
    renderer::RenderDevice,
};

use bytemuck::{Pod, Zeroable};

#[derive(Asset, TypePath, Debug, Clone, Default)]
pub struct InstancedMaterial {
    pub debug: bool,
    pub gpu_cull: bool,
    pub debug_color: Color,
    pub polygon_mode: PolygonMode,
    pub double_sided: bool,
}

#[repr(C)]
#[derive(ShaderType, Clone, Zeroable, Copy, Pod)]
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
    pub buffer: Buffer,
}

impl RenderAsset for PreparedInstancedMaterial {
    type SourceAsset = InstancedMaterial;
    type Param = SRes<RenderDevice>;

    fn prepare_asset(
        source_asset: Self::SourceAsset,
        _asset_id: AssetId<Self::SourceAsset>,
        render_device: &mut SystemParamItem<Self::Param>,
        _previous_asset: Option<&Self>,
    ) -> Result<Self, PrepareAssetError<Self::SourceAsset>> {
        let uniform_data = MaterialUniform {
            debug_color: source_asset.debug_color.to_linear().to_vec4(),
        };

        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("instanced_material_uniform_buffer"),
            contents: bytemuck::bytes_of(&uniform_data),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        Ok(PreparedInstancedMaterial { buffer })
    }
}

impl<'a> From<&'a InstancedMaterial> for MaterialUniform {
    fn from(material: &'a InstancedMaterial) -> Self {
        MaterialUniform::new(material.debug_color.to_linear().to_vec4())
    }
}
