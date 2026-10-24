/// <reference types="vitest/config" />
import { defineConfig, loadEnv } from 'vite';
import react from '@vitejs/plugin-react';

// The dev server proxies /api to the Rust server. `npm run build` writes web/dist, which the
// server serves in production, so the browser always talks to its own origin (no CORS).
// Point the proxy elsewhere with FACTORY_API_TARGET (for example in web/.env.local).
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, '.', 'FACTORY_');
  return {
    plugins: [react()],
    server: {
      host: true,
      port: 5173,
      proxy: {
        '/api': { target: env.FACTORY_API_TARGET ?? 'http://127.0.0.1:8080' },
      },
    },
    build: {
      outDir: 'dist',
      sourcemap: false,
      target: 'es2022',
    },
    test: {
      environment: 'jsdom',
      setupFiles: ['./src/test/setup.ts'],
      css: false,
    },
  };
});
