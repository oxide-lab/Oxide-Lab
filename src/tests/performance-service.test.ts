import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { PerformanceService } from '$lib/services/performance-service';

const eventMocks = vi.hoisted(() => {
  const handlers = new Map<string, (event: { payload: any }) => void>();
  const unlistenA = vi.fn();
  const unlistenB = vi.fn();
  const unlistenC = vi.fn();
  const unlistenD = vi.fn();
  const unlistenE = vi.fn();
  const unlistenF = vi.fn();
  const unlistens = [unlistenA, unlistenB, unlistenC, unlistenD, unlistenE, unlistenF];
  let index = 0;
  const listenMock = vi.fn(async (eventName: string, callback: (event: { payload: any }) => void) => {
    handlers.set(eventName, callback);
    const unlisten = unlistens[index] ?? vi.fn();
    index += 1;
    return unlisten;
  });

  return { unlistenA, unlistenB, unlistenC, handlers, listenMock };
});

vi.mock('@tauri-apps/api/event', () => ({
  listen: eventMocks.listenMock,
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('PerformanceService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    eventMocks.handlers.clear();
  });

  it('cleans up old listeners before registering new ones', async () => {
    const service = new PerformanceService();

    await service.setupEventListeners();
    await service.setupEventListeners();

    expect(eventMocks.unlistenA).toHaveBeenCalledTimes(1);
    expect(eventMocks.unlistenB).toHaveBeenCalledTimes(1);
    expect(eventMocks.unlistenC).toHaveBeenCalledTimes(1);
    expect(eventMocks.listenMock).toHaveBeenCalledTimes(6);
  });

  it('updates reactive performance state on incoming metrics', async () => {
    const { performanceServiceState } = await import('$lib/services/performance-service');
    const service = new PerformanceService();

    await service.setupEventListeners();

    eventMocks.handlers.get('model_load_metrics')?.({
      payload: { total_duration_ms: 100, model_size_mb: 10 },
    });
    eventMocks.handlers.get('inference_metrics')?.({
      payload: { tokens_per_second: 20, generated_tokens: 40, total_duration_ms: 200 },
    });
    eventMocks.handlers.get('startup_metrics')?.({
      payload: { startup_duration_ms: 300 },
    });

    const snapshot = get(performanceServiceState);
    expect(snapshot.lastModelLoadMetrics).toBeTruthy();
    expect(snapshot.lastInferenceMetrics).toBeTruthy();
    expect(snapshot.startupMetrics).toBeTruthy();
    expect(snapshot.inferenceHistory.length).toBe(1);
  });
});
