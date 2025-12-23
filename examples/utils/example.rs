#[path = "camera_controller.rs"]
mod camera_controller;

use bevy::diagnostic::*;
use bevy::light::light_consts::lux::FULL_DAYLIGHT;
use bevy::light::{DirectionalLightShadowMap, ShadowFilteringMethod};
use bevy::post_process::bloom::Bloom;
use bevy::{
    core_pipeline::tonemapping::Tonemapping, light::VolumetricLight, prelude::*,
    render::view::ColorGrading,
};
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_render::view::Hdr;

use camera_controller::*;
use iyes_perf_ui::prelude::*;

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
            .add_plugins(DefaultPlugins.set(AssetPlugin { ..default() }))
            .add_plugins((
                FrameTimeDiagnosticsPlugin::default(),
                EntityCountDiagnosticsPlugin::default(),
                SystemInformationDiagnosticsPlugin,
                PerfUiPlugin,
            ))
            .add_plugins((
                EguiPlugin::default(),
                WorldInspectorPlugin::default()
                    .run_if(|res: Res<ExamplePluginOptions>| res.show_inspector),
            ))
            .add_plugins(CameraControllerPlugin)
            .add_systems(Startup, (setup, spawn_directional_light));
    }
}

fn spawn_directional_light(mut cmd: Commands) {
    cmd.spawn((
        DirectionalLight {
            illuminance: FULL_DAYLIGHT,
            shadows_enabled: true,
            color: Color::srgb(1.0, 0.98, 0.95),
            ..default()
        },
        VolumetricLight,
        Transform::from_xyz(2., 2., 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        ShadowFilteringMethod::Temporal,
    ));
}

pub fn setup(mut cmd: Commands) {
    cmd.spawn((
        Camera::default(),
        (Msaa::Off, TemporalAntiAliasing::default()),
        Hdr,
        Controller::default(),
        Camera3d::default(),
        ColorGrading::default(),
        Bloom::NATURAL,
        Tonemapping::TonyMcMapface,
        Transform::from_xyz(-30., 20., 30.).looking_at(Vec3::ZERO, Vec3::Y),
        ShadowFilteringMethod::Temporal,
    ));

    cmd.spawn(PerfUiDefaultEntries::default());
}
