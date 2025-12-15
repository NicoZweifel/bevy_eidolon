use bevy_render::batching::NoAutomaticBatching;
use crate::pipeline::InstancedMaterialPipeline;
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
use bevy_render::render_resource::AsBindGroupError;
use bevy_render::{
    render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError},
    {
        extract_component::ExtractComponent,
        render_asset::{PrepareAssetError, RenderAsset},
        render_resource::{OwnedBindingResource, PolygonMode, ShaderType},
        renderer::RenderDevice,
    },
};
use bevy_shader::ShaderRef;
use bytemuck::{Pod, Zeroable};
use std::marker::PhantomData;

pub trait InstancedMaterial: Asset + AsBindGroup + TypePath + Clone + Sized + Send + Sync {
    /// The vertex shader.
    fn vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }

    /// The fragment shader.
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Default
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
#[uniform(0, InstancedMaterialUniforms)]
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
pub struct InstancedMaterialUniforms {
    pub debug_color: Vec4,
}

impl InstancedMaterialUniforms {
    pub fn new(debug_color: Vec4) -> Self {
        Self { debug_color }
    }
}

#[derive(Component, Clone, Debug)]
#[require(NoAutomaticBatching)]
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
    pub bindings: Vec<(u32, OwnedBindingResource)>,
    _phantom: PhantomData<M>,
}

impl<M> PreparedInstancedMaterial<M> {
    pub fn new(bindings: Vec<(u32, OwnedBindingResource)>) -> Self {
        Self {
            bindings,
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
    ) -> Result<Self, PrepareAssetError<Self::SourceAsset>> {
        match source_asset.unprepared_bind_group(
            &pipeline.material_layout,
            render_device,
            material_params,
            false,
        ) {
            Ok(unprepared) => Ok(PreparedInstancedMaterial {
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

impl<'a> From<&'a StandardInstancedMaterial> for InstancedMaterialUniforms {
    fn from(material: &'a StandardInstancedMaterial) -> Self {
        InstancedMaterialUniforms::new(material.debug_color.to_linear().to_vec4())
    }
}
