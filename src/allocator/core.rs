//! Defines the types and protocol for the module.

use crate::material::InstancedMaterial;

use bevy_asset::{Asset, AssetId};
use bevy_ecs::entity::{Entity, EntityHashMap};
use bevy_mesh::Mesh;
use bevy_render::{render_resource::ShaderType, sync_world::MainEntity};

use std::hash::{Hash, Hasher};
use std::ops::Range;

use bytemuck::{Pod, Zeroable};

/// A range of instances allocated within a page.
#[derive(Debug, Clone, Copy)]
pub struct InstanceAllocation {
    /// The index of the page (buffer) where this allocation resides.
    pub page: usize,
    /// The start index within the page.
    pub offset: u32,
}

impl From<(usize, u32)> for InstanceAllocation {
    fn from((page, offset): (usize, u32)) -> Self {
        Self { page, offset }
    }
}

/// Interface for managing GPU instance indices.
pub trait InstanceAllocator: Send + Sync + 'static {
    /// Reserves a contiguous block of `count` indices for the given `entity`.
    ///
    /// Returns the [`InstanceAllocation`] if successful, or `None` if allocation failed.
    fn alloc(&mut self, entity: Entity, count: u32) -> Option<InstanceAllocation>;

    /// Releases the indices associated with `entity`, making them available for reuse.
    fn free(&mut self, entity: Entity);

    /// Returns the active "watermark" (the highest used index) for a specific page.
    ///
    /// Used to determine the required size of the GPU buffer.
    fn size(&self, page_id: usize) -> u32;

    /// Returns ranges freed this frame to be written as "tombstones" (zeros) to the GPU.
    fn drain(&mut self, page_id: usize) -> Vec<Range<u32>>;

    /// Clears all pages and allocations.
    fn reset(&mut self);

    /// Retrieves the [`InstanceAllocation`] for an entity if it exists.
    fn get(&self, entity: Entity) -> Option<InstanceAllocation>;

    /// Returns the number of active pages.
    fn page_count(&self) -> usize;
}

#[derive(Clone, Copy)]
pub struct BatchKey<M: Asset> {
    pub material: AssetId<M>,
    pub mesh: AssetId<Mesh>,
    pub gpu_cull: bool,
}

impl<M: InstancedMaterial> Hash for BatchKey<M> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.material.hash(state);
        self.mesh.hash(state);
        self.gpu_cull.hash(state);
    }
}

impl<M: InstancedMaterial> PartialEq<Self> for BatchKey<M> {
    fn eq(&self, other: &Self) -> bool {
        self.material == other.material
            && self.mesh == other.mesh
            && self.gpu_cull == other.gpu_cull
    }
}

impl<M: InstancedMaterial> Eq for BatchKey<M> {}

#[derive(Clone, Debug)]
pub struct BatchInfo {
    pub page: usize,
    pub range: Range<u32>,
}

#[derive(Default)]
pub struct BatchRanges {
    pub batches: Vec<BatchInfo>,
    pub representatives: Vec<(Entity, MainEntity)>,
    pub batch_lookup: EntityHashMap<u32>,
}

impl BatchRanges {
    pub fn clear(&mut self) {
        self.batches.clear();
        self.representatives.clear();
        self.batch_lookup.clear();
    }
}

#[derive(Clone, Copy, Pod, Zeroable, Default, ShaderType, Debug)]
#[repr(C)]
pub struct BatchMetadata {
    pub batch_id: u32,
    pub start_index: u32,
    pub end_index: u32,
}
