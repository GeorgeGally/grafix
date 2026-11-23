// Test-only browser stub. Never ship this file in a release build.
const nodeTypes = [
  {
    type_id: 'asset.boombox', title: 'GLB Model', category: '3D',
    description: 'A bundled glTF model',
    inputs: [], outputs: [{ name: 'object', data_type: 'scene_object' }],
    params: [{ name: 'spin_speed', kind: 'number', default: 0.35, min: -3, max: 3 }],
  },
  {
    type_id: 'geometry.cube', title: 'Cube', category: '3D', description: 'A cube',
    inputs: [{ name: 'material', data_type: 'material' }],
    outputs: [{ name: 'geometry', data_type: 'geometry' }],
    params: [{ name: 'visible', kind: 'bool', default: true, group: 'Object' }],
  },
  {
    type_id: 'geometry.sphere', title: 'Sphere', category: '3D',
    description: 'UV sphere with a real mesh',
    inputs: [{ name: 'material', data_type: 'material' }],
    outputs: [{ name: 'geometry', data_type: 'geometry' }],
    params: [
      { name: 'visible', kind: 'bool', default: true, group: 'Object' },
      { name: 'radius', kind: 'number', default: 0.5, min: 0.01, max: 30 },
      { name: 'segments', kind: 'number', default: 32, min: 3, max: 128 },
      { name: 'rings', kind: 'number', default: 20, min: 2, max: 128 },
    ],
  },
  {
    type_id: 'geometry.cylinder', title: 'Cylinder', category: '3D',
    description: 'Procedural cylinder mesh',
    inputs: [{ name: 'material', data_type: 'material' }],
    outputs: [{ name: 'geometry', data_type: 'geometry' }],
    params: [
      { name: 'visible', kind: 'bool', default: true, group: 'Object' },
      { name: 'radius', kind: 'number', default: 0.5, min: 0.01, max: 30 },
      { name: 'height', kind: 'number', default: 1, min: 0.01, max: 100 },
    ],
  },
  {
    type_id: 'geometry.plane', title: 'Plane', category: '3D',
    description: 'Procedural plane mesh',
    inputs: [{ name: 'material', data_type: 'material' }],
    outputs: [{ name: 'geometry', data_type: 'geometry' }],
    params: [
      { name: 'visible', kind: 'bool', default: true, group: 'Object' },
      { name: 'width', kind: 'number', default: 2, min: 0.01, max: 100 },
      { name: 'depth', kind: 'number', default: 2, min: 0.01, max: 100 },
    ],
  },
  {
    type_id: 'geometry.torus', title: 'Torus', category: '3D',
    description: 'Procedural torus mesh',
    inputs: [{ name: 'material', data_type: 'material' }],
    outputs: [{ name: 'geometry', data_type: 'geometry' }],
    params: [
      { name: 'visible', kind: 'bool', default: true, group: 'Object' },
      { name: 'inner_radius', kind: 'number', default: 0.35, min: 0.01, max: 30 },
      { name: 'outer_radius', kind: 'number', default: 0.65, min: 0.02, max: 40 },
    ],
  },
  {
    type_id: 'material.pbr', title: 'PBR Material', category: 'Materials',
    description: 'Physically-based material',
    inputs: [], outputs: [{ name: 'material', data_type: 'material' }],
    params: [
      { name: 'base_color', kind: 'color', default: [0.66, 0.82, 0.96], group: 'Appearance' },
      { name: 'metallic', kind: 'number', default: 0.35, min: 0, max: 1, group: 'Appearance' },
      { name: 'roughness', kind: 'number', default: 0.42, min: 0.02, max: 1, group: 'Appearance' },
      { name: 'emissive_color', kind: 'color', default: [0, 0, 0], group: 'Appearance' },
      { name: 'emissive_strength', kind: 'number', default: 0, min: 0, max: 10, group: 'Appearance' },
    ],
  },
  {
    type_id: 'clone.fibonacci', title: 'Fibonacci Cloner', category: 'Cloning',
    description: 'GPU-instanced copies in a spherical distribution',
    inputs: [{ name: 'source', data_type: 'geometry' }],
    outputs: [{ name: 'instances', data_type: 'instance_set' }],
    params: [
      { name: 'count', kind: 'number', default: 2000, min: 0, max: 100000 },
      { name: 'mode', kind: 'enum', default: 'custom', options: ['custom', 'builtin', 'unbatched'] },
      { name: 'animate', kind: 'enum', default: 'on', options: ['on', 'off'] },
    ],
  },
  {
    type_id: 'clone.grid', title: 'Grid Cloner', category: 'Cloning',
    description: '3D grid of cube instances',
    inputs: [{ name: 'source', data_type: 'geometry' }],
    outputs: [{ name: 'instances', data_type: 'instance_set' }],
    params: [
      { name: 'count', kind: 'number', default: 125, min: 0, max: 100000 },
      { name: 'spacing', kind: 'number', default: 0.7, min: 0.05, max: 10 },
      { name: 'mode', kind: 'enum', default: 'custom', options: ['custom', 'builtin', 'unbatched'] },
      { name: 'animate', kind: 'enum', default: 'off', options: ['on', 'off'] },
    ],
  },
  {
    type_id: 'clone.radial', title: 'Radial Cloner', category: 'Cloning',
    description: 'Radial ring of cube instances',
    inputs: [{ name: 'source', data_type: 'geometry' }],
    outputs: [{ name: 'instances', data_type: 'instance_set' }],
    params: [
      { name: 'count', kind: 'number', default: 80, min: 0, max: 100000 },
      { name: 'radius', kind: 'number', default: 3.4, min: 0.1, max: 30 },
      { name: 'mode', kind: 'enum', default: 'custom', options: ['custom', 'builtin', 'unbatched'] },
      { name: 'animate', kind: 'enum', default: 'off', options: ['on', 'off'] },
    ],
  },
  {
    type_id: 'scene.compose', title: 'Scene', category: 'Rendering', description: 'Scene compositor',
    inputs: [
      { name: 'geometry', data_type: 'geometry' },
      { name: 'model', data_type: 'scene_object' },
      { name: 'clones', data_type: 'instance_set' },
      { name: 'light', data_type: 'light' },
      { name: 'field', data_type: 'texture2_d' },
    ],
    outputs: [{ name: 'scene', data_type: 'scene' }],
    params: [
      { name: 'show_grid', kind: 'bool', default: false, group: 'Object' },
      { name: 'ambient_color', kind: 'color', default: [0.75, 0.82, 1], group: 'Appearance' },
      { name: 'ambient_brightness', kind: 'number', default: 120, min: 0, max: 2000 },
    ],
  },
  {
    type_id: 'camera.orbit', title: 'Orbit Camera', category: 'Rendering',
    description: 'Viewport camera', inputs: [],
    outputs: [{ name: 'camera', data_type: 'camera' }], params: [],
  },
  {
    type_id: 'animation.lfo', title: 'LFO', category: 'Data',
    description: 'Normalized sine modulation',
    inputs: [], outputs: [{ name: 'value', data_type: 'float' }],
    params: [
      { name: 'frequency', kind: 'number', default: 0.25, min: 0, max: 10 },
      { name: 'minimum', kind: 'number', default: 0, min: 0, max: 1 },
      { name: 'maximum', kind: 'number', default: 0.6, min: 0, max: 1 },
    ],
  },
  {
    type_id: 'fx.bloom', title: 'Bloom', category: 'Rendering',
    description: 'Bloom with LFO amount input',
    inputs: [{ name: 'amount', data_type: 'float' }],
    outputs: [{ name: 'effect', data_type: 'effect' }],
    params: [{ name: 'amount', kind: 'number', default: 0.18, min: 0, max: 1 }],
  },
  {
    type_id: 'fx.aberration', title: 'Chromatic Aberration', category: 'Rendering',
    description: 'Chromatic aberration with LFO amount input',
    inputs: [{ name: 'amount', data_type: 'float' }],
    outputs: [{ name: 'effect', data_type: 'effect' }],
    params: [{ name: 'amount', kind: 'number', default: 0.006, min: 0, max: 0.05 }],
  },
  {
    type_id: 'fx.vignette', title: 'Vignette', category: 'Rendering',
    description: 'Vignette with LFO amount input',
    inputs: [{ name: 'amount', data_type: 'float' }],
    outputs: [{ name: 'effect', data_type: 'effect' }],
    params: [{ name: 'amount', kind: 'number', default: 0.35, min: 0, max: 1 }],
  },
  {
    type_id: 'render.output', title: 'Output', category: 'Rendering',
    description: 'Final render', inputs: [
      { name: 'scene', data_type: 'scene' }, { name: 'camera', data_type: 'camera' },
    ],
    outputs: [], params: [],
  },
];

// These definitions mirror graph-core's shared spatial schema for browser
// interaction tests; renderer semantics are covered by Rust compiler tests.
const spatial = (position, scale, rotation = [0, 0, 0]) =>
  [['position', position, -100, 100], ['rotation', rotation, -360, 360],
    ['scale', scale, 0.001, 1000]]
    .flatMap(([name, defaults, min, max]) => ['x', 'y', 'z'].map((axis, i) => ({
      name: name + '_' + axis, kind: 'number', default: defaults[i], min, max,
    })));
for (const spec of nodeTypes) {
  const defaults = spec.type_id === 'asset.boombox'
    ? [[0, 1, 0], [100, 100, 100]]
    : spec.type_id.startsWith('geometry.') || spec.type_id.startsWith('clone.')
      ? [[0, 0, 0], [1, 1, 1]] : null;
  if (defaults) spec.params.push(...spatial(...defaults));
}
nodeTypes.find((n) => n.type_id === 'camera.orbit').params.push(
  ...['target_x', 'target_y', 'target_z', 'distance', 'yaw', 'pitch']
    .map((name, i) => ({ name, kind: 'number',
      default: [0, 1, 0, 12, 34.377, 16.043][i],
      min: name === 'distance' ? 1.5 : -180,
      max: name === 'distance' ? 60 : 180,
    })),
);

const node = (id, type, x, y, params = {}) => ({
  id, type, ui: { x, y }, params, version: 1,
});
const edge = (a, b, c, d) => ({
  from: { node: a, port: b }, to: { node: c, port: d },
});
const initial = {
  format_version: 1, name: 'Browser Interaction Test',
  nodes: [
    node('cube', 'geometry.cube', 100, 250),
    node('clones', 'clone.fibonacci', 370, 240, { count: 2000, mode: 'custom', animate: 'on' }),
    node('scene', 'scene.compose', 680, 250),
    node('camera', 'camera.orbit', 680, 430),
    node('output', 'render.output', 1010, 250),
  ],
  edges: [
    edge('cube', 'geometry', 'clones', 'source'),
    edge('clones', 'instances', 'scene', 'clones'),
    edge('scene', 'scene', 'output', 'scene'),
    edge('camera', 'camera', 'output', 'camera'),
  ],
};

export default async function init() {}
export function graph_schema() { return JSON.stringify(nodeTypes); }
export function graph_status() {
  const doc = globalThis.__grafixGraph || initial;
  const reachable = new Set();
  const stack = doc.nodes.filter((n) => n.type === 'render.output').map((n) => n.id);
  while (stack.length) {
    const id = stack.pop();
    if (reachable.has(id)) continue;
    reachable.add(id);
    for (const edge of doc.edges) if (edge.to.node === id) stack.push(edge.from.node);
  }
  const approximate = new Set();
  for (const cloner of doc.nodes.filter((n) => n.type.startsWith('clone.') &&
      reachable.has(n.id) && (n.params.mode ?? 'custom') === 'custom')) {
    const source = doc.edges.find((e) => e.to.node === cloner.id && e.to.port === 'source');
    const material = source && doc.edges.find((e) => e.to.node === source.from.node &&
      e.to.port === 'material');
    if (material) approximate.add(material.from.node);
  }
  return JSON.stringify(doc.nodes.map((n) => ({
    id: n.id,
    state: !reachable.has(n.id) ? 'inactive' :
      approximate.has(n.id) ? 'partial' :
      n.type.startsWith('clone.') && !doc.edges.some((e) =>
        e.to.node === n.id && e.to.port === 'source') ? 'incomplete' : 'active',
    message: !reachable.has(n.id) ? 'Not connected to output' :
      approximate.has(n.id) ? 'Fast GPU clone mode approximates PBR lighting' :
      'Connected to render output',
  })));
}
export function default_graph() { return JSON.stringify(initial); }
export function load_graph(data) {
  const project = JSON.parse(data);
  if (project.nodes.filter((n) => n.type === 'render.output').length > 1) {
    throw new Error('only one output supported');
  }
  globalThis.__grafixGraph = project;
}
export function set_node_param(id, key, value) {
  const node = globalThis.__grafixGraph.nodes.find((n) => n.id === id);
  if (!node) throw new Error('unknown node');
  node.params[key] = value;
}
export function start() {}
export function set_param() {}
export function set_instancing() {}
export function set_animate_clones() {}
export function reset_camera() {}
export function take_selection() { return undefined; }
export function frame_node(id) { globalThis.__grafixFramed = id; }
export function set_playback(seconds, playing, rate, duration) {
  globalThis.__grafixPlayback = {seconds, playing, rate, duration};
}
export function telemetry() { return '{}'; }
export function param_schema() { return '[]'; }
