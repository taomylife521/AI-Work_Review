import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import path from 'path';
import fs from 'fs';

function excludeLive2dSdk() {
  return {
    name: 'exclude-live2d-sdk',
    closeBundle() {
      const sdkDir = path.resolve(__dirname, 'dist/js/CubismSdkForWeb-5-r.5');
      if (fs.existsSync(sdkDir)) {
        fs.rmSync(sdkDir, { recursive: true, force: true });
      }
    },
  };
}

export default defineConfig({
  plugins: [svelte(), excludeLive2dSdk()],
  clearScreen: false,
  server: {
    host: '127.0.0.1',
    port: 5173,
    strictPort: true,
    hmr: {
      host: '127.0.0.1',
    },
  },
  envPrefix: ['VITE_', 'TAURI_'],
  optimizeDeps: {
    entries: ['index.html'],
  },
  build: {
    target: ['es2021', 'chrome100', 'safari13'],
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
  resolve: {
    alias: {
      '$lib': path.resolve('./src/lib'),
    },
  },
});
