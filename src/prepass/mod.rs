use bevy_ecs::prelude::*;
use bevy_reflect::prelude::*;
use bevy_render::extract_component::ExtractComponent;

pub mod draw;
pub mod pipeline;
pub mod plugin;
pub mod queue;

#[derive(Component, Debug, Clone, Reflect, ExtractComponent)]
#[reflect(Component, Debug)]
pub struct CullComputeCamera;

pub mod prelude {
    pub use crate::prepass::plugin::*;
}
