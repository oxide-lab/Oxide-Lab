import { beforeEach, describe, expect, it, vi } from 'vitest';
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
  if (patch.web_rag) state.settings.web_rag = patch.web_rag;
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
}));

import { settingsV2Store } from '$lib/stores/settings-v2';

describe('settings-v2 store', () => {
  beforeEach(async () => {
    clearStorage();
    state.settings = createSettingsV2();
    await settingsV2Store.load();
  });

  it('tracks dirty counts by section after updates', async () => {
    expect(get(settingsV2Store.dirtyBySection).general).toBe(0);

    const snapshot = settingsV2Store.getSnapshot();
    expect(snapshot).not.toBeNull();
    if (!snapshot) return;

    await settingsV2Store.updateSection('general', {
      ...snapshot.general,
      expert_mode: true,
    });

    const dirty = get(settingsV2Store.dirtyBySection);
    expect(dirty.general).toBeGreaterThan(0);
    expect(get(settingsV2Store.hasDirtyChanges)).toBe(true);
  });

  it('applies frontend one-time migration from localStorage keys', async () => {
    localStorage.setItem('ui.modelSelectorSearch', 'false');
    localStorage.setItem('local_models_folder_path', 'D:\\Models');
    localStorage.removeItem('settings_v2_frontend_migrated');

    state.settings = createSettingsV2();
    await settingsV2Store.load();

    const snapshot = settingsV2Store.getSnapshot();
    expect(snapshot?.models_storage.model_selector_search).toBe(false);
    expect(snapshot?.models_storage.models_dir).toBe('D:\\Models');
    expect(localStorage.getItem('settings_v2_frontend_migrated')).toBe('1');
    expect(localStorage.getItem('local_models_folder_path')).toBeNull();
  });

  it('tracks hardware-only changes separately from runtime changes', async () => {
    const snapshot = settingsV2Store.getSnapshot();
    expect(snapshot).not.toBeNull();
    if (!snapshot) return;

    await settingsV2Store.updateSection('performance', {
      ...snapshot.performance,
      llama_runtime: {
        ...snapshot.performance.llama_runtime,
        n_gpu_layers: snapshot.performance.llama_runtime.n_gpu_layers + 1,
      },
    });

    const dirty = get(settingsV2Store.dirtyBySection);
    expect(dirty.hardware).toBeGreaterThan(0);
    expect(dirty.performance).toBe(0);
  });

  it('tracks runtime-only changes separately from hardware changes', async () => {
    const snapshot = settingsV2Store.getSnapshot();
    expect(snapshot).not.toBeNull();
    if (!snapshot) return;

    await settingsV2Store.updateSection('performance', {
      ...snapshot.performance,
      llama_runtime: {
        ...snapshot.performance.llama_runtime,
        threads_batch: snapshot.performance.llama_runtime.threads_batch + 1,
      },
    });

    const dirty = get(settingsV2Store.dirtyBySection);
    expect(dirty.performance).toBeGreaterThan(0);
    expect(dirty.hardware).toBe(0);
  });
});
