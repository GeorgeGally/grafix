//! The Gate A scene: a glTF model with PBR materials, a directional key light
//! with shadows, two coloured point lights, a custom-material floor, and a
//! panel showing the compute shader's output. Camera is HDR with bloom,
//! tonemapping and the custom post-process pass.

use bevy::{
    camera::Hdr, core_pipeline::tonemapping::Tonemapping, light::CascadeShadowConfigBuilder,
    post_process::bloom::Bloom, prelude::*,
};
use graphics_gpu::{ComputeFieldImage, ComputeFieldParams, PostProcessSettings};

use crate::{
    commands::{EngineParams, GraphVisibility},
    floor::{GridFloor, GridFloorMaterial},
    orbit::OrbitCamera,
};

pub const DEMO_MODEL: &str = "demo/BoomBox.glb";

/// Root of the loaded glTF model; spun by `spin_speed`.
#[derive(Component)]
pub struct ModelRoot;

/// Strong handle to the model scene so telemetry can report its load state.
#[derive(Resource)]
pub struct ModelHandle(pub Handle<WorldAsset>);

#[derive(Component)]
struct KeyLight;

#[derive(Component)]
struct FieldPanel;

pub(crate) struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.75, 0.82, 1.0),
            brightness: 120.0,
            ..default()
        })
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                spin_model,
                apply_params.run_if(resource_changed::<EngineParams>),
                apply_graph_visibility.run_if(resource_changed::<GraphVisibility>),
            ),
        );
    }
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    params: Res<EngineParams>,
    field: Res<ComputeFieldImage>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut floor_materials: ResMut<Assets<GridFloorMaterial>>,
) {
    let orbit = OrbitCamera::default();
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Tonemapping::TonyMcMapface,
        Bloom {
            intensity: params.bloom,
            ..Bloom::NATURAL
        },
        PostProcessSettings {
            aberration: params.aberration,
            vignette: params.vignette,
            ..default()
        },
        orbit.transform(),
        orbit,
    ));

    commands.spawn((
        KeyLight,
        DirectionalLight {
            illuminance: params.light_intensity,
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 2,
            maximum_distance: 20.0,
            ..default()
        }
        .build(),
        Transform::from_xyz(4.0, 7.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    for (position, color) in [
        (Vec3::new(-3.0, 2.5, 2.0), Color::srgb(1.0, 0.45, 0.25)),
        (Vec3::new(3.0, 1.5, -2.5), Color::srgb(0.25, 0.55, 1.0)),
    ] {
        commands.spawn((
            PointLight {
                color,
                intensity: 250_000.0,
                range: 12.0,
                ..default()
            },
            Transform::from_translation(position),
        ));
    }

    let model = asset_server.load(GltfAssetLabel::Scene(0).from_asset(DEMO_MODEL));
    commands.insert_resource(ModelHandle(model.clone()));
    commands.spawn((
        ModelRoot,
        Visibility::Visible,
        WorldAssetRoot(model),
        // BoomBox is authored ~2 cm across.
        Transform::from_xyz(0.0, 1.0, 0.0).with_scale(Vec3::splat(100.0)),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(40.0, 40.0))),
        MeshMaterial3d(floor_materials.add(GridFloorMaterial {
            base: StandardMaterial {
                base_color: Color::srgb(0.035, 0.038, 0.045),
                perceptual_roughness: 0.85,
                ..default()
            },
            extension: GridFloor::default(),
        })),
    ));

    // The compute shader's storage texture, shown as an emissive panel.
    commands.spawn((
        FieldPanel,
        Visibility::Visible,
        Mesh3d(meshes.add(Rectangle::new(4.0, 4.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: LinearRgba::rgb(2.5, 2.5, 2.5),
            emissive_texture: Some(field.0.clone()),
            perceptual_roughness: 0.6,
            ..default()
        })),
        Transform::from_xyz(0.0, 2.4, -4.0),
    ));
}

fn spin_model(
    time: Res<Time>,
    params: Res<EngineParams>,
    mut roots: Query<&mut Transform, With<ModelRoot>>,
) {
    for mut transform in &mut roots {
        transform.rotate_y(params.spin_speed * time.delta_secs());
    }
}

fn apply_params(
    params: Res<EngineParams>,
    mut lights: Query<&mut DirectionalLight, With<KeyLight>>,
    mut cameras: Query<(&mut Bloom, &mut PostProcessSettings)>,
    mut field: ResMut<ComputeFieldParams>,
) {
    for mut light in &mut lights {
        light.illuminance = params.light_intensity;
    }
    for (mut bloom, mut post) in &mut cameras {
        bloom.intensity = params.bloom;
        post.aberration = params.aberration;
        post.vignette = params.vignette;
    }
    field.speed = params.field_speed;
    field.scale = params.field_scale;
}

/// Apply the graph's connectivity without despawning or reloading GPU assets.
fn apply_graph_visibility(
    graph: Res<GraphVisibility>,
    mut models: Query<&mut Visibility, With<ModelRoot>>,
    mut fields: Query<&mut Visibility, (With<FieldPanel>, Without<ModelRoot>)>,
) {
    for mut visibility in &mut models {
        *visibility = if graph.model { Visibility::Visible } else { Visibility::Hidden };
    }
    for mut visibility in &mut fields {
        *visibility = if graph.field { Visibility::Visible } else { Visibility::Hidden };
    }
}
