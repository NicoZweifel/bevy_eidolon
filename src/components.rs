use bevy_color::{Color, LinearRgba};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use bevy_ecs::query::QueryItem;
use bevy_math::{Vec3, Vec4};
use bevy_reflect::Reflect;
use bevy_render::render_resource::Buffer;
use bevy_render::{extract_component::ExtractComponent, render_resource::BindGroup};
use bevy_utils::default;
use bytemuck::{Pod, Zeroable};
use std::fmt;
use std::sync::Arc;

/// Marker component to opt in to GPU-driven culling/preparation.
#[derive(Component, Clone, Copy, Default, ExtractComponent)]
pub struct GpuCull;

/// Sets the material color.
///
/// Corresponds to `instance_uniforms.color` in shaders.
#[derive(Component, Clone, Copy, Debug, Reflect, Default)]
#[reflect(Component, Clone, Debug)]
pub struct InstanceColor(pub Color);

#[derive(Component, Clone, Copy, Deref, DerefMut)]
pub(crate) struct InstancePipelineKey(pub u64);

impl ExtractComponent for InstancePipelineKey {
    type QueryData = &'static InstancePipelineKey;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self> {
        Some(*item)
    }
}

#[derive(Clone, Copy, Pod, Zeroable, Default)]
#[repr(C)]
pub struct InstanceData {
    pub position: Vec3,
    pub scale: f32,
    
    pub rotation: f32,
    pub index: u32,
    pub _padding: [u32; 2],
}

#[derive(Component, Clone, Reflect)]
#[reflect(Component, Clone, Debug)]
pub struct InstanceMaterialData {
    #[reflect(ignore)]
    pub instances: Arc<Vec<InstanceData>>,
    pub color: LinearRgba,
    pub visibility_range: Vec4,
}

impl fmt::Debug for InstanceMaterialData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InstanceMaterialData")
            .field("instances", &self.instances.len())
            .field("color", &self.color)
            .field("visibility_range", &self.visibility_range)
            .finish()
    }
}

impl ExtractComponent for InstanceMaterialData {
    type QueryData = &'static InstanceMaterialData;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self> {
        Some(item.clone())
    }
}

#[derive(Component)]
pub struct InstanceBuffer {
    pub buffer: Buffer,
    pub length: usize,
}

#[derive(Component)]
pub struct GpuDrawIndexedIndirect {
    pub buffer: Buffer,
    pub offset: u64,
}

#[derive(Component)]
pub struct InstanceLodBuffer {
    pub buffer: Buffer,
}

#[derive(Clone, Copy, Pod, Zeroable, Default)]
#[repr(C)]
pub struct InstanceUniforms {
    pub color: LinearRgba,
    pub visibility_range: Vec4,
}

impl From<&InstanceMaterialData> for InstanceUniforms {
    fn from(value: &InstanceMaterialData) -> Self {
        InstanceUniforms {
            color: value.color,
            visibility_range: value.visibility_range,
            ..default()
        }
    }
}

#[derive(Component)]
pub struct InstanceUniformBuffer {
    pub buffer: Buffer,
    pub bind_group: BindGroup,
}

#[derive(Component)]
pub struct InstancedComputeSourceBuffer {
    pub buffer: Buffer,
    pub count: u32,
}

#[derive(Component)]
pub struct InstancedComputeBindGroup(pub BindGroup);
