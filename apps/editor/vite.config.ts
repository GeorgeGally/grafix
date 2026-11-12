import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  // Relative base so the static build runs from any path.
  base: './',
  plugins: [svelte()],
  build: {
    target: 'es2022',
    // The engine is one large wasm module by design.
    chunkSizeWarningLimit: 4096,
  },
  server: { port: 5173, strictPort: true },
});
