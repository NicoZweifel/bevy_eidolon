use bevy_ecs::change_detection::{Res, ResMut};
use bevy_render::render_resource::{ComputePipelineDescriptor, PipelineCache};
use bevy_utils::default;

use crate::cull::pipeline::InstancedComputePipeline;

pub fn queue_instanced_material_compute_pipeline(
    pipeline_cache: Res<PipelineCache>,
    mut compute_pipeline: ResMut<InstancedComputePipeline>,
) {
    if compute_pipeline.pipeline_id.is_some() {
        return;
    }

    let id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("instanced_material_compute_pipeline".into()),
        layout: vec![
            compute_pipeline.entity_layout.clone(),
            compute_pipeline.global_layout.clone(),
        ],
        push_constant_ranges: vec![],
        shader: compute_pipeline.shader.clone(),
        shader_defs: vec![],
        entry_point: Some("main".into()),
        ..default()
    });

    compute_pipeline.pipeline_id = Some(id);
}
