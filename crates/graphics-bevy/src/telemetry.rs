//! Engine -> host telemetry. Systems write a snapshot every frame into a
//! shared handle; hosts read it on their own schedule (the browser UI polls a
//! few times per second), so no per-frame data is pushed into the DOM.
//!
//! Also installs the render error handler so GPU validation errors and device
//! loss become visible state instead of a silent black viewport.

use std::sync::{Arc, Mutex};

use bevy::{
    asset::LoadState,
    platform::time::Instant,
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        batching::gpu_preprocessing::{GpuPreprocessingMode, GpuPreprocessingSupport},
        error_handler::{ErrorType, RenderError, RenderErrorHandler, RenderErrorPolicy},
        renderer::{RenderAdapterInfo, RenderDevice},
        settings::RenderCreation,
        view::{NoIndirectDrawing, window::SurfaceData},
    },
    window::PrimaryWindow,
};
use graphics_gpu::GpuTelemetry;
use serde::Serialize;

use crate::{
    commands::EngineParams,
    instancing::{self, CloneEntity, InstancingRequest},
    scene::ModelHandle,
};

#[derive(Serialize, Clone, Debug, Default)]
pub struct RenderCapabilities {
    pub adapter_name: String,
    pub vendor: u32,
    pub device: u32,
    pub device_type: String,
    pub driver: String,
    pub driver_info: String,
    pub backend: String,
    pub gpu_preprocessing: String,
    pub features: Vec<String>,
    pub limits: serde_json::Value,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct WindowInfo {
    pub physical_width: u32,
    pub physical_height: u32,
    pub scale_factor: f32,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct TelemetrySnapshot {
    pub frame: u64,
    pub fps: f32,
    pub frame_ms: f32,
    /// Main-world update through render submission, measured inside Rust.
    pub cpu_ms: f32,
    pub entities: u32,
    pub clone_entities: u32,
    pub instances: u32,
    pub instancing: Option<InstancingRequest>,
    pub indirect_drawing: bool,
    pub compute_dispatches: u64,
    pub instance_uploads: u64,
    pub model: String,
    pub errors: Vec<String>,
    pub device_lost: bool,
    /// The render error handler halted rendering; the host should offer a reload.
    pub rendering_stopped: bool,
    /// Times `RenderStartup` ran: 1 normally, 2+ after a device recovery.
    pub renderer_inits: u32,
    pub params: Option<EngineParams>,
    pub window: WindowInfo,
    pub render: Option<RenderCapabilities>,
}

#[derive(Default)]
struct TelemetryState {
    snapshot: TelemetrySnapshot,
    frame_start: Option<Instant>,
    render_errors: Vec<String>,
    recovery_attempts: u32,
}

const MAX_RENDER_ERRORS: usize = 16;

#[derive(Resource, Clone, Default)]
pub struct EngineTelemetry(Arc<Mutex<TelemetryState>>);

impl EngineTelemetry {
    fn with<R>(&self, f: impl FnOnce(&mut TelemetryState) -> R) -> R {
        f(&mut self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }

    pub fn snapshot(&self) -> TelemetrySnapshot {
        self.with(|s| s.snapshot.clone())
    }

    pub fn snapshot_json(&self) -> String {
        serde_json::to_string(&self.snapshot()).unwrap_or_else(|_| "{}".into())
    }
}

pub(crate) struct TelemetryPlugin {
    pub telemetry: EngineTelemetry,
}

impl Plugin for TelemetryPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.telemetry.clone())
            .insert_resource(RenderErrorHandler(handle_render_error))
            .add_systems(First, stamp_frame_start)
            .add_systems(Last, write_snapshot);
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .insert_resource(self.telemetry.clone())
            .add_systems(RenderStartup, (capture_capabilities, drop_stale_surfaces))
            .add_systems(Render, stamp_frame_end.in_set(RenderSystems::Cleanup));
    }
}

fn stamp_frame_start(telemetry: Res<EngineTelemetry>) {
    telemetry.with(|s| s.frame_start = Some(Instant::now()));
}

fn stamp_frame_end(telemetry: Res<EngineTelemetry>) {
    telemetry.with(|s| {
        if let Some(start) = s.frame_start.take() {
            let ms = start.elapsed().as_secs_f32() * 1000.0;
            s.snapshot.cpu_ms = ema(s.snapshot.cpu_ms, ms);
        }
    });
}

fn ema(previous: f32, sample: f32) -> f32 {
    if previous == 0.0 {
        sample
    } else {
        previous * 0.9 + sample * 0.1
    }
}

fn capture_capabilities(
    telemetry: Res<EngineTelemetry>,
    adapter: Res<RenderAdapterInfo>,
    device: Res<RenderDevice>,
    preprocessing: Option<Res<GpuPreprocessingSupport>>,
) {
    let limits = device.limits();
    let features = format!("{:?}", device.features());
    let capabilities = RenderCapabilities {
        adapter_name: adapter.name.clone(),
        vendor: adapter.vendor,
        device: adapter.device,
        device_type: format!("{:?}", adapter.device_type),
        driver: adapter.driver.clone(),
        driver_info: adapter.driver_info.clone(),
        backend: format!("{:?}", adapter.backend),
        gpu_preprocessing: preprocessing
            .map(|p| match p.max_supported_mode {
                GpuPreprocessingMode::None => "none",
                GpuPreprocessingMode::PreprocessingOnly => "preprocessing_only",
                GpuPreprocessingMode::Culling => "culling",
            })
            .unwrap_or("unknown")
            .into(),
        // wgpu 30 formats `Features` as nested bitflag structs; the flag
        // names are the all-caps identifiers in that output.
        features: features
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|t| {
                t.len() > 2
                    && t.chars().any(|c| c.is_ascii_uppercase())
                    && t.chars()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
            })
            .map(str::to_lowercase)
            .collect(),
        limits: serde_json::json!({
            "max_texture_dimension_2d": limits.max_texture_dimension_2d,
            "max_bind_groups": limits.max_bind_groups,
            "max_storage_buffer_binding_size": limits.max_storage_buffer_binding_size,
            "max_buffer_size": limits.max_buffer_size,
            "max_storage_buffers_per_shader_stage": limits.max_storage_buffers_per_shader_stage,
            "max_storage_textures_per_shader_stage": limits.max_storage_textures_per_shader_stage,
            "max_compute_workgroup_size_x": limits.max_compute_workgroup_size_x,
            "max_compute_invocations_per_workgroup": limits.max_compute_invocations_per_workgroup,
            "max_compute_workgroups_per_dimension": limits.max_compute_workgroups_per_dimension,
            "max_vertex_buffers": limits.max_vertex_buffers,
            "max_uniform_buffer_binding_size": limits.max_uniform_buffer_binding_size,
        }),
    };
    telemetry.with(|s| {
        s.snapshot.render = Some(capabilities);
        s.snapshot.renderer_inits += 1;
        s.snapshot.device_lost = false;
        s.snapshot.rendering_stopped = false;
    });
}

/// Bevy 0.20's device-loss recovery re-runs `RenderStartup` with a new device
/// but keeps each window's `SurfaceData`, whose surface and swapchain belong
/// to the lost device (every frame then fails validation in the upscaling
/// pass). Dropping it makes `create_surfaces` build a fresh surface. A no-op
/// on first start, when no surface exists yet.
fn drop_stale_surfaces(mut commands: Commands, surfaces: Query<Entity, With<SurfaceData>>) {
    for entity in &surfaces {
        commands.entity(entity).remove::<SurfaceData>();
    }
}

fn write_snapshot(
    telemetry: Res<EngineTelemetry>,
    gpu: Res<GpuTelemetry>,
    time: Res<Time>,
    params: Res<EngineParams>,
    request: Res<InstancingRequest>,
    entities: Query<()>,
    clones: Query<(), With<CloneEntity>>,
    cameras: Query<Has<NoIndirectDrawing>, With<Camera3d>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    model: Option<Res<ModelHandle>>,
    asset_server: Res<AssetServer>,
) {
    let delta = time.delta_secs();
    let model_state = match model {
        None => "pending".to_string(),
        Some(handle) => match asset_server.load_state(&handle.0) {
            LoadState::Failed(err) => format!("failed: {err}"),
            _ if asset_server.is_loaded_with_dependencies(&handle.0) => "ready".into(),
            _ => "loading".into(),
        },
    };
    telemetry.with(|s| {
        let snap = &mut s.snapshot;
        snap.frame += 1;
        if delta > 0.0 {
            snap.frame_ms = ema(snap.frame_ms, delta * 1000.0);
            snap.fps = 1000.0 / snap.frame_ms;
        }
        snap.entities = entities.iter().len() as u32;
        snap.clone_entities = clones.iter().len() as u32;
        snap.instances = instancing::instance_count(&request);
        snap.instancing = Some(*request);
        snap.indirect_drawing = !cameras.iter().any(|no_indirect| no_indirect);
        snap.compute_dispatches = gpu.compute_dispatches();
        snap.instance_uploads = gpu.custom_instance_uploads();
        let mut errors = gpu.errors();
        errors.extend(s.render_errors.iter().cloned());
        if model_state.starts_with("failed") {
            errors.push(format!("AssetError model: {model_state}"));
        }
        snap.model = model_state;
        snap.params = Some(*params);
        snap.errors = errors;
        if let Ok(window) = windows.single() {
            snap.window = WindowInfo {
                physical_width: window.physical_width(),
                physical_height: window.physical_height(),
                scale_factor: window.scale_factor(),
            };
        }
    });
}

/// Records every render error where the host can see it. Device loss gets one
/// automatic re-initialisation attempt; anything else stops rendering (rather
/// than quitting or strobing) and leaves the host to offer a reload.
fn handle_render_error(
    error: &RenderError,
    main_world: &mut World,
    _render_world: &mut World,
) -> RenderErrorPolicy {
    let Some(telemetry) = main_world.get_resource::<EngineTelemetry>().cloned() else {
        return RenderErrorPolicy::StopRendering;
    };
    let lost = matches!(error.ty, ErrorType::DeviceLost);
    telemetry.with(|s| {
        // While stopped, Bevy re-polls the handler every frame with the same
        // error; record each distinct error once and keep the list bounded.
        let message = format!("{:?}: {}", error.ty, error.description);
        if !s.render_errors.contains(&message) {
            if s.render_errors.len() >= MAX_RENDER_ERRORS {
                s.render_errors.remove(0);
            }
            s.render_errors.push(message);
        }
        if lost && !s.snapshot.device_lost {
            s.snapshot.device_lost = true;
            s.recovery_attempts += 1;
            if s.recovery_attempts == 1 {
                return RenderErrorPolicy::Recover(RenderCreation::default());
            }
        }
        s.snapshot.rendering_stopped = true;
        RenderErrorPolicy::StopRendering
    })
}
