// Test-only browser stub. Never ship this file in a release build.
const nodeTypes = [
  {
    type_id: 'geometry.cube', title: 'Cube', category: '3D', description: 'A cube',
    inputs: [], outputs: [{ name: 'geometry', data_type: 'geometry' }], params: [],
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
    type_id: 'scene.compose', title: 'Scene', category: 'Rendering', description: 'Scene compositor',
    inputs: [{ name: 'clones', data_type: 'instance_set' }],
    outputs: [{ name: 'scene', data_type: 'scene' }], params: [],
  },
  {
    type_id: 'camera.orbit', title: 'Orbit Camera', category: 'Rendering',
    description: 'Viewport camera', inputs: [],
    outputs: [{ name: 'camera', data_type: 'camera' }], params: [],
  },
  {
    type_id: 'render.output', title: 'Output', category: 'Rendering',
    description: 'Final render', inputs: [
      { name: 'scene', data_type: 'scene' }, { name: 'camera', data_type: 'camera' },
    ],
    outputs: [], params: [],
  },
];

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
export function telemetry() { return '{}'; }
export function param_schema() { return '[]'; }
