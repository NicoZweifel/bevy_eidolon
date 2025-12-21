use bevy_camera::Camera;
use bevy_ecs::change_detection::{Res, ResMut};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, Query, With};
use bevy_math::Vec4;
use bevy_pbr::RenderMeshInstances;
use bevy_render::mesh::allocator::MeshAllocator;
use bevy_render::mesh::{RenderMesh, RenderMeshBufferInfo};
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_resource::{
    BindGroupEntry, BufferDescriptor, BufferInitDescriptor, BufferUsages, DrawIndexedIndirectArgs,
};
use bevy_render::renderer::{RenderDevice, RenderQueue};
use bevy_render::sync_world::MainEntity;
use bevy_render::view::ExtractedView;
use bevy_transform::components::GlobalTransform;

use bytemuck::bytes_of;
use tracing::warn;

use crate::cull::pipeline::InstancedComputePipeline;
use crate::prelude::*;
use crate::resources::{CameraCullData, GlobalCullBuffer, LodCullData};

pub fn prepare_global_cull_buffer(
    mut commands: Commands,
    views: Query<(&ExtractedView, &Camera)>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    global_buffer: Option<ResMut<GlobalCullBuffer>>,
    pipeline: Res<InstancedComputePipeline>,
) {
    if views.is_empty() {
        #[cfg(feature = "trace")]
        warn!("No active camera/view found for culling.");
        return;
    }

    let Some((view, _camera)) = views.iter().find(|(_, cam)| cam.is_active) else {
        return;
    };

    let camera_position = view.world_from_view.translation();

    let data = CameraCullData {
        view_pos: Vec4::from((camera_position, 1.0)),
    };

    let contents = bytes_of(&data);

    if let Some(global) = global_buffer {
        render_queue.write_buffer(&global.buffer, 0, contents);
    } else {
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("instanced_material_compute_global_cull_camera_buffer"),
            contents,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let bind_group = render_device.create_bind_group(
            "instanced_global_cull_bind_group",
            &pipeline.global_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        );

        commands.insert_resource(GlobalCullBuffer { buffer, bind_group });
    }
}

pub fn prepare_instanced_material_compute_resources(
    mut commands: Commands,
    query: Query<
        (
            Entity,
            &MainEntity,
            &InstanceMaterialData,
            &GlobalTransform,
            Option<&InstancedComputeSourceBuffer>,
            Option<&GpuDrawIndexedIndirect>,
        ),
        With<GpuCullCompute>,
    >,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    render_mesh_instances: Res<RenderMeshInstances>,
    meshes: Res<RenderAssets<RenderMesh>>,
    mesh_allocator: Res<MeshAllocator>,
    pipeline: Res<InstancedComputePipeline>,
) {
    for (entity, main_entity, instance_data, gtf, existing_source, existing_indirect) in &query {
        let count = instance_data.instances.len();
        if count == 0 {
            continue;
        }

        if existing_source.is_some_and(|s| s.count == count as u32) {
            if let Some(indirect) = existing_indirect {
                render_queue.write_buffer(&indirect.buffer, 4, &[0, 0, 0, 0]);
            }

            continue;
        }

        let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(*main_entity) else {
            continue;
        };
        let Some(gpu_mesh) = meshes.get(mesh_instance.mesh_asset_id) else {
            continue;
        };
        let Some(vertex_slice) = mesh_allocator.mesh_vertex_slice(&mesh_instance.mesh_asset_id)
        else {
            continue;
        };

        let indirect_buffer = if let RenderMeshBufferInfo::Indexed {
            count: index_count, ..
        } = gpu_mesh.buffer_info
        {
            let Some(index_slice) = mesh_allocator.mesh_index_slice(&mesh_instance.mesh_asset_id)
            else {
                continue;
            };

            let command = DrawIndexedIndirectArgs {
                index_count,
                instance_count: 0,
                first_index: index_slice.range.start,
                base_vertex: vertex_slice.range.start as i32,
                first_instance: 0,
            };

            render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("instanced_material_compute_indirect_buffer"),
                contents: command.as_bytes(),
                usage: BufferUsages::STORAGE | BufferUsages::INDIRECT | BufferUsages::COPY_DST,
            })
        } else {
            continue;
        };

        let lod_data = LodCullData {
            visibility_range: instance_data.visibility_range,
            world_from_local: gtf.to_matrix(),
        };

        let contents = bytes_of(&lod_data);

        let lod_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("instanced_material_compute_lod_cull_data_buffer"),
            contents,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let source_buffer = if let Some(existing) = existing_source
            && existing.count == count as u32
        {
            existing.buffer.clone()
        } else {
            render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("instanced_material_compute_source_buffer"),
                contents: bytemuck::cast_slice(&instance_data.instances),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            })
        };

        let output_size = (count * size_of::<InstanceData>()) as u64;
        let output_buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("instanced_material_compute_output_buffer"),
            size: output_size,
            usage: BufferUsages::STORAGE | BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        let bind_group = render_device.create_bind_group(
            "instanced_material_compute_entity_bind_group",
            &pipeline.entity_layout, // Group 0 Layout
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: source_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: output_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: indirect_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: lod_buffer.as_entire_binding(),
                },
            ],
        );

        commands.entity(entity).insert((
            InstancedComputeSourceBuffer {
                buffer: source_buffer,
                count: count as u32,
            },
            InstanceBuffer {
                buffer: output_buffer,
                length: 0,
            },
            GpuDrawIndexedIndirect {
                buffer: indirect_buffer,
                offset: 0,
            },
            InstancedComputeBindGroup(bind_group),
            InstanceLodBuffer { buffer: lod_buffer },
        ));
    }
}
