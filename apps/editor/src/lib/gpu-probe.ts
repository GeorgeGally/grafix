// WebGPU instrumentation installed before the engine loads.
//
// - Wraps GPUAdapter.requestDevice to capture the engine's device, so device
//   loss and uncaptured validation errors reach the UI even if the engine is
//   already wedged.
// - Counts draw/dispatch/submit calls and uploaded bytes at the WebGPU API
//   level. This is the ground truth for "how many draw calls did instancing
//   actually issue", independent of what Bevy reports.

/* eslint-disable @typescript-eslint/no-explicit-any */
type AnyFn = (...args: any[]) => any;

export interface GpuCounters {
  draws: number;
  drawInstances: number;
  indirectDraws: number;
  dispatches: number;
  submits: number;
  bufferUploadBytes: number;
  pipelinesCreated: number;
}

export type GpuEvent =
  | { kind: 'device-lost'; message: string; reason: string }
  | { kind: 'gpu-error'; message: string };

export const counters: GpuCounters = {
  draws: 0,
  drawInstances: 0,
  indirectDraws: 0,
  dispatches: 0,
  submits: 0,
  bufferUploadBytes: 0,
  pipelinesCreated: 0,
};

let engineDevice: any = null;
const listeners = new Set<(event: GpuEvent) => void>();

export function onGpuEvent(listener: (event: GpuEvent) => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function emit(event: GpuEvent) {
  for (const listener of listeners) listener(event);
}

function wrap(proto: any, name: string, before: (args: any[]) => void) {
  const original: AnyFn | undefined = proto?.[name];
  if (typeof original !== 'function') return;
  proto[name] = function (this: unknown, ...args: any[]) {
    before(args);
    return original.apply(this, args);
  };
}

let installed = false;

export function installGpuProbes() {
  if (installed || !('gpu' in navigator)) return;
  installed = true;
  const g = globalThis as any;

  const pass = g.GPURenderPassEncoder?.prototype;
  wrap(pass, 'draw', (a) => {
    counters.draws++;
    counters.drawInstances += a[1] ?? 1;
  });
  wrap(pass, 'drawIndexed', (a) => {
    counters.draws++;
    counters.drawInstances += a[1] ?? 1;
  });
  wrap(pass, 'drawIndirect', () => {
    counters.draws++;
    counters.indirectDraws++;
  });
  wrap(pass, 'drawIndexedIndirect', () => {
    counters.draws++;
    counters.indirectDraws++;
  });
  const bundle = g.GPURenderBundleEncoder?.prototype;
  for (const name of ['draw', 'drawIndexed', 'drawIndirect', 'drawIndexedIndirect']) {
    wrap(bundle, name, () => counters.draws++);
  }
  wrap(g.GPUComputePassEncoder?.prototype, 'dispatchWorkgroups', () => counters.dispatches++);
  wrap(g.GPUComputePassEncoder?.prototype, 'dispatchWorkgroupsIndirect', () => counters.dispatches++);
  wrap(g.GPUQueue?.prototype, 'submit', () => counters.submits++);
  wrap(g.GPUQueue?.prototype, 'writeBuffer', (a) => {
    const data = a[2];
    const size = a[4] ?? (data?.byteLength ?? 0) - (a[3] ?? 0);
    counters.bufferUploadBytes += Math.max(0, size);
  });
  for (const name of [
    'createRenderPipeline',
    'createRenderPipelineAsync',
    'createComputePipeline',
    'createComputePipelineAsync',
  ]) {
    wrap(g.GPUDevice?.prototype, name, () => counters.pipelinesCreated++);
  }

  const adapterProto = g.GPUAdapter?.prototype;
  const requestDevice: AnyFn | undefined = adapterProto?.requestDevice;
  if (typeof requestDevice === 'function') {
    adapterProto.requestDevice = async function (this: unknown, ...args: any[]) {
      const device = await requestDevice.apply(this, args);
      watchDevice(device);
      return device;
    };
  }
}

function watchDevice(device: any) {
  engineDevice = device;
  device.addEventListener?.('uncapturederror', (event: any) => {
    emit({ kind: 'gpu-error', message: String(event?.error?.message ?? event) });
  });
  device.lost?.then((info: any) => {
    // A deliberate destroy() from our own "simulate" control is still loss.
    emit({
      kind: 'device-lost',
      reason: String(info?.reason ?? 'unknown'),
      message: String(info?.message ?? ''),
    });
  });
}

/** Destroys the engine's GPU device to exercise the device-loss path. */
export function simulateDeviceLoss(): boolean {
  if (!engineDevice) return false;
  engineDevice.destroy();
  return true;
}

/**
 * Issues an invalid call on the engine's device to exercise the uncaptured
 * validation-error path. Rendering stops afterwards by design.
 */
export function simulateValidationError(): boolean {
  if (!engineDevice) return false;
  engineDevice.createBuffer({ size: 4, usage: 0 });
  return true;
}

export interface BrowserCapabilities {
  userAgent: string;
  platform: string;
  hardwareConcurrency: number;
  deviceMemoryGb: number | null;
  devicePixelRatio: number;
  screen: { width: number; height: number };
  webgpu: boolean;
  preferredCanvasFormat: string | null;
  adapter: {
    vendor: string;
    architecture: string;
    device: string;
    description: string;
    isFallbackAdapter: boolean | null;
    features: string[];
    limits: Record<string, number>;
  } | null;
  unsupportedReason: string | null;
}

/** Probes WebGPU from JS. Never throws; a missing API is reported as data. */
export async function probeBrowser(): Promise<BrowserCapabilities> {
  const nav = navigator as any;
  const base: BrowserCapabilities = {
    userAgent: navigator.userAgent,
    platform: nav.userAgentData?.platform ?? navigator.platform ?? '',
    hardwareConcurrency: navigator.hardwareConcurrency ?? 0,
    deviceMemoryGb: nav.deviceMemory ?? null,
    devicePixelRatio: window.devicePixelRatio,
    screen: { width: screen.width, height: screen.height },
    webgpu: 'gpu' in navigator,
    preferredCanvasFormat: null,
    adapter: null,
    unsupportedReason: null,
  };
  if (!nav.gpu) {
    base.unsupportedReason = 'navigator.gpu is missing: this browser has no WebGPU.';
    return base;
  }
  base.preferredCanvasFormat = nav.gpu.getPreferredCanvasFormat?.() ?? null;
  let adapter: any = null;
  try {
    adapter = await nav.gpu.requestAdapter({ powerPreference: 'high-performance' });
  } catch (err) {
    base.unsupportedReason = `requestAdapter failed: ${String(err)}`;
    return base;
  }
  if (!adapter) {
    base.unsupportedReason = 'WebGPU is present but no GPU adapter is available.';
    return base;
  }
  const info = adapter.info ?? {};
  const limits: Record<string, number> = {};
  for (const key in adapter.limits) {
    const value = adapter.limits[key];
    if (typeof value === 'number') limits[key] = value;
  }
  base.adapter = {
    vendor: info.vendor ?? '',
    architecture: info.architecture ?? '',
    device: info.device ?? '',
    description: info.description ?? '',
    isFallbackAdapter: info.isFallbackAdapter ?? adapter.isFallbackAdapter ?? null,
    features: [...adapter.features].sort(),
    limits,
  };
  return base;
}
