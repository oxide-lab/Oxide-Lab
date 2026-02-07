import { configDefaults, defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

const isVitest = !!(globalThis as { process?: { env?: Record<string, unknown> } }).process?.env
  ?.VITEST;

export default defineConfig({
  plugins: [svelte({ hot: !isVitest })],
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./src/tests/setup.ts'],
    exclude: [...configDefaults.exclude, 'example/**/*', 'examples/**/*', 'src-ref/**/*'],
  },
  resolve: isVitest
    ? {
        alias: {
          $lib: new URL('./src/lib', import.meta.url).pathname,
        },
      }
    : undefined,
});
