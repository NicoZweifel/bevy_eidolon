use std::num::NonZeroU64;

use bevy_asset::{AssetPath, AssetServer, Handle, embedded_path};
use bevy_ecs::prelude::*;
use bevy_render::{
    render_resource::{
        BindGroupLayout, BindGroupLayoutEntry, BindingType, BufferBindingType,
        CachedComputePipelineId, ShaderStages, ShaderType,
    },
    renderer::RenderDevice,
};
use bevy_shader::Shader;

use crate::components::InstanceData;
use crate::resources::{CameraCullData, LodCullData};

#[derive(Resource)]
pub struct InstancedComputePipeline {
    pub entity_layout: BindGroupLayout,
    pub global_layout: BindGroupLayout,
    pub shader: Handle<Shader>,
    pub pipeline_id: Option<CachedComputePipelineId>,
}

impl FromWorld for InstancedComputePipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let asset_server = world.resource::<AssetServer>();
        let instance_size = size_of::<InstanceData>() as u64;
        let min_size = NonZeroU64::new(instance_size);

        let entity_layout = render_device.create_bind_group_layout(
            "instanced_material_compute_entity_layout",
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: min_size,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: min_size,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(20),
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(LodCullData::min_size()),
                    },
                    count: None,
                },
            ],
        );

        let global_layout = render_device.create_bind_group_layout(
            "instanced_material_compute_global_layout",
            &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(CameraCullData::min_size()),
                },
                count: None,
            }],
        );

        let shader = asset_server
            .load(AssetPath::from_path_buf(embedded_path!("compute.wgsl")).with_source("embedded"));

        InstancedComputePipeline {
            entity_layout,
            global_layout,
            shader,
            pipeline_id: None,
        }
    }
}
