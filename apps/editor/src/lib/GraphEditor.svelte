<script lang="ts">
  import {
    SvelteFlow, Controls, Background, BackgroundVariant,
    type Node as FlowNode, type Edge as FlowEdge, type Connection,
  } from '@xyflow/svelte';
  import '@xyflow/svelte/dist/style.css';
  import GraphNode from './GraphNode.svelte';
  import { engine } from './engine';
  import {
    PROJECT_STORAGE_KEY,
    type GraphDocument, type GraphNode as GraphRecord,
    type GraphCardData, type GraphEdge, type NodeSpec,
  } from './graph-types';

  let { ready = false }: { ready?: boolean } = $props();
  const nodeTypes = { grafix: GraphNode };

  let initialized = $state(false);
  let doc = $state<GraphDocument | null>(null);
  let catalog = $state<NodeSpec[]>([]);
  let nodes = $state.raw<FlowNode<GraphCardData>[]>([]);
  let edges = $state.raw<FlowEdge[]>([]);
  let selectedId = $state<string | null>(null);
  let search = $state('');
  let error = $state<string | null>(null);
  let history = $state<GraphDocument[]>([]);
  let future = $state<GraphDocument[]>([]);
  let gesture: GraphDocument | null = null;
  let nextPlacement = 0;

  const selected = $derived(doc?.nodes.find((n) => n.id === selectedId) ?? null);
  const selectedSpec = $derived(catalog.find((s) => s.type_id === selected?.type) ?? null);
  const results = $derived(catalog.filter((s) =>
    (s.title + ' ' + s.category + ' ' + s.type_id).toLowerCase().includes(search.toLowerCase())));

  function clone<T>(value: T): T { return structuredClone(value); }
  function getSpec(type: string): NodeSpec | undefined {
    return catalog.find((s) => s.type_id === type);
  }
  function edgeId(edge: GraphEdge): string {
    return `${edge.from.node}:${edge.from.port}->${edge.to.node}:${edge.to.port}`;
  }
  function syncFlow(document: GraphDocument) {
    nodes = document.nodes.map((n) => ({
      id: n.id, type: 'grafix', position: { ...n.ui },
      data: { spec: getSpec(n.type)! },
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
      if (remember && doc) { history = [...history.slice(-49), clone(doc)]; future = []; }
      doc = clone(document);
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

  function add(spec: NodeSpec) {
    if (!doc) return;
    nextPlacement++;
    const node: GraphRecord = {
      id: spec.type_id.replaceAll('.', '-') + '-' + crypto.randomUUID().slice(0, 8),
      type: spec.type_id, version: 1,
      params: Object.fromEntries(spec.params.map((p) => [p.name, p.default])),
      ui: { x: 90 + (nextPlacement % 5) * 240, y: 90 + Math.floor(nextPlacement / 5) * 160 },
    };
    const next = clone(doc);
    next.nodes.push(node);
    if (open(next)) { selectedId = node.id; search = ''; }
  }
  function remove() {
    if (!doc || !selectedId) return;
    const next = clone(doc);
    next.nodes = next.nodes.filter((n) => n.id !== selectedId);
    next.edges = next.edges.filter((e) => e.from.node !== selectedId && e.to.node !== selectedId);
    open(next);
  }
  function validConnection(c: Connection): boolean {
    if (!doc || !c.sourceHandle || !c.targetHandle) return false;
    const from = doc.nodes.find((n) => n.id === c.source);
    const to = doc.nodes.find((n) => n.id === c.target);
    if (!from || !to) return false;
    const out = getSpec(from.type)?.outputs.find((p) => p.name === c.sourceHandle);
    const input = getSpec(to.type)?.inputs.find((p) => p.name === c.targetHandle);
    return !!out && !!input && out.data_type === input.data_type;
  }
  function connect(c: Connection) {
    if (!doc || !c.sourceHandle || !c.targetHandle) return;
    const next = clone(doc);
    next.edges = next.edges.filter((e) => !(e.to.node === c.target && e.to.port === c.targetHandle));
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
    }
  }
  function paramValue(name: string, value: number | string) {
    if (!doc || !selected) return;
    const next = clone(doc);
    const target = next.nodes.find((n) => n.id === selected.id);
    if (!target) return;
    target.params[name] = value;
    try {
      if (typeof value === 'number') {
        engine.setNodeParam(target.id, name, value);
        doc = next; persist(); error = null;
      } else {
        open(next);
      }
    } catch (cause) { error = String(cause); }
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
  function reset() {
    if (window.confirm('Replace the composition with the default graph?')) open(engine.defaultGraph());
  }
  function keydown(event: KeyboardEvent) {
    if (event.target instanceof HTMLInputElement || event.target instanceof HTMLSelectElement) return;
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'z') {
      event.preventDefault();
      if (event.shiftKey) redo(); else undo();
    } else if ((event.key === 'Delete' || event.key === 'Backspace') && selectedId) {
      event.preventDefault(); remove();
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
    <button onclick={download} disabled={!doc}>Save</button>
    <label class="import">Open<input type="file" accept=".json,application/json" onchange={upload}/></label>
    <button onclick={reset} disabled={!doc}>Reset</button>
  </header>
  <div class="body">
    <aside class="palette">
      <label for="grafix-node-search">Nodes</label>
      <input id="grafix-node-search" placeholder="Search…" bind:value={search} disabled={!doc}/>
      <div class="palette-list">
        {#each results as spec (spec.type_id)}
          <button title={spec.description} onclick={() => add(spec)} disabled={!doc}>
            <small>{spec.category}</small><span>{spec.title}</span>
          </button>
        {/each}
      </div>
    </aside>
    <div class="flow">
      {#if doc}
        <SvelteFlow bind:nodes bind:edges {nodeTypes} fitView minZoom={0.2}
          deleteKeyCode={null}
          isValidConnection={validConnection}
          onconnect={connect}
          onnodeclick={({ node }) => (selectedId = node.id)}
          onpaneclick={() => (selectedId = null)}
          onnodedragstop={syncPositions}>
          <Background variant={BackgroundVariant.Dots} gap={22} size={1} color="#2e3540"/>
          <Controls/>
        </SvelteFlow>
      {:else}
        <p class="loading">Waiting for WebGPU engine…</p>
      {/if}
    </div>
    <aside class="inspector">
      {#if selected && selectedSpec}
        <small class="eyebrow">{selectedSpec.category} / {selected.type}</small>
        <h3>{selectedSpec.title}</h3>
        <p class="description">{selectedSpec.description}</p>
        {#each selectedSpec.params as param (param.name)}
          <div class="parameter">
            <label for={`${selected.id}-${param.name}`}>{param.name.replaceAll('_', ' ')}</label>
            {#if param.kind === 'number'}
              <output>{Number(selected.params[param.name] ?? param.default).toFixed(2)}</output>
              <input id={`${selected.id}-${param.name}`} type="range"
                min={param.min} max={param.max}
                step={param.name === 'count' ? 1 : param.name === 'intensity' ? 100 : 0.01}
                value={Number(selected.params[param.name] ?? param.default)}
                onpointerdown={beginParamEdit}
                oninput={(event) => paramValue(param.name, Number(event.currentTarget.value))}
                onchange={finishParamEdit}/>
            {:else}
              <select id={`${selected.id}-${param.name}`}
                value={String(selected.params[param.name] ?? param.default)}
                onchange={(event) => paramValue(param.name, event.currentTarget.value)}>
                {#each param.options ?? [] as option}
                  <option value={option}>{option}</option>
                {/each}
              </select>
            {/if}
          </div>
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
<style>
.graph-workspace { display:grid; grid-template-rows:42px minmax(0,1fr); height:100%; min-height:0; background:#101216; border-top:1px solid #303640; }
.bar { display:flex; align-items:center; gap:6px; padding:5px 9px; border-bottom:1px solid #303640; overflow:hidden; }
.bar strong { font:600 11px var(--mono); letter-spacing:.08em; white-space:nowrap; }
.bar strong span { color:#8b919c; font-weight:400; }
.bar .name { flex:1; min-width:70px; max-width:250px; background:#161a20; color:#d7dce4; border:1px solid #333b48; padding:5px 7px; border-radius:4px; }
.bar button,.import { padding:5px 8px; font:11px var(--mono); white-space:nowrap; }
.import { border:1px solid #303640; background:#1b2028; border-radius:5px; cursor:pointer; }
.import input { display:none; }
.body { display:grid; grid-template-columns:150px minmax(0,1fr) 220px; min-height:0; }
.palette,.inspector { min-height:0; overflow-y:auto; padding:12px; }
.palette { border-right:1px solid #303640; }
.palette label { display:block; font-size:11px; font-weight:600; margin-bottom:7px; }
.palette>input { display:block; width:100%; margin-bottom:10px; padding:7px; background:#171b22; border:1px solid #343b46; color:inherit; border-radius:4px; }
.palette-list { display:grid; gap:2px; }
.palette-list button { text-align:left; display:grid; gap:2px; background:transparent; border:none; padding:6px; }
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
.parameter { display:grid; grid-template-columns:1fr auto; column-gap:8px; row-gap:6px; margin:19px 0 0; }
.parameter label { color:#cad2df; font-size:11px; text-transform:capitalize; }
.parameter output { font:11px var(--mono); color:#a3b0c4; }
.parameter input[type="range"],.parameter select { grid-column:1/-1; width:100%; accent-color:#82c8f4; }
.parameter select { background:#1c222c; color:#dce5f2; padding:6px; border:1px solid #3a4450; }
.actions { margin-top:20px; } .danger { color:#ff998d; }
.graph-error { padding:9px; background:#361f22; color:#ffaaa0; overflow-wrap:anywhere; font-size:11px; }
@media (max-width:1000px) { .body { grid-template-columns:120px minmax(0,1fr) 185px; } }
@media (max-width:700px) { .palette { display:none; } .body { grid-template-columns:minmax(0,1fr) 160px; } .bar strong span,.bar .name { display:none; } }
</style>
