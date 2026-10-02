import { defineConfig } from 'vite';
const headers = {
  'Cross-Origin-Opener-Policy': 'same-origin',
  'Cross-Origin-Embedder-Policy': 'require-corp',
};
export default defineConfig({
  server: { headers }, preview: { headers },
  worker: { format: 'iife' },
  build: { target: 'es2022', sourcemap: true },
});
