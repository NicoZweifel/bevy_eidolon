#define_import_path bevy_eidolon::render::bindings

#import bevy_eidolon::render::types::{MaterialUniforms, InstanceUniforms}

@group(3) @binding(0) var<uniform> material: MaterialUniforms;
@group(3) @binding(100) var<uniform> instance_uniforms: InstanceUniforms;
