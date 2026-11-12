//! GPU-side building blocks that extend Bevy's renderer without patching it.
//!
//! Everything here plugs into public Bevy 0.20 extension points: the `Core3d`
//! render schedule, the root `RenderGraph` schedule, `SpecializedMeshPipeline`
//! and the extract/prepare/queue render sets.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

use bevy::{prelude::*, render::RenderApp};

pub mod compute_field;
pub mod custom_instancing;
pub mod post_process;

pub use compute_field::{ComputeFieldImage, ComputeFieldParams, ComputeFieldPlugin};
pub use custom_instancing::{CustomInstancingPlugin, InstanceData, InstanceSet};
pub use post_process::{PostProcessPlugin, PostProcessSettings};

/// Counters and errors written from the render world and read from the main
/// world. Shared by `Arc` so both worlds hold the same instance.
#[derive(Resource, Clone, Default)]
pub struct GpuTelemetry(Arc<GpuTelemetryInner>);

#[derive(Default)]
pub struct GpuTelemetryInner {
    compute_dispatches: AtomicU64,
    custom_instance_uploads: AtomicU64,
    errors: Mutex<Vec<String>>,
}

impl GpuTelemetry {
    pub fn compute_dispatches(&self) -> u64 {
        self.0.compute_dispatches.load(Ordering::Relaxed)
    }

    pub fn custom_instance_uploads(&self) -> u64 {
        self.0.custom_instance_uploads.load(Ordering::Relaxed)
    }

    /// Records an error once; repeated identical messages are dropped so a
    /// failing pipeline does not flood the UI every frame.
    pub fn report_error(&self, message: String) {
        let mut errors = self.0.errors.lock().unwrap_or_else(|e| e.into_inner());
        if !errors.contains(&message) {
            error!("{message}");
            errors.push(message);
        }
    }

    pub fn errors(&self) -> Vec<String> {
        self.0
            .errors
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn count_compute_dispatch(&self) {
        self.0.compute_dispatches.fetch_add(1, Ordering::Relaxed);
    }

    fn count_instance_upload(&self) {
        self.0
            .custom_instance_uploads
            .fetch_add(1, Ordering::Relaxed);
    }
}

/// Adds every GPU extension used by the Gate A scene.
pub struct GraphicsGpuPlugin;

impl Plugin for GraphicsGpuPlugin {
    fn build(&self, app: &mut App) {
        let telemetry = GpuTelemetry::default();
        app.insert_resource(telemetry.clone());
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.insert_resource(telemetry);
        }
        app.add_plugins((
            graphics_shaders::GraphicsShadersPlugin,
            PostProcessPlugin,
            CustomInstancingPlugin,
            ComputeFieldPlugin,
        ));
    }
}
