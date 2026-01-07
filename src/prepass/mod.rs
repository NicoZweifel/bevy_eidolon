pub mod prelude {}

#[cfg(feature = "trace")]
use tracing::*;

use crate::prelude::*;
use crate::render::draw::SetInstancedCombinedBindGroup;
use crate::render::{
    draw::DrawInstancedMaterialMesh,
    pipeline::{InstancedMaterialPipeline, InstancedMaterialPipelineKey},
    prepared_material::PreparedInstancedMaterial,
};
use bevy_app::{App, Plugin};
use bevy_asset::{AssetServer, Handle, embedded_asset};
use bevy_core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy_core_pipeline::prepass::{
    DepthPrepass, MotionVectorPrepass, NormalPrepass, Opaque3dPrepass, prepass_target_descriptors,
};
use bevy_core_pipeline::prepass::{OpaqueNoLightmap3dBatchSetKey, OpaqueNoLightmap3dBinKey};
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemChangeTick;
use bevy_mesh::{Mesh3d, MeshVertexBufferLayoutRef, VertexBufferLayout};
use bevy_pbr::{
    MeshLayouts, MeshPipeline, MeshPipelineKey, PrepassPipeline, RenderMeshInstances,
    SetMeshBindGroup, SetPrepassViewBindGroup, SetPrepassViewEmptyBindGroup, init_prepass_pipeline,
};
use bevy_render::render_phase::{AddRenderCommand, SetItemPipeline};
use bevy_render::view::RenderVisibleEntities;
use bevy_render::{
    Render, RenderApp, RenderStartup, RenderSystems,
    batching::gpu_preprocessing::GpuPreprocessingSupport,
    mesh::{RenderMesh, allocator::MeshAllocator},
    render_asset::RenderAssets,
    render_phase::{BinnedRenderPhaseType, DrawFunctions, ViewBinnedRenderPhases},
    render_resource::*,
    view::{ExtractedView, Msaa},
};
use bevy_shader::Shader;
use bevy_utils::default;

use std::hash::Hash;
use std::marker::PhantomData;

pub struct InstancedPrepassPlugin<M>(PhantomData<M>);

impl<M> Default for InstancedPrepassPlugin<M> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<M> Plugin for InstancedPrepassPlugin<M>
where
    M::Data: PartialEq + Eq + Hash + Clone,
    M: InstancedMaterial,
{
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "prepass.wgsl");

        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .add_render_command::<Opaque3dPrepass, DrawInstancedPrepass<M>>()
            .init_resource::<SpecializedMeshPipelines<InstancedPrepassPipeline<M>>>()
            .add_systems(
                RenderStartup,
                init_instanced_prepass_pipeline::<M>.after(init_prepass_pipeline),
            )
            .add_systems(
                Render,
                queue_instanced_material_prepass::<M>.in_set(RenderSystems::QueueMeshes),
            );
    }
}

#[derive(Resource)]
pub struct InstancedPrepassPipeline<M: InstancedMaterial> {
    pub view_layout_motion_vectors: BindGroupLayout,
    pub view_layout_no_motion_vectors: BindGroupLayout,

    pub empty_layout: BindGroupLayout,
    pub mesh_layouts: MeshLayouts,

    pub combined_layout: BindGroupLayout,

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
        let combined_layout = forward_pipeline.combined_layout.clone();

        let prepass_shader = M::prepass_shader().resolve(asset_server, "prepass/prepass.wgsl");

        InstancedPrepassPipeline {
            view_layout_motion_vectors,
            view_layout_no_motion_vectors,
            mesh_layouts,
            empty_layout,
            combined_layout,
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
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
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
            self.mesh_layouts.model_only.clone(),
            self.combined_layout.clone(),
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
            ],
        });

        Ok(descriptor)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn queue_instanced_material_prepass<M>(
    opaque_draw_functions: Res<DrawFunctions<Opaque3dPrepass>>,
    // TODO alpha_mask_draw_functions: Res<DrawFunctions<AlphaMask3dPrepass>>,
    prepass_pipeline: Res<InstancedPrepassPipeline<M>>,
    mut pipelines: ResMut<SpecializedMeshPipelines<InstancedPrepassPipeline<M>>>,
    pipeline_cache: Res<PipelineCache>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    render_materials: Res<RenderAssets<PreparedInstancedMaterial<M>>>,
    render_material_instances: Res<RenderInstancedMaterialInstances>,
    mesh_allocator: Res<MeshAllocator>,
    gpu_preprocessing_support: Res<GpuPreprocessingSupport>,
    mut opaque_render_phases: ResMut<ViewBinnedRenderPhases<Opaque3dPrepass>>,
    // TODO mut alpha_mask_render_phases: ResMut<ViewBinnedRenderPhases<AlphaMask3dPrepass>>,
    views: Query<(
        &ExtractedView,
        &RenderVisibleEntities,
        &Msaa,
        Option<&DepthPrepass>,
        Option<&NormalPrepass>,
        Option<&MotionVectorPrepass>,
    )>,
    ticks: SystemChangeTick,
) where
    M: InstancedMaterial,
    M::Data: PartialEq + Eq + Hash + Clone,
{
    let draw_opaque = opaque_draw_functions.read().id::<DrawInstancedPrepass<M>>();
    // TODO let draw_alpha_mask = alpha_mask_draw_functions.read().id::<DrawInstancedPrepass<M>>();

    for (view, visible_entities, msaa, depth_prepass, normal_prepass, motion_vector_prepass) in
        &views
    {
        if depth_prepass.is_none() && normal_prepass.is_none() && motion_vector_prepass.is_none() {
            continue;
        }

        let mut view_key = MeshPipelineKey::from_msaa_samples(msaa.samples());
        if depth_prepass.is_some() {
            view_key |= MeshPipelineKey::DEPTH_PREPASS;
        }
        if normal_prepass.is_some() {
            view_key |= MeshPipelineKey::NORMAL_PREPASS;
        }
        if motion_vector_prepass.is_some() {
            view_key |= MeshPipelineKey::MOTION_VECTOR_PREPASS;
        }

        let mut opaque_phase = opaque_render_phases.get_mut(&view.retained_view_entity);
        // TODO let alpha_mask_phase = alpha_mask_render_phases.get_mut(&view.retained_view_entity);

        for (entity, main_entity, prepared_material, mesh, mesh_instance) in visible_entities
            .iter::<Mesh3d>()
            .filter_map(|(entity, main_entity)| {
                #[cfg(feature = "trace")]
                trace!("queue_instanced_material_prepass: \n  - render: {entity:?}\n  - main: {main_entity:?}");

                let material_instance = render_material_instances.instances.get(main_entity)?;
                let prepared_material = render_materials.get(material_instance.asset_id.typed())?;
                let mesh_instance = render_mesh_instances.render_mesh_queue_data(*main_entity)?;
                let mesh = render_meshes.get(mesh_instance.mesh_asset_id)?;

                Some((entity, main_entity, prepared_material, mesh, mesh_instance))
            })
        {
            let key = InstancedMaterialPipelineKey {
                mesh_key: view_key
                    | MeshPipelineKey::from_primitive_topology(mesh.primitive_topology()),
                bind_group_data: prepared_material.key.clone(),
            };

            let pipeline = pipelines
                .specialize(&pipeline_cache, &prepass_pipeline, key, &mesh.layout)
                .unwrap();
            let (vertex_slab, index_slab) = mesh_allocator.mesh_slabs(&mesh_instance.mesh_asset_id);

            if let Some(phase) = opaque_phase.as_mut() {
                phase.add(
                    OpaqueNoLightmap3dBatchSetKey {
                        pipeline,
                        draw_function: draw_opaque,
                        material_bind_group_index: None,
                        vertex_slab: vertex_slab.unwrap_or_default(),
                        index_slab,
                    },
                    OpaqueNoLightmap3dBinKey {
                        asset_id: mesh_instance.mesh_asset_id.into(),
                    },
                    (*entity, *main_entity),
                    mesh_instance.current_uniform_index,
                    BinnedRenderPhaseType::mesh(
                        mesh_instance.should_batch(),
                        &gpu_preprocessing_support,
                    ),
                    ticks.this_run(),
                );
            }

            // TODO AlphaMask
        }
    }
}

pub type DrawInstancedPrepass<M> = (
    SetItemPipeline,
    SetPrepassViewBindGroup<0>,
    SetPrepassViewEmptyBindGroup<1>,
    SetMeshBindGroup<2>,
    SetInstancedCombinedBindGroup<3>,
    DrawInstancedMaterialMesh<M>,
);
