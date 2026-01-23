use bevy_ecs::prelude::*;
use std::ops::Range;

use super::paged_allocator::PagedAllocator;
use super::prelude::*;

/// The [`InstanceAllocator`] backend used by the global instance resource.
///
/// This is a Wrapper for the active [`InstanceAllocator`] implementation.
#[derive(Resource)]
pub enum InstanceAllocatorBackend {
    /// Paged allocator that grows by adding new fixed-size buffers.
    ///
    /// See [`PagedAllocator`].
    Paged(PagedAllocator),
    /// Dependency injection of allocation strategies.
    ///
    /// Might be reworked with generics at some point.
    Custom(Box<dyn InstanceAllocator>),
}

impl Default for InstanceAllocatorBackend {
    fn default() -> Self {
        Self::Paged(PagedAllocator::default())
    }
}

impl InstanceAllocator for InstanceAllocatorBackend {
    fn alloc(&mut self, entity: Entity, count: u32) -> Option<InstanceAllocation> {
        match self {
            Self::Paged(a) => a.alloc(entity, count),
            Self::Custom(c) => c.alloc(entity, count),
        }
    }
    fn free(&mut self, entity: Entity) {
        match self {
            Self::Paged(a) => a.free(entity),
            Self::Custom(c) => c.free(entity),
        }
    }
    fn size(&self, page_id: usize) -> u32 {
        match self {
            Self::Paged(a) => a.size(page_id),
            Self::Custom(c) => c.size(page_id),
        }
    }
    fn drain(&mut self, page_id: usize) -> Vec<Range<u32>> {
        match self {
            Self::Paged(a) => a.drain(page_id),
            Self::Custom(c) => c.drain(page_id),
        }
    }
    fn reset(&mut self) {
        match self {
            Self::Paged(a) => a.reset(),
            Self::Custom(c) => c.reset(),
        }
    }
    fn get_location(&self, entity: Entity) -> Option<InstanceAllocation> {
        match self {
            Self::Paged(a) => a.get_location(entity),
            Self::Custom(c) => c.get_location(entity),
        }
    }
    fn page_count(&self) -> usize {
        match self {
            Self::Paged(a) => a.page_count(),
            Self::Custom(c) => c.page_count(),
        }
    }
}
