//! The JS/host -> engine command boundary. Hosts push commands from any call
//! site; the engine applies them once per frame at a safe point (`PreUpdate`),
//! coalescing bursts such as slider drags so only the final value lands.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use bevy::prelude::*;
use graph_core::{Controls, Lfo};
use serde::Serialize;

use crate::instancing::{InstancingMode, InstancingRequest};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamKey {
    LightIntensity,
    SpinSpeed,
    Bloom,
    Aberration,
    Vignette,
    FieldSpeed,
    FieldScale,
}

impl ParamKey {
    pub const ALL: [ParamKey; 7] = [
        ParamKey::LightIntensity,
        ParamKey::SpinSpeed,
        ParamKey::Bloom,
        ParamKey::Aberration,
        ParamKey::Vignette,
        ParamKey::FieldSpeed,
        ParamKey::FieldScale,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ParamKey::LightIntensity => "light_intensity",
            ParamKey::SpinSpeed => "spin_speed",
            ParamKey::Bloom => "bloom",
            ParamKey::Aberration => "aberration",
            ParamKey::Vignette => "vignette",
            ParamKey::FieldSpeed => "field_speed",
            ParamKey::FieldScale => "field_scale",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|key| key.name() == name)
    }

    /// Valid range; values outside it are clamped.
    pub fn range(self) -> (f32, f32) {
        match self {
            ParamKey::LightIntensity => (0.0, 30_000.0),
            ParamKey::SpinSpeed => (-3.0, 3.0),
            ParamKey::Bloom => (0.0, 1.0),
            ParamKey::Aberration => (0.0, 0.05),
            ParamKey::Vignette => (0.0, 1.0),
            ParamKey::FieldSpeed => (0.0, 3.0),
            ParamKey::FieldScale => (1.0, 12.0),
        }
    }
}

/// Live, Rust-owned parameters. Systems react to changes of this resource.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Serialize)]
pub struct EngineParams {
    pub light_intensity: f32,
    pub spin_speed: f32,
    pub bloom: f32,
    pub aberration: f32,
    pub vignette: f32,
    pub field_speed: f32,
    pub field_scale: f32,
}

impl Default for EngineParams {
    fn default() -> Self {
        Self {
            light_intensity: 8_000.0,
            spin_speed: 0.35,
            bloom: 0.18,
            aberration: 0.006,
            vignette: 0.35,
            field_speed: 0.6,
            field_scale: 4.0,
        }
    }
}

impl EngineParams {
    pub fn get(&self, key: ParamKey) -> f32 {
        match key {
            ParamKey::LightIntensity => self.light_intensity,
            ParamKey::SpinSpeed => self.spin_speed,
            ParamKey::Bloom => self.bloom,
            ParamKey::Aberration => self.aberration,
            ParamKey::Vignette => self.vignette,
            ParamKey::FieldSpeed => self.field_speed,
            ParamKey::FieldScale => self.field_scale,
        }
    }

    pub fn set(&mut self, key: ParamKey, value: f32) {
        let (lo, hi) = key.range();
        let value = if value.is_finite() {
            value.clamp(lo, hi)
        } else {
            lo
        };
        let slot = match key {
            ParamKey::LightIntensity => &mut self.light_intensity,
            ParamKey::SpinSpeed => &mut self.spin_speed,
            ParamKey::Bloom => &mut self.bloom,
            ParamKey::Aberration => &mut self.aberration,
            ParamKey::Vignette => &mut self.vignette,
            ParamKey::FieldSpeed => &mut self.field_speed,
            ParamKey::FieldScale => &mut self.field_scale,
        };
        *slot = value;
    }
}

#[derive(Clone, Debug)]
pub enum EngineCommand {
    SetParam { key: ParamKey, value: f32 },
    SetInstancing { mode: InstancingMode, count: u32 },
    SetAnimateClones(bool),
    ResetCamera,
    ApplyGraph(Controls),
}

/// Visibility of currently connected graph scene elements. Disconnected
/// sources are not rendered; no entities are deleted by graph rewiring.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphVisibility {
    pub model: bool,
    pub field: bool,
}

impl Default for GraphVisibility {
    fn default() -> Self {
        Self {
            model: true,
            field: true,
        }
    }
}

#[derive(Resource, Clone, Default, PartialEq)]
struct GraphProjection {
    bloom_lfo: Option<Lfo>,
}

/// Thread-safe FIFO shared between the host bridge and the engine.
#[derive(Resource, Clone, Default)]
pub struct CommandQueue(Arc<Mutex<VecDeque<EngineCommand>>>);

impl CommandQueue {
    pub fn push(&self, command: EngineCommand) {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push_back(command);
    }

    fn drain(&self) -> Vec<EngineCommand> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect()
    }
}

/// Raised for one frame when the host asks to re-frame the camera.
#[derive(Message, Clone, Copy, Debug)]
pub struct ResetCameraRequest;

pub(crate) struct CommandsPlugin {
    pub queue: CommandQueue,
}

impl Plugin for CommandsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.queue.clone())
            .init_resource::<EngineParams>()
            .init_resource::<GraphVisibility>()
            .init_resource::<GraphProjection>()
            .add_message::<ResetCameraRequest>()
            .add_systems(PreUpdate, (apply_commands, tick_graph_modulation).chain());
    }
}

fn apply_commands(
    queue: Res<CommandQueue>,
    mut params: ResMut<EngineParams>,
    mut instancing: ResMut<InstancingRequest>,
    mut reset_camera: MessageWriter<ResetCameraRequest>,
    mut visibility: ResMut<GraphVisibility>,
    mut projection: ResMut<GraphProjection>,
) {
    let commands = queue.drain();
    if commands.is_empty() {
        return;
    }
    let mut next_params = *params;
    let mut next_instancing = *instancing;
    let mut next_visibility = *visibility;
    let mut next_projection = (*projection).clone();
    for command in commands {
        match command {
            EngineCommand::SetParam { key, value } => next_params.set(key, value),
            EngineCommand::SetInstancing { mode, count } => {
                next_instancing.mode = mode;
                next_instancing.count = count.min(mode.max_count());
            }
            EngineCommand::SetAnimateClones(animate) => next_instancing.animate = animate,
            EngineCommand::ResetCamera => {
                reset_camera.write(ResetCameraRequest);
            }
            EngineCommand::ApplyGraph(graph) => {
                for (name, value) in graph.params {
                    if let Some(key) = ParamKey::parse(&name) {
                        next_params.set(key, value);
                    }
                }
                let mode = InstancingMode::parse(&graph.clone_mode).unwrap_or(InstancingMode::Off);
                next_instancing.mode = mode;
                next_instancing.count = graph.clone_count.min(mode.max_count());
                next_instancing.animate = graph.clone_animate;
                next_visibility = GraphVisibility {
                    model: graph.model_visible,
                    field: graph.field_visible,
                };
                next_projection.bloom_lfo = graph.bloom_lfo;
            }
        }
    }
    // Only mark changed when something actually changed, so change-detection
    // driven systems stay idle during no-op bursts.
    params.set_if_neq(next_params);
    instancing.set_if_neq(next_instancing);
    visibility.set_if_neq(next_visibility);
    projection.set_if_neq(next_projection);
}

fn tick_graph_modulation(
    time: Res<Time>,
    projection: Res<GraphProjection>,
    mut params: ResMut<EngineParams>,
) {
    if let Some(lfo) = &projection.bloom_lfo {
        let phase = time.elapsed_secs() * std::f32::consts::TAU * lfo.frequency;
        let amount = lfo.minimum + (lfo.maximum - lfo.minimum) * (0.5 + 0.5 * phase.sin());
        params.set(ParamKey::Bloom, amount);
    }
}
