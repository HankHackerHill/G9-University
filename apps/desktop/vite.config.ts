import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

// Tauri serves `dist/` from an app-local origin; no remote origins, no inline scripts.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    target: 'es2022',
    sourcemap: false,
    modulePreload: { polyfill: false },
    // Never inline an asset as a data: URL: the content security policy serves fonts from 'self' only.
    assetsInlineLimit: 0,
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.test.{ts,tsx}'],
    css: false,
    restoreMocks: true,
  },
});
