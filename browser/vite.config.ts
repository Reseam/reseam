import { defineConfig } from 'vite';
// Builds only the comparison harness; hosts bundle the package from source.
export default defineConfig({
  worker: { format: 'iife' },
  build: { target: 'es2022', sourcemap: true, rollupOptions: { input: 'harness.html' } },
});
