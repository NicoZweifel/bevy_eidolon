/// This example is for stress testing and showcases a scene with chunks being constantly replaced.
///
/// There is a configurable Resource but be careful, since you can effectively ddos your cpu/gpu by spawning to many chunks/instances.
///
/// **NOTE:** Photosensitive Warning. This example contains flashing colors that could trigger a seizure for individuals with photosensitivity.
#[path = "utils/example.rs"]
mod example;

use bevy_app::{App, AppExit, Startup, Update};
use bevy_asset::{Assets, RenderAssetUsages};
use bevy_camera::primitives::Aabb;
use bevy_color::Color;
use bevy_ecs::prelude::*;
use bevy_eidolon::prelude::*;
use bevy_inspector_egui::quick::ResourceInspectorPlugin;
use bevy_math::{Quat, Vec3, Vec3A};
use bevy_mesh::{Indices, Mesh, Mesh3d, PrimitiveTopology};
use bevy_reflect::Reflect;
use bevy_render::render_resource::PolygonMode;
use bevy_transform::prelude::Transform;
use bevy_utils::default;

use example::*;

use rand::{Rng, rng};
use std::sync::Arc;

fn main() -> AppExit {
    App::new()
        .init_resource::<StressTestConfig>()
        .insert_resource(ExamplePluginOptions {
            show_inspector: true,
        })
        .add_plugins((
            ExamplePlugin,
            ResourceInspectorPlugin::<StressTestConfig>::default(),
            InstancedMaterialCorePlugin,
            InstancedMaterialPlugin::<StandardInstancedMaterial>::default(),
            GpuComputeCullPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                setup.run_if(resource_changed::<StressTestConfig>),
                stress_test_chunk_replacement.run_if(not(resource_changed::<StressTestConfig>)),
            ),
        )
        .run()
}

#[derive(Resource, Reflect)]
#[reflect(Resource)]
struct StressTestConfig {
    pub instances_dim: i32,
    pub spacing: f32,
    pub chunk_x: i32,
    pub chunk_z: i32,
}

#[derive(Component, Clone, Copy)]
struct ChunkGridPosition {
    x: i32,
    z: i32,
}

impl Default for StressTestConfig {
    fn default() -> Self {
        Self {
            instances_dim: 100,
            spacing: 0.5,
            chunk_x: 10,
            chunk_z: 10,
        }
    }
}

fn setup(
    mut cmd: Commands,
    mut instanced_materials: ResMut<Assets<StandardInstancedMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    config: ResMut<StressTestConfig>,
    existing: Query<Entity, With<InstanceMaterialData>>,
) {
    existing.iter().for_each(|e| cmd.entity(e).despawn());
    meshes.ids().collect::<Vec<_>>().into_iter().for_each(|x| {
        meshes.remove(x);
    });
    instanced_materials
        .ids()
        .collect::<Vec<_>>()
        .into_iter()
        .for_each(|x| {
            instanced_materials.remove(x);
        });

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

    let material_handle = instanced_materials.add(StandardInstancedMaterial {
        // Signal to the material that it is in the GPU-driven pipeline (not used currently)
        gpu_cull: true,
        polygon_mode: PolygonMode::Line,
        ..default()
    });

    let instance_count = config.instances_dim.pow(2);

    let StressTestConfig {
        chunk_x,
        chunk_z,
        spacing,
        instances_dim,
    } = *config;

    println!(
        "Spawning {:.2} instances...",
        instance_count * chunk_x * chunk_z
    );

    for chunk_x in 0..chunk_x {
        for chunk_z in 0..chunk_z {
            let chunk_local = Vec3::new(
                chunk_x as f32 * spacing * instances_dim as f32,
                0.0,
                chunk_z as f32 * spacing * instances_dim as f32,
            );

            let instances: Vec<InstanceData> = (-instances_dim / 2..instances_dim / 2)
                .flat_map(|x| (-instances_dim / 2..instances_dim / 2).map(move |z| (x, z)))
                .enumerate()
                .map(|(i, (x, z))| InstanceData {
                    position: chunk_local
                        + Vec3::new(x as f32 * config.spacing, 0.0, z as f32 * config.spacing),
                    scale: 1.0,
                    index: i as u32,
                    ..default()
                })
                .collect();

            let seed_x = chunk_x as f32 * 0.61803398875;
            let seed_z = chunk_z as f32 * 0.754877666;

            let hue = ((seed_x + seed_z) * 360.0).rem_euclid(360.0);

            let color = Color::hsl(hue, 0.7, 0.5).to_linear();

            let instance_material_data = InstanceMaterialData {
                instances: Arc::new(instances),
                color,
                visibility_range: [0.0, 0.0, 2000.0, 2000.0].into(),
            };

            cmd.spawn((
                ChunkGridPosition {
                    x: chunk_x,
                    z: chunk_z,
                },
                Transform::from_xyz(20.0, 0.0, 20.0)
                    .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4)),
                InstancedMeshMaterial(material_handle.clone()),
                Mesh3d(mesh_handle.clone()),
                instance_material_data,
                // Use GPU driven cull pipeline
                GpuCullCompute,
                // Disable frustum culling or provide aabb.
                // NoFrustumCulling,
                Aabb {
                    center: single_aabb.center,
                    half_extents: chunk_local.to_vec3a()
                        + single_aabb.half_extents
                        + Vec3A::new(
                            (config.instances_dim as f32 * config.spacing) / 2.0,
                            0.0,
                            (config.instances_dim as f32 * config.spacing) / 2.0,
                        ),
                },
            ));
        }
    }
}

fn stress_test_chunk_replacement(
    mut cmd: Commands,
    mut query: Query<(
        &ChunkGridPosition,
        Entity,
        &InstanceMaterialData,
        &InstancedMeshMaterial<StandardInstancedMaterial>,
        &Transform,
        &Aabb,
        &Mesh3d,
    )>,
) {
    let mut rng = rng();

    for (chunk_grid_pos, entity, instance_data, material, tf, aabb, mesh) in &mut query {
        if !rng.random_bool(0.01) {
            continue;
        }

        let seed_x = chunk_grid_pos.x as f32 * 0.61803398875;
        let seed_z = chunk_grid_pos.z as f32 * 0.754877666;

        let hue = ((seed_x + seed_z) * 360.0).rem_euclid(360.0);

        let saturation = rng.random_range(0.5..1.0);
        let lightness = rng.random_range(0.3..0.7);

        let color = Color::hsl(hue, saturation, lightness).to_linear();

        let mut instance_data = instance_data.clone();
        instance_data.color = color;

        cmd.entity(entity).despawn();

        cmd.spawn((
            chunk_grid_pos.clone(),
            tf.clone(),
            material.clone(),
            mesh.clone(),
            instance_data,
            GpuCullCompute,
            aabb.clone(),
        ));
    }
}

/// A list of points that will have a line drawn between each consecutive point
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
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, line.points)
        // Required for GPU culling (Indexed drawing)
        .with_inserted_indices(Indices::U32((0..point_count as u32).collect()))
    }
}
