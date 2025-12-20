use bevy_color::{Color, LinearRgba};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::{prelude::*, query::QueryItem};
use bevy_math::{Mat4, Vec3, Vec4};
use bevy_reflect::Reflect;
use bevy_render::{
    extract_component::ExtractComponent,
    render_resource::{BindGroup, Buffer},
};
use bevy_utils::default;

use bytemuck::{Pod, Zeroable};

use crate::prelude::InstancedMaterial;

use bevy_transform::prelude::GlobalTransform;
use std::fmt;
use std::hash::Hash;
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
    type QueryData = (&'static Self, &'static GlobalTransform);
    type QueryFilter = ();
    type Out = (Self, GlobalTransform);

    fn extract_component(
        (data, transform): QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        Some((data.clone(), *transform))
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
    pub world_from_local: Mat4,
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
pub struct InstancedCombinedBindGroup(pub BindGroup);

#[derive(Component)]
pub struct InstanceUniformBuffer {
    pub buffer: Buffer,
}

#[derive(Component)]
pub struct InstancedComputeSourceBuffer {
    pub buffer: Buffer,
    pub count: u32,
}

#[derive(Component)]
pub struct InstancedComputeBindGroup(pub BindGroup);

#[derive(Component, Clone, Deref, DerefMut)]
pub struct MaterialBindGroupData<M: InstancedMaterial>(pub M::Data);

impl<M> ExtractComponent for MaterialBindGroupData<M>
where
    M: InstancedMaterial,
    M::Data: PartialEq + Eq + Hash + Clone + Send + Sync + 'static,
{
    type QueryData = &'static MaterialBindGroupData<M>;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self> {
        Some(item.clone())
    }
}
