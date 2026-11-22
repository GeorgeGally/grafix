<script lang="ts">
  import {
    SvelteFlow, Controls, MiniMap, Background, BackgroundVariant, useSvelteFlow,
    type Node as FlowNode, type Edge as FlowEdge, type Connection,
  } from '@xyflow/svelte';
  import '@xyflow/svelte/dist/style.css';
  import GraphNode from './GraphNode.svelte';
  import { engine, type Telemetry } from './engine';
  import {
    PROJECT_STORAGE_KEY,
    type GraphDocument, type GraphNode as GraphRecord,
    type GraphCardData, type GraphEdge, type NodeSpec, type SpatialSelection, type GraphParamValue,
    type NodeExecutionStatus,
  } from './graph-types';

  let { ready = false, telemetry = null, onSpatialSelection, onGraphChange }: {
    ready?: boolean;
    telemetry?: Telemetry | null;
    onSpatialSelection?: (value: SpatialSelection | null) => void;
    onGraphChange?: (value: GraphDocument) => void;
  } = $props();
  const { screenToFlowPosition, setCenter, getZoom } = useSvelteFlow();
  let flowElement: HTMLDivElement | undefined;
  const nodeTypes = { grafix: GraphNode };

  let initialized = $state(false);
  let doc = $state<GraphDocument | null>(null);
  let catalog = $state<NodeSpec[]>([]);
  let nodes = $state.raw<FlowNode<GraphCardData>[]>([]);
  let edges = $state.raw<FlowEdge[]>([]);
  let execution = $state<NodeExecutionStatus[]>([]);
  let selectedId = $state<string | null>(null);
  let search = $state('');
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let history = $state<GraphDocument[]>([]);
  let future = $state<GraphDocument[]>([]);
  let gesture: GraphDocument | null = null;
  let nextPlacement = 0;
  let commandOpen = $state(false);
  let commandSearch = $state('');
  let commandInput = $state<HTMLInputElement | undefined>(undefined);
  let propertySearch = $state('');
  let clipboard: GraphRecord | null = null;
  let inspectorWidth = $state(290);
  let paletteWidth = $state(165);
  let bodyElement: HTMLDivElement;
  let paneDrag: 'palette' | 'inspector' | null = null;

  const selected = $derived(doc?.nodes.find((n) => n.id === selectedId) ?? null);
  const selectedSpec = $derived(catalog.find((s) => s.type_id === selected?.type) ?? null);
  const hasTransform = $derived(!!selectedSpec?.params.some((p) => p.name === 'position_x'));
  // The engine owns execution semantics, including known approximations;
  // cable reachability alone is not a reliable "live" status.
  const selectedExecution = $derived(execution.find((n) => n.id === selectedId) ?? null);
  const selectedActive = $derived(
    selectedExecution?.state === 'active' || selectedExecution?.state === 'partial'
  );
  const parameterGroups = $derived.by(() => {
    if (!selectedSpec) return [];
    const grouped = new Map<string, NodeSpec['params']>();
    for (const param of selectedSpec.params) {
      if (/^(position|rotation|scale)_[xyz]$/.test(param.name)) continue;
      if (propertySearch && !(param.name + ' ' + param.group)
        .toLowerCase().includes(propertySearch.trim().toLowerCase())) continue;
      const group = param.group ?? 'General';
      if (!grouped.has(group)) grouped.set(group, []);
      grouped.get(group)!.push(param);
    }
    return [...grouped].map(([name, params]) => ({ name, params }));
  });
  // Reserved math.add is validated for backward-compatible saved graphs but
  // has no general evaluator yet. Never offer inert nodes in the palette.
  const results = $derived(catalog.filter((s) =>
    s.type_id !== 'math.add' &&
    (s.title + ' ' + s.category + ' ' + s.type_id).toLowerCase().includes(search.toLowerCase())));

  const commandResults = $derived(catalog.filter((spec) =>
    spec.type_id !== 'math.add' &&
    (spec.title + ' ' + spec.category + ' ' + spec.type_id)
      .toLowerCase().includes(commandSearch.trim().toLowerCase())).slice(0, 12));

  // Svelte 5 $state creates proxies. structuredClone(proxy) throws a
  // DataCloneError and silently breaks add/connect/parameter edits.
  // $state.snapshot produces a deep, plain-data copy of our JSON document.
  function clone<T>(value: T): T { return $state.snapshot(value) as unknown as T; }

  // Mirror the selected spatial node into the viewport. The graph remains the
  // single source of truth; this is just a read-only interaction surface.
  $effect(() => {
    if (!hasTransform || !selected || !selectedSpec) {
      onSpatialSelection?.(null);
      return;
    }
    const position = Object.fromEntries(['x', 'y', 'z'].map((axis) => {
      const name = 'position_' + axis;
      const param = selectedSpec.params.find((p) => p.name === name);
      return [axis, Number(selected.params[name] ?? param?.default ?? 0)];
    })) as { x: number; y: number; z: number };
    onSpatialSelection?.({
      id: selected.id,
      title: selectedSpec.title,
      active: selectedActive,
      position,
    });
  });

  $effect(() => {
    function onViewportMove(event: Event) {
      const detail = (event as CustomEvent<{
        phase: 'begin' | 'update' | 'end';
        axis: 'x' | 'y' | 'z';
        delta: number;
      }>).detail;
      if (!detail || !selected || !selectedSpec || !hasTransform || !selectedActive) return;
      if (detail.phase === 'begin') {
        beginParamEdit();
      } else if (detail.phase === 'end') {
        finishParamEdit();
      } else if (detail.phase === 'update' && Number.isFinite(detail.delta)) {
        const name = 'position_' + detail.axis;
        const def = selectedSpec.params.find((p) => p.name === name);
        if (!def) return;
        const current = Number(selected.params[name] ?? def.default);
        const next = Math.max(def.min ?? -100, Math.min(def.max ?? 100, current + detail.delta));
        paramValue(name, Math.round(next * 1000) / 1000);
      }
    }
    window.addEventListener('grafix:viewport-transform', onViewportMove);
    return () => window.removeEventListener('grafix:viewport-transform', onViewportMove);
  });
  function getSpec(type: string): NodeSpec | undefined {
    return catalog.find((s) => s.type_id === type);
  }
  function edgeId(edge: GraphEdge): string {
    return `${edge.from.node}:${edge.from.port}->${edge.to.node}:${edge.to.port}`;
  }
  function syncFlow(document: GraphDocument) {
    nodes = document.nodes.map((n) => ({
      id: n.id, type: 'grafix', position: { ...n.ui },
      data: {
        spec: getSpec(n.type)!,
        label: n.label,
        execution: execution.find((status) => status.id === n.id),
      },
    }));
    edges = document.edges.map((e) => ({
      id: edgeId(e), source: e.from.node, sourceHandle: e.from.port,
      target: e.to.node, targetHandle: e.to.port, type: 'smoothstep',
    }));
  }
  function persist() {
    if (doc) localStorage.setItem(PROJECT_STORAGE_KEY, JSON.stringify(doc));
  }
  function open(document: GraphDocument, remember = true): boolean {
    try {
      engine.loadGraph(JSON.stringify(document));
      execution = engine.graphStatus();
      if (remember && doc) { history = [...history.slice(-49), clone(doc)]; future = []; }
      doc = clone(document);
      onGraphChange?.(clone(document));
      syncFlow(document);
      if (selectedId && !document.nodes.some((n) => n.id === selectedId)) selectedId = null;
      error = null;
      persist();
      return true;
    } catch (cause) {
      error = String(cause);
      return false;
    }
  }

  $effect(() => {
    if (!ready || initialized) return;
    initialized = true;
    catalog = engine.graphSchema();
    try {
      const saved = localStorage.getItem(PROJECT_STORAGE_KEY);
      if (!saved || !open(JSON.parse(saved) as GraphDocument, false)) {
        open(engine.defaultGraph(), false);
      }
    } catch { open(engine.defaultGraph(), false); }
  });

  function add(spec: NodeSpec, client?: { x: number; y: number }) {
    if (!doc || !flowElement) return;
    const rect = flowElement.getBoundingClientRect();
    const point = client ?? { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
    const position = screenToFlowPosition(point);
    // Center the node under the pointer, rather than adding it offscreen
    // at an arbitrary graph coordinate.
    position.x -= 92;
    position.y -= 42;
    nextPlacement++;
    const node: GraphRecord = {
      id: spec.type_id.replaceAll('.', '-') + '-' + crypto.randomUUID().slice(0, 8),
      type: spec.type_id, version: 1,
      params: Object.fromEntries(spec.params.map((p) => [p.name, p.default])),
      ui: { x: position.x + (client ? 0 : (nextPlacement % 3) * 14), y: position.y },
    };
    const next = clone(doc);
    next.nodes.push(node);
    if (open(next)) {
      selectedId = node.id;
      search = '';
      notice = 'Added ' + spec.title + ' — connect it to Output to make it live.';
      // Keep click-created nodes in view; native drop retains its location.
      if (!client) requestAnimationFrame(() => {
        void setCenter(node.ui.x + 92, node.ui.y + 42, { zoom: Math.max(0.6, getZoom()), duration: 200 });
      });
    }
  }

  function renameNode(label: string) {
    if (!doc || !selected) return;
    const trimmed = label.trim();
    if (!trimmed || trimmed.length > 80) {
      error = 'Node name must be 1–80 characters.';
      return;
    }
    const next = clone(doc);
    const item = next.nodes.find((n) => n.id === selected.id);
    if (!item) return;
    const def = selectedSpec?.title ?? '';
    if (trimmed === (item.label || def)) return;
    item.label = trimmed === def ? undefined : trimmed;
    open(next);
  }

  function duplicate() {
    if (!doc || !selected) return;
    const next = clone(doc);
    const copy = clone(selected);
    copy.id = selected.type.replaceAll('.', '-') + '-' + crypto.randomUUID().slice(0, 8);
    copy.label = copy.label ? copy.label + ' copy' : undefined;
    copy.ui = { x: copy.ui.x + 38, y: copy.ui.y + 56 };
    next.nodes.push(copy);
    if (open(next)) {
      selectedId = copy.id;
      notice = 'Duplicated node. Connect its sockets to use it.';
    }
  }
  function copySelected() {
    if (selected) {
      clipboard = clone(selected);
      notice = 'Node copied. Press ⌘V / Ctrl+V to paste.';
    }
  }
  function pasteSelected() {
    if (!doc || !clipboard) return;
    const next = clone(doc);
    const copy = clone(clipboard);
    copy.id = copy.type.replaceAll('.', '-') + '-' + crypto.randomUUID().slice(0, 8);
    copy.ui = { x: copy.ui.x + 76, y: copy.ui.y + 72 };
    next.nodes.push(copy);
    if (open(next)) {
      selectedId = copy.id;
      notice = 'Pasted a disconnected copy of the node.';
    }
  }
  function resetParam(name: string) {
    if (!selectedSpec) return;
    const p = selectedSpec.params.find((candidate) => candidate.name === name);
    if (p) paramValue(name, clone(p.default));
  }
  function arrangeGraph() {
    if (!doc) return;
    const next = clone(doc);
    const depth = new Map(next.nodes.map((n) => [n.id, 0]));
    for (let step = 0; step < next.nodes.length; step++) {
      let changed = false;
      for (const e of next.edges) {
        const d = Math.min(next.nodes.length - 1, (depth.get(e.from.node) ?? 0) + 1);
        if (d > (depth.get(e.to.node) ?? 0)) {
          depth.set(e.to.node, d);
          changed = true;
        }
      }
      if (!changed) break;
    }
    const rows = new Map<number, number>();
    for (const node of next.nodes) {
      const column = depth.get(node.id) ?? 0;
      const row = rows.get(column) ?? 0;
      node.ui = { x: column * 270 + 60, y: row * 210 + 60 };
      rows.set(column, row + 1);
    }
    if (open(next)) notice = 'Arranged nodes by graph dependency.';
  }
  function showCommand() {
    if (!doc) return;
    commandSearch = '';
    commandOpen = true;
    requestAnimationFrame(() => commandInput?.focus());
  }
  function addFromCommand(spec: NodeSpec) {
    commandOpen = false;
    add(spec);
  }
  function dragPane(event: PointerEvent, which: 'palette' | 'inspector') {
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    paneDrag = which;
    event.preventDefault();
  }
  function movePane(event: PointerEvent) {
    if (!paneDrag || !bodyElement) return;
    const rect = bodyElement.getBoundingClientRect();
    if (paneDrag === 'palette') {
      paletteWidth = Math.max(110, Math.min(330, event.clientX - rect.left));
    } else {
      inspectorWidth = Math.max(210, Math.min(510, rect.right - event.clientX));
    }
  }
  function endPane() { paneDrag = null; }

  function dragStart(event: DragEvent, spec: NodeSpec) {
    if (!event.dataTransfer) return;
    event.dataTransfer.setData('application/grafix-node', spec.type_id);
    event.dataTransfer.effectAllowed = 'copy';
  }
  function dragOver(event: DragEvent) {
    if (event.dataTransfer?.types.includes('application/grafix-node')) {
      event.preventDefault();
      event.dataTransfer.dropEffect = 'copy';
    }
  }
  function drop(event: DragEvent) {
    const typeId = event.dataTransfer?.getData('application/grafix-node');
    if (!typeId) return;
    event.preventDefault();
    const spec = catalog.find((entry) => entry.type_id === typeId);
    if (spec) add(spec, { x: event.clientX, y: event.clientY });
  }

  // One active cloner feeds scene.compose's clones input. A new cloner can
  // live as a scratch node, but must be connected before its parameters render.
  function activateCloner() {
    if (!doc || !selected || !selected.type.startsWith('clone.')) return;
    const scene = doc.nodes.find((n) => n.type === 'scene.compose');
    const cube = doc.nodes.find((n) => n.type.startsWith('geometry.'));
    if (!scene || !cube) {
      error = 'A Scene and Cube node are needed before this cloner can render.';
      return;
    }
    const next = clone(doc);
    next.edges = next.edges.filter((e) =>
      !(e.to.node === scene.id && e.to.port === 'clones'));
    if (!next.edges.some((e) => e.to.node === selected.id && e.to.port === 'source')) {
      next.edges.push({
        from: { node: cube.id, port: 'geometry' },
        to: { node: selected.id, port: 'source' },
      });
    }
    next.edges.push({
      from: { node: selected.id, port: 'instances' },
      to: { node: scene.id, port: 'clones' },
    });
    if (open(next)) notice = 'Fibonacci Cloner is now driving the scene.';
  }
  // A palette node starts as a reusable scratch node. Connecting it to the
  // appropriate live input should require one obvious action, not manual
  // navigation across a large patch.
  const quickConnections: Record<string, {
    target: string;
    input: string;
    output: string;
    label: string;
  }> = {
    'asset.boombox': { target: 'scene.compose', input: 'model', output: 'object',
      label: 'Use this model in scene' },
    'geometry.cube': { target: 'scene.compose', input: 'geometry', output: 'geometry',
      label: 'Show cube as a scene object' },
    'geometry.sphere': { target: 'scene.compose', input: 'geometry', output: 'geometry',
      label: 'Show sphere as a scene object' },
    'geometry.cylinder': { target: 'scene.compose', input: 'geometry', output: 'geometry',
      label: 'Show cylinder as a scene object' },
    'geometry.plane': { target: 'scene.compose', input: 'geometry', output: 'geometry',
      label: 'Show plane as a scene object' },
    'geometry.torus': { target: 'scene.compose', input: 'geometry', output: 'geometry',
      label: 'Show torus as a scene object' },
    'lighting.directional': { target: 'scene.compose', input: 'light', output: 'light',
      label: 'Use this light in scene' },
    'field.compute2d': { target: 'scene.compose', input: 'field', output: 'texture',
      label: 'Show this field in scene' },
    'camera.orbit': { target: 'render.output', input: 'camera', output: 'camera',
      label: 'Use this camera' },
    'fx.bloom': { target: 'render.output', input: 'bloom', output: 'effect',
      label: 'Use this bloom effect' },
    'fx.aberration': { target: 'render.output', input: 'aberration', output: 'effect',
      label: 'Use this aberration effect' },
    'fx.vignette': { target: 'render.output', input: 'vignette', output: 'effect',
      label: 'Use this vignette effect' },
  };
  const quickLink = $derived(selected ? quickConnections[selected.type] : undefined);
  const quickLinkActive = $derived.by(() => {
    if (!doc || !selected || !quickLink) return false;
    const destinations = new Set(doc.nodes.filter((node) => node.type === quickLink.target)
      .map((node) => node.id));
    return doc.edges.some((edge) => edge.from.node === selected.id &&
      edge.from.port === quickLink.output && edge.to.port === quickLink.input &&
      destinations.has(edge.to.node));
  });

  function connectToOutput() {
    if (!doc || !selected || !quickLink) return;
    const destination = doc.nodes.find((node) => node.type === quickLink.target);
    if (!destination) {
      error = 'Add a ' + quickLink.target + ' node first.';
      return;
    }
    const next = clone(doc);
    const multi = destination.type === 'scene.compose' &&
      ['model', 'geometry', 'light'].includes(quickLink.input);
    if (!multi) {
      next.edges = next.edges.filter((edge) =>
        !(edge.to.node === destination.id && edge.to.port === quickLink.input));
    }
    next.edges.push({
      from: { node: selected.id, port: quickLink.output },
      to: { node: destination.id, port: quickLink.input },
    });
    if (open(next)) notice = selectedSpec?.title + ' is now connected to the output.';
  }

  let materialTarget = $state('');
  function attachMaterial() {
    if (!doc || !selected || selected.type !== 'material.pbr') return;
    const cube = doc.nodes.find((node) => node.id === materialTarget &&
      node.type.startsWith('geometry.'));
    if (!cube) return;
    const next = clone(doc);
    next.edges = next.edges.filter((edge) =>
      !(edge.to.node === cube.id && edge.to.port === 'material'));
    next.edges.push({
      from: { node: selected.id, port: 'material' },
      to: { node: cube.id, port: 'material' },
    });
    if (open(next)) notice = 'Material applied to ' + cube.id;
  }

  function resetTransform() {
    if (!doc || !selected || !selectedSpec) return;
    const next = clone(doc);
    const node = next.nodes.find((entry) => entry.id === selected.id);
    if (!node) return;
    for (const param of selectedSpec.params) {
      if (/^(position|rotation|scale)_[xyz]$/.test(param.name)) {
        node.params[param.name] = Number(param.default);
      }
    }
    open(next);
  }

  function remove() {
    if (!doc || !selectedId) return;
    const next = clone(doc);
    next.nodes = next.nodes.filter((n) => n.id !== selectedId);
    next.edges = next.edges.filter((e) => e.from.node !== selectedId && e.to.node !== selectedId);
    open(next);
  }
  function onDeleted(deleted: { nodes: FlowNode<GraphCardData>[]; edges: FlowEdge[] }) {
    if (!doc) return;
    const removedNodes = new Set(deleted.nodes.map((n) => n.id));
    const removedEdges = new Set(deleted.edges.map((e) => e.id));
    const next = clone(doc);
    next.nodes = next.nodes.filter((n) => !removedNodes.has(n.id));
    next.edges = next.edges.filter((e) => !removedNodes.has(e.from.node) &&
      !removedNodes.has(e.to.node) && !removedEdges.has(edgeId(e)));
    open(next);
  }
  function connectionProblem(c: Connection | FlowEdge): string | null {
    if (!doc || !c.sourceHandle || !c.targetHandle) return 'Both sockets must be present';
    const from = doc.nodes.find((n) => n.id === c.source);
    const to = doc.nodes.find((n) => n.id === c.target);
    if (!from || !to) return 'Node missing';
    const out = getSpec(from.type)?.outputs.find((p) => p.name === c.sourceHandle);
    const input = getSpec(to.type)?.inputs.find((p) => p.name === c.targetHandle);
    if (!out || !input) return 'Unknown socket';
    if (out.data_type !== input.data_type) {
      return 'Incompatible sockets: ' + out.data_type + ' → ' + input.data_type;
    }
    if (c.source === c.target) return 'An object cannot connect to itself';
    if (doc.edges.some((e) => e.from.node === c.source &&
      e.from.port === c.sourceHandle && e.to.node === c.target &&
      e.to.port === c.targetHandle)) return 'Already connected';
    // Reject cycles at the pointer, before attempting the Rust transaction.
    const visited = new Set<string>();
    const stack = [c.target];
    while (stack.length) {
      const id = stack.pop()!;
      if (id === c.source) return 'Connection creates a cycle';
      if (visited.has(id)) continue;
      visited.add(id);
      for (const edge of doc.edges) if (edge.from.node === id) stack.push(edge.to.node);
    }
    return null;
  }
  function validConnection(c: Connection | FlowEdge): boolean {
    return connectionProblem(c) === null;
  }
  function connect(c: Connection) {
    if (!doc || !c.sourceHandle || !c.targetHandle) return;
    const problem = connectionProblem(c);
    if (problem) { error = problem; return; }
    const next = clone(doc);
    const multi = doc.nodes.some((n) => n.id === c.target && n.type === 'scene.compose') &&
      ['model', 'geometry', 'light'].includes(c.targetHandle);
    if (!multi) {
      next.edges = next.edges.filter((e) => !(e.to.node === c.target && e.to.port === c.targetHandle));
    }
    if (next.edges.some((e) => e.from.node === c.source && e.from.port === c.sourceHandle &&
      e.to.node === c.target && e.to.port === c.targetHandle)) return;
    next.edges.push({
      from: { node: c.source, port: c.sourceHandle },
      to: { node: c.target, port: c.targetHandle },
    });
    open(next); // Rust rejects invalid cycles without replacing the working document.
  }
  function syncPositions() {
    if (!doc) return;
    const next = clone(doc);
    let moved = false;
    for (const record of next.nodes) {
      const node = nodes.find((n) => n.id === record.id);
      if (node && (record.ui.x !== node.position.x || record.ui.y !== node.position.y)) {
        record.ui = { ...node.position }; moved = true;
      }
    }
    if (moved) {
      history = [...history.slice(-49), clone(doc)];
      future = [];
      doc = next; persist();
      onGraphChange?.(clone(next));
    }
  }
  function paramValue(name: string, value: GraphParamValue) {
    if (!doc || !selected) return;
    const next = clone(doc);
    const target = next.nodes.find((n) => n.id === selected.id);
    if (!target) return;
    target.params[name] = value;
    try {
      if (typeof value === 'number') {
        engine.setNodeParam(target.id, name, value);
        doc = next; persist(); error = null;
        onGraphChange?.(clone(next));
      } else {
        // A colour drag produces many native input events. Commit the latest
        // validated graph without adding one undo item per intermediate swatch.
        open(next, gesture === null);
      }
    } catch (cause) { error = String(cause); }
  }
  function colorHex(value: GraphParamValue): string {
    const channels = Array.isArray(value) && value.length === 3 ? value : [0, 0, 0];
    return '#' + channels.map((c) =>
      Math.round(Math.max(0, Math.min(1, Number(c))) * 255)
        .toString(16).padStart(2, '0')).join('');
  }
  function colorValue(hex: string): number[] {
    const h = hex.startsWith('#') ? hex.slice(1) : hex;
    return [0, 2, 4].map((start) => parseInt(h.slice(start, start + 2), 16) / 255);
  }
  function beginParamEdit() { if (!gesture && doc) gesture = clone(doc); }
  function finishParamEdit() {
    if (gesture && doc && JSON.stringify(gesture) !== JSON.stringify(doc)) {
      history = [...history.slice(-49), gesture]; future = [];
    }
    gesture = null;
  }
  function undo() {
    if (!doc || !history.length) return;
    const previous = history.at(-1)!;
    const current = clone(doc);
    if (open(previous, false)) {
      history = history.slice(0, -1); future = [...future, current];
    }
  }
  function redo() {
    if (!doc || !future.length) return;
    const following = future.at(-1)!;
    const current = clone(doc);
    if (open(following, false)) {
      future = future.slice(0, -1); history = [...history, current];
    }
  }
  function download() {
    if (!doc) return;
    syncPositions();
    const blob = new Blob([JSON.stringify(doc, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url; a.download = 'composition.grafix.json'; a.click();
    URL.revokeObjectURL(url);
  }
  async function upload(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    try { open(JSON.parse(await file.text()) as GraphDocument); }
    catch (cause) { error = String(cause); }
    input.value = '';
  }
  async function loadTwoObjects() {
    try {
      // Dev pages can live under /tests/ while the Vite public directory
      // stays rooted at /. The production build uses Vite's relative base
      // so deployments under a subpath still resolve bundled assets.
      const assetBase = import.meta.env.DEV ? '/' : import.meta.env.BASE_URL;
      const url = new URL(assetBase + 'assets/presets/two-material-cubes.grafix.json', document.baseURI);
      const response = await fetch(url);
      if (!response.ok) throw new Error('Example preset unavailable; rebuild the assets.');
      if (open(await response.json() as GraphDocument)) {
        selectedId = null;
        notice = 'Two independent cubes, materials and lights loaded.';
      }
    } catch (cause) {
      error = String(cause);
    }
  }
  function reset() {
    if (window.confirm('Replace the composition with the default graph?')) open(engine.defaultGraph());
  }
  function keydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && commandOpen) {
      event.preventDefault(); commandOpen = false; return;
    }
    const typing = event.target instanceof HTMLInputElement ||
      event.target instanceof HTMLTextAreaElement ||
      event.target instanceof HTMLSelectElement ||
      (event.target instanceof HTMLElement && event.target.isContentEditable);
    const modifier = event.metaKey || event.ctrlKey;
    if (!typing && ((modifier && event.key.toLowerCase() === 'k') ||
      (event.key === 'Tab' && !event.shiftKey && !event.altKey && !modifier))) {
      event.preventDefault(); showCommand(); return;
    }
    if (typing || commandOpen) return;
    if (modifier && event.key.toLowerCase() === 'z') {
      event.preventDefault();
      if (event.shiftKey) redo(); else undo();
    } else if (modifier && event.key.toLowerCase() === 'd') {
      event.preventDefault(); duplicate();
    } else if (modifier && event.key.toLowerCase() === 'c' && selected) {
      event.preventDefault(); copySelected();
    } else if (modifier && event.key.toLowerCase() === 'v' && clipboard) {
      event.preventDefault(); pasteSelected();
    }
  }
</script>

<svelte:window onkeydown={keydown}/>

<section class="graph-workspace" aria-label="Procedural node editor">
  <header class="bar">
    <strong>GRAFIX <span>/ patch</span></strong>
    <input class="name" aria-label="Composition name" value={doc?.name ?? 'Loading…'} disabled={!doc}
      onchange={(event) => {
        if (!doc) return;
        const next = clone(doc); next.name = event.currentTarget.value; open(next);
      }}/>
    <button onclick={undo} disabled={!history.length} title="Undo">↶</button>
    <button onclick={redo} disabled={!future.length} title="Redo">↷</button>
    <button onclick={showCommand} disabled={!doc} title="Quick add (Tab)">+ Add</button>
    <button onclick={arrangeGraph} disabled={!doc} title="Arrange graph nodes">Arrange</button>
    <button onclick={download} disabled={!doc}>Save</button>
    <label class="import">Open<input type="file" accept=".json,application/json" onchange={upload}/></label>
    <button onclick={reset} disabled={!doc}>Reset</button>
    <button onclick={loadTwoObjects} disabled={!doc} title="Load editable two-material example">Two objects</button>
    {#if notice}<span class="status" role="status">{notice}</span>{/if}
    {#if error}<span class="bar-error" role="alert" title={error}>{error}</span>{/if}
  </header>
  <div class="body" bind:this={bodyElement}
    style:--palette-width={paletteWidth + 'px'}
    style:--inspector-width={inspectorWidth + 'px'}>
    <aside class="palette">
      <label for="grafix-node-search">Nodes</label>
      <input id="grafix-node-search" placeholder="Search…" bind:value={search} disabled={!doc}/>
      <div class="palette-list">
        {#each results as spec (spec.type_id)}
          <button title={spec.description + ' (click or drag to canvas)'}
            aria-label={'Add ' + spec.title}
            draggable={!!doc}
            ondragstart={(event) => dragStart(event, spec)}
            onclick={() => add(spec)}
            disabled={!doc}>
            <small>{spec.category}</small><span>{spec.title}</span>
          </button>
        {/each}
      </div>
    </aside>
    <button type="button" class="pane-divider" aria-label="Resize palette"
      onpointerdown={(event) => dragPane(event, 'palette')} onpointermove={movePane}
      onpointerup={endPane} onpointercancel={endPane}></button>
    <div class="flow" role="application" aria-label="Node graph canvas" bind:this={flowElement} ondragover={dragOver} ondrop={drop}>
      {#if doc}
        <SvelteFlow bind:nodes bind:edges {nodeTypes} fitView minZoom={0.2}
          isValidConnection={validConnection}
          onconnect={connect}
          ondelete={onDeleted}
          onnodeclick={({ node }) => (selectedId = node.id)}
          onpaneclick={() => (selectedId = null)}
          onnodedragstop={syncPositions}>
          <Background variant={BackgroundVariant.Dots} gap={22} size={1}/>
          <Controls/>
          <MiniMap pannable zoomable nodeStrokeWidth={2}/>
        </SvelteFlow>
      {:else}
        <p class="loading">Waiting for WebGPU engine…</p>
      {/if}
    </div>
    <button type="button" class="pane-divider" aria-label="Resize inspector"
      onpointerdown={(event) => dragPane(event, 'inspector')} onpointermove={movePane}
      onpointerup={endPane} onpointercancel={endPane}></button>
    <aside class="inspector">
      {#if selected && selectedSpec}
        <small class="eyebrow">{selectedSpec.category} / {selected.type}</small>
        <h3>{selected.label ?? selectedSpec.title}</h3>
        <label class="rename-label" for="grafix-node-title">Node name</label>
        <input id="grafix-node-title" class="node-title" aria-label="Node name"
          value={selected.label ?? selectedSpec.title}
          onchange={(event) => renameNode(event.currentTarget.value)}/>
        <button class="duplicate-node" onclick={duplicate} title="Duplicate selected node">Duplicate ⌘D</button>
        <p class="description">{selectedSpec.description}</p>
        {#if selectedActive}
          <p class="active-note">● Connected to render output</p>
          {#if selectedExecution?.state === 'partial'}
            <p class="partial-note" role="status">{selectedExecution.message}. Native/built-in cloning uses full Bevy PBR.</p>
          {/if}
          {#if selected.type.startsWith('clone.') && telemetry?.instancing}
            <p class="runtime-value" data-testid="live-clone-count">
              GPU / {telemetry.instancing.count.toLocaleString()} instances · {telemetry.instancing.mode}
            </p>
          {/if}
        {:else}
          <div class="inactive-note">
            {#if selectedExecution?.state === 'inactive'}
              Not connected to Output. Changes here won't affect the viewport.
            {:else}
              {selectedExecution?.message ?? 'Not connected to Output'}. This node is not fully evaluated.
            {/if}
            {#if selected.type.startsWith('clone.')}
              <button onclick={activateCloner}>Use this cloner in the scene</button>
              <small>Replaces the current cloner connection.</small>
            {/if}
          </div>
        {/if}
        {#if quickLink && !quickLinkActive}
          <button class="make-visible" onclick={connectToOutput}>{quickLink.label}</button>
        {/if}
        {#if selected.type === 'material.pbr' && doc}
          <div class="material-target">
            <label for="material-target">Assign material to</label>
            <select id="material-target" bind:value={materialTarget}>
              <option value="">Choose cube…</option>
              {#each doc.nodes.filter((n) => n.type.startsWith('geometry.')) as cube (cube.id)}
                <option value={cube.id}>{cube.id}</option>
              {/each}
            </select>
            <button onclick={attachMaterial} disabled={!materialTarget}>Apply material</button>
          </div>
        {/if}
        {#if hasTransform}
          <div class="transform-controls">
            <div class="transform-heading">
              <h4>Transform</h4>
              <button class="reset-transform" onclick={resetTransform}>Reset</button>
            </div>
            {#each ['position', 'rotation', 'scale'] as group}
              <div class="transform-group">
                <div class="transform-label">{group}{group === 'rotation' ? ' (°)' : ''}</div>
                <div class="transform-axes">
                  {#each ['x', 'y', 'z'] as axis}
                    {@const key = group + '_' + axis}
                    {@const param = selectedSpec.params.find((entry) => entry.name === key)}
                    {#if param}
                      <label class="axis-control" for={selected.id + '-' + key}>
                        <span class:axis-x={axis === 'x'} class:axis-y={axis === 'y'} class:axis-z={axis === 'z'}>{axis.toUpperCase()}</span>
                        <input
                          id={selected.id + '-' + key}
                          type="number"
                          min={param.min}
                          max={param.max}
                          step={group === 'scale' ? 0.01 : 0.1}
                          value={Number(selected.params[key] ?? param.default)}
                          onfocus={beginParamEdit}
                          oninput={(event) => {
                            const raw = event.currentTarget.value;
                            if (raw === '' || raw === '-') return;
                            const value = Number(raw);
                            if (Number.isFinite(value) && value >= (param.min ?? -Infinity) &&
                              value <= (param.max ?? Infinity)) paramValue(key, value);
                          }}
                          onblur={finishParamEdit}
                        />
                      </label>
                    {/if}
                  {/each}
                </div>
              </div>
            {/each}
            <p class="transform-hint">Edit XYZ here, or drag the axis controls over the viewport.</p>
          </div>
        {/if}
        {#if selectedSpec.params.length}
          <input class="property-filter" aria-label="Filter properties" placeholder="Find a property…"
            bind:value={propertySearch}/>
        {/if}
        {#each parameterGroups as group (group.name)}
          <section class="parameter-group" aria-label={group.name + ' parameters'}>
            <h4>{group.name}</h4>
            {#each group.params as param (param.name)}
              <div class="parameter">
                <label for={selected.id + '-' + param.name}>
                  {param.name.replaceAll('_', ' ')}
                  {#if param.unit}<span class="parameter-unit">{param.unit}</span>{/if}
                </label>
                {#if param.kind === 'number'}
                  <label class="exact-label" for={selected.id + '-' + param.name + '-exact'}>
                    <input id={selected.id + '-' + param.name + '-exact'} type="number"
                      aria-label={'Exact ' + param.name}
                      min={param.min} max={param.max} step="any"
                      value={Number(selected.params[param.name] ?? param.default)}
                      onfocus={beginParamEdit}
                      oninput={(event) => {
                        const raw = event.currentTarget.value;
                        const value = Number(raw);
                        if (raw !== '' && Number.isFinite(value) &&
                          value >= (param.min ?? -Infinity) && value <= (param.max ?? Infinity)) {
                            paramValue(param.name, value);
                        }
                      }}
                      onblur={finishParamEdit}/>
                  </label>
                  <button class="reset-property" aria-label={'Reset ' + param.name}
                    title="Reset to default" onclick={() => resetParam(param.name)}>↺</button>
                  <input id={selected.id + '-' + param.name} type="range"
                    min={param.min} max={param.max}
                    step={param.name === 'count' ? 1 : param.name === 'intensity' ? 100 : 0.01}
                    value={Number(selected.params[param.name] ?? param.default)}
                    onpointerdown={beginParamEdit}
                    onfocus={beginParamEdit}
                    oninput={(event) => paramValue(param.name, Number(event.currentTarget.value))}
                    onchange={finishParamEdit}
                    onblur={finishParamEdit}/>
                {:else if param.kind === 'bool'}
                  <button class="reset-property" aria-label={'Reset ' + param.name}
                    onclick={() => resetParam(param.name)}>↺</button>
                  <input id={selected.id + '-' + param.name}
                    type="checkbox" class="property-check"
                    checked={Boolean(selected.params[param.name] ?? param.default)}
                    onchange={(event) => paramValue(param.name, event.currentTarget.checked)}/>
                {:else if param.kind === 'color'}
                  <button class="reset-property" aria-label={'Reset ' + param.name}
                    onclick={() => resetParam(param.name)}>↺</button>
                  <input id={selected.id + '-' + param.name} type="color" class="property-color"
                    value={colorHex(selected.params[param.name] ?? param.default)}
                    onfocus={beginParamEdit}
                    oninput={(event) => paramValue(param.name, colorValue(event.currentTarget.value))}
                    onchange={finishParamEdit}
                    onblur={finishParamEdit}/>
                {:else}
                  <button class="reset-property" aria-label={'Reset ' + param.name}
                    onclick={() => resetParam(param.name)}>↺</button>
                  <select id={selected.id + '-' + param.name}
                    value={String(selected.params[param.name] ?? param.default)}
                    onchange={(event) => paramValue(param.name, event.currentTarget.value)}>
                    {#each param.options ?? [] as option}
                      <option value={option}>{option}</option>
                    {/each}
                  </select>
                {/if}
              </div>
            {/each}
          </section>
        {/each}
        <div class="actions"><button class="danger" onclick={remove}>Delete node</button></div>
      {:else}
        <h3>Inspector</h3>
        <p class="description">Select a node to edit. Drag between typed sockets to connect operations.</p>
        <p class="description">The Rust runtime rejects cycles and incompatible connections without disturbing the live scene.</p>
      {/if}
      {#if error}
        <p class="graph-error" role="alert">{error}</p>
        <button onclick={() => (error = null)}>Dismiss</button>
      {/if}
    </aside>
  </div>
</section>
{#if commandOpen}
  <div class="command-backdrop">
    <dialog open class="command-dialog" aria-label="Quick add node">
      <input class="command-search" bind:this={commandInput} bind:value={commandSearch}
        aria-label="Quick add search" placeholder="Find a node…"
        onkeydown={(event) => {
          if (event.key === 'Enter' && commandResults.length) {
            event.preventDefault(); addFromCommand(commandResults[0]);
          }
        }}/>
      <div class="command-options">
        {#each commandResults as spec (spec.type_id)}
          <button onclick={() => addFromCommand(spec)}>
            <span>{spec.title}</span><small>{spec.category}</small>
          </button>
        {/each}
      </div>
      <small>Enter: add first match · Esc: dismiss</small>
    </dialog>
  </div>
{/if}
<style>
.graph-workspace { display:grid; grid-template-rows:42px minmax(0,1fr); height:100%; min-height:0; background:#101216; border-top:1px solid #303640; }
.bar { display:flex; align-items:center; gap:6px; padding:5px 9px; border-bottom:1px solid #303640; overflow:hidden; }
.bar strong { font:600 11px var(--mono); letter-spacing:.08em; white-space:nowrap; }
.bar strong span { color:#8b919c; font-weight:400; }
.bar .name { flex:1; min-width:70px; max-width:250px; background:#161a20; color:#d7dce4; border:1px solid #333b48; padding:5px 7px; border-radius:4px; }
.bar button,.import { padding:5px 8px; font:11px var(--mono); white-space:nowrap; }
.status { color:#9cc1a7; font:10px var(--mono); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.bar-error { color:#ff8e82; font:10px var(--mono); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; max-width:220px; }
.import { border:1px solid #303640; background:#1b2028; border-radius:5px; cursor:pointer; }
.import input { display:none; }
 .body { display:grid; grid-template-columns: var(--palette-width) 6px minmax(0,1fr) 6px var(--inspector-width); min-height:0; }
.pane-divider { background:#252c36; border-inline:1px solid #39404b; cursor:col-resize; touch-action:none; }
.pane-divider:hover { background:#4d657b; }
.rename-label { display:block; font:10px var(--mono); color:#90a8bb; margin:8px 0 4px; }
.node-title,.property-filter { width:100%; box-sizing:border-box; background:#181e28; border:1px solid #3b4555; border-radius:4px; padding:6px; color:#e2e8f2; font:11px var(--mono); }
.duplicate-node { margin-top:6px; font:10px var(--mono); }
.property-filter { margin-top:18px; }
.parameter .exact-label { grid-column:2; display:flex; align-items:center; }
.exact-label input { width:76px; padding:5px; border:1px solid #465160; background:#1b222b; color:#e0e9f2; border-radius:3px; font:11px var(--mono); }
.reset-property { grid-column:2; justify-self:end; padding:1px 7px; font:12px var(--mono); color:#9aaebf; }
.command-backdrop { position:fixed; inset:0; z-index:999; background:rgb(0 0 0/.62); display:grid; place-items:start center; padding-top:min(15vh,130px); }
.command-dialog { background:#1b222e; border:1px solid #53647b; border-radius:9px; width:min(430px,calc(100vw - 36px)); box-shadow:0 30px 90px #0008; padding:13px; display:grid; gap:10px; }
.command-search { background:#111720; border:1px solid #475c75; color:#f4f6fa; padding:10px; border-radius:5px; width:100%; box-sizing:border-box; }
.command-options { display:grid; gap:2px; max-height:390px; overflow:auto; }
.command-options button { display:flex; justify-content:space-between; align-items:center; padding:9px; text-align:left; }
.command-options small,.command-dialog>small { color:#9aacc0; font:10px var(--mono); }
.flow :global(.svelte-flow__minimap) { background:#151d27; border:1px solid #364657; border-radius:5px; }
.palette,.inspector { min-height:0; overflow-y:auto; padding:12px; }
.palette { border-right:1px solid #303640; }
.palette label { display:block; font-size:11px; font-weight:600; margin-bottom:7px; }
.palette>input { display:block; width:100%; margin-bottom:10px; padding:7px; background:#171b22; border:1px solid #343b46; color:inherit; border-radius:4px; }
.palette-list { display:grid; gap:2px; }
.palette-list button { text-align:left; display:grid; gap:2px; background:transparent; border:none; padding:6px; cursor:grab; }
.palette-list button:hover { background:#222733; }
.palette-list small { font:9px var(--mono); color:#8390a0; }
.palette-list span { font-size:11px; }
.flow { position:relative; min-width:0; background:#11151b; }
.flow :global(.svelte-flow) { width:100%; height:100%; }
.flow :global(.svelte-flow__controls) { background:#1d222b; border:1px solid #3b4553; }
.flow :global(.svelte-flow__controls-button) { background:#222835; color:#c8d0dd; border-color:#3b4553; }
.flow :global(.svelte-flow__edge-path) { stroke:#778a9e; stroke-width:1.4; }
.flow :global(.svelte-flow__edge.selected .svelte-flow__edge-path) { stroke:#82c8f4; }
.loading { padding:14px; color:var(--muted); }
.inspector { border-left:1px solid #303640; }
.inspector h3 { margin:5px 0 10px; font-size:13px; }
.eyebrow { font:10px var(--mono); color:#8d9cab; }
.description { color:#9ca7b7; font-size:11px; line-height:1.55; }
.active-note { color:#9fdbb4; font:11px var(--mono); }
.runtime-value { color:#c2d9e6; font:11px var(--mono); }
.partial-note { color:#e4c18f; background:#30271e; padding:8px; border-radius:4px; font:11px/1.45 var(--mono); }
.inactive-note { border:1px solid #63543d; background:#2b261f; border-radius:5px; padding:9px; font-size:11px; color:#efcca0; line-height:1.45; }
.inactive-note button { display:block; font:11px var(--mono); margin:9px 0 4px; width:100%; white-space:normal; }
.inactive-note small { display:block; color:#baa88c; }
.transform-controls { border-top:1px solid #323c49; padding-top:12px; margin-top:16px; }
.transform-heading { display:flex; justify-content:space-between; align-items:center; }
.transform-heading h4 { margin:0; font:600 11px var(--mono); text-transform:uppercase; letter-spacing:.08em; }
.reset-transform { padding:3px 6px; font:10px var(--mono); }
.transform-group { margin-top:12px; }
.transform-label { color:#aab8cb; font:10px var(--mono); text-transform:capitalize; margin-bottom:5px; }
.transform-axes { display:grid; grid-template-columns:repeat(3,minmax(0,1fr)); gap:4px; }
.axis-control { min-width:0; display:flex; flex-direction:column; align-items:flex-start; gap:4px; font:10px var(--mono); }
.axis-control span { font-weight:700; }
.axis-x { color:#e3a2a2; } .axis-y { color:#a9d5a9; } .axis-z { color:#a9bde8; }
.axis-control input { width:100%; min-width:0; box-sizing:border-box; background:#1a2028; border:1px solid #3a4351; border-radius:3px; color:#e0e6ee; padding:5px 3px; font:11px var(--mono); }
.transform-hint { color:#8093a8; font:10px/1.45 var(--mono); }
.make-visible { margin:12px 0 0; width:100%; font:11px var(--mono); white-space:normal; padding:8px; }
.parameter-group { margin-top:16px; border-top:1px solid #333e4a; padding-top:10px; }
.parameter-group h4 { font:600 10px var(--mono); letter-spacing:.075em; color:#90a8ba; text-transform:uppercase; margin:0 0 6px; }
.parameter-unit { text-transform:none; opacity:.6; margin-left:4px; font:10px var(--mono); }
.property-check { justify-self:end; cursor:pointer; accent-color:#82c8f4; }
.property-color { grid-column:1/-1; width:100%; height:29px; background:#1c222c; border:1px solid #3a4450; padding:2px; cursor:pointer; }
.material-target { display:grid; gap:7px; margin-top:12px; padding:8px; border:1px solid #3a4654; border-radius:5px; }
.material-target label { font:11px var(--mono); }
.material-target select { width:100%; background:#1c222c; color:#dce5f2; }
.parameter { display:grid; grid-template-columns:minmax(0,1fr) auto auto; column-gap:6px; row-gap:6px; margin:16px 0 0; }
.parameter label { color:#cad2df; font-size:11px; text-transform:capitalize; }
.parameter input[type="range"],.parameter select { grid-column:1/-1; width:100%; accent-color:#82c8f4; }
.parameter select { background:#1c222c; color:#dce5f2; padding:6px; border:1px solid #3a4450; }
.actions { margin-top:20px; } .danger { color:#ff998d; }
.graph-error { padding:9px; background:#361f22; color:#ffaaa0; overflow-wrap:anywhere; font-size:11px; }
@media (max-width:1000px) { .body { grid-template-columns:110px 5px minmax(0,1fr) 5px 210px; } }
@media (max-width:700px) { .palette,.pane-divider { display:none; } .body { grid-template-columns:minmax(0,1fr) 200px; } .bar strong span,.bar .name { display:none; } }
</style>
