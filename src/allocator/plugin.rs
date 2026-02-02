use bevy_app::{App, Plugin};
use bevy_render::RenderApp;
use std::marker::PhantomData;

use crate::allocator::prelude::*;
use crate::material::InstancedMaterial;

pub struct AllocatorPlugin<M>(PhantomData<M>);

impl<M> Default for AllocatorPlugin<M> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<M: InstancedMaterial> Plugin for AllocatorPlugin<M> {
    fn build(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);

        render_app
            .init_resource::<InstanceAllocatorBackend>()
            .init_resource::<MaterialBatchRanges<M>>()
            .init_resource::<GlobalInstanceAllocator<M>>();
    }
}
