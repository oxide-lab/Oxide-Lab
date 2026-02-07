import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import type { AppSettingsPatch, AppSettingsV2, OpenAiServerConfig } from '$lib/types/settings-v2';
import { createSettingsV2, statusFromSettings } from '../../tests/fixtures/settings-v2';

const state = vi.hoisted(() => ({
  settings: null as AppSettingsV2 | null,
}));

function cloneSettings(): AppSettingsV2 {
  return structuredClone(state.settings ?? createSettingsV2());
}

function applyPatch(patch: AppSettingsPatch) {
  if (!state.settings) state.settings = createSettingsV2();
  if (patch.general) state.settings.general = patch.general;
  if (patch.models_storage) state.settings.models_storage = patch.models_storage;
  if (patch.performance) state.settings.performance = patch.performance;
  if (patch.chat_presets) state.settings.chat_presets = patch.chat_presets;
  if (patch.privacy_data) state.settings.privacy_data = patch.privacy_data;
  if (patch.developer) state.settings.developer = patch.developer;
}

function clearStorage() {
  const storage = localStorage as unknown as {
    clear?: () => void;
    key?: (index: number) => string | null;
    length?: number;
    removeItem: (key: string) => void;
  };
  if (typeof storage.clear === 'function') {
    storage.clear();
    return;
  }
  if (typeof storage.length === 'number' && typeof storage.key === 'function') {
    const keys: string[] = [];
    for (let i = 0; i < storage.length; i += 1) {
      const key = storage.key(i);
      if (key) keys.push(key);
    }
    for (const key of keys) storage.removeItem(key);
  }
}

vi.mock('$lib/services/settings-v2', () => ({
  getAppSettingsV2: vi.fn(async () => cloneSettings()),
  patchAppSettingsV2: vi.fn(async (patch: AppSettingsPatch) => {
    applyPatch(patch);
    return {
      applied: true,
      requires_restart: false,
      warnings: [],
      settings: cloneSettings(),
    };
  }),
  resetAppSettingsV2: vi.fn(async () => {
    state.settings = createSettingsV2();
    return cloneSettings();
  }),
  getOpenAiServerStatus: vi.fn(async () => statusFromSettings(state.settings ?? createSettingsV2())),
  setOpenAiServerConfig: vi.fn(async (config: OpenAiServerConfig) => {
    if (!state.settings) state.settings = createSettingsV2();
    state.settings.developer.openai_server = config;
    return {
      applied: true,
      requires_restart: false,
      warnings: [],
      settings: cloneSettings(),
    };
  }),
  restartOpenAiServer: vi.fn(async () => statusFromSettings(state.settings ?? createSettingsV2())),
  getDataLocations: vi.fn(),
  exportUserData: vi.fn(),
  clearUserData: vi.fn(),
  sha256Base64NoPad: vi.fn(),
  searchSettingsV2: vi.fn(async (query: string) => {
    const q = query.toLowerCase();
    const rows = [
      {
        id: 'chat_presets.temperature',
        section: 'chat_presets',
        title: 'Temperature',
        description: 'Sampling creativity level',
        hiddenByMode: false,
      },
      {
        id: 'developer.openai_server',
        section: 'developer',
        title: 'OpenAI Server',
        description: 'Configure local OpenAI-compatible server',
        hiddenByMode: !((state.settings ?? createSettingsV2()).general.developer_mode ?? false),
      },
    ];
    if (!q) return [];
    if (q.includes('creativity') || q.includes('temperature')) return [rows[0]];
    if (q.includes('openai')) return [rows[1]];
    return [];
  }),
}));

import { settingsV2Store } from '$lib/stores/settings-v2';
import { settingsSearchStore } from '$lib/stores/settings-search';

describe('settings-search store', () => {
  beforeEach(async () => {
    vi.useFakeTimers();
    clearStorage();
    state.settings = createSettingsV2();
    state.settings.general.developer_mode = false;
    state.settings.general.search_history_enabled = true;
    await settingsV2Store.load();
    settingsSearchStore.clear();
    settingsSearchStore.clearHistory();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('finds settings by alias (creativity -> temperature)', () => {
    settingsSearchStore.setQuery('creativity');
    vi.advanceTimersByTime(200);
    return Promise.resolve().then(() => {
      const results = get(settingsSearchStore.results);
      expect(results.some((row) => row.id === 'chat_presets.temperature')).toBe(true);
    });
  });

  it('marks developer-only results as hidden when developer mode is off', () => {
    settingsSearchStore.setQuery('openai');
    vi.advanceTimersByTime(200);
    return Promise.resolve().then(() => {
      const row = get(settingsSearchStore.results).find((item) => item.id === 'developer.openai_server');
      expect(row).toBeDefined();
      expect(row?.hiddenByMode).toBe(true);
    });
  });

  it('reveals developer-only results when developer mode is on', async () => {
    const snapshot = settingsV2Store.getSnapshot();
    expect(snapshot).not.toBeNull();
    if (!snapshot) return;

    await settingsV2Store.updateSection('general', {
      ...snapshot.general,
      developer_mode: true,
    });

    settingsSearchStore.setQuery('openai');
    vi.advanceTimersByTime(200);
    await Promise.resolve();
    const row = get(settingsSearchStore.results).find((item) => item.id === 'developer.openai_server');
    expect(row).toBeDefined();
    expect(row?.hiddenByMode).toBe(false);
  });

  it('stores and clears local search history', () => {
    settingsSearchStore.pushHistory('temperature', true);
    settingsSearchStore.pushHistory('vram', true);

    const history = get(settingsSearchStore.searchHistory);
    expect(history[0]).toBe('vram');
    expect(history).toContain('temperature');
    expect(localStorage.getItem('settings.search.history.v2')).toContain('vram');

    settingsSearchStore.clearHistory();
    expect(get(settingsSearchStore.searchHistory)).toEqual([]);
    expect(localStorage.getItem('settings.search.history.v2')).toBe('[]');
  });

  it('keeps hardware registry entries mapped to hardware section', () => {
    const splitEntry = settingsSearchStore.registry.find(
      (item) => item.id === 'performance.hardware.split_gpus',
    );
    const batchEntry = settingsSearchStore.registry.find(
      (item) => item.id === 'performance.hardware.batch_size',
    );
    const memoryModeEntry = settingsSearchStore.registry.find(
      (item) => item.id === 'performance.hardware.memory_mode',
    );

    expect(splitEntry?.section).toBe('hardware');
    expect(batchEntry?.section).toBe('hardware');
    expect(memoryModeEntry?.section).toBe('hardware');
  });
});
