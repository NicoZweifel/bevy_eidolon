use bevy_render::render_resource::{Buffer, BufferDescriptor, BufferUsages};
use bevy_render::renderer::{RenderDevice, RenderQueue};
use bevy_utils::default;
use tracing::trace;

/// Ensures a GPU buffer has sufficient capacity, resizing and copying data if necessary.
pub(crate) fn ensure_buffer_capacity(
    device: &RenderDevice,
    queue: &RenderQueue,
    buffer_opt: &mut Option<Buffer>,
    capacity: u64,
    usage: BufferUsages,
    label: &str,
    copy: bool,
) {
    let aligned_size = (capacity + 3) & !3; // 4-byte align
    let current_size = buffer_opt.as_ref().map(|b| b.size()).unwrap_or(0);
    if aligned_size <= current_size {
        return;
    }

    let size = aligned_size.max(1024);

    #[cfg(feature = "trace")]
    trace!(
        "Resizing Buffer [{}]: {} bytes -> {} bytes (Copy: {})",
        label, current_size, size, copy
    );

    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size,
        usage,
        mapped_at_creation: false,
    });

    if copy && let Some(old) = buffer_opt {
        let mut encoder = device.create_command_encoder(&default());
        let copy_size = old.size().min(size);

        encoder.copy_buffer_to_buffer(old, 0, &buffer, 0, copy_size);

        queue.submit(Some(encoder.finish()));
    }

    *buffer_opt = Some(buffer);
}
