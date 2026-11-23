//! The JS/host -> engine command boundary. Hosts push commands from any call
//! site; the engine applies them once per frame at a safe point (`PreUpdate`),
//! coalescing bursts such as slider drags so only the final value lands.

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use bevy::{asset::io::memory::Dir, prelude::*};
use graph_core::{
    CompiledTrack, Controls, GeometryDesc, Lfo, LightDesc, MaterialDesc, OrbitSettings,
    RandomEffectorDesc, SceneItemDesc, SceneKind, Spatial, TwistDesc,
};
use serde::Serialize;

use crate::instancing::{CloneDistribution, InstancingMode, InstancingRequest};

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
    FrameNode(String),
    SetPlayback(PlaybackClock),
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

/// Shared evaluation clock: transport changes never modify authored parameters.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Serialize)]
pub struct PlaybackClock {
    pub seconds: f32,
    pub playing: bool,
    pub rate: f32,
    pub duration: f32,
    #[serde(skip)]
    pub delta: f32,
    /// Incremented only on seek/play/rate edits and loop transitions.
    #[serde(skip)]
    pub revision: u64,
}
impl Default for PlaybackClock {
    fn default() -> Self {
        Self {
            seconds: 0.0,
            playing: true,
            rate: 1.0,
            duration: 12.0,
            delta: 0.0,
            revision: 0,
        }
    }
}

#[derive(Resource, Clone, Default, PartialEq)]
struct GraphProjection {
    bloom_lfo: Option<Lfo>,
    aberration_lfo: Option<Lfo>,
    vignette_lfo: Option<Lfo>,
    base_scene: GraphSceneState,
    tracks: Vec<CompiledTrack>,
}

/// Scene-space projection. Independent of graph IDs so the existing Bevy
/// entities stay alive while the editor changes positions, rotations or scale.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct GraphSpatialState {
    pub model: Spatial,
    pub cube: Spatial,
    pub clones: Spatial,
    pub field: Spatial,
    pub light: Spatial,
    pub cube_visible: bool,
    pub camera: Option<OrbitSettings>,
}

impl Default for GraphSpatialState {
    fn default() -> Self {
        Self {
            model: Spatial {
                position: [0.0, 1.0, 0.0],
                scale: [100.0; 3],
                ..Spatial::default()
            },
            cube: Spatial::default(),
            clones: Spatial::default(),
            field: Spatial {
                position: [0.0, 2.4, -4.0],
                ..Spatial::default()
            },
            light: Spatial {
                position: [4.0, 7.0, 3.0],
                rotation: [-54.0, 53.0, 0.0],
                ..Spatial::default()
            },
            cube_visible: false,
            camera: None,
        }
    }
}

/// Compiled graph-owned objects. Unlike the original fixed Gate A nodes,
/// these IDs are stable across updates and multiple instances may coexist.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct GraphSceneState {
    pub active: bool,
    pub items: Vec<SceneItemDesc>,
    pub lights: Vec<LightDesc>,
    pub show_grid: bool,
    pub ambient_brightness: f32,
    pub ambient_color: [f32; 3],
}

/// The graph-owned source material compiled for clone renderers.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct GraphCloneAppearance {
    pub material: Option<MaterialDesc>,
}

/// Exact geometry selected as a cloner input (including shape dimensions).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct GraphCloneSource {
    pub kind: SceneKind,
    pub geometry: GeometryDesc,
    pub twist: Option<TwistDesc>,
}
impl Default for GraphCloneSource {
    fn default() -> Self {
        Self {
            kind: SceneKind::Cube,
            geometry: GeometryDesc::default(),
            twist: None,
        }
    }
}

/// Thread-safe FIFO shared between the host bridge and the engine.
#[derive(Resource, Clone)]
pub struct CommandQueue(
    Arc<Mutex<VecDeque<EngineCommand>>>,
    Arc<Mutex<VecDeque<String>>>,
    Dir,
);
impl Default for CommandQueue {
    fn default() -> Self {
        Self(Arc::default(), Arc::default(), Dir::new(PathBuf::new()))
    }
}

impl CommandQueue {
    /// Shared memory directory powering the registered uploads asset source.
    pub fn uploads_dir(&self) -> Dir {
        self.2.clone()
    }

    pub fn upload_glb(&self, id: &str, bytes: Vec<u8>) -> Result<(), String> {
        if id.len() != 64
            || !id
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err("asset ID must be a lower-case SHA-256 hex digest".into());
        }
        if bytes.len() < 20
            || bytes.len() > 50_000_000
            || &bytes[0..4] != b"glTF"
            || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) != 2
            || u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize != bytes.len()
        {
            return Err("invalid GLB header or asset exceeds 50 MB".into());
        }
        self.2.insert_asset(Path::new(&format!("{id}.glb")), bytes);
        Ok(())
    }

    pub fn push(&self, command: EngineCommand) {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push_back(command);
    }

    /// Delivered by the Bevy viewport picker, consumed by the browser host.
    pub fn report_selection(&self, id: String) {
        let mut queue = self.1.lock().unwrap_or_else(|e| e.into_inner());
        queue.clear(); // Latest click wins.
        queue.push_back(id);
    }

    pub fn take_selection(&self) -> Option<String> {
        self.1.lock().unwrap_or_else(|e| e.into_inner()).pop_front()
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

#[derive(Message, Clone, Debug)]
pub struct FrameNodeRequest(pub String);

pub(crate) struct CommandsPlugin {
    pub queue: CommandQueue,
}

impl Plugin for CommandsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.queue.clone())
            .init_resource::<EngineParams>()
            .init_resource::<GraphVisibility>()
            .init_resource::<GraphSpatialState>()
            .init_resource::<GraphSceneState>()
            .init_resource::<GraphCloneAppearance>()
            .init_resource::<GraphCloneSource>()
            .init_resource::<GraphProjection>()
            .init_resource::<PlaybackClock>()
            .add_message::<ResetCameraRequest>()
            .add_message::<FrameNodeRequest>()
            .add_systems(
                PreUpdate,
                (apply_commands, advance_playback, tick_graph_modulation).chain(),
            );
    }
}

fn apply_commands(
    queue: Res<CommandQueue>,
    mut params: ResMut<EngineParams>,
    mut instancing: ResMut<InstancingRequest>,
    mut reset_camera: MessageWriter<ResetCameraRequest>,
    mut frame_node: MessageWriter<FrameNodeRequest>,
    mut visibility: ResMut<GraphVisibility>,
    mut spatial: ResMut<GraphSpatialState>,
    mut scene: ResMut<GraphSceneState>,
    mut clone_appearance: ResMut<GraphCloneAppearance>,
    mut clone_source: ResMut<GraphCloneSource>,
    mut projection: ResMut<GraphProjection>,
    mut playback: ResMut<PlaybackClock>,
) {
    let commands = queue.drain();
    if commands.is_empty() {
        return;
    }
    let mut next_params = *params;
    let mut next_instancing = *instancing;
    let mut next_visibility = *visibility;
    let mut next_spatial = *spatial;
    let mut next_scene = (*scene).clone();
    let mut next_clone_appearance = *clone_appearance;
    let mut next_clone_source = *clone_source;
    let mut next_projection = (*projection).clone();
    let mut next_playback = *playback;
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
            EngineCommand::FrameNode(id) => {
                frame_node.write(FrameNodeRequest(id));
            }
            EngineCommand::SetPlayback(next) => {
                if next.seconds.is_finite() && next.rate.is_finite() && next.duration.is_finite() {
                    let duration = next.duration.clamp(1.0, 120.0);
                    next_playback = PlaybackClock {
                        seconds: next.seconds.clamp(0.0, duration),
                        playing: next.playing,
                        rate: next.rate.clamp(0.1, 4.0),
                        duration,
                        delta: 0.0,
                        revision: playback.revision.wrapping_add(1),
                    };
                }
            }
            EngineCommand::ApplyGraph(graph) => {
                next_scene = GraphSceneState {
                    active: true,
                    items: graph.scene_items.clone(),
                    lights: graph.scene_lights.clone(),
                    show_grid: graph.show_grid,
                    ambient_brightness: graph.ambient_brightness,
                    ambient_color: graph.ambient_color,
                };
                next_projection.base_scene = next_scene.clone();
                next_projection.tracks = graph.animation_tracks.clone();
                next_clone_appearance.material = graph.clone_material;
                next_clone_source = GraphCloneSource {
                    kind: graph.clone_source_kind,
                    geometry: graph.clone_source_geometry,
                    twist: graph.clone_source_twist,
                };
                for (name, value) in graph.params {
                    if let Some(key) = ParamKey::parse(&name) {
                        next_params.set(key, value);
                    }
                }
                let mode = InstancingMode::parse(&graph.clone_mode).unwrap_or(InstancingMode::Off);
                next_instancing.mode = mode;
                next_instancing.count = graph.clone_count.min(mode.max_count());
                next_instancing.animate = graph.clone_animate;
                next_instancing.distribution = CloneDistribution::parse(&graph.clone_distribution)
                    .unwrap_or(CloneDistribution::Fibonacci);
                next_instancing.radius = graph.clone_radius;
                next_instancing.spacing = graph.clone_spacing;
                next_instancing.random = graph.clone_random;
                next_visibility = GraphVisibility {
                    model: graph.model_visible,
                    field: graph.field_visible,
                };
                next_projection.bloom_lfo = graph.bloom_lfo;
                next_projection.aberration_lfo = graph.aberration_lfo;
                next_projection.vignette_lfo = graph.vignette_lfo;
                next_spatial = GraphSpatialState {
                    model: graph.model_transform,
                    cube: graph.cube_transform,
                    clones: graph.cloner_transform,
                    field: graph.field_transform,
                    light: graph.light_transform,
                    cube_visible: graph.cube_visible,
                    camera: graph.camera_settings,
                };
            }
        }
    }
    // Only mark changed when something actually changed, so change-detection
    // driven systems stay idle during no-op bursts.
    params.set_if_neq(next_params);
    instancing.set_if_neq(next_instancing);
    visibility.set_if_neq(next_visibility);
    spatial.set_if_neq(next_spatial);
    scene.set_if_neq(next_scene);
    clone_appearance.set_if_neq(next_clone_appearance);
    clone_source.set_if_neq(next_clone_source);
    projection.set_if_neq(next_projection);
    playback.set_if_neq(next_playback);
}

fn advance_playback(time: Res<Time>, mut clock: ResMut<PlaybackClock>) {
    let mut next = *clock;
    next.delta = if next.playing {
        time.delta_secs().min(0.1) * next.rate
    } else {
        0.0
    };
    if next.delta > 0.0 {
        let elapsed = next.seconds + next.delta;
        if elapsed >= next.duration { next.revision = next.revision.wrapping_add(1); }
        next.seconds = elapsed.rem_euclid(next.duration);
    }
    clock.set_if_neq(next);
}

fn apply_track(item: &mut SceneItemDesc, property: &str, value: f32) {
    let (part, axis) = property.rsplit_once('_').unwrap_or((property, ""));
    let i = match axis {
        "x" => Some(0),
        "y" => Some(1),
        "z" => Some(2),
        _ => None,
    };
    match (part, i) {
        ("position", Some(i)) => item.transform.position[i] = value,
        ("rotation", Some(i)) => item.transform.rotation[i] = value,
        ("scale", Some(i)) => item.transform.scale[i] = value,
        _ => match property {
            "metallic" => item.material.metallic = value,
            "roughness" => item.material.roughness = value,
            "emissive_strength" => item.material.emissive_strength = value,
            _ => {}
        },
    }
}

fn tick_graph_modulation(
    clock: Res<PlaybackClock>,
    projection: Res<GraphProjection>,
    mut params: ResMut<EngineParams>,
    mut scene: ResMut<GraphSceneState>,
) {
    if !projection.tracks.is_empty() {
        let mut desired = projection.base_scene.clone();
        for track in &projection.tracks {
            if let Some(value) = graph_core::sample_track(track, clock.seconds) {
                if let Some(item) = desired
                    .items
                    .iter_mut()
                    .find(|item| item.id == track.item_id)
                {
                    apply_track(item, &track.parameter, value);
                }
            }
        }
        scene.set_if_neq(desired);
    }
    for (key, signal) in [
        (ParamKey::Bloom, &projection.bloom_lfo),
        (ParamKey::Aberration, &projection.aberration_lfo),
        (ParamKey::Vignette, &projection.vignette_lfo),
    ] {
        if let Some(lfo) = signal {
            let phase = clock.seconds * std::f32::consts::TAU * lfo.frequency;
            let normalized = lfo.minimum + (lfo.maximum - lfo.minimum) * (0.5 + 0.5 * phase.sin());
            let (lo, hi) = key.range();
            params.set(key, lo + normalized * (hi - lo));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_commands_update_real_bevy_resources_and_restore_detached_materials() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<InstancingRequest>();
        let queue = CommandQueue::default();
        app.add_plugins(CommandsPlugin {
            queue: queue.clone(),
        });

        let mut doc = graph_core::Document::demo();
        queue.push(EngineCommand::ApplyGraph(
            graph_core::compile(&doc).unwrap(),
        ));
        app.update();

        let scene = app.world().resource::<GraphSceneState>();
        assert!(scene.active);
        assert_eq!(scene.items.len(), 2);
        assert_eq!(scene.lights.len(), 1);
        assert!(
            app.world()
                .resource::<GraphCloneAppearance>()
                .material
                .is_some()
        );
        assert_eq!(app.world().resource::<InstancingRequest>().count, 2000);

        doc.edges
            .retain(|edge| !(edge.to.node == "cube" && edge.to.port == "material"));
        queue.push(EngineCommand::ApplyGraph(
            graph_core::compile(&doc).unwrap(),
        ));
        app.update();

        assert!(
            app.world()
                .resource::<GraphCloneAppearance>()
                .material
                .is_none()
        );
        assert_eq!(app.world().resource::<GraphSceneState>().items.len(), 2);

        doc.edges
            .retain(|edge| !(edge.to.node == "scene" && edge.to.port == "clones"));
        queue.push(EngineCommand::ApplyGraph(
            graph_core::compile(&doc).unwrap(),
        ));
        app.update();

        assert_eq!(app.world().resource::<InstancingRequest>().count, 0);
        assert_eq!(
            app.world().resource::<InstancingRequest>().mode,
            InstancingMode::Off
        );
    }
}

#[cfg(test)]
mod playback_tests {
    use super::*;
    #[test]
    fn pausing_and_seeking_updates_scene_without_changing_base_graph() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<InstancingRequest>();
        let queue = CommandQueue::default();
        app.add_plugins(CommandsPlugin {
            queue: queue.clone(),
        });
        let mut doc = graph_core::Document::demo();
        doc.tracks.push(graph_core::AnimationTrack {
            node: "cube".into(),
            parameter: "position_y".into(),
            interpolation: "linear".into(),
            keys: vec![
                graph_core::AnimationKey {
                    time: 0.0,
                    value: 0.0,
                },
                graph_core::AnimationKey {
                    time: 2.0,
                    value: 4.0,
                },
            ],
        });
        queue.push(EngineCommand::ApplyGraph(
            graph_core::compile(&doc).unwrap(),
        ));
        queue.push(EngineCommand::SetPlayback(PlaybackClock {
            seconds: 1.0,
            playing: false,
            ..Default::default()
        }));
        app.update();
        let cube = app
            .world()
            .resource::<GraphSceneState>()
            .items
            .iter()
            .find(|item| item.id == "cube")
            .unwrap();
        assert_eq!(cube.transform.position[1], 2.0);
        queue.push(EngineCommand::SetPlayback(PlaybackClock {
            seconds: 2.0,
            playing: false,
            ..Default::default()
        }));
        app.update();
        let cube = app
            .world()
            .resource::<GraphSceneState>()
            .items
            .iter()
            .find(|item| item.id == "cube")
            .unwrap();
        assert_eq!(cube.transform.position[1], 4.0);
        assert_eq!(
            graph_core::compile(&doc)
                .unwrap()
                .scene_items
                .iter()
                .find(|item| item.id == "cube")
                .unwrap()
                .transform
                .position[1],
            0.0
        );
    }
}
