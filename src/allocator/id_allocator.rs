use bevy_ecs::entity::{Entity, EntityHashMap};

/// A dense ID allocator that maps sparse [`Entity`] identifiers to contiguous `u32` indices.
///
/// Used to map Entities to indices in a GPU buffer, arrays, or other
/// dense storage mechanisms where gaps are undesirable.
#[derive(Default, Debug)]
pub struct IdAllocator {
    /// High-water mark indicating the next fresh ID to generate if no recycled IDs are available.
    pub watermark: u32,
    /// Stack of previously returned IDs available for reuse.
    pub free_ids: Vec<u32>,
    /// Mapping of Entities to their allocated IDs.
    pub allocations: EntityHashMap<u32>,
}

impl IdAllocator {
    /// Allocates a `u32` ID for the given `Entity`.
    pub fn alloc(&mut self, entity: Entity) -> u32 {
        if let Some(&id) = self.allocations.get(&entity) {
            return id;
        }

        let id = self.free_ids.pop().unwrap_or_else(|| {
            let watermark = self.watermark;
            self.watermark += 1;
            watermark
        });

        self.allocations.insert(entity, id);
        id
    }

    /// Frees the ID associated with the given `Entity`, if it exists.
    pub fn free(&mut self, entity: Entity) {
        if let Some(id) = self.allocations.remove(&entity) {
            self.free_ids.push(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(id: u32) -> Entity {
        Entity::from_raw_u32(id).unwrap()
    }

    #[test]
    fn test_should_alloc() {
        // Arrange
        let mut allocator = IdAllocator::default();

        // Act
        let id = allocator.alloc(entity(1));

        // Assert
        assert_eq!(id, 0);
        assert_eq!(allocator.watermark, 1);
        assert!(allocator.free_ids.is_empty());
    }

    #[test]
    fn test_alloc_should_generate_sequential_ids() {
        // Arrange
        let mut allocator = IdAllocator::default();
        allocator.alloc(entity(1));

        // Act
        let id = allocator.alloc(entity(2));

        // Assert
        assert_eq!(id, 1);
        assert_eq!(allocator.watermark, 2);
        assert!(allocator.free_ids.is_empty());
    }

    #[test]
    fn test_alloc_twice_should_return_same_id() {
        // Arrange
        let mut allocator = IdAllocator::default();
        let e = entity(1);
        allocator.alloc(e);

        // Act
        let id = allocator.alloc(e);

        // Assert
        assert_eq!(id, 0);
        assert_eq!(allocator.watermark, 1);
    }

    #[test]
    fn test_alloc_should_reuse_freed_id() {
        // Arrange
        let mut allocator = IdAllocator::default();
        let e1 = entity(1);
        allocator.alloc(e1);
        allocator.alloc(entity(2));
        allocator.free(e1);

        let e3 = entity(3);

        // Act
        let id = allocator.alloc(e3);

        // Assert
        assert_eq!(id, 0);
        assert!(allocator.free_ids.is_empty());
        assert_eq!(allocator.watermark, 2);
    }

    #[test]
    fn test_free_non_existent_should_not_panic() {
        // Arrange
        let mut allocator = IdAllocator::default();

        // Act
        allocator.free(entity(999));

        // Assert
        assert!(allocator.free_ids.is_empty());
        assert!(allocator.allocations.is_empty());
    }

    #[test]
    fn test_free_should_handle_double_free() {
        // Arrange
        let mut allocator = IdAllocator::default();
        let e = entity(1);
        allocator.alloc(e);
        allocator.free(e);

        // Act
        allocator.free(e);

        // Assert
        assert_eq!(allocator.free_ids.len(), 1);
        assert_eq!(allocator.free_ids[0], 0);
    }
}
