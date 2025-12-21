#[path = "utils/example.rs"]
mod example;

use bevy_app::{App, AppExit, Startup};
use bevy_asset::{Assets, RenderAssetUsages};
use bevy_camera::primitives::Aabb;
use bevy_color::palettes::tailwind::*;
use bevy_ecs::prelude::*;
use bevy_eidolon::prelude::*;
use bevy_math::{Vec3, Vec3A};
use bevy_mesh::{Indices, Mesh, Mesh3d, PrimitiveTopology};
use bevy_render::render_resource::PolygonMode;
use bevy_utils::default;

use example::*;
use std::sync::Arc;

fn main() -> AppExit {
    App::new()
        .add_plugins((
            ExamplePlugin,
            InstancedMaterialCorePlugin,
            InstancedMaterialPlugin::<StandardInstancedMaterial>::default(),
        ))
        .add_systems(Startup, setup)
        .run()
}

fn setup(
    mut cmd: Commands,
    mut instanced_materials: ResMut<Assets<StandardInstancedMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let line_strip = LineStrip {
        points: vec![
            Vec3::new(0.0, -0.25, 0.0),
            Vec3::new(0.1, 0.0, 0.0),
            Vec3::new(-0.1, 0.25, 0.0),
            Vec3::new(0.0, 0.5, 0.0),
        ],
    };

    let mesh_handle = meshes.add(Mesh::from(line_strip));

    let aabb = Aabb {
        center: Vec3A::new(0.0, 0.125, 0.0),
        half_extents: Vec3A::new(0.1, 0.375, 0.0),
    };

    let material_handle = instanced_materials.add(StandardInstancedMaterial {
        polygon_mode: PolygonMode::Line,
        ..default()
    });

    const SIZE: i32 = 10;

    let instances = (-SIZE..SIZE)
        .enumerate()
        .map(|(i, x)| InstanceData {
            position: Vec3::new(x as f32, 0.25 * 4., x as f32),
            scale: 4.0,
            index: i as u32,
            ..default()
        })
        .collect();

    let instance_material_data = InstanceMaterialData {
        instances: Arc::new(instances),
        color: GREEN_500.into(),
        visibility_range: [0.0, 0.0, 1000.0, 1000.0].into(),
    };

    cmd.spawn((
        InstancedMeshMaterial(material_handle),
        Mesh3d(mesh_handle),
        instance_material_data,
        Aabb {
            center: aabb.center,
            half_extents: aabb.half_extents * SIZE as f32 * 2.,
        },
    ));
}

/// A list of points that will have a line drawn between each consecutive points
#[derive(Debug, Clone)]
struct LineStrip {
    points: Vec<Vec3>,
}

impl From<LineStrip> for Mesh {
    fn from(line: LineStrip) -> Self {
        let point_count = line.points.len();
        Mesh::new(
            // This tells wgpu that the positions are a list of points
            // where a line will be drawn between each consecutive point
            PrimitiveTopology::LineStrip,
            RenderAssetUsages::RENDER_WORLD,
        )
        // Add the point positions as an attribute
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, line.points)
        // Indexed drawing
        .with_inserted_indices(Indices::U32((0..point_count as u32).collect()))
    }
}
