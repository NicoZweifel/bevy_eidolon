use std::fmt;
use std::mem::size_of;
use std::num::NonZeroU64;

use bevy_asset::*;
use bevy_ecs::prelude::*;
use bevy_mesh::{MeshVertexBufferLayoutRef, VertexBufferLayout};
use bevy_pbr::{MeshPipeline, MeshPipelineKey};
use bevy_render::{render_resource::*, renderer::RenderDevice};
use bevy_shader::Shader;

use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use crate::prelude::*;

pub struct InstancedMaterialPipelineKey<M: InstancedMaterial> {
    pub mesh_key: MeshPipelineKey,
    pub bind_group_data: M::Data,
}

impl<M> Clone for InstancedMaterialPipelineKey<M>
where
    M: InstancedMaterial,
    M::Data: Clone,
{
    fn clone(&self) -> Self {
        Self {
            mesh_key: self.mesh_key,
            bind_group_data: self.bind_group_data.clone(),
        }
    }
}

impl<M> PartialEq for InstancedMaterialPipelineKey<M>
where
    M: InstancedMaterial,
    M::Data: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.mesh_key == other.mesh_key && self.bind_group_data == other.bind_group_data
    }
}

impl<M> Eq for InstancedMaterialPipelineKey<M>
where
    M: InstancedMaterial,
    M::Data: Eq,
{
}

impl<M> Hash for InstancedMaterialPipelineKey<M>
where
    M: InstancedMaterial,
    M::Data: Hash,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.mesh_key.hash(state);
        self.bind_group_data.hash(state);
    }
}

impl<M> fmt::Debug for InstancedMaterialPipelineKey<M>
where
    M: InstancedMaterial,
    M::Data: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InstancedMaterialPipelineKey")
            .field("mesh_key", &self.mesh_key)
            .field("bind_group_data", &self.bind_group_data)
            .finish()
    }
}

#[derive(Resource)]
pub struct InstancedMaterialPipeline<M: InstancedMaterial> {
    pub vertex_shader: Handle<Shader>,
    pub fragment_shader: Handle<Shader>,
    pub mesh_pipeline: MeshPipeline,
    pub instance_layout: BindGroupLayout,
    pub material_layout: BindGroupLayout,
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

        let instance_layout = render_device.create_bind_group_layout(
            "instanced_material_instance_layout",
            &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX_FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(size_of::<InstanceUniforms>() as u64),
                },
                count: None,
            }],
        );

        let vertex_shader = M::vertex_shader().resolve(asset_server, "render/mesh.wgsl");
        let fragment_shader = M::fragment_shader().resolve(asset_server, "render/shading.wgsl");

        InstancedMaterialPipeline {
            vertex_shader,
            fragment_shader,
            mesh_pipeline,
            instance_layout,
            material_layout,
            _phantom: PhantomData,
        }
    }
}

impl<M> SpecializedMeshPipeline for InstancedMaterialPipeline<M>
where
    M: InstancedMaterial,
    M::Data: PartialEq + Eq + Hash + Clone,
{
    type Key = InstancedMaterialPipelineKey<M>;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.mesh_pipeline.specialize(key.mesh_key, layout)?;

        if descriptor.layout.len() > 2 {
            descriptor.layout[2] = self.instance_layout.clone();
        } else {
            descriptor.layout.push(self.instance_layout.clone());
        }

        descriptor.layout.push(self.material_layout.clone());

        if let Some(ds) = descriptor.depth_stencil.as_mut() {
            ds.depth_write_enabled = true;
            ds.depth_compare = CompareFunction::GreaterEqual;
        }

        let shader_defs = &mut descriptor.vertex.shader_defs;

        shader_defs.push("VISIBILITY_RANGE_DITHER".into());

        if let Some(fragment) = descriptor.fragment.as_mut() {
            if let Some(target) = fragment.targets.get_mut(0)
                && let Some(target) = target
            {
                target.blend = None;
            }

            fragment.shader_defs.push("VISIBILITY_RANGE_DITHER".into());
        }

        M::specialize(&mut descriptor, layout, key.bind_group_data)?;

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
                // Batch ID
                VertexAttribute {
                    format: VertexFormat::Uint32,
                    offset: VertexFormat::Float32x4.size()
                        + VertexFormat::Float32.size()
                        + VertexFormat::Uint32.size(),
                    shader_location: 11,
                },
                // Seed
                VertexAttribute {
                    format: VertexFormat::Uint32,
                    offset: VertexFormat::Float32x4.size()
                        + VertexFormat::Float32.size()
                        + VertexFormat::Uint32.size() * 2,
                    shader_location: 12,
                },
            ],
        });

        Ok(descriptor)
    }
}
