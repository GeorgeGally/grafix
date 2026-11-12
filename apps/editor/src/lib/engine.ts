// Thin typed wrapper over the wasm-bindgen package produced by
// scripts/build-web.sh. The UI talks to the engine only through this file.

import init, * as wasm from '../wasm-pkg/bridge_wasm.js';

export type InstancingMode = 'off' | 'builtin' | 'unbatched' | 'custom';

export interface ParamSpec {
  name: string;
  min: number;
  max: number;
}

export interface RenderCapabilities {
  adapter_name: string;
  vendor: number;
  device: number;
  device_type: string;
  driver: string;
  driver_info: string;
  backend: string;
  gpu_preprocessing: string;
  features: string[];
  limits: Record<string, number>;
}

export interface Telemetry {
  frame: number;
  fps: number;
  frame_ms: number;
  cpu_ms: number;
  entities: number;
  clone_entities: number;
  instances: number;
  instancing: { mode: InstancingMode; count: number; animate: boolean } | null;
  indirect_drawing: boolean;
  compute_dispatches: number;
  instance_uploads: number;
  model: string;
  errors: string[];
  device_lost: boolean;
  rendering_stopped: boolean;
  renderer_inits: number;
  params: Record<string, number> | null;
  window: { physical_width: number; physical_height: number; scale_factor: number };
  render: RenderCapabilities | null;
}

export async function startEngine(canvasSelector: string): Promise<void> {
  await init();
  wasm.start(canvasSelector, 'assets');
}

export const engine = {
  setParam: (name: string, value: number) => wasm.set_param(name, value),
  setInstancing: (mode: InstancingMode, count: number) => wasm.set_instancing(mode, count),
  setAnimateClones: (animate: boolean) => wasm.set_animate_clones(animate),
  resetCamera: () => wasm.reset_camera(),
  telemetry: (): Telemetry => JSON.parse(wasm.telemetry()),
  paramSchema: (): ParamSpec[] => JSON.parse(wasm.param_schema()),
};
