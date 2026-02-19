#define_import_path bevy_eidolon::render::bindings

#import bevy_eidolon::render::types::{MaterialUniforms, InstanceUniforms}

@group(2) @binding(0) var<storage, read> instance_uniforms: array<InstanceUniforms>;
@group(3) @binding(0) var<uniform> material: MaterialUniforms;
