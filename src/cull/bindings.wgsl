#define_import_path bevy_eidolon::cull::bindings

#import bevy_eidolon::cull::types::{InstanceData, DrawIndexedIndirectArgs, LodCullData, CameraCullData}

@group(0) @binding(0) var<storage, read> source_buffer: array<InstanceData>;
@group(0) @binding(1) var<storage, read_write> instance_buffer: array<InstanceData>;
@group(0) @binding(2) var<storage, read_write> indirect_args: DrawIndexedIndirectArgs;
@group(0) @binding(3) var<uniform> lod_data: LodCullData;

@group(1) @binding(0) var<uniform> camera: CameraCullData;
