<script lang="ts">
  import { onMount } from 'svelte';
  import Slider from './lib/Slider.svelte';
  import GraphEditor from './lib/GraphEditor.svelte';
  import {
    counters,
    installGpuProbes,
    onGpuEvent,
    probeBrowser,
    simulateDeviceLoss,
    simulateValidationError,
    type BrowserCapabilities,
  } from './lib/gpu-probe';
  import { engine, startEngine, type InstancingMode, type Telemetry } from './lib/engine';
  import { runBench, type BenchResult } from './lib/bench';

  // Labels and slider steps for the Rust-owned parameter list. Names and
  // ranges come from `param_schema()`; only presentation lives here.
  const PARAM_UI: Record<string, { label: string; step: number; format: (v: number) => string }> = {
    light_intensity: { label: 'Key light', step: 100, format: (v) => `${Math.round(v)} lx` },
    spin_speed: { label: 'Model spin', step: 0.01, format: (v) => `${v.toFixed(2)} rad/s` },
    bloom: { label: 'Bloom', step: 0.01, format: (v) => v.toFixed(2) },
    aberration: { label: 'Aberration', step: 0.0005, format: (v) => v.toFixed(4) },
    vignette: { label: 'Vignette', step: 0.01, format: (v) => v.toFixed(2) },
    field_speed: { label: 'Field speed', step: 0.01, format: (v) => v.toFixed(2) },
    field_scale: { label: 'Field scale', step: 0.1, format: (v) => v.toFixed(1) },
  };

  const MODES: { id: InstancingMode; label: string }[] = [
    { id: 'off', label: 'Off' },
    { id: 'builtin', label: 'Built-in' },
    { id: 'unbatched', label: 'Unbatched' },
    { id: 'custom', label: 'Custom' },
  ];
  const COUNT_STOPS = [100, 1_000, 2_000, 5_000, 10_000, 25_000, 50_000, 100_000];
  const UNBATCHED_MAX = 10_000;

  type Overlay = { title: string; message: string; action: 'reload' | 'none' } | null;

  let params = $state<{ name: string; min: number; max: number; value: number }[]>([]);
  let mode = $state<InstancingMode>('custom');
  let countIndex = $state(2);
  let animate = $state(true);
  let telemetry = $state<Telemetry | null>(null);
  let drawsPerFrame = $state(0);
  let browser = $state<BrowserCapabilities | null>(null);
  let overlay = $state<Overlay>(null);
  let gpuErrors = $state<string[]>([]);
  let benchProgress = $state<string | null>(null);
  let benchResults = $state<BenchResult[] | null>(null);
  let copied = $state(false);
  let engineFailed = $state(false);

  const count = $derived(
    mode === 'unbatched' ? Math.min(COUNT_STOPS[countIndex], UNBATCHED_MAX) : COUNT_STOPS[countIndex],
  );
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

  function applyInstancing() {
    engine.setInstancing(mode, mode === 'off' ? 0 : count);
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
    // Restore the interactive selection.
    engine.setAnimateClones(animate);
    applyInstancing();
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
      params = engine.paramSchema().map((p) => ({ ...p, value: p.min }));
      let synced = false;
      timer = setInterval(() => {
        const t = engine.telemetry();
        // Slider positions start from Rust's defaults, once.
        if (!synced && t.params) {
          params = params.map((p) => ({ ...p, value: t.params?.[p.name] ?? p.value }));
          if (t.instancing) {
            mode = t.instancing.mode;
            animate = t.instancing.animate;
            const i = COUNT_STOPS.indexOf(t.instancing.count);
            if (i >= 0) countIndex = i;
          }
          synced = true;
        }
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

<main class="shell">
  <div class="stage">
  <section class="viewport">
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

  <GraphEditor ready={!!telemetry}/>
  </div>

  <aside class="panel">
    <section>
      <h2>Scene</h2>
      <div class="stack">
        {#each params as param (param.name)}
          {@const ui = PARAM_UI[param.name]}
          <Slider
            label={ui?.label ?? param.name}
            min={param.min}
            max={param.max}
            step={ui?.step ?? 0.01}
            format={ui?.format}
            bind:value={param.value}
            oninput={(v) => engine.setParam(param.name, v)}
          />
        {/each}
      </div>
      <div class="actions">
        <button onclick={() => engine.resetCamera()} disabled={!telemetry}>Reset camera</button>
      </div>
    </section>

    <section>
      <h2>Clones</h2>
      <div class="segmented" role="radiogroup" aria-label="Instancing mode">
        {#each MODES as m (m.id)}
          <button
            role="radio"
            aria-checked={mode === m.id}
            class:active={mode === m.id}
            disabled={!telemetry}
            onclick={() => {
              mode = m.id;
              applyInstancing();
            }}>{m.label}</button
          >
        {/each}
      </div>
      <div class="stack">
        <Slider
          label="Count"
          min={0}
          max={COUNT_STOPS.length - 1}
          step={1}
          format={() => count.toLocaleString()}
          disabled={!telemetry}
          bind:value={countIndex}
          oninput={applyInstancing}
        />
        <label class="check">
          <input
            type="checkbox"
            disabled={!telemetry}
            bind:checked={animate}
            onchange={() => engine.setAnimateClones(animate)}
          />
          Animate on CPU (built-in, unbatched)
        </label>
      </div>
    </section>

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
</main>

<style>
  .shell {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 300px;
    height: 100%;
  }
  .stage { display:grid; grid-template-rows:minmax(0, 54%) minmax(0, 46%); min-width:0; min-height:0; }
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
  .segmented {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 2px;
    padding: 2px;
    margin-bottom: 16px;
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: var(--radius);
  }
  .segmented button {
    padding: 6px 0;
    border: none;
    background: transparent;
    color: var(--muted);
    border-radius: 4px;
  }
  .segmented button.active {
    background: var(--panel-raised);
    color: var(--text);
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text);
  }
  .check input { accent-color: var(--accent); margin: 0; }
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
