#[path = "utils/example.rs"]
mod example;

use bevy_app::{App, AppExit, Startup};
use bevy_asset::Assets;
use bevy_camera::primitives::Aabb;
use bevy_camera::visibility::Visibility;
use bevy_color::palettes::tailwind::*;
use bevy_ecs::prelude::*;
use bevy_math::{Vec3, Vec3A};
use bevy_mesh::{Indices, Mesh, Mesh3d, PrimitiveTopology};
use bevy_render::batching::NoAutomaticBatching;
use bevy_transform::prelude::Transform;
use bevy_utils::default;
use std::sync::Arc;

use bevy_eidolon::prelude::*;
use example::*;

fn main() -> AppExit {
    App::new()
        .add_plugins((ExamplePlugin, InstancedMaterialPlugin::<StandardInstancedMaterial>::default()))
        .add_systems(Startup, setup)
        .run()
}

fn setup(
    mut cmd: Commands,
    mut instanced_materials: ResMut<Assets<StandardInstancedMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let mesh_handle = meshes.add(Mesh::from(TriMesh));

    let aabb = Aabb {
        center: Vec3A::new(0.25, 0.375, 0.0),
        half_extents: Vec3A::new(0.25, 1.125, 0.0),
    };

    let material_handle = instanced_materials.add(StandardInstancedMaterial {
        debug: false,
        gpu_cull: false,
        debug_color: Default::default(),
        // Make the triangles double-sided so they can be seen from both sides.
        double_sided: true,
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
        NoAutomaticBatching,
        Transform::default(),
        Visibility::Visible,
        // Disable frustum culling or provide aabb.
        // NoFrustumCulling,
        Aabb {
            center: aabb.center,
            half_extents: aabb.half_extents * SIZE as f32 * 2.,
        },
    ));
}

/// A simple struct representing a subdivided triangle mesh.
/// UV's and normals aren't used in the shaders currently, but for testing purposes in the future it won't hurt.
struct TriMesh;

impl From<TriMesh> for Mesh {
    fn from(_: TriMesh) -> Self {
        let pos_bottom_left = [0.0, -0.25, 0.0];
        let pos_top_center = [0.25, 0.5, 0.0];
        let pos_bottom_right = [0.5, -0.25, 0.0];

        let uv_bottom_left = [0.0, 0.0];
        let uv_top_center = [0.5, 1.0];
        let uv_bottom_right = [1.0, 0.0];

        // Midpoints
        let pos_bottom_center = [0.25, -0.25, 0.0];
        let pos_mid_left = [0.125, 0.125, 0.0];
        let pos_mid_right = [0.375, 0.125, 0.0];

        let uv_bottom_center = [0.5, 0.0];
        let uv_mid_left = [0.25, 0.5];
        let uv_mid_right = [0.75, 0.5];

        let positions = vec![
            pos_bottom_left,
            pos_top_center,
            pos_bottom_right,
            pos_bottom_center,
            pos_mid_left,
            pos_mid_right,
        ];

        let uvs = vec![
            uv_bottom_left,
            uv_top_center,
            uv_bottom_right,
            uv_bottom_center,
            uv_mid_left,
            uv_mid_right,
        ];

        // All normals point forward
        let normals = vec![[0.0, 0.0, 1.0]; 6];

        // Indices for the 4 new triangles (CCW)
        let indices = Indices::U32(vec![
            0, 3, 4, // Bottom-left triangle
            3, 2, 5, // Bottom-right triangle
            4, 5, 1, // Top triangle
            3, 5, 4, // Center triangle
        ]);

        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, Default::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);

        mesh.generate_tangents().unwrap();
        mesh.insert_indices(indices);
        mesh
    }
}
