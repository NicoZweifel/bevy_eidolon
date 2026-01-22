use crate::components::InstanceData;

pub(super) fn instance_data_offset(offset: u32) -> u64 {
    offset as u64 * size_of::<InstanceData>() as u64
}
