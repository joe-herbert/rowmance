import { defineConfig } from 'vite';
import { sveltekit } from '@sveltejs/kit/vite';

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [sveltekit()],

  // Prevent Vite from obscuring Rust errors.
  clearScreen: false,

  server: {
    // Tauri expects a fixed port; fail if unavailable.
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },

});
