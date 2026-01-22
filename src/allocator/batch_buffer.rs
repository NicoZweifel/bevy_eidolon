use crate::allocator::resources::BatchMetadata;
use crate::components::InstanceUniforms;

use bevy_render::{
    render_resource::{Buffer, DrawIndexedIndirectArgs},
    renderer::RenderQueue,
};
use bevy_utils::default;
use bytemuck::{Pod, cast_slice};
use std::ops::RangeInclusive;

#[cfg(feature = "trace")]
use tracing::trace;

#[derive(Clone, Copy, Default)]
pub struct BatchData {
    pub indirect: DrawIndexedIndirectArgs,
    pub uniform: InstanceUniforms,
    pub metadata: BatchMetadata,
}

/// Manages CPU-side mirrors of per-page GPU buffers during the prepare stage.
/// Tracks dirty ranges to perform sparse uploads, i.e. O(changes) instead of O(total).
#[derive(Default)]
pub struct BatchBuffer {
    pub indirect: Vec<DrawIndexedIndirectArgs>,
    pub uniforms: Vec<InstanceUniforms>,
    pub metadata: Vec<BatchMetadata>,

    /// Indices that have changed since the last flush.
    pub dirty_indices: Vec<usize>,

    /// Capacity of the underlying [`Vec`]'s and [`Buffer`]'s.
    pub capacity: usize,
}

impl BatchBuffer {
    #[inline]
    pub fn ensure_capacity(&mut self, capacity: usize) {
        if capacity > self.capacity {
            #[cfg(feature = "trace")]
            trace!(
                "BatchBuffer: Growing capacity {} -> {}",
                self.capacity, capacity
            );

            self.indirect.resize(capacity, default());
            self.uniforms.resize(capacity, default());
            self.metadata.resize(capacity, default());
            self.capacity = capacity;
        }
    }

    pub fn update(&mut self, batch: usize, data: BatchData) {
        if batch >= self.capacity {
            self.ensure_capacity(batch + 1);
        }

        self.indirect[batch] = data.indirect;
        self.uniforms[batch] = data.uniform;
        self.metadata[batch] = data.metadata;

        self.dirty_indices.push(batch);
    }

    /// Marks all currently used indices as dirty.
    /// Used when the underlying GPU buffer is resized/recreated and needs a full refresh.
    pub fn mark_all_dirty(&mut self) {
        self.dirty_indices.clear();
        self.dirty_indices.extend(0..self.capacity);
    }

    pub fn clear(&mut self, batch: usize) {
        if batch >= self.capacity {
            return;
        }

        // avoid unnecessary writes if it's already zeroed
        let zeroed = cast_slice::<_, u8>(std::slice::from_ref(&self.metadata[batch]))
            .iter()
            .all(|&b| b == 0);

        if !zeroed {
            self.indirect[batch] = default();
            self.uniforms[batch] = default();
            self.metadata[batch] = default();
            self.dirty_indices.push(batch);
        }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Uploads the dirty ranges to the GPU.
    pub fn flush_to_gpu(
        &mut self,
        render_queue: &RenderQueue,
        indirect_buffer: &Buffer,
        uniform_buffer: &Buffer,
        metadata_buffer: &Buffer,
    ) {
        if self.dirty_indices.is_empty() {
            return;
        }

        self.dirty_indices.sort_unstable();
        self.dirty_indices.dedup();

        #[cfg(feature = "trace")]
        trace!(
            "BatchBuffer: Flushing {} dirty batches to GPU",
            self.dirty_indices.len()
        );

        let mut i = 0;
        while i < self.dirty_indices.len() {
            let start = self.dirty_indices[i];
            let mut end = start;

            // find contiguous dirty indices
            while let Some(&next) = self.dirty_indices.get(i + 1) {
                if next != end + 1 {
                    break;
                }
                end = next;
                i += 1;
            }

            self.write_buffers(WriteBuffersContext {
                start,
                end,
                render_queue,
                indirect_buffer,
                uniform_buffer,
                metadata_buffer,
            });

            i += 1;
        }

        self.dirty_indices.clear();
    }

    pub fn reset(&mut self) {
        self.indirect.clear();
        self.uniforms.clear();
        self.metadata.clear();
        self.dirty_indices.clear();
    }

    #[inline(always)]
    fn write_buffers(&mut self, ctx: WriteBuffersContext) {
        self.write_buffer::<DrawIndexedIndirectArgs>(ctx);
        self.write_buffer::<InstanceUniforms>(ctx);
        self.write_buffer::<BatchMetadata>(ctx);
    }

    #[inline(always)]
    fn write_buffer<T: BatchBufferWritable>(&self, ctx: WriteBuffersContext) {
        let WriteBufferContext {
            range,
            render_queue,
            buffer,
        } = T::ctx(ctx);

        let offset = (range.start() * size_of::<T>()) as u64;
        let data = T::get(self, range);

        render_queue.write_buffer(buffer, offset, cast_slice(data));
    }
}

#[derive(Clone, Copy)]
struct WriteBuffersContext<'a> {
    start: usize,
    end: usize,
    render_queue: &'a RenderQueue,
    indirect_buffer: &'a Buffer,
    uniform_buffer: &'a Buffer,
    metadata_buffer: &'a Buffer,
}

#[derive(Clone)]
struct WriteBufferContext<'a> {
    range: RangeInclusive<usize>,
    render_queue: &'a RenderQueue,
    buffer: &'a Buffer,
}

trait BatchBufferWritable: Pod {
    fn get(buffer: &BatchBuffer, range: RangeInclusive<usize>) -> &[Self];

    fn ctx(ctx: WriteBuffersContext) -> WriteBufferContext;
}

macro_rules! batch_buffer_writable {
    ($type:ty, $buffer_field:ident, $ctx_field:ident) => {
        impl BatchBufferWritable for $type {
            #[inline(always)]
            fn get(buffer: &BatchBuffer, range: RangeInclusive<usize>) -> &[Self] {
                &buffer.$buffer_field[range]
            }

            #[inline(always)]
            fn ctx(ctx: WriteBuffersContext) -> WriteBufferContext {
                WriteBufferContext {
                    range: ctx.start..=ctx.end,
                    render_queue: ctx.render_queue,
                    buffer: ctx.$ctx_field,
                }
            }
        }
    };
}

batch_buffer_writable!(DrawIndexedIndirectArgs, indirect, indirect_buffer);
batch_buffer_writable!(InstanceUniforms, uniforms, uniform_buffer);
batch_buffer_writable!(BatchMetadata, metadata, metadata_buffer);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_grow() {
        // Arrange
        let mut batcher = BatchBuffer::default();

        // Act
        batcher.update(10, BatchData::default());

        // Assert
        assert!(batcher.capacity >= 11);
        assert_eq!(batcher.dirty_indices.len(), 1);
    }

    #[test]
    fn test_should_track() {
        // Arrange
        let mut batcher = BatchBuffer::default();

        // Act
        batcher.update(0, BatchData::default());
        batcher.update(1, BatchData::default());
        batcher.update(5, BatchData::default());

        // Assert
        assert_eq!(batcher.dirty_indices, vec![0, 1, 5]);
    }
}
