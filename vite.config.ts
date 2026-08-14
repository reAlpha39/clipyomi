import { defineConfig } from 'vitest/config';

// Port 1420 is fixed because tauri.conf.json's devUrl names it; strictPort makes
// a busy port fail loudly instead of silently serving where Tauri is not looking.
export default defineConfig({
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: 'esnext' },
  test: { environment: 'happy-dom', include: ['src/**/*.test.ts'] },
});
