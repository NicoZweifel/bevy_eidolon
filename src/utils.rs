use bevy_asset::{AssetPath, AssetServer, Handle, embedded_path};
use bevy_shader::{Shader, ShaderRef};

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
