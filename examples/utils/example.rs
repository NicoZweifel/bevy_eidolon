#[path = "camera_controller.rs"]
mod camera_controller;

use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::diagnostic::*;
use bevy::light::light_consts::lux::FULL_DAYLIGHT;
use bevy::light::{DirectionalLightShadowMap, ShadowFilteringMethod};
use bevy::post_process::bloom::Bloom;
use bevy::{
    core_pipeline::tonemapping::Tonemapping, light::VolumetricLight, prelude::*,
    render::view::ColorGrading,
};
use bevy_camera::Hdr;
use bevy_core_pipeline::prepass::DeferredPrepass;
use bevy_eidolon::prepass::CullComputeCamera;
use bevy_render::RenderPlugin;
use bevy_render::settings::{RenderCreation, WgpuLimits, WgpuSettings};
use camera_controller::*;

#[derive(Resource, Default, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct ExamplePluginOptions {
    pub show_inspector: bool,
}

pub struct ExamplePlugin;

impl Plugin for ExamplePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ExamplePluginOptions>()
            .insert_resource(DirectionalLightShadowMap { size: 4096 })
            .add_plugins(
                DefaultPlugins
                    .set(AssetPlugin { ..default() })
                    .set(RenderPlugin {
                        render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                            limits: WgpuLimits {
                                max_storage_buffer_binding_size: 1024 << 20,
                                max_buffer_size: 1024 << 20,
                                ..default()
                            },
                            ..default()
                        })),
                        ..default()
                    }),
            )
            .add_plugins((
                FrameTimeDiagnosticsPlugin::default(),
                EntityCountDiagnosticsPlugin::default(),
                SystemInformationDiagnosticsPlugin,
            ))
            .add_plugins(CameraControllerPlugin)
            .add_systems(Startup, (setup, spawn_directional_light));
    }
}

fn spawn_directional_light(mut cmd: Commands) {
    cmd.spawn((
        DirectionalLight {
            illuminance: FULL_DAYLIGHT,
            shadow_maps_enabled: true,
            contact_shadows_enabled: true,
            color: Color::srgb(1.0, 0.98, 0.95),
            ..default()
        },
        VolumetricLight,
        Transform::from_xyz(2., 2., 0.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

pub fn setup(mut cmd: Commands) {
    cmd.spawn((
        CullComputeCamera,
        Camera::default(),
        Hdr,
        Controller::default(),
        Msaa::Off,
        TemporalAntiAliasing::default(),
        Camera3d::default(),
        ColorGrading::default(),
        Bloom::NATURAL,
        Tonemapping::TonyMcMapface,
        Transform::from_xyz(-30., 20., 30.).looking_at(Vec3::ZERO, Vec3::Y),
        ShadowFilteringMethod::Temporal,
        DeferredPrepass,
    ));
}
