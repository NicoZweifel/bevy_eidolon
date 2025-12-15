use crate::prelude::*;
use crate::resources::{CameraCullData, LodCullData};
use crate::systems::InstancedMaterialKey;

use bevy_asset::*;
use bevy_ecs::prelude::*;
use bevy_mesh::{MeshVertexBufferLayoutRef, VertexBufferLayout};
use bevy_pbr::{MeshPipeline, MeshPipelineKey};
use bevy_render::{render_resource::*, renderer::RenderDevice};
use bevy_shader::{Shader, ShaderRef};
use std::hash::Hash;
use std::marker::PhantomData;

use crate::prepare::INSTANCE_BINDING_INDEX;
use std::mem::size_of;
use std::num::NonZeroU64;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct InstancedMaterialPipelineKey {
    pub mesh_key: MeshPipelineKey,
    pub material_key: InstancedMaterialKey,
}

#[derive(Resource)]
pub struct InstancedMaterialPipeline<M: InstancedMaterial> {
    pub vertex_shader: Handle<Shader>,
    pub fragment_shader: Handle<Shader>,
    pub mesh_pipeline: MeshPipeline,

    /// The layout of the material's bindings only.
    /// Used in `prepare_asset` to call `unprepared_bind_group`.
    pub material_layout: BindGroupLayout,

    /// The final layout including Material bindings + Instance Uniforms.
    /// Used in the render pipeline.
    pub combined_layout: BindGroupLayout,

    pub _phantom: PhantomData<M>,
}

impl<M: InstancedMaterial> FromWorld for InstancedMaterialPipeline<M> {
    fn from_world(world: &mut World) -> Self {
        let mesh_pipeline = world.resource::<MeshPipeline>().clone();
        let render_device = world.resource::<RenderDevice>();
        let asset_server = world.resource::<AssetServer>();

        let material_entries = M::bind_group_layout_entries(render_device, false);
        let material_layout = render_device.create_bind_group_layout(
            format!("instanced_material_layout_{}", std::any::type_name::<M>()).as_str(),
            &material_entries,
        );

        let mut combined_entries = material_entries.clone();
        if combined_entries
            .iter()
            .any(|e| e.binding == INSTANCE_BINDING_INDEX)
        {
            panic!(
                "InstancedMaterial {} uses reserved binding slot {}!",
                std::any::type_name::<M>(),
                INSTANCE_BINDING_INDEX
            );
        }

        combined_entries.push(BindGroupLayoutEntry {
            binding: INSTANCE_BINDING_INDEX,
            visibility: ShaderStages::VERTEX_FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: NonZeroU64::new(size_of::<InstanceUniforms>() as u64),
            },
            count: None,
        });

        let combined_layout = render_device.create_bind_group_layout(
            format!(
                "instanced_material_combined_layout_{}",
                std::any::type_name::<M>()
            )
            .as_str(),
            &combined_entries,
        );

        let resolve_shader = |shader_ref: ShaderRef| -> Handle<Shader> {
            match shader_ref {
                ShaderRef::Default => asset_server.load(
                    AssetPath::from_path_buf(embedded_path!("render.wgsl")).with_source("embedded"),
                ),
                ShaderRef::Handle(handle) => handle,
                ShaderRef::Path(path) => asset_server.load(path),
            }
        };

        let vertex_shader = resolve_shader(M::vertex_shader());
        let fragment_shader = resolve_shader(M::fragment_shader());

        InstancedMaterialPipeline {
            vertex_shader,
            fragment_shader,
            mesh_pipeline,
            material_layout,
            combined_layout,
            _phantom: PhantomData,
        }
    }
}

impl<M: InstancedMaterial> SpecializedMeshPipeline for InstancedMaterialPipeline<M> {
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

        if let Some(fragment) = descriptor.fragment.as_mut() {
            if let Some(target) = fragment.targets.get_mut(0)
                && let Some(target) = target
            {
                target.blend = None;
            }

            fragment.shader_defs.push("VISIBILITY_RANGE_DITHER".into());

            if key.material_key.contains(InstancedMaterialKey::DEBUG) {
                fragment.shader_defs.push("MATERIAL_DEBUG".into());
            }
        }

        descriptor.vertex.shader = self.vertex_shader.clone();
        descriptor.fragment.as_mut().unwrap().shader = self.fragment_shader.clone();

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
            .load(AssetPath::from_path_buf(embedded_path!("cull.wgsl")).with_source("embedded"));

        InstancedComputePipeline {
            entity_layout,
            global_layout,
            shader,
            pipeline_id: None,
        }
    }
}
