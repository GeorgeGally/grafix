//! Clone-count stress test with three rendering strategies so the spike can
//! compare them under identical layouts:
//!
//! - `Builtin`: N ECS entities sharing one mesh + one material; Bevy batches
//!   and instances them automatically. Animation runs on the CPU.
//! - `Unbatched`: N entities with a unique material each, which defeats
//!   batching. The baseline that shows what instancing saves.
//! - `Custom`: one entity carrying an `InstanceSet` vertex buffer, drawn with a
//!   single instanced call. Animation runs in the vertex shader.

use std::{f32::consts::PI, sync::Arc};

use bevy::{prelude::*, render::view::NoIndirectDrawing};
use graph_core::{GeometryDesc, MaterialDesc, SceneKind, Spatial};
use graphics_gpu::{InstanceData, InstanceSet};

use crate::{
    commands::{GraphCloneAppearance, GraphCloneSource, GraphSpatialState, PlaybackClock},
    scene::geometry_mesh,
};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstancingMode {
    #[default]
    Off,
    Builtin,
    Unbatched,
    Custom,
}

impl InstancingMode {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "off" => Some(Self::Off),
            "builtin" => Some(Self::Builtin),
            "unbatched" => Some(Self::Unbatched),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }

    /// Upper bound per mode. Unbatched issues one draw per clone, so it is
    /// capped well below the instanced modes.
    pub fn max_count(self) -> u32 {
        match self {
            Self::Off => 0,
            Self::Unbatched => 10_000,
            Self::Builtin | Self::Custom => 100_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CloneDistribution {
    #[default]
    Fibonacci,
    Grid,
    Radial,
}

impl CloneDistribution {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "fibonacci" => Some(Self::Fibonacci),
            "grid" => Some(Self::Grid),
            "radial" => Some(Self::Radial),
            _ => None,
        }
    }
}

#[derive(Resource, Clone, Copy, Debug, PartialEq, Serialize)]
pub struct InstancingRequest {
    pub mode: InstancingMode,
    pub count: u32,
    pub animate: bool,
    pub distribution: CloneDistribution,
    pub radius: f32,
    pub spacing: f32,
}

impl Default for InstancingRequest {
    fn default() -> Self {
        Self {
            mode: InstancingMode::Custom,
            count: 2_000,
            animate: true,
            distribution: CloneDistribution::Fibonacci,
            radius: 3.4,
            spacing: 0.7,
        }
    }
}

/// Marks every entity spawned for the current clone set.
#[derive(Component)]
pub struct CloneEntity;

/// Per-clone animation seed for the CPU-animated modes.
#[derive(Component)]
struct CloneSeed {
    base: Vec3,
    phase: f32,
    orientation: Quat,
    scale: Vec3,
    bob_axis: Vec3,
}

#[derive(Resource)]
struct CloneAssets {
    mesh: Handle<Mesh>,
    kind: SceneKind,
    geometry: GeometryDesc,
    material: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
struct InstanceGeneration(u64);

pub(crate) struct InstancingPlugin;

impl Plugin for InstancingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InstancingRequest>()
            .init_resource::<InstanceGeneration>()
            .add_systems(Startup, create_clone_assets)
            .add_systems(Update, (rebuild_clones, animate_cpu_clones).chain());
    }
}

fn create_clone_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(CloneAssets {
        mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        kind: SceneKind::Cube,
        geometry: GeometryDesc::default(),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.62, 0.78, 0.95),
            perceptual_roughness: 0.35,
            metallic: 0.6,
            ..default()
        }),
    });
}

/// Deterministic layout: a thick spherical shell around the model, on a
/// Fibonacci spiral so density is even at any count.
fn layout(
    count: u32,
    distribution: CloneDistribution,
    radius: f32,
    spacing: f32,
) -> impl Iterator<Item = (Vec3, f32, f32)> {
    let golden = PI * (3.0 - 5.0_f32.sqrt());
    let scale = (0.11 * (1_000.0 / count.max(1) as f32).cbrt()).clamp(0.015, 0.11);
    let side = (count.max(1) as f32).cbrt().ceil() as u32;
    (0..count).map(move |i| {
        let t = (i as f32 + 0.5) / count as f32;
        let h = (i.wrapping_mul(2_654_435_761) >> 8) as f32 / 16_777_216.0;
        let position = match distribution {
            CloneDistribution::Fibonacci => {
                let y = 1.0 - 2.0 * t;
                let ring = (1.0 - y * y).max(0.0).sqrt();
                let theta = golden * i as f32;
                let distance = (2.4 + 2.0 * h) * (radius / 3.4);
                Vec3::new(ring * theta.cos(), y * 0.7, ring * theta.sin()) * distance
                    + Vec3::Y * 1.0
            }
            CloneDistribution::Grid => {
                let x = i % side;
                let y = (i / side) % side;
                let z = i / (side * side);
                let center = (side as f32 - 1.0) * 0.5;
                Vec3::new(
                    (x as f32 - center) * spacing,
                    (y as f32 - center) * spacing + 1.0,
                    (z as f32 - center) * spacing,
                )
            }
            CloneDistribution::Radial => {
                let angle = std::f32::consts::TAU * i as f32 / count as f32;
                Vec3::new(angle.cos() * radius, 1.0, angle.sin() * radius)
            }
        };
        (position, scale, h)
    })
}

fn clone_color(phase: f32) -> [f32; 4] {
    let c = LinearRgba::from(Color::oklch(0.74, 0.11, 190.0 + phase * 70.0));
    [c.red, c.green, c.blue, phase]
}

fn rebuild_clones(
    mut commands: Commands,
    request: Res<InstancingRequest>,
    spatial: Res<GraphSpatialState>,
    appearance: Res<GraphCloneAppearance>,
    source: Res<GraphCloneSource>,
    existing: Query<Entity, With<CloneEntity>>,
    mut assets: ResMut<CloneAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut generation: ResMut<InstanceGeneration>,
    cameras: Query<Entity, With<Camera3d>>,
    mut last_shape: Local<
        Option<(
            InstancingMode,
            u32,
            bool,
            Spatial,
            Spatial,
            Option<MaterialDesc>,
            CloneDistribution,
            f32,
            f32,
            SceneKind,
            GeometryDesc,
        )>,
    >,
) {
    // Compare source, group and instancing config to avoid unnecessary GPU uploads.
    let shape = (
        request.mode,
        request.count,
        request.animate,
        spatial.cube,
        spatial.clones,
        appearance.material,
        request.distribution,
        request.radius,
        request.spacing,
        source.kind,
        source.geometry,
    );
    if *last_shape == Some(shape) {
        return;
    }
    *last_shape = Some(shape);

    // Update the instanced source mesh only if its actual shape/dimensions
    // change, rather than allocating geometry on every cloner position edit.
    if assets.kind != source.kind || assets.geometry != source.geometry {
        let previous = assets.mesh.clone();
        assets.mesh = geometry_mesh(&mut meshes, source.kind, source.geometry);
        assets.kind = source.kind;
        assets.geometry = source.geometry;
        meshes.remove(&previous);
    }

    for entity in &existing {
        commands.entity(entity).despawn();
    }
    // The custom draw command issues direct draws, so the camera opts out of
    // Bevy's GPU-driven indirect path only while that mode is active.
    for camera in &cameras {
        if request.mode == InstancingMode::Custom {
            commands.entity(camera).insert(NoIndirectDrawing);
        } else {
            commands.entity(camera).remove::<NoIndirectDrawing>();
        }
    }

    let group = spatial.clones;
    let source = spatial.cube;
    let group_rotation = spatial_rotation(group);
    let source_rotation = spatial_rotation(source);
    let orientation = group_rotation * source_rotation;
    let shape_scale = Vec3::from_array(group.scale) * Vec3::from_array(source.scale);
    let group_scale = Vec3::from_array(group.scale);
    let group_position = Vec3::from_array(group.position);
    let source_offset = Vec3::from_array(source.position);
    let bob_axis = group_rotation * Vec3::Y * 0.25;
    let linked = appearance.material;

    // The cube's transform is geometry-local; the cloner's transform
    // controls the distribution. Both renderer strategies share the layout.
    let prepared: Vec<_> = layout(
        request.count,
        request.distribution,
        request.radius,
        request.spacing,
    )
    .map(|(point, size, phase)| {
        let center = group_position
            + group_rotation * (point * group_scale)
            + group_rotation * (source_offset * group_scale * size);
        (center, size, phase)
    })
    .collect();

    if let Some(mut material) = materials.get_mut(&assets.material) {
        *material = linked
            .map(pbr_clone_material)
            .unwrap_or_else(default_clone_material);
    }

    match request.mode {
        InstancingMode::Off => {}
        InstancingMode::Builtin | InstancingMode::Unbatched => {
            let unique = request.mode == InstancingMode::Unbatched;
            let batch: Vec<_> = prepared
                .into_iter()
                .map(|(position, scale, phase)| {
                    let material = if unique {
                        materials.add(if let Some(source) = linked {
                            pbr_clone_material(source)
                        } else {
                            let [r, g, b, _] = clone_color(phase);
                            StandardMaterial {
                                base_color: Color::linear_rgb(r, g, b),
                                perceptual_roughness: 0.35,
                                metallic: 0.6,
                                ..default()
                            }
                        })
                    } else {
                        assets.material.clone()
                    };
                    (
                        CloneEntity,
                        CloneSeed {
                            base: position,
                            phase,
                            orientation,
                            scale: shape_scale * scale,
                            bob_axis,
                        },
                        Mesh3d(assets.mesh.clone()),
                        MeshMaterial3d(material),
                        Transform::from_translation(position)
                            .with_rotation(orientation)
                            .with_scale(shape_scale * scale),
                    )
                })
                .collect();
            commands.spawn_batch(batch);
        }
        InstancingMode::Custom => {
            generation.0 += 1;
            let data: Vec<InstanceData> = prepared
                .into_iter()
                .map(|(position, scale, phase)| InstanceData {
                    position,
                    scale,
                    color: clone_colour_for_source(phase, linked),
                    orientation: orientation.to_array(),
                    shape_scale: [
                        shape_scale.x,
                        shape_scale.y,
                        shape_scale.z,
                        if request.animate { 1.0 } else { 0.0 },
                    ],
                    surface: [
                        linked.map_or(0.6, |m| m.metallic),
                        linked.map_or(0.35, |m| m.roughness),
                        0.0,
                        0.0,
                    ],
                    emissive: linked.map_or([0.0; 4], |m| {
                        let rgba = LinearRgba::from(Color::srgb(
                            m.emissive_color[0],
                            m.emissive_color[1],
                            m.emissive_color[2],
                        ));
                        [
                            rgba.red * m.emissive_strength,
                            rgba.green * m.emissive_strength,
                            rgba.blue * m.emissive_strength,
                            0.0,
                        ]
                    }),
                })
                .collect();
            commands.spawn((
                CloneEntity,
                Mesh3d(assets.mesh.clone()),
                InstanceSet {
                    data: Arc::new(data),
                    generation: generation.0,
                },
                Transform::IDENTITY,
                Visibility::default(),
            ));
        }
    }
}

fn default_clone_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgb(0.62, 0.78, 0.95),
        perceptual_roughness: 0.35,
        metallic: 0.6,
        ..default()
    }
}

fn pbr_clone_material(source: MaterialDesc) -> StandardMaterial {
    let emissive = LinearRgba::from(Color::srgb(
        source.emissive_color[0],
        source.emissive_color[1],
        source.emissive_color[2],
    ));
    StandardMaterial {
        base_color: Color::srgb(
            source.base_color[0],
            source.base_color[1],
            source.base_color[2],
        ),
        metallic: source.metallic,
        perceptual_roughness: source.roughness,
        emissive: LinearRgba::rgb(
            emissive.red * source.emissive_strength,
            emissive.green * source.emissive_strength,
            emissive.blue * source.emissive_strength,
        ),
        ..default()
    }
}

fn clone_colour_for_source(phase: f32, linked: Option<MaterialDesc>) -> [f32; 4] {
    if let Some(source) = linked {
        let rgb = LinearRgba::from(Color::srgb(
            source.base_color[0],
            source.base_color[1],
            source.base_color[2],
        ));
        [rgb.red, rgb.green, rgb.blue, phase]
    } else {
        clone_color(phase)
    }
}

fn spatial_rotation(node: Spatial) -> Quat {
    Quat::from_euler(
        EulerRot::YXZ,
        node.rotation[1].to_radians(),
        node.rotation[0].to_radians(),
        node.rotation[2].to_radians(),
    )
}

/// CPU animation for the ECS modes: the same spin + bob the custom shader
/// computes on the GPU, so both paths do equivalent visual work.
fn animate_cpu_clones(
    playback: Res<PlaybackClock>,
    request: Res<InstancingRequest>,
    mut clones: Query<(&CloneSeed, &mut Transform)>,
) {
    if !request.animate {
        return;
    }
    let t = playback.seconds;
    clones.par_iter_mut().for_each(|(seed, mut transform)| {
        let phase = seed.phase * std::f32::consts::TAU;
        transform.rotation = seed.orientation * Quat::from_rotation_y(t * 0.8 + phase);
        transform.translation = seed.base + seed.bob_axis * (t * 1.3 + phase).sin();
        transform.scale = seed.scale;
    });
}

/// Summary for telemetry.
pub(crate) fn instance_count(request: &InstancingRequest) -> u32 {
    if request.mode == InstancingMode::Off {
        0
    } else {
        request.count
    }
}

#[cfg(test)]
mod distribution_tests {
    use super::*;
    #[test]
    fn each_distribution_is_finite_deterministic_and_distinct() {
        let grid: Vec<_> = layout(27, CloneDistribution::Grid, 3.4, 1.25).collect();
        let radial: Vec<_> = layout(27, CloneDistribution::Radial, 3.4, 1.25).collect();
        let sphere: Vec<_> = layout(27, CloneDistribution::Fibonacci, 3.4, 1.25).collect();
        assert_eq!(grid.len(), 27);
        assert_eq!(
            grid,
            layout(27, CloneDistribution::Grid, 3.4, 1.25).collect::<Vec<_>>()
        );
        assert_ne!(grid[0].0, radial[0].0);
        assert_ne!(radial[0].0, sphere[0].0);
        assert!(grid.iter().chain(radial.iter()).chain(sphere.iter()).all(
            |(point, scale, phase)| point.is_finite() && scale.is_finite() && phase.is_finite()
        ));
        let empty: Vec<_> = layout(0, CloneDistribution::Grid, 3.4, 1.25).collect();
        assert!(empty.is_empty());
    }
}
