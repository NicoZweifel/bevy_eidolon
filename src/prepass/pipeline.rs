use std::hash::Hash;
use std::marker::PhantomData;

use bevy_asset::{AssetServer, Handle};
use bevy_core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy_core_pipeline::prepass::prepass_target_descriptors;
use bevy_ecs::error;
use bevy_ecs::prelude::*;
use bevy_mesh::{MeshVertexBufferLayoutRef, VertexBufferLayout, VertexFormat};
use bevy_pbr::{MeshLayouts, MeshPipeline, MeshPipelineKey, PrepassPipeline};
use bevy_render::render_resource::{
    BindGroupLayoutDescriptor, CompareFunction, DepthBiasState, DepthStencilState, FragmentState,
    MultisampleState, PrimitiveState, RenderPipelineDescriptor, SpecializedMeshPipeline,
    SpecializedMeshPipelineError, StencilState, VertexAttribute, VertexState, VertexStepMode,
};
use bevy_shader::Shader;
use bevy_utils::default;

use crate::components::InstanceData;
use crate::material::InstancedMaterial;
use crate::prelude::*;
use crate::render::pipeline::{InstancedMaterialPipeline, InstancedMaterialPipelineKey};

#[derive(Resource)]
pub struct InstancedPrepassPipeline<M: InstancedMaterial> {
    pub view_layout_motion_vectors: BindGroupLayoutDescriptor,
    pub view_layout_no_motion_vectors: BindGroupLayoutDescriptor,

    pub empty_layout: BindGroupLayoutDescriptor,
    pub mesh_layouts: MeshLayouts,

    pub common_layout: BindGroupLayoutDescriptor,
    pub material_layout: BindGroupLayoutDescriptor,

    pub prepass_shader: Handle<Shader>,

    pub _phantom: PhantomData<M>,
}

pub fn init_instanced_prepass_pipeline<M: InstancedMaterial>(mut cmd: Commands) {
    cmd.init_resource::<InstancedPrepassPipeline<M>>();
}

impl<M: InstancedMaterial> FromWorld for InstancedPrepassPipeline<M> {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();

        let bevy_prepass = world.resource::<PrepassPipeline>();
        let view_layout_motion_vectors = bevy_prepass.view_layout_motion_vectors.clone();
        let view_layout_no_motion_vectors = bevy_prepass.view_layout_no_motion_vectors.clone();
        let empty_layout = bevy_prepass.empty_layout.clone();

        let mesh_pipeline = world.resource::<MeshPipeline>();
        let mesh_layouts = mesh_pipeline.mesh_layouts.clone();

        let forward_pipeline = world.resource::<InstancedMaterialPipeline<M>>();
        let common_layout = forward_pipeline.common_layout.clone();
        let material_layout = forward_pipeline.material_layout.clone();

        let prepass_shader = M::prepass_shader().resolve(asset_server, "prepass/prepass.wgsl");

        InstancedPrepassPipeline {
            view_layout_motion_vectors,
            view_layout_no_motion_vectors,
            mesh_layouts,
            empty_layout,
            common_layout,
            material_layout,
            prepass_shader,
            _phantom: PhantomData,
        }
    }
}

impl<M> SpecializedMeshPipeline for InstancedPrepassPipeline<M>
where
    M: InstancedMaterial,
    M::Data: PartialEq + Eq + Hash + Clone,
{
    type Key = InstancedMaterialPipelineKey<M>;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> error::Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut shader_defs = Vec::new();
        shader_defs.push("PREPASS_PIPELINE".into());
        shader_defs.push("VISIBILITY_RANGE_DITHER".into());

        let mut vertex_attributes = Vec::new();
        if layout.0.contains(bevy_mesh::Mesh::ATTRIBUTE_POSITION) {
            shader_defs.push("VERTEX_POSITIONS".into());
            vertex_attributes.push(bevy_mesh::Mesh::ATTRIBUTE_POSITION.at_shader_location(0));
        }
        if key.mesh_key.contains(MeshPipelineKey::NORMAL_PREPASS) {
            shader_defs.push("NORMAL_PREPASS".into());
            if layout.0.contains(bevy_mesh::Mesh::ATTRIBUTE_NORMAL) {
                shader_defs.push("VERTEX_NORMALS".into());
                vertex_attributes.push(bevy_mesh::Mesh::ATTRIBUTE_NORMAL.at_shader_location(1));
            }
        }
        if key
            .mesh_key
            .contains(MeshPipelineKey::MOTION_VECTOR_PREPASS)
        {
            shader_defs.push("MOTION_VECTOR_PREPASS".into());
        }

        let vertex_buffer_layout = layout.0.get_layout(&vertex_attributes)?;

        let view_layout = if key
            .mesh_key
            .contains(MeshPipelineKey::MOTION_VECTOR_PREPASS)
        {
            self.view_layout_motion_vectors.clone()
        } else {
            self.view_layout_no_motion_vectors.clone()
        };

        let bind_group_layouts = vec![
            view_layout,
            self.empty_layout.clone(),
            self.common_layout.clone(),
            self.material_layout.clone(),
        ];

        let mut targets = prepass_target_descriptors(
            key.mesh_key.contains(MeshPipelineKey::NORMAL_PREPASS),
            key.mesh_key
                .contains(MeshPipelineKey::MOTION_VECTOR_PREPASS),
            false,
        );

        if targets.iter().all(Option::is_none) {
            targets.clear();
        }

        if !targets.is_empty() {
            shader_defs.push("PREPASS_FRAGMENT".into());
        }

        let mut descriptor = RenderPipelineDescriptor {
            label: Some("instanced_material_prepass_pipeline".into()),
            layout: bind_group_layouts,
            vertex: VertexState {
                shader: self.prepass_shader.clone(),
                entry_point: Some("vertex".into()),
                shader_defs: shader_defs.clone(),
                buffers: vec![vertex_buffer_layout],
            },
            fragment: if targets.is_empty() {
                None
            } else {
                Some(FragmentState {
                    shader: self.prepass_shader.clone(),
                    shader_defs: shader_defs.clone(),
                    entry_point: Some("fragment".into()),
                    targets,
                })
            },
            primitive: PrimitiveState {
                topology: key.mesh_key.primitive_topology(),
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: CompareFunction::GreaterEqual,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: key.mesh_key.msaa_samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            ..default()
        };

        M::specialize(&mut descriptor, layout, key.bind_group_data)?;

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
