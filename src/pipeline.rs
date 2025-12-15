use super::components::InstanceData;
use crate::prelude::{InstanceUniform, MaterialUniform};
use crate::resources::{CameraCullData, LodCullData};
use crate::systems::InstancedMaterialKey;

use std::hash::Hash;

use bevy_asset::*;
use bevy_ecs::prelude::*;
use bevy_mesh::{MeshVertexBufferLayoutRef, VertexBufferLayout};
use bevy_pbr::{MeshPipeline, MeshPipelineKey};
use bevy_render::{render_resource::*, renderer::RenderDevice};
use bevy_shader::Shader;

use std::mem::size_of;
use std::num::NonZeroU64;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct InstancedMaterialPipelineKey {
    pub mesh_key: MeshPipelineKey,
    pub material_key: InstancedMaterialKey,
}

#[derive(Resource)]
pub struct InstancedMaterialPipeline {
    pub shader: Handle<Shader>,
    pub mesh_pipeline: MeshPipeline,
    pub combined_layout: BindGroupLayout,
}

impl FromWorld for InstancedMaterialPipeline {
    fn from_world(world: &mut World) -> Self {
        let mesh_pipeline = world.resource::<MeshPipeline>().clone();
        let render_device = world.resource::<RenderDevice>();
        let asset_server = world.resource::<AssetServer>();

        let combined_layout = render_device.create_bind_group_layout(
            "instanced_material_combined_layout",
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: BufferSize::new(size_of::<MaterialUniform>() as u64),
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::VERTEX_FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: BufferSize::new(size_of::<InstanceUniform>() as u64),
                    },
                    count: None,
                },
            ],
        );

        InstancedMaterialPipeline {
            shader: asset_server.load(
                AssetPath::from_path_buf(embedded_path!("render.wgsl")).with_source("embedded"),
            ),
            mesh_pipeline,
            combined_layout,
        }
    }
}

impl SpecializedMeshPipeline for InstancedMaterialPipeline {
    type Key = InstancedMaterialPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.mesh_pipeline.specialize(key.mesh_key, layout)?;

        descriptor.layout.push(self.combined_layout.clone());

        if let Some(ds) = descriptor.depth_stencil.as_mut() {
            ds.depth_write_enabled = true;
            ds.depth_compare = CompareFunction::GreaterEqual;
        }

        let shader_defs = &mut descriptor.vertex.shader_defs;

        if !shader_defs.contains(&"MAY_DISCARD".into()) {
            shader_defs.push("MAY_DISCARD".into());
        }

        shader_defs.push("VISIBILITY_RANGE_DITHER".into());

        // TODO cull in compute shader
        /*
        let gpu_cull = key.wind_key.contains(WindAffectedKey::GPU_CULL);
        if gpu_cull {
            key.mesh_key
                .remove(MeshPipelineKey::VISIBILITY_RANGE_DITHER);
        }
        */

        if let Some(fragment) = descriptor.fragment.as_mut() {
            if let Some(target) = fragment.targets.get_mut(0)
                && let Some(target) = target
            {
                target.blend = None;
            }

            // TODO cull in compute shader
            /*
            if !gpu_cull {
                fragment.shader_defs.push("VISIBILITY_RANGE_DITHER".into());
            }
             */
            fragment.shader_defs.push("VISIBILITY_RANGE_DITHER".into());

            if key.material_key.contains(InstancedMaterialKey::DEBUG) {
                fragment.shader_defs.push("MATERIAL_DEBUG".into());
            }
        }

        descriptor.vertex.shader = self.shader.clone();

        descriptor.vertex.buffers.push(VertexBufferLayout {
            array_stride: size_of::<InstanceData>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: vec![
                // Position + Scale
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 0,
                    shader_location: 8,
                },
                // Rotation
                VertexAttribute {
                    format: VertexFormat::Float32,
                    offset: VertexFormat::Float32x4.size(),
                    shader_location: 9,
                },
                // Index
                VertexAttribute {
                    format: VertexFormat::Uint32,
                    offset: VertexFormat::Float32x4.size() + VertexFormat::Float32.size(),
                    shader_location: 10,
                },
            ],
        });

        descriptor.fragment.as_mut().unwrap().shader = self.shader.clone();

        if key.material_key.contains(InstancedMaterialKey::POINTS) {
            descriptor.primitive.polygon_mode = PolygonMode::Point;
        }

        if key.material_key.contains(InstancedMaterialKey::LINES) {
            descriptor.primitive.polygon_mode = PolygonMode::Line;
        }

        if key
            .material_key
            .contains(InstancedMaterialKey::DOUBLE_SIDED)
        {
            descriptor.primitive.cull_mode = None;
        }

        Ok(descriptor)
    }
}

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
                // Source
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
                // Output
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
                // Indirect Args
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
                // LOD Data
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
            .load(AssetPath::from_path_buf(embedded_path!("cull.wgsl")).with_source("embedded"));

        InstancedComputePipeline {
            entity_layout,
            global_layout,
            shader,
            pipeline_id: None,
        }
    }
}
