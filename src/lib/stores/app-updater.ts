import { get, writable } from 'svelte/store';
import {
  checkForAppUpdate,
  downloadAndInstallUpdate as installUpdate,
  getUpdateCheckIntervalMs,
  isAutoUpdaterDisabledByEnv,
  type AppUpdateHandle,
  type AppUpdateInfo,
  type AppUpdateProgressEvent,
} from '$lib/services/app-updater';
import { getAppSettingsV2 } from '$lib/services/settings-v2';

export interface AppUpdaterState {
  checking: boolean;
  isUpdateAvailable: boolean;
  updateInfo: AppUpdateInfo | null;
  isDownloading: boolean;
  downloadProgress: number;
  downloadedBytes: number;
  totalBytes: number;
  remindMeLater: boolean;
  error: string | null;
  lastCheckedAt: number | null;
  disabledByEnv: boolean;
}

interface CheckOptions {
  userInitiated?: boolean;
  resetRemind?: boolean;
  suppressErrors?: boolean;
}

export const AUTO_UPDATER_DISABLED_MESSAGE = 'Auto updater is disabled by build configuration';

const disabledByEnv = isAutoUpdaterDisabledByEnv();

const initialState: AppUpdaterState = {
  checking: false,
  isUpdateAvailable: false,
  updateInfo: null,
  isDownloading: false,
  downloadProgress: 0,
  downloadedBytes: 0,
  totalBytes: 0,
  remindMeLater: false,
  error: null,
  lastCheckedAt: null,
  disabledByEnv,
};

const store = writable<AppUpdaterState>(initialState);
let pendingUpdate: AppUpdateHandle | null = null;

function clearPendingUpdate() {
  if (!pendingUpdate) return;
  void pendingUpdate.update.close().catch(() => {});
  pendingUpdate = null;
}

async function isRuntimeAutoUpdateEnabled(): Promise<boolean> {
  try {
    const settings = await getAppSettingsV2();
    return settings.general.auto_update === true;
  } catch {
    return true;
  }
}

async function checkForUpdate(options: CheckOptions = {}): Promise<AppUpdateInfo | null> {
  const snapshot = get(store);
  if (snapshot.checking || snapshot.isDownloading) {
    return snapshot.updateInfo;
  }

  if (disabledByEnv) {
    store.update((state) => ({
      ...state,
      disabledByEnv: true,
      error: options.userInitiated ? AUTO_UPDATER_DISABLED_MESSAGE : state.error,
    }));
    return null;
  }

  if (!options.userInitiated) {
    const enabled = await isRuntimeAutoUpdateEnabled();
    if (!enabled) return null;
  }

  store.update((state) => ({
    ...state,
    checking: true,
    error: null,
    remindMeLater: options.resetRemind ? false : state.remindMeLater,
  }));

  try {
    const checkedAt = Date.now();
    const result = await checkForAppUpdate();

    if (!result) {
      clearPendingUpdate();
      store.update((state) => ({
        ...state,
        checking: false,
        isUpdateAvailable: false,
        updateInfo: null,
        lastCheckedAt: checkedAt,
      }));
      return null;
    }

    clearPendingUpdate();
    pendingUpdate = result;

    store.update((state) => ({
      ...state,
      checking: false,
      isUpdateAvailable: true,
      updateInfo: result.info,
      remindMeLater: false,
      lastCheckedAt: checkedAt,
    }));

    return result.info;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    store.update((state) => ({
      ...state,
      checking: false,
      error: message,
      lastCheckedAt: Date.now(),
    }));

    if (options.userInitiated && !options.suppressErrors) {
      throw new Error(message);
    }
    return null;
  }
}

function applyProgress(event: AppUpdateProgressEvent) {
  store.update((state) => {
    if (event.event === 'Started') {
      return {
        ...state,
        totalBytes: event.data.contentLength ?? 0,
        downloadedBytes: 0,
        downloadProgress: 0,
      };
    }
    if (event.event === 'Progress') {
      const downloadedBytes = state.downloadedBytes + event.data.chunkLength;
      const totalBytes = state.totalBytes;
      const progress = totalBytes > 0 ? Math.min(1, downloadedBytes / totalBytes) : 0;
      return {
        ...state,
        downloadedBytes,
        downloadProgress: progress,
      };
    }
    return {
      ...state,
      downloadProgress: 1,
    };
  });
}

async function downloadAndInstallUpdate(): Promise<void> {
  const snapshot = get(store);
  if (snapshot.checking || snapshot.isDownloading) return;

  if (disabledByEnv) {
    throw new Error(AUTO_UPDATER_DISABLED_MESSAGE);
  }

  if (!pendingUpdate) {
    throw new Error('No update is available for installation');
  }

  store.update((state) => ({
    ...state,
    isDownloading: true,
    error: null,
    downloadProgress: 0,
    downloadedBytes: 0,
    totalBytes: 0,
  }));

  try {
    await installUpdate(pendingUpdate, applyProgress);
    clearPendingUpdate();
    store.update((state) => ({
      ...state,
      isDownloading: false,
      downloadProgress: 1,
      isUpdateAvailable: false,
      updateInfo: null,
      remindMeLater: true,
    }));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    store.update((state) => ({
      ...state,
      isDownloading: false,
      error: message,
    }));
    throw new Error(message);
  }
}

function setRemindMeLater(remind: boolean) {
  store.update((state) => ({
    ...state,
    remindMeLater: remind,
  }));
}

function reset() {
  clearPendingUpdate();
  store.set(initialState);
}

export const appUpdaterStore = {
  subscribe: store.subscribe,
  checkForUpdate,
  downloadAndInstallUpdate,
  setRemindMeLater,
  reset,
};

export { getUpdateCheckIntervalMs };
