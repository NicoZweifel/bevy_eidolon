use crate::pipeline::{InstancedComputePipeline, InstancedMaterialPipeline};
use crate::prelude::*;
use crate::resources::{CameraCullData, GlobalCullBuffer, LodCullData};

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

use bytemuck::bytes_of;

#[cfg(feature = "trace")]
use tracing::warn;

pub(super) fn prepare_instance_buffer(
    mut cmd: Commands,
    query: Query<(Entity, &InstanceMaterialData, Option<&InstanceBuffer>), Without<GpuCull>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    for (entity, instance_data, instance_buffer) in &query {
        let instance_vec = &instance_data.instances;

        let Some(instance_buffer) = instance_buffer else {
            create_buffer(&mut cmd, entity, instance_vec, &render_device);
            continue;
        };

        if instance_vec.len() != instance_buffer.length {
            create_buffer(&mut cmd, entity, instance_vec, &render_device);
            continue;
        }

        render_queue.write_buffer(
            &instance_buffer.buffer,
            0,
            bytemuck::cast_slice(instance_vec.as_slice()),
        );
    }
}

fn create_buffer(
    cmd: &mut Commands,
    entity: Entity,
    instance_vec: &Vec<InstanceData>,
    render_device: &Res<RenderDevice>,
) {
    let contents = bytemuck::cast_slice(instance_vec.as_slice());

    let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("instanced_material_data_buffer"),
        contents,
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
    });

    cmd.entity(entity).insert(InstanceBuffer {
        buffer,
        length: instance_vec.len(),
    });
}

pub const INSTANCE_BINDING_INDEX: u32 = 100;

pub(super) fn prepare_instanced_bind_group<M>(
    mut commands: Commands,
    query: Query<(
        Entity,
        &InstancedMeshMaterial<M>,
        &InstanceMaterialData,
        Option<&InstanceUniformBuffer>,
    )>,
    render_materials: Res<RenderAssets<PreparedInstancedMaterial<M>>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline: Res<InstancedMaterialPipeline<M>>,
) where
    M: InstancedMaterial,
{
    for (entity, material_handle, instance_data, uniform_buffer) in &query {
        let Some(prepared_material) = render_materials.get(&material_handle.0) else {
            continue;
        };

        let uniforms: InstanceUniforms = instance_data.into();
        let contents = bytes_of(&uniforms);

        let buffer = if let Some(instance_uniform_buffer) = uniform_buffer {
            render_queue.write_buffer(&instance_uniform_buffer.buffer, 0, contents);
            instance_uniform_buffer.buffer.clone()
        } else {
            let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("instanced_material_uniform_buffer"),
                contents,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            });

            commands.entity(entity).insert(InstanceUniformBuffer {
                buffer: buffer.clone(),
            });

            buffer
        };

        let mut entries: Vec<BindGroupEntry> = prepared_material
            .bindings
            .iter()
            .map(|(index, resource)| BindGroupEntry {
                binding: *index,
                resource: resource.get_binding(),
            })
            .collect();

        entries.push(BindGroupEntry {
            binding: INSTANCE_BINDING_INDEX,
            resource: buffer.as_entire_binding(),
        });

        let bind_group = render_device.create_bind_group(
            "instanced_material_combined_bind_group",
            &pipeline.combined_layout,
            &entries,
        );

        commands
            .entity(entity)
            .insert(InstancedCombinedBindGroup(bind_group));
    }
}

pub(super) fn prepare_indirect_draw_buffer(
    mut cmd: Commands,
    query: Query<
        (
            Entity,
            &MainEntity,
            &InstanceBuffer,
            Option<&GpuDrawIndexedIndirect>,
        ),
        Without<GpuCull>,
    >,
    render_mesh_instances: Res<RenderMeshInstances>,
    meshes: Res<RenderAssets<RenderMesh>>,
    mesh_allocator: Res<MeshAllocator>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    for (entity, main_entity, instance_buffer, indirect_buffer_opt) in &query {
        let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(*main_entity) else {
            continue;
        };
        let mesh_asset_id = mesh_instance.mesh_asset_id;

        let Some(gpu_mesh) = meshes.get(mesh_asset_id) else {
            continue;
        };
        let Some(vertex_buffer_slice) = mesh_allocator.mesh_vertex_slice(&mesh_asset_id) else {
            continue;
        };

        if let RenderMeshBufferInfo::Indexed { count, .. } = gpu_mesh.buffer_info {
            let Some(index_buffer_slice) = mesh_allocator.mesh_index_slice(&mesh_asset_id) else {
                continue;
            };

            let command = DrawIndexedIndirectArgs {
                index_count: count,
                instance_count: instance_buffer.length as u32,
                first_index: index_buffer_slice.range.start,
                base_vertex: vertex_buffer_slice.range.start as i32,
                first_instance: 0,
            };

            let contents = command.as_bytes();

            if let Some(indirect_buffer) = indirect_buffer_opt {
                render_queue.write_buffer(&indirect_buffer.buffer, 0, contents);
            } else {
                let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
                    label: Some("draw_indexed_indirect buffer"),
                    contents,
                    usage: BufferUsages::INDIRECT | BufferUsages::COPY_DST,
                });

                cmd.entity(entity)
                    .insert(GpuDrawIndexedIndirect { buffer, offset: 0 });
            }
        }
    }
}

pub(super) fn prepare_global_cull_buffer(
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

pub(super) fn prepare_instanced_material_compute_resources(
    mut commands: Commands,
    query: Query<
        (
            Entity,
            &MainEntity,
            &InstanceMaterialData,
            Option<&InstancedComputeSourceBuffer>,
            Option<&GpuDrawIndexedIndirect>,
        ),
        With<GpuCull>,
    >,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    render_mesh_instances: Res<RenderMeshInstances>,
    meshes: Res<RenderAssets<RenderMesh>>,
    mesh_allocator: Res<MeshAllocator>,
    pipeline: Res<InstancedComputePipeline>,
) {
    for (entity, main_entity, instance_data, existing_source, existing_indirect) in &query {
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
