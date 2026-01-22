use crate::components::{InstanceData, InstanceUniforms};
use crate::render::prepare::utils;

use bevy_asset::AssetId;
use bevy_ecs::entity::Entity;
use bevy_mesh::Mesh;
use bevy_render::sync_world::MainEntity;

use std::ops::Range;
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub(super) struct Write {
    pub(super) entity: Entity,
    pub(super) offset: u32,
    pub(super) batch_id: u32,
}

#[derive(Clone)]
pub(super) struct Clear {
    pub(super) offset: u64,
    pub(super) clear: Vec<InstanceData>,
}

impl From<Range<u32>> for Clear {
    fn from(Range { start: offset, end }: Range<u32>) -> Self {
        let invalid_instance = InstanceData::invalid();
        Clear {
            offset: utils::instance_data_offset(offset),
            clear: vec![invalid_instance; (end - offset) as usize],
        }
    }
}

pub(super) struct BatchInput {
    pub(super) entity: Entity,
    pub(super) main_entity: MainEntity,
    pub(super) mesh_id: AssetId<Mesh>,
    pub(super) instances: Arc<Vec<InstanceData>>,
    pub(super) gpu_cull: bool,
    pub(super) batch: InstanceUniforms,
}

pub(crate) struct CachedInputState {
    pub(super) instances: Arc<Vec<InstanceData>>,
    pub(super) key_hash: u64,
}
