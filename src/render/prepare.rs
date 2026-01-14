use crate::prelude::*;
use crate::render::pipeline::InstancedMaterialPipeline;

use bevy_ecs::prelude::*;
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
};
use bevy_transform::components::GlobalTransform;

use bytemuck::bytes_of;

pub(crate) fn prepare_instance_buffer(
    mut cmd: Commands,
    mut query: Query<
        (
            Entity,
            Ref<InstanceMaterialData>,
            Option<&mut InstanceBuffer>,
        ),
        Without<GpuCullCompute>,
    >,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    for (entity, instance_data, mut instance_buffer) in &mut query {
        if !instance_data.is_changed() && instance_buffer.is_some() {
            continue;
        }

        let instance_vec = &instance_data.instances;
        let count = instance_vec.len();

        if let Some(ref mut buffer) = instance_buffer {
            if count <= buffer.capacity as usize {
                if count > 0 {
                    render_queue.write_buffer(
                        &buffer.buffer,
                        0,
                        bytemuck::cast_slice(instance_vec.as_slice()),
                    );
                }
                buffer.length = count;
                continue;
            }
        }

        create_buffer(
            &mut cmd,
            entity,
            instance_vec,
            &render_device,
            &render_queue,
        );
    }
}

fn create_buffer(
    cmd: &mut Commands,
    entity: Entity,
    instance_vec: &Vec<InstanceData>,
    render_device: &Res<RenderDevice>,
    render_queue: &Res<RenderQueue>,
) {
    let count = instance_vec.len();
    let capacity = instance_vec.capacity().max(count);
    let size = (capacity * size_of::<InstanceData>()) as u64;

    if size == 0 {
        return;
    }

    let buffer = render_device.create_buffer(&BufferDescriptor {
        label: Some("instanced_material_data_buffer"),
        size,
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    if count > 0 {
        render_queue.write_buffer(&buffer, 0, bytemuck::cast_slice(instance_vec.as_slice()));
    }

    cmd.entity(entity).insert(InstanceBuffer {
        buffer,
        length: count,
        capacity: capacity as u32,
    });
}

pub(crate) fn prepare_instanced_bind_group<M>(
    mut commands: Commands,
    query: Query<(
        Entity,
        Ref<InstanceMaterialData>,
        Ref<GlobalTransform>,
        Option<&InstanceUniformBuffer>,
        Option<Ref<InstanceHistory>>,
        Option<&InstanceBindGroup>,
    )>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline: Res<InstancedMaterialPipeline<M>>,
) where
    M: InstancedMaterial,
{
    for (entity, instance_data, gtf, uniform_buffer, instance_history, existing_bind_group) in
        &query
    {
        let any_changed = instance_data.is_changed()
            || gtf.is_changed()
            || instance_history.as_ref().map_or(false, |h| h.is_changed());

        let has_buffer = uniform_buffer.is_some();
        let has_bind_group = existing_bind_group.is_some();

        if !any_changed && has_buffer && has_bind_group {
            continue;
        }

        let buffer = if any_changed || !has_buffer {
            let world_from_local = gtf.to_matrix();
            let uniforms = InstanceUniforms {
                world_from_local,
                previous_world_from_local: instance_history
                    .map(|x| **x)
                    .unwrap_or(world_from_local),
                ..instance_data.as_ref().into()
            };

            let contents = bytes_of(&uniforms);

            if let Some(InstanceUniformBuffer { buffer }) = uniform_buffer {
                render_queue.write_buffer(buffer, 0, contents);
                buffer.clone()
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
            }
        } else {
            uniform_buffer.unwrap().buffer.clone()
        };

        if has_bind_group {
            continue;
        }

        let bind_group = render_device.create_bind_group(
            "instanced_material_instance_bind_group",
            &pipeline.instance_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        );

        commands
            .entity(entity)
            .insert(InstanceBindGroup(bind_group));
    }
}

pub fn prepare_indirect_draw_buffer(
    mut cmd: Commands,
    query: Query<
        (
            Entity,
            &MainEntity,
            Ref<InstanceBuffer>,
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
    for ((entity, _, _, o_indirect_buffer), command) in
        query
            .iter()
            .filter_map(|(entity, main_entity, instance_buffer, o_indirect)| {
                if !instance_buffer.is_changed() && o_indirect.is_some() {
                    return None;
                }

                let mesh_instance = render_mesh_instances.render_mesh_queue_data(*main_entity)?;
                let mesh_asset_id = mesh_instance.mesh_asset_id;

                let gpu_mesh = meshes.get(mesh_asset_id)?;
                let vertex_buffer_slice = mesh_allocator.mesh_vertex_slice(&mesh_asset_id)?;
                let index_buffer_slice = mesh_allocator.mesh_index_slice(&mesh_asset_id)?;

                if let RenderMeshBufferInfo::Indexed { count, .. } = gpu_mesh.buffer_info {
                    let command = DrawIndexedIndirectArgs {
                        index_count: count,
                        instance_count: instance_buffer.length as u32,
                        first_index: index_buffer_slice.range.start,
                        base_vertex: vertex_buffer_slice.range.start as i32,
                        first_instance: 0,
                    };

                    Some(((entity, main_entity, instance_buffer, o_indirect), command))
                } else {
                    None
                }
            })
    {
        let contents = command.as_bytes();

        if let Some(indirect_buffer) = o_indirect_buffer {
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
