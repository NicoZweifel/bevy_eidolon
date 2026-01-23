use bevy_ecs::entity::{Entity, EntityHashMap};

#[derive(Default)]
pub struct IdAllocator {
    pub watermark: u32,
    pub free_ids: Vec<u32>,
    pub allocations: EntityHashMap<u32>,
}

impl IdAllocator {
    pub fn alloc(&mut self, entity: Entity) -> u32 {
        let id = self.free_ids.pop().unwrap_or_else(|| {
            let watermark = self.watermark;
            self.watermark += 1;
            watermark
        });

        self.allocations.insert(entity, id);
        id
    }

    pub fn free(&mut self, entity: Entity) {
        if let Some(id) = self.allocations.remove(&entity) {
            self.free_ids.push(id);
        }
    }

    pub fn reset(&mut self) {
        self.watermark = 0;
        self.free_ids.clear();
        self.allocations.clear();
    }
}
