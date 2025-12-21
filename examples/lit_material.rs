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

use bevy::prelude::{Image, Quat};
use bevy_shader::ShaderRef;
use bevy_transform::prelude::Transform;

use example::*;

use bevy_camera::prelude::Visibility;
use std::sync::Arc;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[uniform(0, LitMaterialUniform)]
struct LitMaterial {
    // Another color to multiply with the existing colors
    pub color: LinearRgba,

    // A texture that gets sampled in the fragment shader
    #[texture(1)]
    #[sampler(2)]
    pub texture: Handle<Image>,
}

#[derive(Clone, Default, ShaderType, Debug)]
struct LitMaterialUniform {
    pub color: Vec4,
}

impl From<&LitMaterial> for LitMaterialUniform {
    fn from(material: &LitMaterial) -> Self {
        Self {
            color: material.color.to_vec4(),
        }
    }
}

impl InstancedMaterial for LitMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/lit_material.wgsl".into()
    }
}

fn main() -> AppExit {
    App::new()
        .add_plugins((
            ExamplePlugin,
            InstancedMaterialCorePlugin,
            InstancedMaterialPlugin::<LitMaterial>::default(),
        ))
        .add_systems(Startup, setup)
        .run()
}

fn setup(
    mut cmd: Commands,
    mut custom_materials: ResMut<Assets<LitMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    let texture_handle = asset_server.load("test.png");

    let material_handle = custom_materials.add(LitMaterial {
        color: BLUE_500.into(),
        texture: texture_handle.clone(),
    });

    let mesh_handle = meshes.add(CuboidMeshBuilder::default().build());

    const SIZE: i32 = 10;
    const SPACING: f32 = 2.5;

    let instances: Vec<InstanceData> = (-SIZE..SIZE)
        .enumerate()
        .flat_map(|(i, x)| {
            (-SIZE..SIZE).map(move |z| InstanceData {
                position: Vec3::new(x as f32 * SPACING, 0.0, z as f32 * SPACING),
                scale: 1.0,
                index: i as u32,
                ..default()
            })
        })
        .collect();

    let instance_material_data = InstanceMaterialData {
        instances: Arc::new(instances),
        color: Color::WHITE.into(),
        visibility_range: [0.0, 0.0, 1000.0, 1000.0].into(),
    };

    let tf = Transform::from_xyz(20.0, 0.0, 20.0)
        .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4));

    cmd.spawn((
        tf,
        Visibility::Visible,
        InstancedMeshMaterial(material_handle),
        Mesh3d(mesh_handle.clone()),
        instance_material_data,
        Aabb {
            center: Vec3A::ZERO,
            half_extents: Vec3A::splat(SIZE as f32 * SPACING),
        },
    ));
}
