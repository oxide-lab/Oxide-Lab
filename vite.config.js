import { defineConfig } from 'vite';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [tailwindcss(), sveltekit()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1451,
    strictPort: false,
    host: host || true,
    hmr: host
      ? {
        protocol: 'ws',
        host,
        port: 1421,
      }
      : undefined,
    fs: {
      allow: ['.', '.svelte-kit'],
    },
    watch: {
      // 3. tell Vite to ignore watching `src-tauri` and documentation files
      // @ts-ignore - chokidar supports function in ignored
      ignored: (path) => {
        const normalized = path.replace(/\\/g, '/');
        const isUi =
          normalized.includes('/src/') ||
          normalized.endsWith('/src') ||
          normalized.includes('/src/') ||
          normalized.endsWith('/src') ||
          normalized.includes('/static/') ||
          normalized.endsWith('/static');

        // Allow HMR только для UI-кода
        if (isUi) return false;

        // Всё остальное игнорируем, чтобы UI не перезагружался
        return true;
      },
    },
  },
}));
