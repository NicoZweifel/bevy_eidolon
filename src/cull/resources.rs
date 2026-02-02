use bevy_ecs::prelude::Resource;
use bevy_math::Vec4;
use bevy_render::render_resource::{BindGroup, Buffer, ShaderType};

use bytemuck::{Pod, Zeroable};

#[derive(Clone, Copy, Pod, Zeroable, Default, ShaderType)]
#[repr(C)]
pub struct CameraCullData {
    pub view_pos: Vec4,
    pub frustum: [Vec4; 6],
}

#[derive(Resource)]
pub struct GlobalCullBuffer {
    pub buffer: Buffer,
    pub bind_group: BindGroup,
}
