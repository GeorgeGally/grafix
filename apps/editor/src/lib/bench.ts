// Repeatable instancing sweep. Each step sets a clone mode/count, waits for
// the scene to settle, then samples browser frame timing, Rust-side CPU time
// and WebGPU-level call counters.

import { counters } from './gpu-probe';
import { engine, type InstancingMode } from './engine';

export interface BenchStep {
  mode: InstancingMode;
  count: number;
  animate: boolean;
}

export interface BenchResult extends BenchStep {
  frames: number;
  frameMsMean: number;
  frameMsP95: number;
  fps: number;
  cpuMs: number;
  drawsPerFrame: number;
  instancesDrawnPerFrame: number;
  uploadKbPerFrame: number;
  submitsPerFrame: number;
  entities: number;
  indirectDrawing: boolean;
}

export const DEFAULT_STEPS: BenchStep[] = [
  { mode: 'off', count: 0, animate: true },
  { mode: 'builtin', count: 1_000, animate: true },
  { mode: 'builtin', count: 10_000, animate: true },
  { mode: 'builtin', count: 50_000, animate: true },
  { mode: 'builtin', count: 10_000, animate: false },
  { mode: 'builtin', count: 50_000, animate: false },
  { mode: 'unbatched', count: 1_000, animate: true },
  { mode: 'unbatched', count: 10_000, animate: true },
  { mode: 'custom', count: 1_000, animate: true },
  { mode: 'custom', count: 10_000, animate: true },
  { mode: 'custom', count: 50_000, animate: true },
  { mode: 'custom', count: 100_000, animate: true },
];

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

function sampleFrames(durationMs: number): Promise<number[]> {
  return new Promise((resolve) => {
    const intervals: number[] = [];
    let last = performance.now();
    const end = last + durationMs;
    const tick = (now: number) => {
      intervals.push(now - last);
      last = now;
      if (now < end) requestAnimationFrame(tick);
      else resolve(intervals.slice(1));
    };
    requestAnimationFrame(tick);
  });
}

const round = (v: number, d = 2) => Math.round(v * 10 ** d) / 10 ** d;

export async function runBench(
  steps: BenchStep[] = DEFAULT_STEPS,
  onProgress?: (done: number, total: number) => void,
  { warmupMs = 1500, sampleMs = 3000 } = {},
): Promise<BenchResult[]> {
  const results: BenchResult[] = [];
  for (const [i, step] of steps.entries()) {
    onProgress?.(i, steps.length);
    engine.setAnimateClones(step.animate);
    engine.setInstancing(step.mode, step.count);
    await wait(warmupMs);
    const before = { ...counters };
    const frameBefore = engine.telemetry().frame;
    const intervals = await sampleFrames(sampleMs);
    const t = engine.telemetry();
    const frames = Math.max(1, t.frame - frameBefore);
    const sorted = [...intervals].sort((a, b) => a - b);
    const mean = intervals.reduce((a, b) => a + b, 0) / Math.max(1, intervals.length);
    results.push({
      ...step,
      frames,
      frameMsMean: round(mean),
      frameMsP95: round(sorted[Math.floor(sorted.length * 0.95)] ?? 0),
      fps: round(1000 / mean, 1),
      cpuMs: round(t.cpu_ms),
      drawsPerFrame: round((counters.draws - before.draws) / frames, 1),
      instancesDrawnPerFrame: Math.round((counters.drawInstances - before.drawInstances) / frames),
      uploadKbPerFrame: round((counters.bufferUploadBytes - before.bufferUploadBytes) / frames / 1024, 1),
      submitsPerFrame: round((counters.submits - before.submits) / frames, 1),
      entities: t.entities,
      indirectDrawing: t.indirect_drawing,
    });
  }
  onProgress?.(steps.length, steps.length);
  return results;
}
