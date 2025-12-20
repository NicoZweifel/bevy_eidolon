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
use bevy_mesh::{CuboidMeshBuilder, Mesh, Mesh3d, MeshBuilder, MeshVertexBufferLayoutRef};
use bevy_reflect::TypePath;
use bevy_render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy_utils::default;

use bevy::prelude::Image;
use bevy_shader::ShaderRef;
use bevy_transform::prelude::Transform;

use example::*;


use std::sync::Arc;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[uniform(0, CustomMaterialUniform)]
#[bind_group_data(CustomMaterialKey)]
struct CustomMaterial {
    // Another color to multiply with the existing colors
    pub color: LinearRgba,
    // A custom shader def, using a key, makes the fragment shader return red to demonstrate
    pub is_red: bool,

    // A texture that gets sampled in the fragment shader
    #[texture(1)]
    #[sampler(2)]
    pub texture: Handle<Image>,
}

#[derive(Clone, Default, ShaderType, Debug)]
struct CustomMaterialUniform {
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

    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: Self::Data,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if key.is_red {
            let fragment = descriptor.fragment.as_mut().unwrap();
            fragment.shader_defs.push("IS_RED".into());
        }
        Ok(())
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
        texture: texture_handle.clone(),
        is_red: false,
    });

    let red_material_handle = custom_materials.add(CustomMaterial {
        color: BLUE_500.into(),
        texture: texture_handle.clone(),
        is_red: true,
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

    let (instances, red_instances) = instances.iter().fold(
        (Vec::new(), Vec::new()),
        |(mut data, mut red_data), instance| {
            if instance.index % 2 == 0 {
                data.push(instance.clone());
            } else {
                red_data.push(instance.clone());
            }
            (data, red_data)
        },
    );

    let instance_material_data = InstanceMaterialData {
        instances: Arc::new(instances),
        color: Color::WHITE.into(),
        visibility_range: [0.0, 0.0, 1000.0, 1000.0].into(),
    };

    let red_instance_material_data = InstanceMaterialData {
        instances: Arc::new(red_instances),
        color: Color::WHITE.into(),
        visibility_range: [0.0, 0.0, 1000.0, 1000.0].into(),
    };

    let tf = Transform::default();

    cmd.spawn((
        tf.clone(),
        InstancedMeshMaterial(material_handle),
        Mesh3d(mesh_handle.clone()),
        instance_material_data,
        Aabb {
            center: Vec3A::ZERO,
            half_extents: Vec3A::splat(SIZE as f32 * SPACING),
        },
    ));

    cmd.spawn((
        tf,
        InstancedMeshMaterial(red_material_handle),
        Mesh3d(mesh_handle),
        red_instance_material_data,
        Aabb {
            center: Vec3A::ZERO,
            half_extents: Vec3A::splat(SIZE as f32 * SPACING),
        },
    ));
}

#[repr(C)]
#[derive(Eq, PartialEq, Hash, Copy, Clone)]
struct CustomMaterialKey {
    is_red: bool,
}

impl From<&CustomMaterial> for CustomMaterialKey {
    fn from(material: &CustomMaterial) -> Self {
        Self {
            is_red: material.is_red,
        }
    }
}
