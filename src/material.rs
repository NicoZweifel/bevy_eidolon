use std::marker::PhantomData;
use bevy_asset::{Asset, AssetId, Handle};
use bevy_color::{Color, ColorToComponents};
use bevy_ecs::{
    prelude::*,
    query::QueryItem,
    system::{SystemParamItem, lifetimeless::SRes},
};
use bevy_math::Vec4;
use bevy_mesh::MeshVertexBufferLayoutRef;
use bevy_pbr::MeshPipelineKey;
use bevy_reflect::TypePath;
use bevy_render::{
    prelude::AlphaMode,
    render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError},
    {
        extract_component::ExtractComponent,
        render_asset::{PrepareAssetError, RenderAsset},
        render_resource::{Buffer, BufferInitDescriptor, BufferUsages, PolygonMode, ShaderType},
        renderer::RenderDevice,
    },
};
use bevy_shader::ShaderRef;

use bytemuck::{Pod, Zeroable};

pub trait InstancedMaterial: Asset + AsBindGroup + TypePath + Clone + Sized + Send + Sync {
    /// The vertex shader. Should usually import the instancing logic.
    fn vertex_shader() -> ShaderRef {
        "embedded://render.wgsl".into()
    }

    /// The fragment shader.
    fn fragment_shader() -> ShaderRef {
        "embedded://render.wgsl".into()
    }

    /// Alpha mode for transparency (Opaque, Blend, Mask).
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }

    /// Allow specializing the pipeline (e.g. enabling shader defs based on material settings).
    fn specialize(
        &self,
        _descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MeshPipelineKey,
    ) -> Result<(), SpecializedMeshPipelineError> {
        Ok(())
    }

    fn polygon_mode(&self) -> PolygonMode {
        PolygonMode::Fill
    }

    fn debug(&self) -> bool {
        false
    }

    fn debug_color(&self) -> Color {
        Color::WHITE
    }

    fn double_sided(&self) -> bool {
        false
    }

    fn gpu_cull(&self) -> bool {
       false
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Default)]
pub struct StandardInstancedMaterial {
    pub debug: bool,
    pub gpu_cull: bool,
    pub debug_color: Color,
    pub polygon_mode: PolygonMode,
    pub double_sided: bool,
}

impl InstancedMaterial for StandardInstancedMaterial {
    fn polygon_mode(&self) -> PolygonMode {
        self.polygon_mode
    }

    fn debug(&self) -> bool {
        self.debug
    }
    fn debug_color(&self) -> Color {
        self.debug_color
    }

    fn double_sided(&self) -> bool {
        self.double_sided
    }

    fn gpu_cull(&self) -> bool {
        self.gpu_cull
    }
}

#[repr(C)]
#[derive(ShaderType, Clone, Zeroable, Copy, Pod)]
pub struct MaterialUniforms {
    pub debug_color: Vec4,
}

impl MaterialUniforms {
    pub fn new(debug_color: Vec4) -> Self {
        Self { debug_color }
    }
}

#[derive(Component, Clone, Debug)]
pub struct InstancedMeshMaterial<M>(pub Handle<M>)
where
    M: InstancedMaterial;

impl<M: InstancedMaterial> ExtractComponent for InstancedMeshMaterial<M> {
    type QueryData = &'static InstancedMeshMaterial<M>;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self> {
        Some(item.clone())
    }
}

pub struct PreparedInstancedMaterial<M> {
    pub buffer: Buffer,
    _phantom: PhantomData<M>,
}

impl <M> PreparedInstancedMaterial<M>{
    pub fn new(buffer: Buffer) -> Self {
        Self { buffer, _phantom: PhantomData }
    }
}

impl<M: InstancedMaterial> RenderAsset for PreparedInstancedMaterial<M> {
    type SourceAsset = M;
    type Param = SRes<RenderDevice>;

    fn prepare_asset(
        source_asset: Self::SourceAsset,
        _asset_id: AssetId<Self::SourceAsset>,
        render_device: &mut SystemParamItem<Self::Param>,
        _previous_asset: Option<&Self>,
    ) -> Result<Self, PrepareAssetError<Self::SourceAsset>> {
        let uniform_data = MaterialUniforms {
            debug_color: source_asset.debug_color().to_linear().to_vec4(),
        };

        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("instanced_material_uniform_buffer"),
            contents: bytemuck::bytes_of(&uniform_data),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        Ok(PreparedInstancedMaterial::new( buffer ))
    }
}

impl<'a> From<&'a StandardInstancedMaterial> for MaterialUniforms {
    fn from(material: &'a StandardInstancedMaterial) -> Self {
        MaterialUniforms::new(material.debug_color.to_linear().to_vec4())
    }
}
