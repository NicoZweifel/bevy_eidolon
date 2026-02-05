use crate::prelude::*;

use bevy_asset::*;
use bevy_ecs::prelude::*;
use bevy_mesh::{MeshVertexBufferLayoutRef, VertexBufferLayout};
use bevy_pbr::{MeshPipeline, MeshPipelineKey};
use bevy_render::render_resource::binding_types::storage_buffer_read_only;
use bevy_render::{render_resource::*, renderer::RenderDevice};
use bevy_shader::Shader;

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::mem::size_of;

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
    pub common_layout: BindGroupLayoutDescriptor,
    pub material_layout: BindGroupLayoutDescriptor,
    pub _phantom: PhantomData<M>,
}

impl<M: InstancedMaterial> FromWorld for InstancedMaterialPipeline<M> {
    fn from_world(world: &mut World) -> Self {
        let mesh_pipeline = world.resource::<MeshPipeline>().clone();
        let render_device = world.resource::<RenderDevice>();
        let asset_server = world.resource::<AssetServer>();

        let material_entries = M::bind_group_layout_entries(render_device, false);
        let material_label = format!("instanced_material_layout_{}", std::any::type_name::<M>());
        let material_layout = BindGroupLayoutDescriptor::new(material_label, &material_entries);

        let common_layout = BindGroupLayoutDescriptor::new(
            "instanced_material_common_layout",
            &BindGroupLayoutEntries::single(
                ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE,
                storage_buffer_read_only::<InstanceUniforms>(false),
            ),
        );

        let vertex_shader = M::vertex_shader().resolve(asset_server, "render/mesh.wgsl");
        let fragment_shader = M::fragment_shader().resolve(asset_server, "render/shading.wgsl");

        InstancedMaterialPipeline {
            vertex_shader,
            fragment_shader,
            mesh_pipeline,
            common_layout,
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

        while descriptor.layout.len() < 2 {
            #[cfg(feature = "trace")]
            tracing::warn!("MeshPipeline produced < 2 layouts. Padding with Common.");
            descriptor.layout.push(self.common_layout.clone());
        }

        if descriptor.layout.len() > 2 {
            descriptor.layout[2] = self.common_layout.clone();
        } else {
            descriptor.layout.push(self.common_layout.clone());
        }

        if descriptor.layout.len() > 3 {
            descriptor.layout[3] = self.material_layout.clone();
        } else {
            descriptor.layout.push(self.material_layout.clone());
        }

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

        let rotation_offset = VertexFormat::Float32x4.size();
        let index_offset = rotation_offset + VertexFormat::Float32.size();
        let batch_id_offset = index_offset + VertexFormat::Uint32.size();
        let seed_offset = batch_id_offset + VertexFormat::Uint32.size();

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
                    offset: rotation_offset,
                    shader_location: 9,
                },
                // Index
                VertexAttribute {
                    format: VertexFormat::Uint32,
                    offset: index_offset,
                    shader_location: 10,
                },
                // Batch ID
                VertexAttribute {
                    format: VertexFormat::Uint32,
                    offset: batch_id_offset,
                    shader_location: 11,
                },
                // Seed
                VertexAttribute {
                    format: VertexFormat::Uint32,
                    offset: seed_offset,
                    shader_location: 12,
                },
            ],
        });

        Ok(descriptor)
    }
}
