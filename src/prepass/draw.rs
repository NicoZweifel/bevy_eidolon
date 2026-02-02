use crate::render::draw::draw_instanced_material::SetInstanceBindGroup;
use crate::render::draw::draw_instanced_material_mesh::DrawInstancedMaterialMesh;
use crate::render::draw::set_material_bind_group::SetMaterialBindGroup;
use bevy_pbr::{SetPrepassViewBindGroup, SetPrepassViewEmptyBindGroup};
use bevy_render::render_phase::SetItemPipeline;

pub type DrawInstancedPrepass<M> = (
    SetItemPipeline,
    SetPrepassViewBindGroup<0>,
    SetPrepassViewEmptyBindGroup<1>,
    SetInstanceBindGroup<M, 2>,
    SetMaterialBindGroup<M, 3>,
    DrawInstancedMaterialMesh<M>,
);
