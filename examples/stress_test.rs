#[path = "utils/example.rs"]
mod example;

use bevy_app::{App, AppExit, Startup};
use bevy_asset::{Assets, RenderAssetUsages};
use bevy_camera::primitives::Aabb;
use bevy_camera::visibility::Visibility;
use bevy_color::palettes::tailwind::*;
use bevy_ecs::prelude::*;
use bevy_eidolon::prelude::*;
use bevy_math::{Vec3, Vec3A};
use bevy_mesh::{Indices, Mesh, Mesh3d, MeshBuilder, PlaneMeshBuilder, PrimitiveTopology};
use bevy_pbr::{MeshMaterial3d, StandardMaterial};
use bevy_render::batching::NoAutomaticBatching;
use bevy_render::render_resource::PolygonMode;
use bevy_transform::prelude::Transform;
use bevy_utils::default;

use std::sync::Arc;

use example::*;

fn main() -> AppExit {
    App::new()
        .add_plugins((ExamplePlugin, InstancedMaterialPlugin))
        .add_systems(Startup, setup)
        .run()
}

fn setup(
    mut cmd: Commands,
    mut instanced_materials: ResMut<Assets<InstancedMaterial>>,
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

    let single_aabb = Aabb {
        center: Vec3A::new(0.0, 0.125, 0.0),
        half_extents: Vec3A::new(0.1, 0.375, 0.0),
    };

    let material_handle = instanced_materials.add(InstancedMaterial {
        // Signal to the material that it is in the GPU-driven pipeline (not used currently)
        gpu_cull: true,
        polygon_mode: PolygonMode::Line,
        ..default()
    });

    const SIDE_LENGTH: i32 = 1400;
    const SPACING: f32 = 0.1;

    let instances: Vec<InstanceData> = (-SIDE_LENGTH / 2..SIDE_LENGTH / 2)
        .flat_map(|x| (-SIDE_LENGTH / 2..SIDE_LENGTH / 2).map(move |z| (x, z)))
        .enumerate()
        .map(|(i, (x, z))| InstanceData {
            position: Vec3::new(x as f32 * SPACING, 0.0, z as f32 * SPACING),
            scale: 1.0,
            index: i as u32,
            ..default()
        })
        .collect();

    let instance_count = instances.len();
    println!("Spawning {} instances...", instance_count);

    let instance_material_data = InstanceMaterialData {
        instances: Arc::new(instances),
        color: GREEN_500.into(),
        visibility_range: [0.0, 0.0, 2000.0, 2000.0].into(),
    };

    cmd.spawn((
        InstancedMeshMaterial(material_handle),
        Mesh3d(mesh_handle),
        instance_material_data,
        NoAutomaticBatching,
        Transform::default(),
        Visibility::Visible,
        // Use GPU driven pipeline
        GpuCull,
        // Disable frustum culling or provide aabb.
        // NoFrustumCulling,
        Aabb {
            center: single_aabb.center,
            half_extents: single_aabb.half_extents
                + Vec3A::new(
                    (SIDE_LENGTH as f32 * SPACING) / 2.0,
                    0.0,
                    (SIDE_LENGTH as f32 * SPACING) / 2.0,
                ),
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
            PrimitiveTopology::LineStrip,
            RenderAssetUsages::RENDER_WORLD,
        )
        // Required for GPU culling (Indexed drawing)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, line.points)
        .with_inserted_indices(Indices::U32((0..point_count as u32).collect()))
    }
}
