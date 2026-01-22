#define_import_path bevy_eidolon::cull::bindings

#import bevy_eidolon::cull::types::{InstanceData, InstanceUniforms, DrawIndexedIndirectArgs, LodCullData, CameraCullData, BatchMetadata}

@group(0) @binding(0) var<storage, read> source_buffer: array<InstanceData>;
@group(0) @binding(1) var<storage, read_write> instance_buffer: array<InstanceData>;
@group(0) @binding(2) var<storage, read_write> indirect_args: array<DrawIndexedIndirectArgs>;
@group(0) @binding(3) var<storage, read> batch_offsets: array<BatchMetadata>;

@group(1) @binding(0) var<storage, read> instance_uniforms: array<InstanceUniforms>;

@group(2) @binding(0) var<uniform> camera: CameraCullData;
