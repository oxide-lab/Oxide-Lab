import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { initLoadedModels, loadedModelIds } from '$lib/stores/local-models';

const tauriMocks = vi.hoisted(() => {
  const listeners = new Map<string, (event: { payload?: unknown }) => void | Promise<void>>();
  return {
    invoke: vi.fn(async (command: string) => {
      if (command === 'get_loaded_models') return [];
      throw new Error(`Unexpected command ${command}`);
    }),
    listen: vi.fn(async (eventName: string, handler: (event: { payload?: unknown }) => void | Promise<void>) => {
      listeners.set(eventName, handler);
      return () => listeners.delete(eventName);
    }),
    listeners,
  };
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: tauriMocks.invoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: tauriMocks.listen,
}));

describe('initLoadedModels', () => {
  beforeEach(() => {
    tauriMocks.invoke.mockClear();
    tauriMocks.listen.mockClear();
    tauriMocks.listeners.clear();
    loadedModelIds.set([]);
    (window as { __oxideSchedulerSnapshot?: { loaded_models?: string[] } }).__oxideSchedulerSnapshot = {
      loaded_models: ['model-a.gguf'],
    };
  });

  it('uses scheduler_snapshot payload as primary loaded-model source', async () => {
    const cleanup = await initLoadedModels();
    const handler = tauriMocks.listeners.get('scheduler_snapshot');
    expect(handler).toBeTypeOf('function');

    await handler?.({ payload: { loaded_models: ['model-b.gguf', 'model-c.gguf'] } });
    expect(get(loadedModelIds)).toEqual(['model-b.gguf', 'model-c.gguf']);

    cleanup();
  });
});
