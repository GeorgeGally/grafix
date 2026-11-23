//! The Gate A scene: a glTF model with PBR materials, a directional key light
//! with shadows, two coloured point lights, a custom-material floor, and a
//! panel showing the compute shader's output. Camera is HDR with bloom,
//! tonemapping and the custom post-process pass.

use std::collections::{HashMap, HashSet};

use bevy::window::PrimaryWindow;

use bevy::mesh::VertexAttributeValues;
use bevy::{
    camera::Hdr, core_pipeline::tonemapping::Tonemapping, light::CascadeShadowConfigBuilder,
    post_process::bloom::Bloom, prelude::*,
};
use graph_core::{GeometryDesc, MaterialDesc, SceneKind, Spatial, TwistDesc};
use graphics_gpu::{ComputeFieldImage, ComputeFieldParams, PostProcessSettings};

use crate::{
    commands::{
        CommandQueue, EngineParams, GraphSceneState, GraphSpatialState, GraphVisibility,
        PlaybackClock,
    },
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
struct SceneCube;

#[derive(Component)]
struct DemoPointLight;

#[derive(Component)]
struct DemoFloor;

/// Identity is graph-owned, never a transient Bevy Entity ID.
#[derive(Component)]
struct GraphObject {
    id: String,
    kind: SceneKind,
    geometry: GeometryDesc,
    twist: Option<TwistDesc>,
    asset_path: Option<String>,
    material: MaterialDesc,
    spin_speed: f32,
}

#[derive(Component)]
struct GraphLight {
    id: String,
}

#[derive(Resource)]
struct GraphSceneAssets {
    cube: Handle<Mesh>,
}

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
                apply_graph_transforms.run_if(resource_changed::<GraphSpatialState>),
                spin_model,
                apply_params.run_if(resource_changed::<EngineParams>),
                apply_graph_visibility,
                sync_graph_scene.run_if(resource_changed::<GraphSceneState>),
                sync_ambient.run_if(resource_changed::<GraphSceneState>),
                spin_graph_models,
                disable_demo_lights,
                pick_graph_object,
            )
                .chain(),
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
            DemoPointLight,
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
        DemoFloor,
        Visibility::Visible,
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

    // Standalone geometry is a separate scene output from its use as a
    // cloner source. Connecting Cube -> Scene.geometry makes it visible.
    let cube_mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    commands.insert_resource(GraphSceneAssets {
        cube: cube_mesh.clone(),
    });
    commands.spawn((
        SceneCube,
        Visibility::Hidden,
        Mesh3d(cube_mesh),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.66, 0.82, 0.96),
            metallic: 0.35,
            perceptual_roughness: 0.42,
            ..default()
        })),
        Transform::IDENTITY,
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

/// Apply transforms to stable scene entities without triggering GLB reloads.
/// Rotations are authored in degrees in the editor and use YXZ Euler order.
fn apply_graph_transforms(
    spatial: Res<GraphSpatialState>,
    mut objects: Query<
        (
            &mut Transform,
            Option<&ModelRoot>,
            Option<&SceneCube>,
            Option<&FieldPanel>,
            Option<&KeyLight>,
        ),
        Or<(
            With<ModelRoot>,
            With<SceneCube>,
            With<FieldPanel>,
            With<KeyLight>,
        )>,
    >,
) {
    for (mut transform, model, cube, field, light) in &mut objects {
        let source = if model.is_some() {
            spatial.model
        } else if cube.is_some() {
            spatial.cube
        } else if field.is_some() {
            spatial.field
        } else if light.is_some() {
            spatial.light
        } else {
            continue;
        };
        transform.translation = Vec3::from_array(source.position);
        transform.scale = Vec3::from_array(source.scale);
        transform.rotation = Quat::from_euler(
            EulerRot::YXZ,
            source.rotation[1].to_radians(),
            source.rotation[0].to_radians(),
            source.rotation[2].to_radians(),
        );
    }
}

fn apply_graph_visibility(
    graph: Res<GraphVisibility>,
    scene: Res<GraphSceneState>,
    spatial: Res<GraphSpatialState>,
    mut objects: Query<
        (
            &mut Visibility,
            Option<&ModelRoot>,
            Option<&SceneCube>,
            Option<&FieldPanel>,
            Option<&DemoFloor>,
        ),
        Or<(
            With<ModelRoot>,
            With<SceneCube>,
            With<FieldPanel>,
            With<DemoFloor>,
        )>,
    >,
) {
    for (mut visibility, model, cube, field, floor) in &mut objects {
        let visible = if model.is_some() {
            !scene.active && graph.model
        } else if cube.is_some() {
            !scene.active && spatial.cube_visible
        } else if field.is_some() {
            graph.field
        } else if floor.is_some() {
            !scene.active || scene.show_grid
        } else {
            false
        };
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn sync_ambient(scene: Res<GraphSceneState>, mut ambient: ResMut<GlobalAmbientLight>) {
    if !scene.active {
        return;
    }
    ambient.color = Color::srgb(
        scene.ambient_color[0],
        scene.ambient_color[1],
        scene.ambient_color[2],
    );
    ambient.brightness = scene.ambient_brightness;
}

fn bevy_transform(source: Spatial) -> Transform {
    Transform {
        translation: Vec3::from_array(source.position),
        rotation: Quat::from_euler(
            EulerRot::YXZ,
            source.rotation[1].to_radians(),
            source.rotation[0].to_radians(),
            source.rotation[2].to_radians(),
        ),
        scale: Vec3::from_array(source.scale),
    }
}

fn apply_material(target: &mut StandardMaterial, source: &MaterialDesc) {
    target.base_color = Color::srgb(
        source.base_color[0],
        source.base_color[1],
        source.base_color[2],
    );
    target.metallic = source.metallic;
    target.perceptual_roughness = source.roughness;
    target.emissive = LinearRgba::rgb(
        source.emissive_color[0] * source.emissive_strength,
        source.emissive_color[1] * source.emissive_strength,
        source.emissive_color[2] * source.emissive_strength,
    );
}

fn new_material(source: &MaterialDesc) -> StandardMaterial {
    let mut material = StandardMaterial::default();
    apply_material(&mut material, source);
    material
}

/// Diff the desired graph-owned objects against existing Bevy entity IDs.
/// Existing cube entities and material assets are reused when positions or
/// colours change; deletion despawns only the associated graph node entity.
/// Produce a new GPU mesh only when source geometry changes. Graph objects
/// each own their mesh handle so changing sphere radius cannot mutate cube B.
pub(crate) fn geometry_mesh(
    meshes: &mut Assets<Mesh>,
    kind: SceneKind,
    geometry: GeometryDesc,
) -> Handle<Mesh> {
    match kind {
        SceneKind::Cube => meshes.add(Cuboid::new(geometry.width, geometry.height, geometry.depth)),
        SceneKind::Sphere => meshes.add(
            Sphere::new(geometry.radius)
                .mesh()
                .uv(geometry.segments, geometry.rings),
        ),
        SceneKind::Cylinder => meshes.add(Cylinder::new(geometry.radius, geometry.height)),
        SceneKind::Plane => meshes.add(
            Plane3d::default()
                .mesh()
                .size(geometry.width, geometry.depth),
        ),
        SceneKind::Torus => meshes.add(Torus::new(geometry.inner_radius, geometry.outer_radius)),
        SceneKind::Model => unreachable!("GLB scene is not a procedural mesh"),
    }
}

/// Deform a copied procedural mesh in local space. The source mesh is never
/// modified, so each deformer can have its own geometry and materials.
pub(crate) fn twisted_geometry_mesh(
    meshes: &mut Assets<Mesh>,
    kind: SceneKind,
    geometry: GeometryDesc,
    twist: Option<TwistDesc>,
) -> Handle<Mesh> {
    let handle = geometry_mesh(meshes, kind, geometry);
    let Some(twist) = twist else {
        return handle;
    };
    let Some(mut mesh) = meshes.get_mut(&handle) else {
        return handle;
    };
    let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(values)) => values.clone(),
        _ => return handle,
    };
    let normals = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(values)) => Some(values.clone()),
        _ => None,
    };
    let axis_length = match twist.axis {
        0 => geometry.width.max(geometry.radius * 2.0),
        2 => geometry.depth.max(geometry.radius * 2.0),
        _ => geometry.height.max(geometry.radius * 2.0),
    };
    let rotated: Vec<[f32; 3]> = positions
        .iter()
        .map(|position| {
            let (point, _) = twist_point(Vec3::from_array(*position), twist, axis_length);
            point.to_array()
        })
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, rotated);
    if let Some(normals) = normals {
        let rotated_normals: Vec<[f32; 3]> = normals
            .iter()
            .zip(&positions)
            .map(|(normal, position)| {
                let (_, rotation) = twist_point(Vec3::from_array(*position), twist, axis_length);
                (rotation * Vec3::from_array(*normal))
                    .normalize_or_zero()
                    .to_array()
            })
            .collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, rotated_normals);
    }
    handle
}

fn twist_point(point: Vec3, twist: TwistDesc, length: f32) -> (Vec3, Quat) {
    let coord = point[twist.axis as usize] / (length * 0.5).max(0.001);
    let normalized = coord.clamp(-1.0, 1.0);
    let angle =
        normalized.signum() * normalized.abs().powf(twist.falloff) * twist.angle.to_radians();
    let axis = match twist.axis {
        0 => Vec3::X,
        2 => Vec3::Z,
        _ => Vec3::Y,
    };
    let rotation = Quat::from_axis_angle(axis, angle);
    (rotation * point, rotation)
}

fn sync_graph_scene(
    mut commands: Commands,
    desired: Res<GraphSceneState>,
    model_asset: Res<ModelHandle>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut objects: Query<(
        Entity,
        &mut GraphObject,
        &mut Transform,
        &mut Visibility,
        Option<&MeshMaterial3d<StandardMaterial>>,
        Option<&Mesh3d>,
    )>,
    mut lights: Query<(
        Entity,
        &GraphLight,
        &mut Transform,
        &mut Visibility,
        &mut DirectionalLight,
    )>,
) {
    if !desired.active {
        return;
    }
    let mut seen_objects = HashSet::new();
    let mut existing_objects = HashMap::new();
    for (entity, object, _, _, material, mesh) in &mut objects {
        existing_objects.insert(
            object.id.clone(),
            (
                entity,
                object.kind,
                material.map(|m| m.0.clone()),
                mesh.map(|m| m.0.clone()),
            ),
        );
    }
    for item in &desired.items {
        seen_objects.insert(item.id.clone());
        if let Some((entity, kind, _, _)) = existing_objects.get(&item.id) {
            let entity = *entity;
            if *kind == item.kind
                && (item.kind != SceneKind::Model || objects.get(entity)
                    .is_ok_and(|(_, object, _, _, _, _)| object.asset_path == item.asset_path)) {
                if let Ok((_, mut object, mut transform, mut visibility, material, mesh)) =
                    objects.get_mut(entity)
                {
                    *transform = bevy_transform(item.transform);
                    *visibility = if item.visible {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                    object.spin_speed = item.spin_speed;
                    if (object.geometry != item.geometry || object.twist != item.twist)
                        && kind != &SceneKind::Model
                    {
                        if let Some(mesh) = mesh {
                            let previous = mesh.0.clone();
                            commands.entity(entity).insert(Mesh3d(twisted_geometry_mesh(
                                &mut meshes,
                                item.kind,
                                item.geometry,
                                item.twist,
                            )));
                            meshes.remove(&previous);
                        }
                        object.geometry = item.geometry;
                        object.twist = item.twist;
                    }
                    if object.material != item.material {
                        if let Some(material) = material {
                            if let Some(mut asset) = materials.get_mut(&material.0) {
                                apply_material(&mut asset, &item.material);
                                object.material = item.material;
                            }
                        }
                    }
                    continue;
                }
            }
            if let Some((_, _, material, mesh)) = existing_objects.get(&item.id) {
                if let Some(mesh) = mesh {
                    meshes.remove(mesh);
                }
                if let Some(material) = material {
                    materials.remove(material);
                }
            }
            commands.entity(entity).despawn();
        }
        let identity = GraphObject {
            id: item.id.clone(),
            kind: item.kind,
            geometry: item.geometry,
            twist: item.twist,
            asset_path: item.asset_path.clone(),
            material: item.material,
            spin_speed: item.spin_speed,
        };
        let transform = bevy_transform(item.transform);
        let visibility = if item.visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        match item.kind {
            SceneKind::Cube
            | SceneKind::Sphere
            | SceneKind::Cylinder
            | SceneKind::Plane
            | SceneKind::Torus => {
                commands.spawn((
                    identity,
                    Mesh3d(twisted_geometry_mesh(
                        &mut meshes,
                        item.kind,
                        item.geometry,
                        item.twist,
                    )),
                    MeshMaterial3d(materials.add(new_material(&item.material))),
                    transform,
                    visibility,
                ));
            }
            SceneKind::Model => {
                let handle = item.asset_path.as_ref().map_or_else(
                    || model_asset.0.clone(),
                    |path| asset_server.load(GltfAssetLabel::Scene(0).from_asset(path.as_str())),
                );
                commands.spawn((identity, WorldAssetRoot(handle), transform, visibility));
            }
        }
    }
    for (id, (entity, _, material, mesh)) in existing_objects {
        if !seen_objects.contains(&id) {
            if let Some(mesh) = mesh {
                meshes.remove(&mesh);
            }
            if let Some(material) = material {
                materials.remove(&material);
            }
            commands.entity(entity).despawn();
        }
    }

    let mut seen_lights = HashSet::new();
    let mut existing_lights = HashMap::new();
    for (entity, light, _, _, _) in &mut lights {
        existing_lights.insert(light.id.clone(), entity);
    }
    for descriptor in &desired.lights {
        seen_lights.insert(descriptor.id.clone());
        let transform = bevy_transform(descriptor.transform);
        let visibility = if descriptor.enabled {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        let color = Color::srgb(
            descriptor.color[0],
            descriptor.color[1],
            descriptor.color[2],
        );
        if let Some(&entity) = existing_lights.get(&descriptor.id) {
            if let Ok((_, _, mut t, mut v, mut light)) = lights.get_mut(entity) {
                *t = transform;
                *v = visibility;
                light.illuminance = descriptor.intensity;
                light.color = color;
                continue;
            }
        }
        commands.spawn((
            GraphLight {
                id: descriptor.id.clone(),
            },
            DirectionalLight {
                illuminance: descriptor.intensity,
                color,
                shadow_maps_enabled: true,
                ..default()
            },
            CascadeShadowConfigBuilder {
                num_cascades: 2,
                maximum_distance: 20.0,
                ..default()
            }
            .build(),
            transform,
            visibility,
        ));
    }
    for (id, entity) in existing_lights {
        if !seen_lights.contains(&id) {
            commands.entity(entity).despawn();
        }
    }
}

fn spin_graph_models(
    playback: Res<PlaybackClock>,
    scene: Res<GraphSceneState>,
    mut objects: Query<(&GraphObject, &mut Transform)>,
) {
    for (object, mut transform) in &mut objects {
        if object.kind == SceneKind::Model {
            if let Some(item) = scene.items.iter().find(|item| item.id == object.id) {
                transform.rotation = bevy_transform(item.transform).rotation
                    * Quat::from_rotation_y(object.spin_speed * playback.seconds);
            }
        }
    }
}

fn disable_demo_lights(
    graph: Res<GraphSceneState>,
    mut key: Query<&mut DirectionalLight, With<KeyLight>>,
    mut fills: Query<&mut PointLight, With<DemoPointLight>>,
    params: Res<EngineParams>,
) {
    // Demo fill lights must not contaminate a graph-authored lighting rig.
    for mut light in &mut key {
        let intensity = if graph.active {
            0.0
        } else {
            params.light_intensity
        };
        if light.illuminance != intensity {
            light.illuminance = intensity;
        }
    }
    for mut light in &mut fills {
        let intensity = if graph.active { 0.0 } else { 250_000.0 };
        if light.intensity != intensity {
            light.intensity = intensity;
        }
    }
}

/// Real camera rays select graph-owned objects, never demo entities.
/// This uses conservative bounding spheres until mesh-precise picking ships.
fn sphere_hit(origin: Vec3, direction: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    if radius <= 0.0 || !radius.is_finite() {
        return None;
    }
    let offset = origin - center;
    let b = offset.dot(direction);
    let c = offset.length_squared() - radius * radius;
    let d = b * b - c;
    if d < 0.0 {
        return None;
    }
    let root = d.sqrt();
    let near = -b - root;
    let far = -b + root;
    if near >= 0.0 {
        Some(near)
    } else if far >= 0.0 {
        Some(far)
    } else {
        None
    }
}

fn graph_radius(object: &GraphObject, scale: Vec3) -> f32 {
    let local = match object.kind {
        SceneKind::Sphere => object.geometry.radius,
        SceneKind::Cylinder => {
            (object.geometry.radius.powi(2) + (object.geometry.height * 0.5).powi(2)).sqrt()
        }
        SceneKind::Torus => object.geometry.outer_radius,
        SceneKind::Model => 1.0,
        _ => {
            Vec3::new(
                object.geometry.width,
                object.geometry.height,
                object.geometry.depth,
            )
            .length()
                * 0.5
        }
    };
    local * scale.abs().max_element()
}

fn pick_graph_object(
    clicks: Res<ButtonInput<MouseButton>>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<OrbitCamera>>,
    graph: Query<(&GraphObject, &GlobalTransform, &Visibility)>,
    queue: Res<CommandQueue>,
) {
    if !clicks.just_pressed(MouseButton::Left) {
        return;
    }
    let (Ok(window), Ok((camera, camera_transform))) = (window.single(), camera.single()) else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor) else {
        return;
    };
    let mut nearest: Option<(&str, f32)> = None;
    for (object, transform, visible) in &graph {
        if *visible == Visibility::Hidden {
            continue;
        }
        let (scale, _, position) = transform.to_scale_rotation_translation();
        let radius = graph_radius(object, scale);
        if let Some(distance) = sphere_hit(ray.origin, *ray.direction, position, radius) {
            if nearest.is_none_or(|(_, closest)| distance < closest) {
                nearest = Some((&object.id, distance));
            }
        }
    }
    if let Some((id, _)) = nearest {
        queue.report_selection(id.to_owned());
    }
}

#[cfg(test)]
mod viewport_tests {
    use super::*;
    #[test]
    fn picking_rejects_misses_and_objects_behind_the_camera() {
        let origin = Vec3::ZERO;
        let direction = Vec3::NEG_Z;
        assert_eq!(
            sphere_hit(origin, direction, Vec3::new(0.0, 0.0, -5.0), 1.0),
            Some(4.0)
        );
        assert!(sphere_hit(origin, direction, Vec3::new(5.0, 0.0, -5.0), 1.0).is_none());
        assert!(sphere_hit(origin, direction, Vec3::new(0.0, 0.0, 5.0), 1.0).is_none());
    }
}

#[cfg(test)]
mod twist_tests {
    use super::*;
    #[test]
    fn twist_rotates_ends_oppositely_and_respects_axis() {
        let twist = TwistDesc {
            angle: 90.0,
            axis: 1,
            falloff: 1.0,
        };
        let (top, _) = twist_point(Vec3::new(1.0, 1.0, 0.0), twist, 2.0);
        let (bottom, _) = twist_point(Vec3::new(1.0, -1.0, 0.0), twist, 2.0);
        assert!(top.z.abs() > 0.99);
        assert!(bottom.z.abs() > 0.99);
        assert!(top.z * bottom.z < 0.0);
        let (middle, _) = twist_point(Vec3::new(1.0, 0.0, 0.0), twist, 2.0);
        assert!((middle - Vec3::X).length() < 0.00001);
    }
}
