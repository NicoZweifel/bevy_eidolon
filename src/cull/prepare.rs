use crate::cull::pipeline::InstancedComputePipeline;
use crate::prelude::*;

use bevy_camera::Camera;
use bevy_ecs::prelude::*;
use bevy_math::Vec4;
use bevy_pbr::RenderMeshInstances;
use bevy_render::{
    mesh::allocator::MeshAllocator,
    mesh::{RenderMesh, RenderMeshBufferInfo},
    render_asset::RenderAssets,
    render_resource::{
        BindGroupEntry, BufferDescriptor, BufferInitDescriptor, BufferUsages,
        DrawIndexedIndirectArgs,
    },
    renderer::{RenderDevice, RenderQueue},
    sync_world::MainEntity,
    view::ExtractedView,
};
use bevy_transform::components::GlobalTransform;

use bytemuck::bytes_of;

pub fn prepare_global_cull_buffer(
    mut commands: Commands,
    views: Query<(&ExtractedView, &Camera)>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    global_buffer: Option<ResMut<GlobalCullBuffer>>,
    pipeline: Res<InstancedComputePipeline>,
) {
    let Some((view, _)) = views.iter().find(|(_, cam)| cam.is_active) else {
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

#[derive(Component)]
pub struct CachedLodCullData(LodCullData);

pub fn prepare_instanced_material_compute_resources(
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            &MainEntity,
            Ref<InstanceMaterialData>,
            &GlobalTransform,
            Option<&mut InstancedComputeSourceBuffer>,
            Option<&GpuDrawIndexedIndirect>,
            Option<&InstanceLodBuffer>,
            Option<&mut CachedLodCullData>,
            Option<&mut InstanceBuffer>,
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
    for (
        entity,
        main_entity,
        instance_data,
        gtf,
        mut source_buffer_opt,
        indirect_buffer_opt,
        lod_buffer_opt,
        mut cached_lod_data,
        instance_buffer_opt,
    ) in &mut query
    {
        let count = instance_data.instances.len();
        if count == 0 {
            continue;
        }
        let count_u32 = count as u32;

        let lod_data = LodCullData {
            visibility_range: instance_data.visibility_range,
            world_from_local: gtf.to_matrix(),
        };

        let mut reuse_buffers = false;
        if let (Some(source), Some(_indirect), Some(_lod), Some(output)) = (
            source_buffer_opt.as_ref(),
            indirect_buffer_opt,
            lod_buffer_opt,
            instance_buffer_opt.as_ref(),
        ) && count_u32 <= source.capacity
            && count_u32 <= output.capacity
        {
            reuse_buffers = true;
        }

        if reuse_buffers {
            let source = source_buffer_opt.as_mut().unwrap();
            let indirect = indirect_buffer_opt.unwrap();
            let lod = lod_buffer_opt.unwrap();

            let mut write_lod = false;
            if let Some(ref mut cached) = cached_lod_data {
                if bytes_of(&cached.0) != bytes_of(&lod_data) {
                    cached.0 = lod_data;
                    write_lod = true;
                }
            } else {
                commands.entity(entity).insert(CachedLodCullData(lod_data));
                write_lod = true;
            }

            if write_lod {
                render_queue.write_buffer(&lod.buffer, 0, bytes_of(&lod_data));
            }

            render_queue.write_buffer(&indirect.buffer, 4, &[0, 0, 0, 0]);

            if instance_data.is_changed() {
                render_queue.write_buffer(
                    &source.buffer,
                    0,
                    bytemuck::cast_slice(&instance_data.instances),
                );
            }

            source.count = count_u32;

            continue;
        }

        let new_capacity = count_u32.checked_next_power_of_two().unwrap_or(count_u32);

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

        let lod_buffer = if let Some(existing) = lod_buffer_opt {
            render_queue.write_buffer(&existing.buffer, 0, bytes_of(&lod_data));
            existing.buffer.clone()
        } else {
            render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("instanced_material_compute_lod_cull_data_buffer"),
                contents: bytes_of(&lod_data),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            })
        };

        let indirect_buffer = if let Some(existing) = indirect_buffer_opt {
            render_queue.write_buffer(&existing.buffer, 4, &[0, 0, 0, 0]);
            existing.buffer.clone()
        } else if let RenderMeshBufferInfo::Indexed {
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

        let source_buffer = {
            let item_size = size_of::<InstanceData>();
            let total_size = (new_capacity as usize) * item_size;

            let buffer = render_device.create_buffer(&BufferDescriptor {
                label: Some("instanced_material_compute_source_buffer"),
                size: total_size as u64,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            render_queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&instance_data.instances));
            buffer
        };

        let output_size = (new_capacity as u64) * (std::mem::size_of::<InstanceData>() as u64);
        let output_buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("instanced_material_compute_output_buffer"),
            size: output_size,
            usage: BufferUsages::STORAGE | BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        let bind_group = render_device.create_bind_group(
            "instanced_material_compute_entity_bind_group",
            &pipeline.entity_layout,
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
                count: count_u32,
                capacity: new_capacity,
            },
            InstanceBuffer {
                buffer: output_buffer,
                length: 0,
                capacity: new_capacity,
            },
            GpuDrawIndexedIndirect {
                buffer: indirect_buffer,
                offset: 0,
            },
            InstancedComputeBindGroup(bind_group),
            InstanceLodBuffer { buffer: lod_buffer },
            CachedLodCullData(lod_data),
        ));
    }
}
