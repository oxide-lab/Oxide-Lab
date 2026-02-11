import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { createSettingsV2 } from '../../tests/fixtures/settings-v2';

const mocks = vi.hoisted(() => ({
  checkForAppUpdate: vi.fn(),
  downloadAndInstallUpdate: vi.fn(),
  getAppSettingsV2: vi.fn(),
}));

vi.mock('$lib/services/app-updater', async (importOriginal) => {
  const actual = await importOriginal<typeof import('$lib/services/app-updater')>();
  return {
    ...actual,
    checkForAppUpdate: mocks.checkForAppUpdate,
    downloadAndInstallUpdate: mocks.downloadAndInstallUpdate,
  };
});

vi.mock('$lib/services/settings-v2', () => ({
  getAppSettingsV2: mocks.getAppSettingsV2,
}));

async function loadStore() {
  const mod = await import('$lib/stores/app-updater');
  return mod.appUpdaterStore;
}

describe('app-updater store', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.unstubAllEnvs();
    vi.stubEnv('VITE_AUTO_UPDATER_DISABLED', 'false');
    vi.stubEnv('VITE_UPDATE_CHECK_INTERVAL_MS', '3600000');
    mocks.checkForAppUpdate.mockReset();
    mocks.downloadAndInstallUpdate.mockReset();
    mocks.getAppSettingsV2.mockReset();
    const settings = createSettingsV2();
    settings.general.auto_update = true;
    mocks.getAppSettingsV2.mockResolvedValue(settings);
  });

  it('skips automatic check when runtime auto_update is disabled', async () => {
    const settings = createSettingsV2();
    settings.general.auto_update = false;
    mocks.getAppSettingsV2.mockResolvedValue(settings);
    const store = await loadStore();

    const result = await store.checkForUpdate();

    expect(result).toBeNull();
    expect(mocks.checkForAppUpdate).not.toHaveBeenCalled();
    expect(get(store).checking).toBe(false);
  });

  it('runs manual check and stores update info', async () => {
    const store = await loadStore();
    mocks.checkForAppUpdate.mockResolvedValue({
      info: {
        version: '0.16.0',
        date: '2026-02-11',
        body: 'New features',
      },
      update: {
        close: vi.fn(async () => {}),
      },
    });

    const result = await store.checkForUpdate({ userInitiated: true, resetRemind: true });
    const state = get(store);

    expect(result?.version).toBe('0.16.0');
    expect(state.isUpdateAvailable).toBe(true);
    expect(state.updateInfo?.version).toBe('0.16.0');
    expect(state.remindMeLater).toBe(false);
  });

  it('marks no update when check returns null', async () => {
    const store = await loadStore();
    mocks.checkForAppUpdate.mockResolvedValue(null);

    await store.checkForUpdate({ userInitiated: true });
    const state = get(store);

    expect(state.isUpdateAvailable).toBe(false);
    expect(state.updateInfo).toBeNull();
  });

  it('throws on manual check error and stores message', async () => {
    const store = await loadStore();
    mocks.checkForAppUpdate.mockRejectedValue(new Error('network down'));

    await expect(store.checkForUpdate({ userInitiated: true })).rejects.toThrow('network down');
    expect(get(store).error).toContain('network down');
  });

  it('downloads and updates progress', async () => {
    const store = await loadStore();
    mocks.checkForAppUpdate.mockResolvedValue({
      info: {
        version: '0.16.0',
        date: '2026-02-11',
        body: 'New features',
      },
      update: {
        close: vi.fn(async () => {}),
      },
    });
    mocks.downloadAndInstallUpdate.mockImplementation(async (_update, onProgress) => {
      onProgress?.({ event: 'Started', data: { contentLength: 100 } });
      onProgress?.({ event: 'Progress', data: { chunkLength: 30 } });
      onProgress?.({ event: 'Progress', data: { chunkLength: 70 } });
      onProgress?.({ event: 'Finished' });
    });

    await store.checkForUpdate({ userInitiated: true });
    await store.downloadAndInstallUpdate();
    const state = get(store);

    expect(mocks.downloadAndInstallUpdate).toHaveBeenCalledOnce();
    expect(state.isDownloading).toBe(false);
    expect(state.downloadProgress).toBe(1);
    expect(state.downloadedBytes).toBe(100);
    expect(state.totalBytes).toBe(100);
  });

  it('blocks check and install when disabled by env', async () => {
    vi.resetModules();
    vi.stubEnv('VITE_AUTO_UPDATER_DISABLED', 'true');
    const store = await loadStore();

    const result = await store.checkForUpdate({ userInitiated: true });

    expect(result).toBeNull();
    expect(mocks.checkForAppUpdate).not.toHaveBeenCalled();
    await expect(store.downloadAndInstallUpdate()).rejects.toThrow(/disabled/i);
  });

  it('updates remind me later flag', async () => {
    const store = await loadStore();

    store.setRemindMeLater(true);
    expect(get(store).remindMeLater).toBe(true);
    store.setRemindMeLater(false);
    expect(get(store).remindMeLater).toBe(false);
  });
});
