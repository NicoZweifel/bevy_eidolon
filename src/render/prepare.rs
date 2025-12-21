use crate::prelude::*;
use crate::render::pipeline::InstancedMaterialPipeline;

use bevy_ecs::prelude::*;
use bevy_pbr::RenderMeshInstances;
use bevy_render::{
    mesh::allocator::MeshAllocator,
    mesh::{RenderMesh, RenderMeshBufferInfo},
    render_asset::RenderAssets,
    render_resource::{
        BindGroupEntry, BufferInitDescriptor, BufferUsages, DrawIndexedIndirectArgs,
    },
    renderer::{RenderDevice, RenderQueue},
    sync_world::MainEntity,
};
use bevy_transform::components::GlobalTransform;

use bytemuck::bytes_of;

pub(crate) fn prepare_instance_buffer(
    mut cmd: Commands,
    query: Query<(Entity, &InstanceMaterialData, Option<&InstanceBuffer>), Without<GpuCullCompute>>,
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

pub(crate) fn prepare_instanced_bind_group<M>(
    mut commands: Commands,
    query: Query<(
        Entity,
        &InstancedMeshMaterial<M>,
        &InstanceMaterialData,
        &GlobalTransform,
        Option<&InstanceUniformBuffer>,
    )>,
    render_materials: Res<RenderAssets<PreparedInstancedMaterial<M>>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline: Res<InstancedMaterialPipeline<M>>,
) where
    M: InstancedMaterial,
{
    for (entity, material_handle, instance_data, gtf, uniform_buffer) in &query {
        let Some(prepared_material) = render_materials.get(&material_handle.0) else {
            continue;
        };

        let uniforms = InstanceUniforms {
            world_from_local: gtf.to_matrix(),
            ..instance_data.into()
        };
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

pub(crate) fn prepare_indirect_draw_buffer(
    mut cmd: Commands,
    query: Query<
        (
            Entity,
            &MainEntity,
            &InstanceBuffer,
            Option<&GpuDrawIndexedIndirect>,
        ),
        Without<GpuCullCompute>,
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
