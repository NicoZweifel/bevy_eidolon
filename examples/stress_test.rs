/// This example is for stress testing and showcases a scene with chunks being constantly replaced.
///
/// There is a configurable Resource but be careful, since you can effectively ddos your cpu/gpu by spawning to many chunks/instances.
///
/// **NOTE:** Photosensitive Warning. This example contains flashing colors that could trigger a seizure for individuals with photosensitivity.
#[path = "utils/example.rs"]
mod example;

use bevy_app::{App, AppExit, Startup, Update};
use bevy_asset::{Assets, Handle, RenderAssetUsages};
use bevy_camera::primitives::Aabb;
use bevy_color::Color;
use bevy_ecs::prelude::*;
use bevy_eidolon::prelude::*;
use bevy_inspector_egui::quick::ResourceInspectorPlugin;
use bevy_math::{IVec2, Vec3, Vec3A};
use bevy_mesh::{
    CuboidMeshBuilder, Indices, Mesh, Mesh3d, MeshBuilder, PrimitiveTopology, SphereKind,
    SphereMeshBuilder,
};
use bevy_reflect::Reflect;
use bevy_render::render_resource::PolygonMode;
use bevy_transform::prelude::Transform;
use bevy_utils::default;

use example::*;

use rand::{Rng, rng};
use std::sync::Arc;
use bevy_camera::visibility::NoAutoAabb;

fn main() -> AppExit {
    App::new()
        .init_resource::<StressTestConfig>()
        .register_type::<MeshMode>()
        .insert_resource(ExamplePluginOptions {
            show_inspector: true,
        })
        .add_plugins((
            ExamplePlugin,
            ResourceInspectorPlugin::<StressTestConfig>::default(),
            InstancedMaterialCorePlugin,
            InstancedMaterialPlugin::<StandardInstancedMaterial>::default(),
            GpuComputeCullCorePlugin,
            GpuCullComputePlugin::<StandardInstancedMaterial>::default(),
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

#[derive(Reflect, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshMode {
    #[default]
    Mixed,
    Line,
    Cube,
    Sphere,
}

#[derive(Resource, Reflect)]
#[reflect(Resource)]
struct StressTestConfig {
    pub instances_dim: i32,
    pub spacing: f32,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub mesh_mode: MeshMode,
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
            mesh_mode: MeshMode::Mixed,
        }
    }
}

#[derive(Resource)]
struct StressTestMeshes {
    line: Handle<Mesh>,
    cube: Handle<Mesh>,
    sphere: Handle<Mesh>,
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

    let line_mesh = meshes.add(Mesh::from(LineStrip {
        points: vec![
            Vec3::new(0.0, -0.25, 0.0),
            Vec3::new(0.1, 0.0, 0.0),
            Vec3::new(-0.1, 0.25, 0.0),
            Vec3::new(0.0, 0.5, 0.0),
        ],
    }));

    let cube_mesh = meshes.add(CuboidMeshBuilder::default().build());
    let sphere_mesh =
        meshes.add(SphereMeshBuilder::new(0.3, SphereKind::Ico { subdivisions: 0 }).build());

    cmd.insert_resource(StressTestMeshes {
        line: line_mesh.clone(),
        cube: cube_mesh.clone(),
        sphere: sphere_mesh.clone(),
    });

    let material_handle = instanced_materials.add(StandardInstancedMaterial {
        polygon_mode: PolygonMode::Fill,
        ..default()
    });

    let StressTestConfig {
        chunk_x, chunk_z, ..
    } = *config;
    let mut rng = rng();

    let instance_count = config.instances_dim.pow(2);
    println!(
        "Spawning {:.2} instances...",
        instance_count * chunk_x * chunk_z
    );

    let meshes = StressTestMeshes {
        line: line_mesh,
        cube: cube_mesh,
        sphere: sphere_mesh,
    };

    for x in 0..chunk_x {
        for y in 0..chunk_z {
            spawn_chunk(
                IVec2::new(x, y),
                &mut cmd,
                &config,
                &material_handle,
                &meshes,
                &mut rng,
            );
        }
    }
}

fn spawn_chunk(
    chunk: IVec2,
    cmd: &mut Commands,
    cfg: &StressTestConfig,
    material: &Handle<StandardInstancedMaterial>,
    meshes: &StressTestMeshes,
    rng: &mut impl Rng,
) {
    let chunk_local = Vec3::new(
        chunk.x as f32 * cfg.spacing * cfg.instances_dim as f32,
        0.0,
        chunk.y as f32 * cfg.spacing * cfg.instances_dim as f32,
    );

    let all_meshes = vec![
        meshes.line.clone(),
        meshes.cube.clone(),
        meshes.sphere.clone(),
    ];
    let mesh = match cfg.mesh_mode {
        MeshMode::Mixed => all_meshes[rng.random_range(0..all_meshes.len())].clone(),
        MeshMode::Line => meshes.line.clone(),
        MeshMode::Cube => meshes.cube.clone(),
        MeshMode::Sphere => meshes.sphere.clone(),
    };

    let instances: Vec<InstanceData> = (-cfg.instances_dim / 2..cfg.instances_dim / 2)
        .flat_map(|x| (-cfg.instances_dim / 2..cfg.instances_dim / 2).map(move |z| (x, z)))
        .map(|(x, z)| InstanceData {
            position: chunk_local + Vec3::new(x as f32 * cfg.spacing, 0.0, z as f32 * cfg.spacing),
            scale: 1.0,
            ..default()
        })
        .collect();

    let hue = ((chunk.x as f32 * 0.618 + chunk.y as f32 * 0.754) * 360.0).rem_euclid(360.0);
    let color = Color::hsl(hue, 0.7, 0.5).to_linear();

    cmd.spawn((
        ChunkGridPosition {
            x: chunk.x,
            z: chunk.y,
        },
        Transform::from_translation(Vec3::ZERO),
        InstancedMeshMaterial(material.clone()),
        Mesh3d(mesh),
        InstanceMaterialData {
            instances: Arc::new(instances),
            color,
            visibility_range: [0.0, 0.0, 2000.0, 2000.0].into(),
        },
        GpuCullCompute,
        // required since bevy 0.18 if adding Aabb manually.
        NoAutoAabb,
        Aabb {
            center: Vec3A::ZERO,
            half_extents: Vec3A::splat(1000.0),
        },
    ));
}

fn stress_test_chunk_replacement(
    mut cmd: Commands,
    mut query: Query<(
        &ChunkGridPosition,
        Entity,
        &InstancedMeshMaterial<StandardInstancedMaterial>,
    )>,
    config: Res<StressTestConfig>,
    meshes_res: Res<StressTestMeshes>,
) {
    let mut rng = rng();

    for (chunk_grid_pos, entity, material) in &mut query {
        if !rng.random_bool(0.001) {
            continue;
        }

        cmd.entity(entity).despawn();

        spawn_chunk(
            IVec2::new(chunk_grid_pos.x, chunk_grid_pos.z),
            &mut cmd,
            &config,
            &material.0,
            &meshes_res,
            &mut rng,
        );
    }
}

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
        .with_inserted_indices(Indices::U32((0..point_count as u32).collect()))
    }
}
