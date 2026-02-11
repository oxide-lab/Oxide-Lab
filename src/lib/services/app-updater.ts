import { check, type DownloadEvent, type Update } from '@tauri-apps/plugin-updater';

export interface AppUpdateInfo {
  version: string;
  date?: string;
  body?: string;
}

export interface AppUpdateHandle {
  info: AppUpdateInfo;
  update: Update;
}

export type AppUpdateProgressEvent = DownloadEvent;

const DEFAULT_UPDATE_CHECK_INTERVAL_MS = 60 * 60 * 1000;

export function isAutoUpdaterDisabledByEnv(): boolean {
  return String(import.meta.env.VITE_AUTO_UPDATER_DISABLED ?? 'false').toLowerCase() === 'true';
}

export function getUpdateCheckIntervalMs(): number {
  const raw = Number(import.meta.env.VITE_UPDATE_CHECK_INTERVAL_MS ?? DEFAULT_UPDATE_CHECK_INTERVAL_MS);
  if (!Number.isFinite(raw) || raw <= 0) return DEFAULT_UPDATE_CHECK_INTERVAL_MS;
  return Math.floor(raw);
}

export async function checkForAppUpdate(): Promise<AppUpdateHandle | null> {
  const update = await check();
  if (!update) return null;

  return {
    update,
    info: {
      version: update.version,
      date: update.date,
      body: update.body,
    },
  };
}

export async function downloadAndInstallUpdate(
  handle: AppUpdateHandle,
  onProgress?: (event: AppUpdateProgressEvent) => void,
): Promise<void> {
  await handle.update.downloadAndInstall((event) => {
    onProgress?.(event);
  });
}
