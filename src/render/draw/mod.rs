use bevy_pbr::{SetMeshViewBindGroup, SetMeshViewBindingArrayBindGroup};
use bevy_render::render_phase::SetItemPipeline;

use crate::render::draw::prelude::*;

pub mod draw_instanced_material;
pub mod draw_instanced_material_mesh;
pub mod set_material_bind_group;

pub type DrawInstancedMaterial<M> = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshViewBindingArrayBindGroup<1>,
    SetInstanceBindGroup<M, 2>,
    SetMaterialBindGroup<M, 3>,
    DrawInstancedMaterialMesh<M>,
);

pub mod prelude {
    pub use super::DrawInstancedMaterial;
    pub use super::draw_instanced_material::*;
    pub use super::draw_instanced_material_mesh::*;
    pub use super::set_material_bind_group::*;
}
