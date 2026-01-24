//! Defines the types and protocol for the module.

use bevy_ecs::entity::Entity;
use std::ops::Range;

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
