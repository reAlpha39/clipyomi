import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { defineConfig } from 'vitest/config';

const __dirname = dirname(fileURLToPath(import.meta.url));

// Port 1420 is fixed because tauri.conf.json's devUrl names it; strictPort makes
// a busy port fail loudly instead of silently serving where Tauri is not looking.
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/**', '**/target/**'],
    },
  },
  build: {
    target: 'esnext',
    // Two pages: the app, and the tooltip window. Without naming both here
    // Rollup emits only index.html and the popup 404s in a production build —
    // which `npm run dev` does NOT reveal, because the dev server serves any
    // HTML file on disk.
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
        popover: resolve(__dirname, 'popover.html'),
      },
    },
  },
  test: { environment: 'happy-dom', include: ['src/**/*.test.ts'] },
});
