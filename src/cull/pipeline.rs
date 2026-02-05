use bevy_asset::{AssetServer, Handle};
use bevy_ecs::prelude::*;
use bevy_render::render_resource::{
    BindGroupLayoutDescriptor, BindGroupLayoutEntries, CachedComputePipelineId, ShaderStages,
    binding_types::*,
};
use bevy_shader::Shader;

use super::resources::CameraCullData;
use crate::components::{InstanceData, InstanceUniforms};

use crate::material::InstancedMaterial;
use crate::utils::ResolveShaderRef;
use std::marker::PhantomData;

#[derive(Resource)]
pub struct InstancedComputePipeline<M: InstancedMaterial> {
    pub compute_layout: BindGroupLayoutDescriptor,
    pub common_layout: BindGroupLayoutDescriptor,
    pub global_layout: BindGroupLayoutDescriptor,
    pub shader: Handle<Shader>,
    pub pipeline_id: Option<CachedComputePipelineId>,
    pub _marker: PhantomData<M>,
}

impl<M: InstancedMaterial> FromWorld for InstancedComputePipeline<M> {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();

        let compute_layout = BindGroupLayoutDescriptor::new(
            "instanced_material_compute_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    // Source
                    storage_buffer_read_only::<InstanceData>(false),
                    // Output
                    storage_buffer::<InstanceData>(false),
                    // DrawIndirect
                    storage_buffer::<[u32; 5]>(false),
                    // Metadata
                    storage_buffer_read_only::<[u32; 4]>(false),
                ),
            ),
        );

        let common_layout = BindGroupLayoutDescriptor::new(
            "instanced_material_common_layout",
            &BindGroupLayoutEntries::single(
                ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE,
                storage_buffer_read_only::<InstanceUniforms>(false),
            ),
        );

        let global_layout = BindGroupLayoutDescriptor::new(
            "instanced_material_compute_global_layout",
            &BindGroupLayoutEntries::single(
                ShaderStages::COMPUTE,
                uniform_buffer::<CameraCullData>(false),
            ),
        );

        let shader = M::cull_shader().resolve(asset_server, "cull/compute.wgsl");

        InstancedComputePipeline {
            compute_layout,
            common_layout,
            global_layout,
            shader,
            pipeline_id: None,
            _marker: PhantomData,
        }
    }
}
