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
use bevy_reflect::TypePath;
use bevy_render::{
    batching::NoAutomaticBatching,
    render_resource::AsBindGroupError,
    render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError},
    {
        extract_component::ExtractComponent,
        render_asset::{PrepareAssetError, RenderAsset},
        render_resource::{OwnedBindingResource, PolygonMode, ShaderType},
        renderer::RenderDevice,
    },
};
use bevy_shader::ShaderRef;
use bitflags::bitflags;
use bytemuck::{Pod, Zeroable};

use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;

pub trait InstancedMaterial: Asset + AsBindGroup + Clone + Sized + Send + Sync + 'static {
    /// The vertex shader.
    fn vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }

    /// The fragment shader.
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Default
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

    /// Allow specializing the pipeline (e.g. enabling shader defs based on material settings).
    fn specialize(
        _descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: Self::Data,
    ) -> Result<(), SpecializedMeshPipelineError> {
        Ok(())
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Default)]
#[uniform(0, InstancedMaterialUniforms)]
#[bind_group_data(InstancedMaterialKey)]
pub struct StandardInstancedMaterial {
    pub debug: bool,
    pub gpu_cull: bool,
    pub debug_color: Color,
    pub polygon_mode: PolygonMode,
    pub double_sided: bool,
}

impl From<&StandardInstancedMaterial> for InstancedMaterialKey {
    fn from(material: &StandardInstancedMaterial) -> Self {
        let mut key = InstancedMaterialKey::empty();
        if material.debug {
            key.insert(InstancedMaterialKey::DEBUG);
        }

        if material.gpu_cull {
            key.insert(InstancedMaterialKey::GPU_CULL);
        }

        if material.double_sided {
            key.insert(InstancedMaterialKey::DOUBLE_SIDED);
        }

        match material.polygon_mode {
            PolygonMode::Point => key.insert(InstancedMaterialKey::POINTS),
            PolygonMode::Line => key.insert(InstancedMaterialKey::LINES),
            _ => {}
        }

        key
    }
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

    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: Self::Data,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if key.contains(InstancedMaterialKey::DOUBLE_SIDED) {
            descriptor.primitive.cull_mode = None;
        }

        if key.contains(InstancedMaterialKey::GPU_CULL) {
            // TODO
        }

        if key.contains(InstancedMaterialKey::POINTS) {
            descriptor.primitive.polygon_mode = PolygonMode::Point;
        }
        if key.contains(InstancedMaterialKey::LINES) {
            descriptor.primitive.polygon_mode = PolygonMode::Line;
        }
        if key.contains(InstancedMaterialKey::DOUBLE_SIDED) {
            descriptor.primitive.cull_mode = None;
        }

        if key.contains(InstancedMaterialKey::DEBUG) {
            if let Some(fragment) = descriptor.fragment.as_mut() {
                fragment.shader_defs.push("MATERIAL_DEBUG".into());
            };
        }

        Ok(())
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
    ) -> Result<Self, PrepareAssetError<Self::SourceAsset>> {
        match source_asset.unprepared_bind_group(
            &pipeline.material_layout,
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

impl<'a> From<&'a StandardInstancedMaterial> for InstancedMaterialUniforms {
    fn from(material: &'a StandardInstancedMaterial) -> Self {
        InstancedMaterialUniforms::new(material.debug_color.to_linear().to_vec4())
    }
}

bitflags! {
    #[repr(C)]
    #[derive(Clone, Debug, Copy, PartialEq, Eq, Hash, Pod, Zeroable)]
    pub struct InstancedMaterialKey: u64 {
        const DEBUG = 1 << 0;
        const GPU_CULL = 1 << 2;
        const LINES = 1 << 3;
        const POINTS = 1 << 4;
        const DOUBLE_SIDED = 1<< 5;
    }
}
