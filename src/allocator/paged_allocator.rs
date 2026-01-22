use bevy_ecs::entity::{Entity, EntityHashMap};
use offset_allocator::{Allocation, Allocator};
use std::ops::Range;

use crate::allocator::prelude::*;

/// Manages multiple fixed-size pages using O(1) offset allocators.
///
/// Handles fragmentation within pages using [`offset_allocator`].
pub struct PagedAllocator {
    /// O(1) allocators for each memory page.
    pub pages: Vec<Allocator>,
    /// Capacity of a single page.
    pub page_size: u32,
    /// Maps entities to their current allocation.
    pub allocations: EntityHashMap<PageAllocation>,
    /// Ranges queued for GPU upload.
    pub pending_clears: Vec<Vec<Range<u32>>>,
    /// Highest used index per page.
    pub watermarks: Vec<u32>,
    /// Active elements per page, used for empty page detection.
    pub active_counts: Vec<u32>,
}

impl Default for PagedAllocator {
    fn default() -> Self {
        Self {
            pages: Vec::new(),
            page_size: 1_000_000,
            allocations: EntityHashMap::default(),
            pending_clears: Vec::new(),
            watermarks: Vec::new(),
            active_counts: Vec::new(),
        }
    }
}

impl PagedAllocator {
    /// Commits a [`PageAllocation`] and updates the active count, as well as the watermark.
    fn alloc(&mut self, entity: Entity, page_allocation: impl Into<PageAllocation>) -> bool {
        let page_allocation = page_allocation.into();
        let PageAllocation {
            page,
            allocation: Allocation { offset, .. },
            count,
        } = page_allocation;

        self.allocations.insert(entity, page_allocation);
        self.active_counts[page] += count;
        self.update_watermark(page, offset + count);

        true
    }

    /// Adds a new physical page to the pool.
    fn add_page(&mut self) -> usize {
        self.pages.push(Allocator::new(self.page_size));
        self.pending_clears.push(Vec::new());
        self.watermarks.push(0);
        self.active_counts.push(0);

        self.pages.len() - 1
    }

    /// Updates the high-water mark for a page if the new allocation exceeds it.
    fn update_watermark(&mut self, page: usize, end_offset: u32) {
        if end_offset > self.watermarks[page] {
            self.watermarks[page] = end_offset;
        }
    }

    /// Attempts to find space in existing pages before creating a new one.
    fn alloc_existing(&mut self, entity: Entity, count: u32) -> Option<PageAllocation> {
        self.pages
            .iter_mut()
            .enumerate()
            .filter_map(|(page, allocator)| {
                allocator
                    .allocate(count)
                    .map(|allocation| PageAllocation::new(page, allocation, count))
            })
            .next()
            .and_then(|page_allocation| {
                self.alloc(entity, page_allocation)
                    .then_some(page_allocation)
            })
    }

    /// Creates a new page and allocates the entity.
    fn alloc_new(&mut self, entity: Entity, count: u32) -> Option<PageAllocation> {
        let page = self.add_page();
        let allocation = self.pages[page].allocate(count)?;
        let page_allocation = PageAllocation::new(page, allocation, count);

        self.alloc(entity, page_allocation)
            .then_some(page_allocation)
    }

    /// Resets a page to reclaim memory.
    fn reset_page(&mut self, page: usize) {
        self.pages[page] = Allocator::new(self.page_size);
        self.watermarks[page] = 0;
        self.pending_clears[page].clear();
    }
}

impl InstanceAllocator for PagedAllocator {
    fn allocate(&mut self, entity: Entity, count: u32) -> Option<InstanceAllocation> {
        if count == 0 {
            #[cfg(feature = "trace")]
            tracing::warn!("Count is 0!");
            return Some((0, 0).into());
        }

        if count > self.page_size {
            #[cfg(feature = "trace")]
            tracing::warn!("Count exceeds page size! {count} > {}", self.page_size);
            return None;
        }

        self.alloc_existing(entity, count)
            .map(Some)
            .unwrap_or_else(|| self.alloc_new(entity, count))
            .map(|x| x.into())
    }

    fn free(&mut self, entity: Entity) {
        let Some(PageAllocation {
            page,
            allocation,
            count,
        }) = self.allocations.remove(&entity)
        else {
            #[cfg(feature = "trace")]
            tracing::warn!("Attempted to free unknown entity {:?}", entity);
            return;
        };

        if page >= self.pages.len() {
            #[cfg(feature = "trace")]
            tracing::error!("Attempted to free allocation on invalid page {}", page);
            return;
        }

        self.pages[page].free(allocation);
        self.active_counts[page] -= count;

        if self.active_counts[page] == 0 {
            self.reset_page(page);
        } else {
            self.pending_clears[page].push(allocation.offset..(allocation.offset + count));
        }
    }

    fn size(&self, page_id: usize) -> u32 {
        if page_id < self.watermarks.len() {
            self.watermarks[page_id]
        } else {
            0
        }
    }

    fn drain(&mut self, page_id: usize) -> Vec<Range<u32>> {
        if page_id < self.pending_clears.len() {
            std::mem::take(&mut self.pending_clears[page_id])
        } else {
            Vec::new()
        }
    }

    fn reset(&mut self) {
        self.pages.clear();
        self.allocations.clear();
        self.pending_clears.clear();
        self.watermarks.clear();
    }

    fn get_location(&self, entity: Entity) -> Option<InstanceAllocation> {
        self.allocations
            .get(&entity)
            .map(|page_allocation| (*page_allocation).into())
    }

    fn page_count(&self) -> usize {
        self.pages.len()
    }
}

/// Stores the physical location, backend handle, and size of an allocation.
#[derive(Copy, Clone)]
pub struct PageAllocation {
    pub page: usize,
    pub allocation: Allocation,
    pub count: u32,
}

impl PageAllocation {
    pub fn new(page: usize, allocation: Allocation, count: u32) -> Self {
        Self {
            page,
            allocation,
            count,
        }
    }
}

impl From<PageAllocation> for InstanceAllocation {
    fn from(value: PageAllocation) -> Self {
        Self {
            page: value.page,
            offset: value.allocation.offset,
        }
    }
}

impl From<(usize, Allocation, u32)> for PageAllocation {
    fn from((page, allocation, count): (usize, Allocation, u32)) -> Self {
        Self {
            page,
            allocation,
            count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_alloc() {
        // Arrange
        let mut allocator = PagedAllocator::default();
        let e = Entity::from_raw_u32(1).unwrap();

        // Act
        let res = allocator.allocate(e, 100).expect("Alloc failed");

        // Assert
        assert_eq!(res.page, 0);
        assert_eq!(res.offset, 0);
        assert_eq!(allocator.active_counts[0], 100);
        assert_eq!(allocator.watermarks[0], 100);
    }

    #[test]
    fn test_should_free() {
        // Arrange
        let mut alloc = PagedAllocator::default();
        let e = Entity::from_raw_u32(1).unwrap();
        alloc.allocate(e, 100).expect("Alloc failed");

        // Act
        alloc.free(e);

        // Assert
        assert_eq!(alloc.active_counts[0], 0);
        assert_eq!(alloc.watermarks[0], 0);
    }

    #[test]
    fn test_should_overflow_and_alloc_new_page() {
        // Arrange
        let mut allocator = PagedAllocator::default();
        allocator.page_size = 100;

        let e1 = Entity::from_raw_u32(1).unwrap();
        let e2 = Entity::from_raw_u32(2).unwrap();

        allocator.allocate(e1, 80).unwrap(); // Page 1

        // Act
        let allocation = allocator.allocate(e2, 50).unwrap(); // Page 2

        // Assert
        assert_eq!(allocation.page, 1);
        assert_eq!(allocator.pages.len(), 2);
        assert_eq!(allocator.active_counts[0], 80);
        assert_eq!(allocator.active_counts[1], 50);
    }

    #[test]
    fn test_fragmentation_should_reuse() {
        // Arrange
        let mut allocator = PagedAllocator::default();
        let e1 = Entity::from_raw_u32(1).unwrap();
        let e2 = Entity::from_raw_u32(2).unwrap();
        let e3 = Entity::from_raw_u32(3).unwrap();

        // 128 to align with allocator bin precision
        allocator.allocate(e1, 128).unwrap();
        allocator.allocate(e2, 128).unwrap();
        allocator.allocate(e3, 128).unwrap();

        allocator.free(e2);

        // Act
        let allocation = allocator
            .allocate(Entity::from_raw_u32(4).unwrap(), 128)
            .unwrap();

        // Assert
        assert_eq!(allocation.offset, 128, "Should reuse the freed slot");
    }
}
