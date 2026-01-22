//! Manages the allocation of GPU instance indices for entities.
//!
//! Provides [`InstanceAllocator`], a trait for mapping entities to contiguous
//! GPU buffer ranges. The default backend, [`PagedAllocator`], handles fragmentation and
//! dynamic growth using multiple fixed-size pages and O(1) offset allocation by wrapping [`offset_allocator`].

pub mod backend;
pub mod batch_buffer;
pub mod core;
pub mod id_allocator;
pub mod paged_allocator;
pub mod plugin;
pub mod resources;

pub mod prelude {
    pub use super::backend::*;
    pub use super::core::*;
    pub use super::plugin::*;
    pub use super::resources::*;
}
