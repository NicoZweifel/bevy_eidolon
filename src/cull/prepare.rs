use crate::cull::pipeline::InstancedComputePipeline;
use crate::prelude::*;

use bevy_camera::Camera;
use bevy_camera::primitives::Frustum;
use bevy_ecs::prelude::*;
use bevy_math::Vec4;
use bevy_render::render_resource::PipelineCache;
use bevy_render::{
    render_resource::{BindGroupEntry, BufferInitDescriptor, BufferUsages},
    renderer::{RenderDevice, RenderQueue},
    view::ExtractedView,
};
use bytemuck::bytes_of;

pub fn prepare_global_cull_buffer<M: InstancedMaterial>(
    mut commands: Commands,
    views: Query<(&ExtractedView, &Frustum, &Camera)>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    global_buffer: Option<ResMut<GlobalCullBuffer>>,
    pipeline: Res<InstancedComputePipeline<M>>,
    pipeline_cache: Res<PipelineCache>,
) {
    let Some((view, frustum, _)) = views.iter().find(|(_, _, cam)| cam.is_active) else {
        return;
    };

    let camera_position = view.world_from_view.translation();
    let frustum = frustum.half_spaces.map(|h| h.normal_d());
    let data = CameraCullData {
        view_pos: Vec4::from((camera_position, 1.0)),
        frustum,
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
            &pipeline_cache.get_bind_group_layout(&pipeline.global_layout),
            &[BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        );

        commands.insert_resource(GlobalCullBuffer { buffer, bind_group });
    }
}
