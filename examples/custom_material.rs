#[path = "utils/example.rs"]
mod example;

use bevy_app::{App, AppExit, Startup};
use bevy_asset::{Asset, AssetServer, Assets, Handle};
use bevy_camera::primitives::Aabb;
use bevy_color::palettes::tailwind::*;
use bevy_color::{Color, ColorToComponents, LinearRgba};
use bevy_ecs::prelude::*;
use bevy_eidolon::prelude::*;
use bevy_math::{Vec3, Vec3A, Vec4};
use bevy_mesh::{CuboidMeshBuilder, Mesh, Mesh3d, MeshBuilder};
use bevy_reflect::TypePath;
use bevy_render::render_resource::{AsBindGroup, ShaderType};
use bevy_utils::default;

use bevy::prelude::Image;
use bevy_shader::ShaderRef;
use example::*;
use std::sync::Arc;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[uniform(0, CustomMaterialUniform)]
pub struct CustomMaterial {
    pub color: LinearRgba,

    #[texture(1)]
    #[sampler(2)]
    pub texture: Handle<Image>,
}

#[derive(Clone, Default, ShaderType, Debug)]
pub struct CustomMaterialUniform {
    pub color: Vec4,
}

impl From<&CustomMaterial> for CustomMaterialUniform {
    fn from(material: &CustomMaterial) -> Self {
        Self {
            color: material.color.to_vec4(),
        }
    }
}

impl InstancedMaterial for CustomMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/custom_material.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/custom_material.wgsl".into()
    }
}

fn main() -> AppExit {
    App::new()
        .add_plugins((
            ExamplePlugin,
            InstancedMaterialCorePlugin,
            InstancedMaterialPlugin::<CustomMaterial>::default(),
        ))
        .add_systems(Startup, setup)
        .run()
}

fn setup(
    mut cmd: Commands,
    mut custom_materials: ResMut<Assets<CustomMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    let texture_handle = asset_server.load("icon.png");

    let material_handle = custom_materials.add(CustomMaterial {
        color: BLUE_500.into(),
        texture: texture_handle,
    });

    let mesh_handle = meshes.add(CuboidMeshBuilder::default().build());

    const SIZE: i32 = 10;
    const SPACING: f32 = 2.5;

    let instances: Vec<InstanceData> = (-SIZE..SIZE)
        .flat_map(|x| {
            (-SIZE..SIZE).map(move |z| InstanceData {
                position: Vec3::new(x as f32 * SPACING, 0.0, z as f32 * SPACING),
                scale: 1.0,
                index: 0,
                ..default()
            })
        })
        .collect();

    let instance_material_data = InstanceMaterialData {
        instances: Arc::new(instances),
        color: Color::WHITE.into(),
        visibility_range: [0.0, 0.0, 1000.0, 1000.0].into(),
    };

    cmd.spawn((
        InstancedMeshMaterial(material_handle),
        Mesh3d(mesh_handle),
        instance_material_data,
        Aabb {
            center: Vec3A::ZERO,
            half_extents: Vec3A::splat(SIZE as f32 * SPACING),
        },
    ));
}
