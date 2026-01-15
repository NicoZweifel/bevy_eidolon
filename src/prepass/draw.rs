use crate::render::draw::*;

use bevy_pbr::{SetPrepassViewBindGroup, SetPrepassViewEmptyBindGroup};
use bevy_render::render_phase::SetItemPipeline;

pub type DrawInstancedPrepass<M> = (
    SetItemPipeline,
    SetPrepassViewBindGroup<0>,
    SetPrepassViewEmptyBindGroup<1>,
    SetInstanceBindGroup<2>,
    SetMaterialBindGroup<M, 3>,
    DrawInstancedMaterialMesh<M>,
);
