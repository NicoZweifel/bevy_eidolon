use crate::cull::pipeline::InstancedComputePipeline;
use crate::material::InstancedMaterial;
use bevy_ecs::change_detection::{Res, ResMut};
use bevy_render::render_resource::{ComputePipelineDescriptor, PipelineCache};
use bevy_utils::default;

pub fn queue_instanced_material_compute_pipeline<M: InstancedMaterial>(
    pipeline_cache: Res<PipelineCache>,
    mut compute_pipeline: ResMut<InstancedComputePipeline<M>>,
) {
    if compute_pipeline.pipeline_id.is_some() {
        return;
    }

    let id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("instanced_material_compute_pipeline".into()),
        layout: vec![
            compute_pipeline.compute_layout.clone(),
            compute_pipeline.common_layout.clone(),
            compute_pipeline.global_layout.clone(),
        ],
        shader: compute_pipeline.shader.clone(),
        entry_point: Some("main".into()),
        ..default()
    });

    compute_pipeline.pipeline_id = Some(id);
}
