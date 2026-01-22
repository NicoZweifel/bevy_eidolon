use bevy_asset::{AssetPath, AssetServer, Handle, embedded_path};
use bevy_render::render_resource::Buffer;
use bevy_shader::{Shader, ShaderRef};
use bytemuck::Pod;

pub trait ResolveShaderRef {
    fn resolve(self, asset_server: &AssetServer, default: impl Into<String>) -> Handle<Shader>;
}

impl ResolveShaderRef for ShaderRef {
    fn resolve(self, asset_server: &AssetServer, default: impl Into<String>) -> Handle<Shader> {
        match self {
            ShaderRef::Default => asset_server.load(
                AssetPath::from_path_buf(embedded_path!(default.into())).with_source("embedded"),
            ),
            ShaderRef::Handle(handle) => handle,
            ShaderRef::Path(path) => asset_server.load(path),
        }
    }
}

pub trait BufferBoundsCheck<T> {
    fn check_bounds(&self, offset: u64, data: &[T]) -> bool;
}

impl<T: Pod> BufferBoundsCheck<T> for Buffer {
    #[inline]
    fn check_bounds(&self, offset: u64, data: &[T]) -> bool {
        offset + (data.len() as u64 * size_of::<T>() as u64) <= self.size()
    }
}
