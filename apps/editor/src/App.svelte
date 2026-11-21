<script lang="ts">
  import { onMount } from 'svelte';
  import GraphEditor from './lib/GraphEditor.svelte';
  import type { SpatialSelection, GraphDocument } from './lib/graph-types';
  import { SvelteFlowProvider } from '@xyflow/svelte';
  import {
    counters,
    installGpuProbes,
    onGpuEvent,
    probeBrowser,
    simulateDeviceLoss,
    simulateValidationError,
    type BrowserCapabilities,
  } from './lib/gpu-probe';
  import { engine, startEngine, type Telemetry } from './lib/engine';
  import { runBench, type BenchResult } from './lib/bench';

  type Overlay = { title: string; message: string; action: 'reload' | 'none' } | null;

  let telemetry = $state<Telemetry | null>(null);
  let drawsPerFrame = $state(0);
  let browser = $state<BrowserCapabilities | null>(null);
  let overlay = $state<Overlay>(null);
  let gpuErrors = $state<string[]>([]);
  let benchProgress = $state<string | null>(null);
  let benchResults = $state<BenchResult[] | null>(null);
  let copied = $state(false);
  let engineFailed = $state(false);
  let selectedSpatial = $state<SpatialSelection | null>(null);
  let activeDocument = $state<GraphDocument | null>(null);
  let showDiagnostics = $state(false);
  let viewportDrag: {
    axis: 'x' | 'y' | 'z';
    pointerId: number;
    x: number;
    y: number;
  } | null = null;

  function notifyTransform(phase: 'begin' | 'update' | 'end',
    axis: 'x' | 'y' | 'z', delta = 0) {
    window.dispatchEvent(new CustomEvent('grafix:viewport-transform', {
      detail: { phase, axis, delta },
    }));
  }

  function beginViewportDrag(event: PointerEvent, axis: 'x' | 'y' | 'z') {
    if (!selectedSpatial?.active) return;
    (event.currentTarget as HTMLButtonElement).setPointerCapture(event.pointerId);
    viewportDrag = { axis, pointerId: event.pointerId, x: event.clientX, y: event.clientY };
    notifyTransform('begin', axis);
    event.preventDefault();
    event.stopPropagation();
  }

  function moveViewportDrag(event: PointerEvent) {
    if (!viewportDrag || viewportDrag.pointerId !== event.pointerId) return;
    const delta = viewportDrag.axis === 'y'
      ? viewportDrag.y - event.clientY
      : event.clientX - viewportDrag.x;
    // Small, fixed world-unit sensitivity so movements are deliberate.
    notifyTransform('update', viewportDrag.axis, delta * 0.02);
    viewportDrag.x = event.clientX;
    viewportDrag.y = event.clientY;
    event.preventDefault();
    event.stopPropagation();
  }

  function endViewportDrag(event: PointerEvent) {
    if (!viewportDrag || viewportDrag.pointerId !== event.pointerId) return;
    notifyTransform('end', viewportDrag.axis);
    viewportDrag = null;
    event.stopPropagation();
  }

  let dismissedErrors = $state(new Set<string>());
  const engineErrors = $derived(telemetry?.errors ?? []);
  // The engine's own list is authoritative; the JS-side device listener only
  // fills in when the engine cannot report (e.g. it is wedged).
  const allErrors = $derived(engineErrors.length > 0 ? engineErrors : gpuErrors);
  const visibleErrors = $derived(allErrors.filter((e) => !dismissedErrors.has(e)));
  let notice = $state<string | null>(null);

  function report() {
    return {
      collected_at: new Date().toISOString(),
      browser,
      engine: telemetry?.render ?? null,
      canvas: telemetry?.window ?? null,
    };
  }

  async function copyReport() {
    await navigator.clipboard.writeText(JSON.stringify(report(), null, 2));
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  async function bench() {
    benchResults = null;
    const results = await runBench(undefined, (done, total) => {
      benchProgress = `${done} / ${total}`;
    });
    benchProgress = null;
    benchResults = results;
    (window as unknown as Record<string, unknown>).__grafixBench = results;
    // Benchmarks use imperative engine controls. Restore the last validated
    // project immediately afterwards; the benchmark must never rewrite it.
    if (activeDocument) engine.loadGraph(JSON.stringify($state.snapshot(activeDocument)));
  }

  function simulateLoss() {
    if (!simulateDeviceLoss()) gpuErrors = [...gpuErrors, 'No engine GPU device captured yet.'];
  }

  onMount(() => {
    let lostAtInit = -1;
    const offGpu = onGpuEvent((event) => {
      if (event.kind === 'device-lost') {
        lostAtInit = telemetry?.renderer_inits ?? 0;
        overlay = {
          title: 'GPU device lost',
          message: `${event.reason}${event.message ? `: ${event.message}` : ''}`,
          action: 'reload',
        };
      } else {
        if (!gpuErrors.includes(event.message)) gpuErrors = [...gpuErrors.slice(-4), event.message];
      }
    });
    const onFatal = (event: Event) => {
      overlay = {
        title: 'Engine stopped',
        message: String((event as CustomEvent).detail),
        action: 'reload',
      };
    };
    window.addEventListener('grafix:fatal', onFatal);

    let timer: ReturnType<typeof setInterval> | undefined;
    let lastDraws = 0;
    let lastFrame = 0;

    (async () => {
      installGpuProbes();
      browser = await probeBrowser();
      if (browser.unsupportedReason) {
        overlay = {
          title: 'WebGPU unavailable',
          message: `${browser.unsupportedReason} This build renders with WebGPU only; there is no WebGL fallback.`,
          action: 'none',
        };
        engineFailed = true;
        return;
      }
      try {
        await startEngine('#grafix-canvas');
      } catch (err) {
        overlay = { title: 'Engine failed to start', message: String(err), action: 'reload' };
        engineFailed = true;
        return;
      }
      timer = setInterval(() => {
        const t = engine.telemetry();
        const frames = t.frame - lastFrame;
        if (frames > 0) drawsPerFrame = (counters.draws - lastDraws) / frames;
        lastDraws = counters.draws;
        lastFrame = t.frame;
        if (lostAtInit >= 0 && t.renderer_inits > lostAtInit && !t.device_lost) {
          overlay = null;
          lostAtInit = -1;
          notice = 'GPU device was lost and the renderer recovered.';
          setTimeout(() => (notice = null), 6000);
        }
        if (t.rendering_stopped && !overlay) {
          overlay = {
            title: 'Rendering stopped',
            message: t.errors[t.errors.length - 1] ?? 'The GPU reported an error.',
            action: 'reload',
          };
        }
        telemetry = t;
      }, 250);
      (window as unknown as Record<string, unknown>).__grafix = {
        engine,
        counters,
        report,
        runBench,
        simulateDeviceLoss,
        simulateValidationError,
      };
      if (new URLSearchParams(location.search).has('bench')) {
        await new Promise((r) => setTimeout(r, 3000));
        await bench();
      }
    })();

    return () => {
      offGpu();
      window.removeEventListener('grafix:fatal', onFatal);
      clearInterval(timer);
    };
  });
</script>

<main class="shell" class:show-diagnostics={showDiagnostics}>
  <div class="stage">
  <section class="viewport">
    <button class="diagnostics-toggle" aria-pressed={showDiagnostics}
      title={showDiagnostics ? 'Close diagnostics' : 'Open diagnostics'}
      onclick={() => (showDiagnostics = !showDiagnostics)}>
      {showDiagnostics ? 'Close debug' : 'Debug'}
    </button>
    <canvas id="grafix-canvas" tabindex="-1"></canvas>
    {#if telemetry}
      <div class="hud">
        <span>{telemetry.fps.toFixed(0)} fps</span>
        <span>{telemetry.frame_ms.toFixed(1)} ms</span>
        <span>cpu {telemetry.cpu_ms.toFixed(2)} ms</span>
        <span>{drawsPerFrame.toFixed(0)} draws</span>
        <span>{telemetry.window.physical_width}×{telemetry.window.physical_height} @{telemetry.window.scale_factor}x</span>
      </div>
    {/if}
    {#if notice}
      <p class="notice" role="status">{notice}</p>
    {/if}
    {#if selectedSpatial}
      <div class="viewport-transform" aria-label="Selected object translation">
        <div class="viewport-transform-title">
          <strong>MOVE · {selectedSpatial.title}</strong>
          {#if !selectedSpatial.active}<small>Not connected</small>{/if}
        </div>
        <div class="viewport-axes">
          {#each ['x', 'y', 'z'] as axis}
            <button
              class={'viewport-axis axis-' + axis}
              aria-label={'Drag to move ' + selectedSpatial.title + ' on ' + axis.toUpperCase()}
              title={'Drag to change position ' + axis.toUpperCase()}
              disabled={!selectedSpatial.active}
              onpointerdown={(event) => beginViewportDrag(event, axis as 'x' | 'y' | 'z')}
              onpointermove={moveViewportDrag}
              onpointerup={endViewportDrag}
              onpointercancel={endViewportDrag}>
              {axis.toUpperCase()}
              <span>{selectedSpatial.position[axis as 'x' | 'y' | 'z'].toFixed(2)}</span>
            </button>
          {/each}
        </div>
        <small>Drag an axis to reposition. Edit rotation and scale in Inspector.</small>
      </div>
    {/if}
    {#if visibleErrors.length > 0}
      <div class="errors" role="alert">
        <ul>
          {#each visibleErrors as error, i (i)}
            <li title={error}>{error.length > 400 ? `${error.slice(0, 400)}…` : error}</li>
          {/each}
        </ul>
        <button onclick={() => (dismissedErrors = new Set([...dismissedErrors, ...allErrors]))}>Dismiss</button>
      </div>
    {/if}
    {#if overlay}
      <div class="overlay" role="alertdialog" aria-labelledby="overlay-title">
        <div class="overlay-card">
          <h1 id="overlay-title">{overlay.title}</h1>
          <p>{overlay.message}</p>
          {#if overlay.action === 'reload'}
            <button onclick={() => location.reload()}>Reload</button>
          {/if}
        </div>
      </div>
    {/if}
  </section>

  <SvelteFlowProvider>
    <GraphEditor ready={!!telemetry} {telemetry}
      onSpatialSelection={(value) => (selectedSpatial = value)}
      onGraphChange={(value) => (activeDocument = value)}/>
  </SvelteFlowProvider>
  </div>

  {#if showDiagnostics}
  <aside class="panel" aria-label="Debug diagnostics">
    <p class="debug-note">Read-only engine diagnostics. Creative settings live in the node inspector. Benchmarks temporarily override the renderer and restore the saved graph afterwards.</p>
    <section>
      <h2>Diagnostics</h2>
      {#if telemetry}
        <dl class="stats">
          <dt>Model</dt><dd>{telemetry.model}</dd>
          <dt>Entities</dt><dd>{telemetry.entities.toLocaleString()}</dd>
          <dt>Instances</dt><dd>{telemetry.instances.toLocaleString()}</dd>
          <dt>Draws / frame</dt><dd>{drawsPerFrame.toFixed(1)}</dd>
          <dt>Indirect draws</dt><dd>{telemetry.indirect_drawing ? 'on' : 'off'}</dd>
          <dt>Compute passes</dt><dd>{telemetry.compute_dispatches.toLocaleString()}</dd>
          <dt>Instance uploads</dt><dd>{telemetry.instance_uploads}</dd>
          <dt>Pipelines</dt><dd>{counters.pipelinesCreated}</dd>
        </dl>
      {:else}
        <p class="muted">{engineFailed ? 'Engine not running' : 'Starting engine…'}</p>
      {/if}
      <div class="actions">
        <button onclick={bench} disabled={!telemetry || benchProgress !== null}>
          {benchProgress ? `Benchmarking ${benchProgress}` : 'Run benchmark'}
        </button>
      </div>
      {#if benchResults}
        <div class="bench">
          <table>
            <thead>
              <tr><th>Mode</th><th>Count</th><th>ms</th><th>CPU</th><th>Draws</th></tr>
            </thead>
            <tbody>
              {#each benchResults as r, i (i)}
                <tr>
                  <td>{r.mode}{r.animate ? '' : ' static'}</td>
                  <td>{r.count.toLocaleString()}</td>
                  <td>{r.frameMsMean}</td>
                  <td>{r.cpuMs}</td>
                  <td>{r.drawsPerFrame}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </section>

    <section>
      <h2>Device</h2>
      {#if browser?.adapter || telemetry?.render}
        <dl class="stats">
          <dt>Adapter</dt><dd>{telemetry?.render?.adapter_name || browser?.adapter?.description || browser?.adapter?.vendor}</dd>
          <dt>Architecture</dt><dd>{browser?.adapter?.architecture || '—'}</dd>
          <dt>Backend</dt><dd>{telemetry?.render?.backend ?? '—'}</dd>
          <dt>GPU culling</dt><dd>{telemetry?.render?.gpu_preprocessing ?? '—'}</dd>
          <dt>Max storage</dt><dd>{((telemetry?.render?.limits.max_storage_buffer_binding_size ?? 0) / 2 ** 20).toFixed(0)} MiB</dd>
        </dl>
      {/if}
      <div class="actions">
        <button onclick={copyReport} disabled={!browser}>{copied ? 'Copied' : 'Copy report'}</button>
        <button onclick={simulateLoss} disabled={!telemetry}>Simulate device loss</button>
      </div>
    </section>
  </aside>
  {/if}
</main>

<style>
  .shell {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    height: 100%;
  }
  .shell.show-diagnostics { grid-template-columns: minmax(0, 1fr) 300px; }
  .diagnostics-toggle { position:absolute; z-index:4; right:12px; bottom:12px;
    padding:7px 11px; border:1px solid #465264; background:rgb(17 22 30 / 0.94);
    color:#c6d6e6; border-radius:5px; font:11px var(--mono); }
  .debug-note { margin:0; padding:12px 16px; color:#96a8ba;
    font:11px/1.5 var(--mono); border-bottom:1px solid var(--line); }
  .stage { display:grid; grid-template-rows:minmax(0, 54%) minmax(0, 46%); min-width:0; min-height:0; }
  .viewport-transform {
    position:absolute; z-index:3; top:12px; right:12px; width:210px;
    padding:10px; border:1px solid #465264; background:rgb(17 22 30 / 0.92);
    border-radius:7px; color:#d9e3f2; font:10px var(--mono);
  }
  .viewport-transform-title { display:flex; flex-direction:column; gap:3px; margin-bottom:9px; }
  .viewport-transform-title strong { font-size:10px; }
  .viewport-transform-title small { color:#dfa981; }
  .viewport-axes { display:grid; grid-template-columns:repeat(3,1fr); gap:5px; margin-bottom:8px; }
  .viewport-axis {
    display:flex; flex-direction:column; gap:4px; padding:7px 4px;
    align-items:center; touch-action:none; user-select:none; cursor:ew-resize;
    border:1px solid #3c4653; background:#202833; border-radius:4px;
    font:700 12px var(--mono);
  }
  .viewport-axis span { color:#bec9d7; font:10px var(--mono); }
  .viewport-axis.axis-x { color:#e2a7a7; }
  .viewport-axis.axis-y { color:#a7daa6; cursor:ns-resize; }
  .viewport-axis.axis-z { color:#a4c6f4; }
  .viewport-axis:disabled { cursor:not-allowed; opacity:0.5; }
  .viewport-transform > small { line-height:1.45; color:#9caabc; display:block; }
  .viewport {
    position: relative;
    min-width: 0;
    min-height: 0;
    background: #000;
  }
  canvas {
    display: block;
    width: 100%;
    height: 100%;
    outline: none;
  }
  .hud {
    position: absolute;
    top: 12px;
    left: 12px;
    display: flex;
    gap: 14px;
    padding: 6px 10px;
    font: 12px var(--mono);
    color: var(--muted);
    background: rgb(11 12 15 / 0.7);
    border-radius: var(--radius);
    pointer-events: none;
  }
  .errors {
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: 12px;
    display: flex;
    align-items: flex-start;
    gap: 16px;
    padding: 10px 12px;
    background: rgb(30 12 12 / 0.92);
    border: 1px solid #4a2320;
    border-radius: var(--radius);
    max-height: 30%;
    overflow: auto;
  }
  .errors ul {
    flex: 1;
    min-width: 0;
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 6px;
    font: 12px var(--mono);
    color: var(--danger);
    overflow-wrap: anywhere;
  }
  .errors button { flex: none; }
  .notice {
    position: absolute;
    left: 12px;
    top: 52px;
    margin: 0;
    padding: 8px 12px;
    color: var(--ok);
    background: rgb(11 12 15 / 0.85);
    border: 1px solid var(--line);
    border-radius: var(--radius);
  }
  .overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgb(5 6 8 / 0.82);
  }
  .overlay-card {
    max-width: 440px;
    padding: 24px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 10px;
  }
  .overlay-card h1 { font-size: 17px; }
  .overlay-card p {
    margin: 10px 0 0;
    color: var(--muted);
    overflow-wrap: anywhere;
  }
  .overlay-card button { margin-top: 24px; }

  .panel {
    overflow-y: auto;
    background: var(--panel);
    border-left: 1px solid var(--line);
  }
  .panel section {
    padding: 18px 18px 20px;
    border-bottom: 1px solid var(--line);
  }
  .panel h2 { margin-bottom: 14px; }
  .stack {
    display: grid;
    gap: 14px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 20px;
  }
  .stats {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 12px;
    margin: 0;
  }
  .stats dt { color: var(--muted); }
  .stats dd {
    margin: 0;
    text-align: right;
    font-family: var(--mono);
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  .muted { color: var(--muted); margin: 0; }
  .bench { margin-top: 16px; overflow-x: auto; }
  table { width: 100%; border-collapse: collapse; font: 11px var(--mono); }
  th, td { padding: 3px 4px; text-align: right; white-space: nowrap; }
  th:first-child, td:first-child { text-align: left; }
  th { color: var(--muted); font-weight: 500; }
</style>
