//! Engine-independent procedural document, schema, type checking and compilation.
//! No Bevy, DOM, WebGPU or JavaScript types belong in this crate.
//! Only explicitly supported operations are executable; other node families
//! will be added as their renderer backends ship.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Document {
    pub format_version: u32,
    pub name: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Node {
    pub id: String,
    #[serde(rename = "type")]
    pub type_id: String,
    #[serde(default = "one")]
    pub version: u32,
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
    #[serde(default)]
    pub ui: Position,
}

fn one() -> u32 {
    1
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PortRef {
    pub node: String,
    pub port: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Edge {
    pub from: PortRef,
    pub to: PortRef,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataType {
    Geometry,
    Material,
    SceneObject,
    InstanceSet,
    Texture2D,
    Light,
    Scene,
    Camera,
    Effect,
    Float,
}

#[derive(Clone, Debug, Serialize)]
pub struct PortSpec {
    pub name: &'static str,
    pub data_type: DataType,
}

#[derive(Clone, Debug, Serialize)]
pub struct ParamSpec {
    pub name: &'static str,
    pub kind: &'static str,
    pub default: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<&'static str>,
    pub group: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NodeSpec {
    pub type_id: &'static str,
    pub title: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub inputs: Vec<PortSpec>,
    pub outputs: Vec<PortSpec>,
    pub params: Vec<ParamSpec>,
}

fn port(name: &'static str, data_type: DataType) -> PortSpec {
    PortSpec { name, data_type }
}
fn num(name: &'static str, default: f64, min: f64, max: f64) -> ParamSpec {
    ParamSpec {
        name,
        kind: "number",
        default: json!(default),
        min: Some(min),
        max: Some(max),
        options: vec![],
        group: "General",
        unit: None,
    }
}
fn choice(name: &'static str, default: &'static str, options: Vec<&'static str>) -> ParamSpec {
    ParamSpec {
        name,
        kind: "enum",
        default: json!(default),
        min: None,
        max: None,
        options,
        group: "General",
        unit: None,
    }
}
fn toggle(name: &'static str, default: bool) -> ParamSpec {
    ParamSpec {
        name,
        kind: "bool",
        default: json!(default),
        min: None,
        max: None,
        options: vec![],
        group: "General",
        unit: None,
    }
}
fn rgb(name: &'static str, default: [f64; 3]) -> ParamSpec {
    ParamSpec {
        name,
        kind: "color",
        default: json!(default),
        min: Some(0.0),
        max: Some(1.0),
        options: vec![],
        group: "Appearance",
        unit: None,
    }
}

fn spatial_params(position: [f64; 3], scale: [f64; 3], rotation: [f64; 3]) -> Vec<ParamSpec> {
    let mut result = Vec::new();
    for (prefix, values, low, high) in [
        ("position", position, -100.0, 100.0),
        ("rotation", rotation, -360.0, 360.0),
        ("scale", scale, 0.001, 1000.0),
    ] {
        for (axis, value) in ["x", "y", "z"].into_iter().zip(values) {
            let key = format!("{prefix}_{axis}");
            // Static keys are shared by each schema; we deliberately use a
            // fixed set of strings rather than leaking dynamically allocated names.
            let name = match (prefix, axis) {
                ("position", "x") => "position_x",
                ("position", "y") => "position_y",
                ("position", "z") => "position_z",
                ("rotation", "x") => "rotation_x",
                ("rotation", "y") => "rotation_y",
                ("rotation", "z") => "rotation_z",
                ("scale", "x") => "scale_x",
                ("scale", "y") => "scale_y",
                ("scale", "z") => "scale_z",
                _ => unreachable!("{key}"),
            };
            result.push(num(name, value, low, high));
        }
    }
    result
}

/// Registry is the source of the editor's node palette and parameter inspectors.
/// A node is only listed if it has an actual runtime implementation.
pub fn registry() -> Vec<NodeSpec> {
    use DataType::*;
    let mut specs = vec![
        NodeSpec {
            type_id: "asset.boombox",
            title: "GLB Model",
            category: "3D",
            description: "The licensed BoomBox glTF asset used by the renderer proof.",
            inputs: vec![],
            outputs: vec![port("object", SceneObject)],
            params: vec![num("spin_speed", 0.35, -3.0, 3.0), toggle("visible", true)],
        },
        NodeSpec {
            type_id: "geometry.cube",
            title: "Cube",
            category: "3D",
            description: "Editable cube geometry with optional PBR material.",
            inputs: vec![port("material", Material)],
            outputs: vec![port("geometry", Geometry)],
            params: vec![toggle("visible", true)],
        },
        NodeSpec {
            type_id: "material.pbr",
            title: "PBR Material",
            category: "Materials",
            description: "Physically-based material for connected geometry.",
            inputs: vec![],
            outputs: vec![port("material", Material)],
            params: vec![
                rgb("base_color", [0.66, 0.82, 0.96]),
                num("metallic", 0.35, 0.0, 1.0),
                num("roughness", 0.42, 0.02, 1.0),
                rgb("emissive_color", [0.0, 0.0, 0.0]),
                num("emissive_strength", 0.0, 0.0, 10.0),
            ],
        },
        NodeSpec {
            type_id: "clone.fibonacci",
            title: "Fibonacci Cloner",
            category: "Cloning",
            description: "GPU-instanced copies in an even spherical distribution.",
            inputs: vec![port("source", Geometry)],
            outputs: vec![port("instances", InstanceSet)],
            params: vec![
                num("count", 2000.0, 0.0, 100000.0),
                choice("mode", "custom", vec!["custom", "builtin", "unbatched"]),
                choice("animate", "on", vec!["on", "off"]),
            ],
        },
        NodeSpec {
            type_id: "lighting.directional",
            title: "Key Light",
            category: "Lighting",
            description: "PBR directional light and shadows.",
            inputs: vec![],
            outputs: vec![port("light", Light)],
            params: vec![
                num("intensity", 8000.0, 0.0, 30000.0),
                rgb("color", [1.0, 1.0, 1.0]),
                toggle("enabled", true),
            ],
        },
        NodeSpec {
            type_id: "field.compute2d",
            title: "Compute Field 2D",
            category: "Fields",
            description: "The working 2D GPU procedural texture. Not a voxel fluid solver.",
            inputs: vec![],
            outputs: vec![port("texture", Texture2D)],
            params: vec![num("speed", 0.6, 0.0, 3.0), num("scale", 4.0, 1.0, 12.0)],
        },
        NodeSpec {
            type_id: "scene.compose",
            title: "Scene",
            category: "Rendering",
            description: "Collects scene objects, instances, lights and field display.",
            inputs: vec![
                port("model", SceneObject),
                port("geometry", Geometry),
                port("clones", InstanceSet),
                port("light", Light),
                port("field", Texture2D),
            ],
            outputs: vec![port("scene", Scene)],
            params: vec![],
        },
        NodeSpec {
            type_id: "camera.orbit",
            title: "Orbit Camera",
            category: "Rendering",
            description: "Interactive orbit and perspective camera.",
            inputs: vec![],
            outputs: vec![port("camera", Camera)],
            params: vec![
                num("target_x", 0.0, -100.0, 100.0),
                num("target_y", 1.0, -100.0, 100.0),
                num("target_z", 0.0, -100.0, 100.0),
                num("distance", 12.0, 1.5, 60.0),
                num("yaw", 34.377, -180.0, 180.0),
                num("pitch", 16.043, -85.0, 85.0),
            ],
        },
        NodeSpec {
            type_id: "fx.bloom",
            title: "Bloom",
            category: "Rendering",
            description: "HDR bloom post-effect.",
            inputs: vec![port("amount", Float)],
            outputs: vec![port("effect", Effect)],
            params: vec![num("amount", 0.18, 0.0, 1.0)],
        },
        NodeSpec {
            type_id: "fx.aberration",
            title: "Chromatic Aberration",
            category: "Rendering",
            description: "RGB lens offset.",
            inputs: vec![],
            outputs: vec![port("effect", Effect)],
            params: vec![num("amount", 0.006, 0.0, 0.05)],
        },
        NodeSpec {
            type_id: "fx.vignette",
            title: "Vignette",
            category: "Rendering",
            description: "Soft edge falloff.",
            inputs: vec![],
            outputs: vec![port("effect", Effect)],
            params: vec![num("amount", 0.35, 0.0, 1.0)],
        },
        NodeSpec {
            type_id: "animation.lfo",
            title: "LFO",
            category: "Data",
            description: "Time-driven sine signal for a connected numeric parameter.",
            inputs: vec![],
            outputs: vec![port("value", Float)],
            params: vec![
                num("frequency", 0.25, 0.0, 10.0),
                num("minimum", 0.0, 0.0, 1.0),
                num("maximum", 0.6, 0.0, 1.0),
            ],
        },
        NodeSpec {
            type_id: "math.add",
            title: "Add",
            category: "Data",
            description: "Reserved float data operator (validated but not yet evaluated).",
            inputs: vec![port("a", Float), port("b", Float)],
            outputs: vec![port("value", Float)],
            params: vec![],
        },
        NodeSpec {
            type_id: "render.output",
            title: "Output",
            category: "Rendering",
            description: "Final camera, scene and post-effects presented in the viewport.",
            inputs: vec![
                port("scene", Scene),
                port("camera", Camera),
                port("bloom", Effect),
                port("aberration", Effect),
                port("vignette", Effect),
            ],
            outputs: vec![],
            params: vec![],
        },
    ];
    // Spatial nodes use the same editable Position/Rotation/Scale interface.
    // The GLB demo asset is authored ~2 cm wide, so its default scale is 100.
    for spec in &mut specs {
        let defaults = match spec.type_id {
            "asset.boombox" => Some(([0.0, 1.0, 0.0], [100.0; 3], [0.0; 3])),
            "geometry.cube" => Some(([0.0; 3], [1.0; 3], [0.0; 3])),
            "clone.fibonacci" => Some(([0.0; 3], [1.0; 3], [0.0; 3])),
            "field.compute2d" => Some(([0.0, 2.4, -4.0], [1.0; 3], [0.0; 3])),
            "lighting.directional" => Some(([4.0, 7.0, 3.0], [1.0; 3], [-54.0, 53.0, 0.0])),
            _ => None,
        };
        if let Some((position, scale, rotation)) = defaults {
            spec.params
                .extend(spatial_params(position, scale, rotation));
        }
    }
    for spec in &mut specs {
        for p in &mut spec.params {
            p.group = if p.name.starts_with("position_")
                || p.name.starts_with("rotation_")
                || p.name.starts_with("scale_")
            {
                "Transform"
            } else if [
                "base_color",
                "metallic",
                "roughness",
                "emissive_color",
                "emissive_strength",
                "color",
            ]
            .contains(&p.name)
            {
                "Appearance"
            } else if ["visible", "enabled"].contains(&p.name) {
                "Object"
            } else {
                "General"
            };
            if p.name.starts_with("rotation_") || ["yaw", "pitch"].contains(&p.name) {
                p.unit = Some("°");
            } else if p.name.starts_with("position_") || p.name == "distance" {
                p.unit = Some("m");
            }
        }
    }
    specs
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphError(pub String);

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for GraphError {}

fn err(message: impl Into<String>) -> GraphError {
    GraphError(message.into())
}

#[derive(Debug)]
pub struct Validated {
    pub order: Vec<String>,
    pub active: BTreeSet<String>,
}

/// Structural validation happens independently of a graphics device.
/// Partial graphs are allowed, but malformed connections and cycles are not.
pub fn validate(doc: &Document) -> Result<Validated, GraphError> {
    if doc.format_version != FORMAT_VERSION {
        return Err(err(format!(
            "unsupported graph format version {}",
            doc.format_version
        )));
    }
    if doc.nodes.len() > 2000 || doc.edges.len() > 8000 {
        return Err(err(
            "graph exceeds the safety limit (2000 nodes / 8000 edges)",
        ));
    }
    let specs = registry();
    let mut seen = BTreeSet::new();
    let mut nodes = BTreeMap::new();
    for node in &doc.nodes {
        if node.id.is_empty() || node.id.len() > 128 || !seen.insert(node.id.as_str()) {
            return Err(err(format!("invalid or duplicate node id: {}", node.id)));
        }
        if !node.ui.x.is_finite() || !node.ui.y.is_finite() {
            return Err(err(format!("invalid position for {}", node.id)));
        }
        let spec = specs
            .iter()
            .find(|s| s.type_id == node.type_id)
            .ok_or_else(|| err(format!("unknown node type {}", node.type_id)))?;
        if node.version != 1 {
            return Err(err(format!("unsupported node version: {}", node.id)));
        }
        for (name, value) in &node.params {
            let param = spec
                .params
                .iter()
                .find(|p| p.name == name)
                .ok_or_else(|| err(format!("unknown parameter {}.{}", node.id, name)))?;
            match param.kind {
                "number" => {
                    let n = value
                        .as_f64()
                        .ok_or_else(|| err(format!("{}.{} must be a number", node.id, name)))?;
                    if !n.is_finite()
                        || param.min.is_some_and(|min| n < min)
                        || param.max.is_some_and(|max| n > max)
                    {
                        return Err(err(format!("{}.{} is out of range", node.id, name)));
                    }
                }
                "bool" => {
                    if !value.is_boolean() {
                        return Err(err(format!("{}.{} must be a boolean", node.id, name)));
                    }
                }
                "color" => {
                    let channels = value
                        .as_array()
                        .ok_or_else(|| err(format!("{}.{} must be an RGB array", node.id, name)))?;
                    if channels.len() != 3
                        || channels.iter().any(|channel| {
                            channel
                                .as_f64()
                                .is_none_or(|n| !n.is_finite() || !(0.0..=1.0).contains(&n))
                        })
                    {
                        return Err(err(format!(
                            "{}.{} needs three RGB channels in 0..1",
                            node.id, name
                        )));
                    }
                }
                "enum" => {
                    let selection = value
                        .as_str()
                        .ok_or_else(|| err(format!("{}.{} must be a choice", node.id, name)))?;
                    if !param.options.contains(&selection) {
                        return Err(err(format!("{}.{} is an unknown choice", node.id, name)));
                    }
                }
                _ => return Err(err("unsupported parameter schema")),
            }
        }
        nodes.insert(node.id.as_str(), spec);
    }

    let mut destinations = BTreeSet::new();
    let mut outgoing: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut incoming: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut indegree: BTreeMap<&str, usize> =
        doc.nodes.iter().map(|n| (n.id.as_str(), 0)).collect();
    let mut unique_edges = BTreeSet::new();
    for edge in &doc.edges {
        if !unique_edges.insert((
            &edge.from.node,
            &edge.from.port,
            &edge.to.node,
            &edge.to.port,
        )) {
            return Err(err("input already connected (duplicate graph connection)"));
        }
        let source_spec = nodes
            .get(edge.from.node.as_str())
            .ok_or_else(|| err(format!("missing source node {}", edge.from.node)))?;
        let target_spec = nodes
            .get(edge.to.node.as_str())
            .ok_or_else(|| err(format!("missing target node {}", edge.to.node)))?;
        let source_port = source_spec
            .outputs
            .iter()
            .find(|p| p.name == edge.from.port)
            .ok_or_else(|| {
                err(format!(
                    "unknown output {}.{}",
                    edge.from.node, edge.from.port
                ))
            })?;
        let target_port = target_spec
            .inputs
            .iter()
            .find(|p| p.name == edge.to.port)
            .ok_or_else(|| err(format!("unknown input {}.{}", edge.to.node, edge.to.port)))?;
        if source_port.data_type != target_port.data_type {
            return Err(err(format!(
                "incompatible ports: {}.{} -> {}.{}",
                edge.from.node, edge.from.port, edge.to.node, edge.to.port
            )));
        }
        let is_multi = target_spec.type_id == "scene.compose"
            && ["model", "geometry", "light"].contains(&edge.to.port.as_str());
        if !is_multi && !destinations.insert((edge.to.node.as_str(), edge.to.port.as_str())) {
            return Err(err(format!(
                "input already connected: {}.{}",
                edge.to.node, edge.to.port
            )));
        }
        outgoing
            .entry(&edge.from.node)
            .or_default()
            .push(&edge.to.node);
        incoming
            .entry(&edge.to.node)
            .or_default()
            .push(&edge.from.node);
        *indegree
            .get_mut(edge.to.node.as_str())
            .expect("target exists") += 1;
    }
    let mut ready: BTreeSet<&str> = indegree
        .iter()
        .filter_map(|(&id, &count)| (count == 0).then_some(id))
        .collect();
    let mut order = Vec::with_capacity(doc.nodes.len());
    while let Some(id) = ready.pop_first() {
        order.push(id.to_owned());
        for &target in outgoing.get(id).into_iter().flatten() {
            let count = indegree.get_mut(target).expect("connected node");
            *count -= 1;
            if *count == 0 {
                ready.insert(target);
            }
        }
    }
    if order.len() != doc.nodes.len() {
        return Err(err("graph contains a cycle"));
    }

    let mut active: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<&str> = doc
        .nodes
        .iter()
        .filter(|n| n.type_id == "render.output")
        .map(|n| n.id.as_str())
        .collect();
    if stack.len() > 1 {
        return Err(err("only one render.output is supported"));
    }
    while let Some(id) = stack.pop() {
        if active.insert(id.to_owned()) {
            stack.extend(incoming.get(id).into_iter().flatten().copied());
        }
    }
    Ok(Validated { order, active })
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lfo {
    pub frequency: f32,
    pub minimum: f32,
    pub maximum: f32,
}

/// Editable transform in world units, rotation in degrees. The graph document
/// remains source-of-truth; this is a compiled renderer projection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spatial {
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

impl Default for Spatial {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            rotation: [0.0; 3],
            scale: [1.0; 3],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitSettings {
    pub target: [f32; 3],
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for OrbitSettings {
    fn default() -> Self {
        Self {
            target: [0.0, 1.0, 0.0],
            distance: 12.0,
            yaw: 34.377,
            pitch: 16.043,
        }
    }
}

fn spatial(node: &Node, defaults: Spatial) -> Spatial {
    let mut result = defaults;
    for (prefix, target) in [
        ("position", &mut result.position),
        ("rotation", &mut result.rotation),
        ("scale", &mut result.scale),
    ] {
        for (i, axis) in ["x", "y", "z"].into_iter().enumerate() {
            let name = format!("{prefix}_{axis}");
            target[i] = number(node, &name, target[i]);
        }
    }
    result
}

/// Renderable results with stable graph-owned IDs. These are *not* Bevy
/// entity identifiers; the renderer resolves them to its own stable entities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneKind {
    Cube,
    Model,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialDesc {
    pub base_color: [f32; 3],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive_color: [f32; 3],
    pub emissive_strength: f32,
}

impl Default for MaterialDesc {
    fn default() -> Self {
        Self {
            base_color: [0.66, 0.82, 0.96],
            metallic: 0.35,
            roughness: 0.42,
            emissive_color: [0.0; 3],
            emissive_strength: 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneItemDesc {
    pub id: String,
    pub kind: SceneKind,
    pub transform: Spatial,
    pub visible: bool,
    pub material: MaterialDesc,
    pub spin_speed: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LightDesc {
    pub id: String,
    pub transform: Spatial,
    pub intensity: f32,
    pub color: [f32; 3],
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Controls {
    pub params: BTreeMap<String, f32>,
    pub scene_items: Vec<SceneItemDesc>,
    pub scene_lights: Vec<LightDesc>,
    pub model_visible: bool,
    pub cube_visible: bool,
    pub field_visible: bool,
    pub model_transform: Spatial,
    pub cube_transform: Spatial,
    pub cloner_transform: Spatial,
    pub field_transform: Spatial,
    pub light_transform: Spatial,
    pub camera_settings: Option<OrbitSettings>,
    pub clone_mode: String,
    pub clone_count: u32,
    pub clone_animate: bool,
    /// Source cube material compiled for both instancing backends.
    /// None preserves the original procedural clone palette.
    pub clone_material: Option<MaterialDesc>,
    pub bloom_lfo: Option<Lfo>,
}

fn input<'a>(doc: &'a Document, node: &str, port: &str) -> Option<&'a Node> {
    let source = doc
        .edges
        .iter()
        .find(|e| e.to.node == node && e.to.port == port)?;
    doc.nodes.iter().find(|n| n.id == source.from.node)
}

fn number(node: &Node, name: &str, default: f32) -> f32 {
    node.params
        .get(name)
        .and_then(Value::as_f64)
        .map_or(default, |n| n as f32)
}

fn flag(node: &Node, name: &str, default: bool) -> bool {
    node.params
        .get(name)
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

fn color(node: &Node, name: &str, default: [f32; 3]) -> [f32; 3] {
    node.params
        .get(name)
        .and_then(Value::as_array)
        .and_then(|v| {
            if v.len() != 3 {
                return None;
            }
            Some([
                v[0].as_f64()? as f32,
                v[1].as_f64()? as f32,
                v[2].as_f64()? as f32,
            ])
        })
        .unwrap_or(default)
}

fn material_for(doc: &Document, cube: &Node) -> MaterialDesc {
    let Some(node) = input(doc, &cube.id, "material") else {
        return MaterialDesc::default();
    };
    if node.type_id != "material.pbr" {
        return MaterialDesc::default();
    }
    let defaults = MaterialDesc::default();
    MaterialDesc {
        base_color: color(node, "base_color", defaults.base_color),
        metallic: number(node, "metallic", defaults.metallic),
        roughness: number(node, "roughness", defaults.roughness),
        emissive_color: color(node, "emissive_color", defaults.emissive_color),
        emissive_strength: number(node, "emissive_strength", defaults.emissive_strength),
    }
}

fn inputs<'a>(doc: &'a Document, node: &str, port: &str) -> Vec<&'a Node> {
    doc.edges
        .iter()
        .filter(|e| e.to.node == node && e.to.port == port)
        .filter_map(|e| doc.nodes.iter().find(|n| n.id == e.from.node))
        .collect()
}

fn option<'a>(node: &'a Node, name: &str, default: &'a str) -> &'a str {
    node.params
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or(default)
}

/// Compile only nodes reachable from render.output into the currently supported
/// Bevy scene controls. This is an actual renderer projection, not UI-only nodes.
pub fn compile(doc: &Document) -> Result<Controls, GraphError> {
    let validated = validate(doc)?;
    let mut params = BTreeMap::from([
        ("light_intensity".to_owned(), 0.0),
        ("spin_speed".to_owned(), 0.0),
        ("bloom".to_owned(), 0.0),
        ("aberration".to_owned(), 0.0),
        ("vignette".to_owned(), 0.0),
        ("field_speed".to_owned(), 0.6),
        ("field_scale".to_owned(), 4.0),
    ]);
    let mut output = Controls {
        params: BTreeMap::new(),
        scene_items: Vec::new(),
        scene_lights: Vec::new(),
        model_visible: false,
        cube_visible: false,
        field_visible: false,
        model_transform: Spatial {
            position: [0.0, 1.0, 0.0],
            scale: [100.0; 3],
            ..Spatial::default()
        },
        cube_transform: Spatial::default(),
        cloner_transform: Spatial::default(),
        field_transform: Spatial {
            position: [0.0, 2.4, -4.0],
            ..Spatial::default()
        },
        light_transform: Spatial {
            position: [4.0, 7.0, 3.0],
            rotation: [-54.0, 53.0, 0.0],
            ..Spatial::default()
        },
        camera_settings: None,
        clone_mode: "off".into(),
        clone_count: 0,
        clone_animate: false,
        clone_material: None,
        bloom_lfo: None,
    };
    let Some(renderer) = doc.nodes.iter().find(|n| n.type_id == "render.output") else {
        output.params = params;
        return Ok(output);
    };
    if !validated.active.contains(&renderer.id) {
        output.params = params;
        return Ok(output);
    }
    if let Some(scene) = input(doc, &renderer.id, "scene") {
        if scene.type_id == "scene.compose" {
            for model in inputs(doc, &scene.id, "model") {
                if model.type_id != "asset.boombox" {
                    continue;
                }
                output.scene_items.push(SceneItemDesc {
                    id: model.id.clone(),
                    kind: SceneKind::Model,
                    transform: spatial(
                        model,
                        Spatial {
                            position: [0.0, 1.0, 0.0],
                            scale: [100.0; 3],
                            ..Spatial::default()
                        },
                    ),
                    visible: flag(model, "visible", true),
                    material: MaterialDesc::default(),
                    spin_speed: number(model, "spin_speed", 0.35),
                });
            }
            for cube in inputs(doc, &scene.id, "geometry") {
                if cube.type_id != "geometry.cube" {
                    continue;
                }
                output.scene_items.push(SceneItemDesc {
                    id: cube.id.clone(),
                    kind: SceneKind::Cube,
                    transform: spatial(cube, Spatial::default()),
                    visible: flag(cube, "visible", true),
                    material: material_for(doc, cube),
                    spin_speed: 0.0,
                });
            }
            for light in inputs(doc, &scene.id, "light") {
                if light.type_id != "lighting.directional" {
                    continue;
                }
                output.scene_lights.push(LightDesc {
                    id: light.id.clone(),
                    transform: spatial(
                        light,
                        Spatial {
                            position: [4.0, 7.0, 3.0],
                            rotation: [-54.0, 53.0, 0.0],
                            ..Spatial::default()
                        },
                    ),
                    intensity: number(light, "intensity", 8000.0),
                    color: color(light, "color", [1.0; 3]),
                    enabled: flag(light, "enabled", true),
                });
            }
            if let Some(model) = input(doc, &scene.id, "model") {
                output.model_visible = true;
                params.insert("spin_speed".into(), number(model, "spin_speed", 0.35));
                output.model_transform = spatial(model, output.model_transform);
            }
            if let Some(cube) = input(doc, &scene.id, "geometry") {
                if cube.type_id == "geometry.cube" {
                    output.cube_visible = true;
                    output.cube_transform = spatial(cube, output.cube_transform);
                }
            }
            if let Some(light) = input(doc, &scene.id, "light") {
                params.insert("light_intensity".into(), number(light, "intensity", 8000.0));
                output.light_transform = spatial(light, output.light_transform);
            }
            if let Some(field) = input(doc, &scene.id, "field") {
                output.field_visible = true;
                params.insert("field_speed".into(), number(field, "speed", 0.6));
                params.insert("field_scale".into(), number(field, "scale", 4.0));
                output.field_transform = spatial(field, output.field_transform);
            }
            if let Some(cloner) = input(doc, &scene.id, "clones") {
                if input(doc, &cloner.id, "source").is_some() {
                    output.clone_count = number(cloner, "count", 2000.0) as u32;
                    output.clone_mode = option(cloner, "mode", "custom").into();
                    output.clone_animate = option(cloner, "animate", "on") == "on";
                    output.cloner_transform = spatial(cloner, output.cloner_transform);
                    if let Some(cube) = input(doc, &cloner.id, "source") {
                        if cube.type_id == "geometry.cube" {
                            output.cube_transform = spatial(cube, output.cube_transform);
                            if input(doc, &cube.id, "material")
                                .is_some_and(|material| material.type_id == "material.pbr")
                            {
                                output.clone_material = Some(material_for(doc, cube));
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(camera) = input(doc, &renderer.id, "camera") {
        output.camera_settings = Some(OrbitSettings {
            target: [
                number(camera, "target_x", 0.0),
                number(camera, "target_y", 1.0),
                number(camera, "target_z", 0.0),
            ],
            distance: number(camera, "distance", 12.0),
            yaw: number(camera, "yaw", 34.377),
            pitch: number(camera, "pitch", 16.043),
        });
        for (port, param, default) in [
            ("bloom", "bloom", 0.18),
            ("aberration", "aberration", 0.006),
            ("vignette", "vignette", 0.35),
        ] {
            if let Some(effect) = input(doc, &renderer.id, port) {
                params.insert(param.into(), number(effect, "amount", default));
                if port == "bloom" {
                    if let Some(lfo) = input(doc, &effect.id, "amount") {
                        if lfo.type_id != "animation.lfo" {
                            return Err(err("bloom amount currently supports an LFO source only"));
                        }
                        output.bloom_lfo = Some(Lfo {
                            frequency: number(lfo, "frequency", 0.25),
                            minimum: number(lfo, "minimum", 0.0),
                            maximum: number(lfo, "maximum", 0.6),
                        });
                    }
                }
            }
        }
    }
    output.params = params;
    Ok(output)
}

impl Node {
    pub fn new(id: &str, type_id: &str, x: f32, y: f32) -> Self {
        let mut params = BTreeMap::new();
        if let Some(spec) = registry().into_iter().find(|s| s.type_id == type_id) {
            for parameter in spec.params {
                params.insert(parameter.name.to_owned(), parameter.default);
            }
        }
        Self {
            id: id.into(),
            type_id: type_id.into(),
            version: 1,
            params,
            ui: Position { x, y },
        }
    }
}

impl Document {
    pub fn demo() -> Self {
        let nodes = vec![
            Node::new("model", "asset.boombox", 80.0, 90.0),
            Node::new("cube", "geometry.cube", 80.0, 280.0),
            Node::new("material", "material.pbr", 80.0, 390.0),
            Node::new("clones", "clone.fibonacci", 350.0, 280.0),
            Node::new("light", "lighting.directional", 350.0, 530.0),
            Node::new("field", "field.compute2d", 80.0, 510.0),
            Node::new("scene", "scene.compose", 650.0, 190.0),
            Node::new("camera", "camera.orbit", 650.0, 500.0),
            Node::new("bloom", "fx.bloom", 930.0, 420.0),
            Node::new("chromatic", "fx.aberration", 930.0, 590.0),
            Node::new("vignette", "fx.vignette", 930.0, 740.0),
            Node::new("output", "render.output", 1230.0, 180.0),
        ];
        let connections = [
            ("model", "object", "scene", "model"),
            ("cube", "geometry", "scene", "geometry"),
            ("cube", "geometry", "clones", "source"),
            ("material", "material", "cube", "material"),
            ("clones", "instances", "scene", "clones"),
            ("light", "light", "scene", "light"),
            ("field", "texture", "scene", "field"),
            ("scene", "scene", "output", "scene"),
            ("camera", "camera", "output", "camera"),
            ("bloom", "effect", "output", "bloom"),
            ("chromatic", "effect", "output", "aberration"),
            ("vignette", "effect", "output", "vignette"),
        ];
        let edges = connections
            .into_iter()
            .map(|(a, b, c, d)| Edge {
                from: PortRef {
                    node: a.into(),
                    port: b.into(),
                },
                to: PortRef {
                    node: c.into(),
                    port: d.into(),
                },
            })
            .collect();
        Self {
            format_version: FORMAT_VERSION,
            name: "Grafix — First Composition".into(),
            nodes,
            edges,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn demo_compiles_to_live_renderer_controls() {
        let doc = Document::demo();
        let p = compile(&doc).unwrap();
        assert!(p.model_visible && p.field_visible);
        assert_eq!(p.clone_count, 2000);
        assert_eq!(p.clone_mode, "custom");
        assert_eq!(p.params["bloom"], 0.18);
        assert_eq!(validate(&doc).unwrap().order.len(), doc.nodes.len());
        assert_eq!(
            serde_json::from_str::<Document>(&serde_json::to_string(&doc).unwrap()).unwrap(),
            doc
        );
    }
    #[test]
    fn removing_a_connection_deactivates_its_output() {
        let mut doc = Document::demo();
        doc.edges
            .retain(|e| !(e.from.node == "clones" && e.to.node == "scene"));
        assert_eq!(compile(&doc).unwrap().clone_mode, "off");
        doc.edges
            .retain(|e| !(e.from.node == "model" && e.to.node == "scene"));
        assert!(!compile(&doc).unwrap().model_visible);
    }
    #[test]
    fn invalid_ports_and_duplicates_are_rejected() {
        let mut doc = Document::demo();
        doc.edges.push(Edge {
            from: PortRef {
                node: "cube".into(),
                port: "geometry".into(),
            },
            to: PortRef {
                node: "output".into(),
                port: "scene".into(),
            },
        });
        assert!(validate(&doc).unwrap_err().0.contains("incompatible ports"));
        doc.edges.pop();
        doc.edges.push(doc.edges[0].clone());
        assert!(validate(&doc).unwrap_err().0.contains("already connected"));
    }
    #[test]
    fn cycles_and_invalid_parameters_fail() {
        let mut doc = Document::demo();
        doc.nodes.push(Node::new("math", "math.add", 60.0, 800.0));
        doc.edges.push(Edge {
            from: PortRef {
                node: "math".into(),
                port: "value".into(),
            },
            to: PortRef {
                node: "math".into(),
                port: "a".into(),
            },
        });
        assert_eq!(validate(&doc).unwrap_err().0, "graph contains a cycle");
        doc.edges.pop();
        doc.nodes[0].params.insert("spin_speed".into(), json!(99.0));
        assert!(validate(&doc).unwrap_err().0.contains("out of range"));
    }
    #[test]
    fn fibonacci_settings_reach_bevy_only_for_connected_cloner() {
        let mut document = Document::demo();
        let original = document
            .nodes
            .iter_mut()
            .find(|n| n.id == "clones")
            .unwrap();
        original.params.insert("count".into(), json!(7654));
        original.params.insert("mode".into(), json!("builtin"));
        let compiled = compile(&document).unwrap();
        assert_eq!(compiled.clone_count, 7654);
        assert_eq!(compiled.clone_mode, "builtin");

        let mut extra = Node::new("second-cloner", "clone.fibonacci", 100.0, 900.0);
        extra.params.insert("count".into(), json!(300));
        document.nodes.push(extra);
        // A detached node is editable but does not alter the rendered output.
        assert_eq!(compile(&document).unwrap().clone_count, 7654);

        document
            .edges
            .retain(|e| !(e.to.node == "scene" && e.to.port == "clones"));
        document.edges.push(Edge {
            from: PortRef {
                node: "cube".into(),
                port: "geometry".into(),
            },
            to: PortRef {
                node: "second-cloner".into(),
                port: "source".into(),
            },
        });
        document.edges.push(Edge {
            from: PortRef {
                node: "second-cloner".into(),
                port: "instances".into(),
            },
            to: PortRef {
                node: "scene".into(),
                port: "clones".into(),
            },
        });
        assert_eq!(compile(&document).unwrap().clone_count, 300);
        assert_eq!(compile(&document).unwrap().clone_mode, "custom");
    }

    #[test]
    fn spatial_settings_compile_to_renderer_and_stay_backward_compatible() {
        let mut doc = Document::demo();
        let model = doc.nodes.iter_mut().find(|n| n.id == "model").unwrap();
        model.params.insert("position_x".into(), json!(4.25));
        model.params.insert("rotation_y".into(), json!(90.0));
        model.params.insert("scale_x".into(), json!(32.0));
        let cube = doc.nodes.iter_mut().find(|n| n.id == "cube").unwrap();
        cube.params.insert("position_y".into(), json!(2.0));
        cube.params.insert("scale_z".into(), json!(2.5));
        let cloner = doc.nodes.iter_mut().find(|n| n.id == "clones").unwrap();
        cloner.params.insert("rotation_z".into(), json!(22.0));
        let result = compile(&doc).unwrap();
        assert_eq!(result.model_transform.position[0], 4.25);
        assert_eq!(result.model_transform.rotation[1], 90.0);
        assert_eq!(result.model_transform.scale[0], 32.0);
        assert_eq!(result.cube_transform.position[1], 2.0);
        assert_eq!(result.cube_transform.scale[2], 2.5);
        assert_eq!(result.cloner_transform.rotation[2], 22.0);
        assert!(result.cube_visible);
        // Older saved graphs omit these keys: default physical transforms must survive.
        doc.nodes.iter_mut().for_each(|n| {
            n.params.retain(|k, _| {
                !k.starts_with("position_")
                    && !k.starts_with("rotation_")
                    && !k.starts_with("scale_")
            })
        });
        let prior = compile(&doc).unwrap();
        assert_eq!(prior.model_transform.scale, [100.0; 3]);
        assert_eq!(prior.cube_transform.scale, [1.0; 3]);
    }

    #[test]
    fn spatial_ranges_are_validated_and_unconnected_objects_are_inactive() {
        let mut doc = Document::demo();
        doc.nodes
            .iter_mut()
            .find(|n| n.id == "model")
            .unwrap()
            .params
            .insert("scale_z".into(), json!(0.0));
        assert!(validate(&doc).unwrap_err().0.contains("out of range"));
        doc.nodes
            .iter_mut()
            .find(|n| n.id == "model")
            .unwrap()
            .params
            .insert("scale_z".into(), json!(1.0));
        doc.edges.retain(|e| {
            !(e.from.node == "cube" && e.to.node == "scene" && e.to.port == "geometry")
        });
        assert!(!compile(&doc).unwrap().cube_visible);
        assert_eq!(compile(&doc).unwrap().clone_count, 2000);
    }

    #[test]
    fn two_cubes_with_independent_materials_and_transforms_compile() {
        let mut doc = Document::demo();
        let mut second = Node::new("cube-two", "geometry.cube", 140.0, 750.0);
        second.params.insert("position_x".into(), json!(3.25));
        doc.nodes.push(second);
        let mut red = Node::new("mat-red", "material.pbr", 50.0, 950.0);
        red.params
            .insert("base_color".into(), json!([1.0, 0.1, 0.2]));
        doc.nodes.push(red);
        doc.edges.push(Edge {
            from: PortRef {
                node: "mat-red".into(),
                port: "material".into(),
            },
            to: PortRef {
                node: "cube-two".into(),
                port: "material".into(),
            },
        });
        doc.edges.push(Edge {
            from: PortRef {
                node: "cube-two".into(),
                port: "geometry".into(),
            },
            to: PortRef {
                node: "scene".into(),
                port: "geometry".into(),
            },
        });
        let result = compile(&doc).unwrap();
        let cubes: Vec<_> = result
            .scene_items
            .iter()
            .filter(|i| i.kind == SceneKind::Cube)
            .collect();
        assert_eq!(cubes.len(), 2);
        assert_eq!(cubes[0].id, "cube");
        assert_eq!(cubes[1].id, "cube-two");
        assert_eq!(cubes[1].transform.position[0], 3.25);
        assert_eq!(cubes[1].material.base_color, [1.0, 0.1, 0.2]);
        assert_eq!(
            cubes[0].material.base_color,
            MaterialDesc::default().base_color
        );
        assert!(validate(&doc).is_ok());
        // Replacing the material must not change the other item's style.
        let first = result.scene_items[0].clone();
        assert_eq!(compile(&doc).unwrap().scene_items[0], first);
    }

    #[test]
    fn typed_parameter_bounds_and_multiport_duplicate_rules() {
        let mut doc = Document::demo();
        doc.nodes
            .iter_mut()
            .find(|n| n.id == "material")
            .unwrap()
            .params
            .insert("base_color".into(), json!([1.0, 0.4]));
        assert!(validate(&doc).unwrap_err().0.contains("RGB channels"));
        doc.nodes
            .iter_mut()
            .find(|n| n.id == "material")
            .unwrap()
            .params
            .insert("base_color".into(), json!([1.0, 0.4, 0.2]));
        doc.nodes
            .iter_mut()
            .find(|n| n.id == "cube")
            .unwrap()
            .params
            .insert("visible".into(), json!(0));
        assert!(validate(&doc).unwrap_err().0.contains("boolean"));
        doc.nodes
            .iter_mut()
            .find(|n| n.id == "cube")
            .unwrap()
            .params
            .insert("visible".into(), json!(false));
        assert!(
            !compile(&doc)
                .unwrap()
                .scene_items
                .iter()
                .find(|i| i.id == "cube")
                .unwrap()
                .visible
        );
        doc.edges.push(
            doc.edges
                .iter()
                .find(|e| e.from.node == "cube" && e.to.node == "scene" && e.to.port == "geometry")
                .unwrap()
                .clone(),
        );
        assert!(validate(&doc).unwrap_err().0.contains("duplicate"));
    }

    #[test]
    fn two_object_preset_compiles_into_stable_scene_ids() {
        let fixture = include_str!("../../../assets/presets/two-material-cubes.grafix.json");
        let doc: Document = serde_json::from_str(fixture).unwrap();
        let result = compile(&doc).unwrap();
        assert_eq!(result.scene_items.len(), 2);
        assert_eq!(result.scene_lights.len(), 2);
        assert_eq!(result.scene_items[0].id, "cube-red");
        assert_eq!(result.scene_items[1].id, "cube-blue");
        assert_eq!(result.scene_items[0].transform.position[0], -2.0);
        assert_eq!(result.scene_items[1].transform.position[0], 2.0);
        assert_ne!(
            result.scene_items[0].material,
            result.scene_items[1].material
        );
        assert!(result.scene_items.iter().all(|item| item.visible));
        assert_eq!(result.clone_mode, "off");
    }

    #[test]
    fn connected_cloner_inherits_source_material() {
        let mut doc = Document::demo();
        let mat = doc.nodes.iter_mut().find(|n| n.id == "material").unwrap();
        mat.params.insert("base_color".into(), json!([0.92, 0.12, 0.08]));
        mat.params.insert("metallic".into(), json!(0.8));
        let compiled = compile(&doc).unwrap();
        let linked = compiled.clone_material.expect("source PBR material");
        assert_eq!(linked.base_color, [0.92, 0.12, 0.08]);
        assert_eq!(linked.metallic, 0.8);
        assert_eq!(compiled.scene_items.iter().find(|item| item.id == "cube")
            .unwrap().material, linked);
        doc.edges.retain(|e| !(e.to.node == "cube" && e.to.port == "material"));
        assert!(compile(&doc).unwrap().clone_material.is_none());
        doc.nodes.push(Node::new("detached", "material.pbr", 30.0, 850.0));
        assert!(compile(&doc).unwrap().clone_material.is_none());
    }

    #[test]
    fn modulation_only_activates_when_connected() {
        let mut doc = Document::demo();
        doc.nodes
            .push(Node::new("lfo", "animation.lfo", 640.0, 750.0));
        doc.edges.push(Edge {
            from: PortRef {
                node: "lfo".into(),
                port: "value".into(),
            },
            to: PortRef {
                node: "bloom".into(),
                port: "amount".into(),
            },
        });
        assert!(compile(&doc).unwrap().bloom_lfo.is_some());
        doc.edges.pop();
        assert!(compile(&doc).unwrap().bloom_lfo.is_none());
    }
}
