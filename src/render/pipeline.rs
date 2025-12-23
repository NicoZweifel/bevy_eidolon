use std::fmt;
use std::mem::size_of;
use std::num::NonZeroU64;

use bevy_asset::*;
use bevy_ecs::prelude::*;
use bevy_mesh::{MeshVertexBufferLayoutRef, VertexBufferLayout};
use bevy_pbr::{MeshPipeline, MeshPipelineKey};
use bevy_render::{render_resource::*, renderer::RenderDevice};
use bevy_shader::{Shader, ShaderRef};

use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use crate::prelude::*;
use crate::render::prepare::INSTANCE_BINDING_INDEX;

pub struct InstancedMaterialPipelineKey<M: InstancedMaterial> {
    pub mesh_key: MeshPipelineKey,
    pub bind_group_data: M::Data,
    pub is_prepass: bool,
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
            is_prepass: self.is_prepass,
        }
    }
}

impl<M> PartialEq for InstancedMaterialPipelineKey<M>
where
    M: InstancedMaterial,
    M::Data: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.mesh_key == other.mesh_key
            && self.bind_group_data == other.bind_group_data
            && self.is_prepass == other.is_prepass
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
        self.is_prepass.hash(state);
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
    pub prepass_shader: Handle<Shader>,
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

        let vertex_shader = resolve_shader(asset_server, M::vertex_shader(), "mesh.wgsl");
        let fragment_shader = resolve_shader(asset_server, M::fragment_shader(), "shading.wgsl");
        let prepass_shader = resolve_shader(asset_server, ShaderRef::Default, "prepass.wgsl");

        InstancedMaterialPipeline {
            vertex_shader,
            fragment_shader,
            prepass_shader,
            mesh_pipeline,
            material_layout,
            combined_layout,
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

        descriptor.layout.push(self.combined_layout.clone());

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

        if key.is_prepass {
            descriptor.vertex.shader = self.prepass_shader.clone();

            let mut targets = vec![];

            if key.mesh_key.contains(MeshPipelineKey::NORMAL_PREPASS) {
                targets.push(Some(ColorTargetState {
                    format: TextureFormat::Rgb10a2Unorm,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                }));
            }

            if key
                .mesh_key
                .contains(MeshPipelineKey::MOTION_VECTOR_PREPASS)
            {
                targets.push(Some(ColorTargetState {
                    format: TextureFormat::Rg16Float,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                }));
            }

           let mut shader_defs = descriptor.vertex.shader_defs.clone();
            if !targets.is_empty() {
                shader_defs.push("PREPASS_FRAGMENT".into());
            }

            descriptor.fragment = Some(FragmentState {
                shader: self.prepass_shader.clone(),
                shader_defs: descriptor.vertex.shader_defs.clone(),
                entry_point: Some("fragment".into()),
                targets,
            });
        } else {
            descriptor.vertex.shader = self.vertex_shader.clone();
            descriptor.fragment.as_mut().unwrap().shader = self.fragment_shader.clone();
        }

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

        Ok(descriptor)
    }
}

fn resolve_shader(
    asset_server: &AssetServer,
    shader_ref: ShaderRef,
    default: impl Into<String>,
) -> Handle<Shader> {
    let name = default.into();
    match shader_ref {
        ShaderRef::Default => asset_server
            .load(AssetPath::from_path_buf(embedded_path!(name)).with_source("embedded")),
        ShaderRef::Handle(handle) => handle,
        ShaderRef::Path(path) => asset_server.load(path),
    }
}
