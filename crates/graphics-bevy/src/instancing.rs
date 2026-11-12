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
use graphics_gpu::{InstanceData, InstanceSet};
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

#[derive(Resource, Clone, Copy, Debug, PartialEq, Serialize)]
pub struct InstancingRequest {
    pub mode: InstancingMode,
    pub count: u32,
    pub animate: bool,
}

impl Default for InstancingRequest {
    fn default() -> Self {
        Self {
            mode: InstancingMode::Custom,
            count: 2_000,
            animate: true,
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
}

#[derive(Resource)]
struct CloneAssets {
    mesh: Handle<Mesh>,
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
            .add_systems(
                Update,
                (
                    rebuild_clones.run_if(resource_changed::<InstancingRequest>),
                    animate_cpu_clones,
                )
                    .chain(),
            );
    }
}

fn create_clone_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(CloneAssets {
        mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
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
fn layout(count: u32) -> impl Iterator<Item = (Vec3, f32, f32)> {
    let golden = PI * (3.0 - 5.0_f32.sqrt());
    let scale = (0.11 * (1_000.0 / count.max(1) as f32).cbrt()).clamp(0.015, 0.11);
    (0..count).map(move |i| {
        let t = (i as f32 + 0.5) / count as f32;
        let y = 1.0 - 2.0 * t;
        let ring = (1.0 - y * y).max(0.0).sqrt();
        let theta = golden * i as f32;
        // Cheap integer hash for a stable per-clone radius and phase.
        let h = (i.wrapping_mul(2_654_435_761) >> 8) as f32 / 16_777_216.0;
        let radius = 2.4 + 2.0 * h;
        let position =
            Vec3::new(ring * theta.cos(), y * 0.7, ring * theta.sin()) * radius + Vec3::Y * 1.0;
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
    existing: Query<Entity, With<CloneEntity>>,
    assets: Res<CloneAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut generation: ResMut<InstanceGeneration>,
    cameras: Query<Entity, With<Camera3d>>,
    mut last_shape: Local<Option<(InstancingMode, u32)>>,
) {
    // Toggling animation alone must not respawn the set.
    let shape = (request.mode, request.count);
    if *last_shape == Some(shape) {
        return;
    }
    *last_shape = Some(shape);

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

    match request.mode {
        InstancingMode::Off => {}
        InstancingMode::Builtin | InstancingMode::Unbatched => {
            let unique = request.mode == InstancingMode::Unbatched;
            let batch: Vec<_> = layout(request.count)
                .map(|(position, scale, phase)| {
                    let material = if unique {
                        let [r, g, b, _] = clone_color(phase);
                        materials.add(StandardMaterial {
                            base_color: Color::linear_rgb(r, g, b),
                            perceptual_roughness: 0.35,
                            metallic: 0.6,
                            ..default()
                        })
                    } else {
                        assets.material.clone()
                    };
                    (
                        CloneEntity,
                        CloneSeed {
                            base: position,
                            phase,
                        },
                        Mesh3d(assets.mesh.clone()),
                        MeshMaterial3d(material),
                        Transform::from_translation(position).with_scale(Vec3::splat(scale)),
                    )
                })
                .collect();
            commands.spawn_batch(batch);
        }
        InstancingMode::Custom => {
            generation.0 += 1;
            let data: Vec<InstanceData> = layout(request.count)
                .map(|(position, scale, phase)| InstanceData {
                    position,
                    scale,
                    color: clone_color(phase),
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

/// CPU animation for the ECS modes: the same spin + bob the custom shader
/// computes on the GPU, so both paths do equivalent visual work.
fn animate_cpu_clones(
    time: Res<Time>,
    request: Res<InstancingRequest>,
    mut clones: Query<(&CloneSeed, &mut Transform)>,
) {
    if !request.animate {
        return;
    }
    let t = time.elapsed_secs_wrapped();
    clones.par_iter_mut().for_each(|(seed, mut transform)| {
        let phase = seed.phase * std::f32::consts::TAU;
        transform.rotation = Quat::from_rotation_y(t * 0.8 + phase);
        transform.translation = seed.base + Vec3::Y * ((t * 1.3 + phase).sin() * 0.25);
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
