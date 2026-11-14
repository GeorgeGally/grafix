//! Engine-independent procedural document, schema, type checking and compilation.
//! No Bevy, DOM, WebGPU or JavaScript types belong in this crate.
//! Only explicitly supported operations are executable; other node families
//! will be added as their renderer backends ship.

use std::collections::{BTreeMap, BTreeSet};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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

fn one() -> u32 { 1 }

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

fn port(name: &'static str, data_type: DataType) -> PortSpec { PortSpec { name, data_type } }
fn num(name: &'static str, default: f64, min: f64, max: f64) -> ParamSpec {
    ParamSpec { name, kind: "number", default: json!(default), min: Some(min), max: Some(max), options: vec![] }
}
fn choice(name: &'static str, default: &'static str, options: Vec<&'static str>) -> ParamSpec {
    ParamSpec { name, kind: "enum", default: json!(default), min: None, max: None, options }
}

/// Registry is the source of the editor's node palette and parameter inspectors.
/// A node is only listed if it has an actual runtime implementation.
pub fn registry() -> Vec<NodeSpec> {
    use DataType::*;
    vec![
        NodeSpec { type_id: "asset.boombox", title: "GLB Model", category: "3D",
            description: "The licensed BoomBox glTF asset used by the renderer proof.",
            inputs: vec![], outputs: vec![port("object", SceneObject)],
            params: vec![num("spin_speed", 0.35, -3.0, 3.0)] },
        NodeSpec { type_id: "geometry.cube", title: "Cube", category: "3D",
            description: "Instanced cube geometry.", inputs: vec![],
            outputs: vec![port("geometry", Geometry)], params: vec![] },
        NodeSpec { type_id: "clone.fibonacci", title: "Fibonacci Cloner", category: "Cloning",
            description: "GPU-instanced copies in an even spherical distribution.",
            inputs: vec![port("source", Geometry)], outputs: vec![port("instances", InstanceSet)],
            params: vec![num("count", 2000.0, 0.0, 100000.0),
                         choice("mode", "custom", vec!["custom", "builtin", "unbatched"]),
                         choice("animate", "on", vec!["on", "off"])] },
        NodeSpec { type_id: "lighting.directional", title: "Key Light", category: "Lighting",
            description: "PBR directional light and shadows.",
            inputs: vec![], outputs: vec![port("light", Light)],
            params: vec![num("intensity", 8000.0, 0.0, 30000.0)] },
        NodeSpec { type_id: "field.compute2d", title: "Compute Field 2D", category: "Fields",
            description: "The working 2D GPU procedural texture. Not a voxel fluid solver.",
            inputs: vec![], outputs: vec![port("texture", Texture2D)],
            params: vec![num("speed", 0.6, 0.0, 3.0), num("scale", 4.0, 1.0, 12.0)] },
        NodeSpec { type_id: "scene.compose", title: "Scene", category: "Rendering",
            description: "Collects scene objects, instances, lights and field display.",
            inputs: vec![port("model", SceneObject), port("clones", InstanceSet),
                         port("light", Light), port("field", Texture2D)],
            outputs: vec![port("scene", Scene)], params: vec![] },
        NodeSpec { type_id: "camera.orbit", title: "Orbit Camera", category: "Rendering",
            description: "Interactive orbit and perspective camera.",
            inputs: vec![], outputs: vec![port("camera", Camera)], params: vec![] },
        NodeSpec { type_id: "fx.bloom", title: "Bloom", category: "Rendering",
            description: "HDR bloom post-effect.", inputs: vec![port("amount", Float)],
            outputs: vec![port("effect", Effect)], params: vec![num("amount", 0.18, 0.0, 1.0)] },
        NodeSpec { type_id: "fx.aberration", title: "Chromatic Aberration", category: "Rendering",
            description: "RGB lens offset.", inputs: vec![],
            outputs: vec![port("effect", Effect)], params: vec![num("amount", 0.006, 0.0, 0.05)] },
        NodeSpec { type_id: "fx.vignette", title: "Vignette", category: "Rendering",
            description: "Soft edge falloff.", inputs: vec![],
            outputs: vec![port("effect", Effect)], params: vec![num("amount", 0.35, 0.0, 1.0)] },
        NodeSpec { type_id: "animation.lfo", title: "LFO", category: "Data",
            description: "Time-driven sine signal for a connected numeric parameter.",
            inputs: vec![], outputs: vec![port("value", Float)],
            params: vec![num("frequency", 0.25, 0.0, 10.0),
                         num("minimum", 0.0, 0.0, 1.0), num("maximum", 0.6, 0.0, 1.0)] },
        NodeSpec { type_id: "math.add", title: "Add", category: "Data",
            description: "Reserved float data operator (validated but not yet evaluated).",
            inputs: vec![port("a", Float), port("b", Float)],
            outputs: vec![port("value", Float)], params: vec![] },
        NodeSpec { type_id: "render.output", title: "Output", category: "Rendering",
            description: "Final camera, scene and post-effects presented in the viewport.",
            inputs: vec![port("scene", Scene), port("camera", Camera),
                         port("bloom", Effect), port("aberration", Effect),
                         port("vignette", Effect)],
            outputs: vec![], params: vec![] },
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphError(pub String);

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.0) }
}
impl std::error::Error for GraphError {}

fn err(message: impl Into<String>) -> GraphError { GraphError(message.into()) }

pub struct Validated {
    pub order: Vec<String>,
    pub active: BTreeSet<String>,
}

/// Structural validation happens independently of a graphics device.
/// Partial graphs are allowed, but malformed connections and cycles are not.
pub fn validate(doc: &Document) -> Result<Validated, GraphError> {
    if doc.format_version != FORMAT_VERSION {
        return Err(err(format!("unsupported graph format version {}", doc.format_version)));
    }
    if doc.nodes.len() > 2000 || doc.edges.len() > 8000 {
        return Err(err("graph exceeds the safety limit (2000 nodes / 8000 edges)"));
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
        let spec = specs.iter().find(|s| s.type_id == node.type_id)
            .ok_or_else(|| err(format!("unknown node type {}", node.type_id)))?;
        if node.version != 1 { return Err(err(format!("unsupported node version: {}", node.id))); }
        for (name, value) in &node.params {
            let param = spec.params.iter().find(|p| p.name == name)
                .ok_or_else(|| err(format!("unknown parameter {}.{}", node.id, name)))?;
            match param.kind {
                "number" => {
                    let n = value.as_f64().ok_or_else(|| err(format!("{}.{} must be a number", node.id, name)))?;
                    if !n.is_finite() || param.min.is_some_and(|min| n < min)
                        || param.max.is_some_and(|max| n > max) {
                        return Err(err(format!("{}.{} is out of range", node.id, name)));
                    }
                }
                "enum" => {
                    let selection = value.as_str().ok_or_else(|| err(format!("{}.{} must be a choice", node.id, name)))?;
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
    let mut indegree: BTreeMap<&str, usize> = doc.nodes.iter().map(|n| (n.id.as_str(), 0)).collect();
    for edge in &doc.edges {
        let source_spec = nodes.get(edge.from.node.as_str())
            .ok_or_else(|| err(format!("missing source node {}", edge.from.node)))?;
        let target_spec = nodes.get(edge.to.node.as_str())
            .ok_or_else(|| err(format!("missing target node {}", edge.to.node)))?;
        let source_port = source_spec.outputs.iter().find(|p| p.name == edge.from.port)
            .ok_or_else(|| err(format!("unknown output {}.{}", edge.from.node, edge.from.port)))?;
        let target_port = target_spec.inputs.iter().find(|p| p.name == edge.to.port)
            .ok_or_else(|| err(format!("unknown input {}.{}", edge.to.node, edge.to.port)))?;
        if source_port.data_type != target_port.data_type {
            return Err(err(format!("incompatible ports: {}.{} -> {}.{}",
                edge.from.node, edge.from.port, edge.to.node, edge.to.port)));
        }
        if !destinations.insert((edge.to.node.as_str(), edge.to.port.as_str())) {
            return Err(err(format!("input already connected: {}.{}", edge.to.node, edge.to.port)));
        }
        outgoing.entry(&edge.from.node).or_default().push(&edge.to.node);
        incoming.entry(&edge.to.node).or_default().push(&edge.from.node);
        *indegree.get_mut(edge.to.node.as_str()).expect("target exists") += 1;
    }
    let mut ready: BTreeSet<&str> = indegree.iter()
        .filter_map(|(&id, &count)| (count == 0).then_some(id)).collect();
    let mut order = Vec::with_capacity(doc.nodes.len());
    while let Some(id) = ready.pop_first() {
        order.push(id.to_owned());
        for &target in outgoing.get(id).into_iter().flatten() {
            let count = indegree.get_mut(target).expect("connected node");
            *count -= 1;
            if *count == 0 { ready.insert(target); }
        }
    }
    if order.len() != doc.nodes.len() { return Err(err("graph contains a cycle")); }

    let mut active: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<&str> = doc.nodes.iter()
        .filter(|n| n.type_id == "render.output").map(|n| n.id.as_str()).collect();
    if stack.len() > 1 { return Err(err("only one render.output is supported")); }
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

#[derive(Clone, Debug, PartialEq)]
pub struct Controls {
    pub params: BTreeMap<String, f32>,
    pub model_visible: bool,
    pub field_visible: bool,
    pub clone_mode: String,
    pub clone_count: u32,
    pub clone_animate: bool,
    pub bloom_lfo: Option<Lfo>,
}

fn input<'a>(doc: &'a Document, node: &str, port: &str) -> Option<&'a Node> {
    let source = doc.edges.iter().find(|e| e.to.node == node && e.to.port == port)?;
    doc.nodes.iter().find(|n| n.id == source.from.node)
}

fn number(node: &Node, name: &str, default: f32) -> f32 {
    node.params.get(name).and_then(Value::as_f64)
        .map_or(default, |n| n as f32)
}

fn option<'a>(node: &'a Node, name: &str, default: &'a str) -> &'a str {
    node.params.get(name).and_then(Value::as_str).unwrap_or(default)
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
        model_visible: false,
        field_visible: false,
        clone_mode: "off".into(),
        clone_count: 0,
        clone_animate: false,
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
            if let Some(model) = input(doc, &scene.id, "model") {
                output.model_visible = true;
                params.insert("spin_speed".into(), number(model, "spin_speed", 0.35));
            }
            if let Some(light) = input(doc, &scene.id, "light") {
                params.insert("light_intensity".into(), number(light, "intensity", 8000.0));
            }
            if let Some(field) = input(doc, &scene.id, "field") {
                output.field_visible = true;
                params.insert("field_speed".into(), number(field, "speed", 0.6));
                params.insert("field_scale".into(), number(field, "scale", 4.0));
            }
            if let Some(cloner) = input(doc, &scene.id, "clones") {
                if input(doc, &cloner.id, "source").is_some() {
                    output.clone_count = number(cloner, "count", 2000.0) as u32;
                    output.clone_mode = option(cloner, "mode", "custom").into();
                    output.clone_animate = option(cloner, "animate", "on") == "on";
                }
            }
        }
    }
    if input(doc, &renderer.id, "camera").is_some() {
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
        Self { id: id.into(), type_id: type_id.into(), version: 1, params, ui: Position { x, y } }
    }
}

impl Document {
    pub fn demo() -> Self {
        let nodes = vec![
            Node::new("model", "asset.boombox", 80.0, 90.0),
            Node::new("cube", "geometry.cube", 80.0, 280.0),
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
            ("cube", "geometry", "clones", "source"),
            ("clones", "instances", "scene", "clones"),
            ("light", "light", "scene", "light"),
            ("field", "texture", "scene", "field"),
            ("scene", "scene", "output", "scene"),
            ("camera", "camera", "output", "camera"),
            ("bloom", "effect", "output", "bloom"),
            ("chromatic", "effect", "output", "aberration"),
            ("vignette", "effect", "output", "vignette"),
        ];
        let edges = connections.into_iter().map(|(a,b,c,d)| Edge {
            from: PortRef { node: a.into(), port: b.into() },
            to: PortRef { node: c.into(), port: d.into() },
        }).collect();
        Self { format_version: FORMAT_VERSION, name: "Grafix — First Composition".into(), nodes, edges }
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
        assert_eq!(serde_json::from_str::<Document>(&serde_json::to_string(&doc).unwrap()).unwrap(), doc);
    }
    #[test]
    fn removing_a_connection_deactivates_its_output() {
        let mut doc = Document::demo();
        doc.edges.retain(|e| !(e.from.node == "clones" && e.to.node == "scene"));
        assert_eq!(compile(&doc).unwrap().clone_mode, "off");
        doc.edges.retain(|e| !(e.from.node == "model" && e.to.node == "scene"));
        assert!(!compile(&doc).unwrap().model_visible);
    }
    #[test]
    fn invalid_ports_and_duplicates_are_rejected() {
        let mut doc = Document::demo();
        doc.edges.push(Edge { from: PortRef { node:"cube".into(), port:"geometry".into() },
            to: PortRef { node:"output".into(), port:"scene".into() } });
        assert!(validate(&doc).unwrap_err().0.contains("incompatible ports"));
        doc.edges.pop();
        doc.edges.push(doc.edges[0].clone());
        assert!(validate(&doc).unwrap_err().0.contains("already connected"));
    }
    #[test]
    fn cycles_and_invalid_parameters_fail() {
        let mut doc = Document::demo();
        doc.nodes.push(Node::new("math", "math.add", 60.0, 800.0));
        doc.edges.push(Edge { from: PortRef {node:"math".into(), port:"value".into()},
                              to: PortRef {node:"math".into(), port:"a".into()} });
        assert_eq!(validate(&doc).unwrap_err().0, "graph contains a cycle");
        doc.edges.pop();
        doc.nodes[0].params.insert("spin_speed".into(), json!(99.0));
        assert!(validate(&doc).unwrap_err().0.contains("out of range"));
    }
    #[test]
    fn modulation_only_activates_when_connected() {
        let mut doc = Document::demo();
        doc.nodes.push(Node::new("lfo", "animation.lfo", 640.0, 750.0));
        doc.edges.push(Edge { from: PortRef {node:"lfo".into(), port:"value".into()},
                              to: PortRef {node:"bloom".into(), port:"amount".into()} });
        assert!(compile(&doc).unwrap().bloom_lfo.is_some());
        doc.edges.pop();
        assert!(compile(&doc).unwrap().bloom_lfo.is_none());
    }
}
