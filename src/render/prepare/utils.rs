pub(super) fn data_offset<T>(offset: u32) -> u64 {
    offset as u64 * size_of::<T>() as u64
}
