import { invoke } from '@tauri-apps/api/core';
import type {
  AppSettingsPatch,
  AppSettingsV2,
  ClearDataResult,
  ClearDataScope,
  DataLocations,
  ExportResult,
  OpenAiServerConfig,
  OpenAiServerStatus,
  SettingsApplyResult,
  SettingsSearchResult,
  SettingsScope,
} from '$lib/types/settings-v2';

const CLEAR_DATA_CONFIRM_TOKEN = 'CONFIRM_CLEAR_DATA';

export async function getAppSettingsV2(): Promise<AppSettingsV2> {
  return await invoke<AppSettingsV2>('get_app_settings_v2');
}

export async function patchAppSettingsV2(patch: AppSettingsPatch): Promise<SettingsApplyResult> {
  return await invoke<SettingsApplyResult>('patch_app_settings_v2', { patch });
}

export async function resetAppSettingsV2(scope?: SettingsScope): Promise<AppSettingsV2> {
  return await invoke<AppSettingsV2>('reset_app_settings_v2', { scope });
}

export async function getDataLocations(): Promise<DataLocations> {
  return await invoke<DataLocations>('get_data_locations');
}

export async function exportUserData(targetPath: string): Promise<ExportResult> {
  return await invoke<ExportResult>('export_user_data', { targetPath });
}

export async function clearUserData(scope: ClearDataScope): Promise<ClearDataResult> {
  return await invoke<ClearDataResult>('clear_user_data', {
    scope,
    confirmToken: CLEAR_DATA_CONFIRM_TOKEN,
  });
}

export async function getOpenAiServerStatus(): Promise<OpenAiServerStatus> {
  return await invoke<OpenAiServerStatus>('get_openai_server_status');
}

export async function setOpenAiServerConfig(
  config: OpenAiServerConfig,
): Promise<SettingsApplyResult> {
  return await invoke<SettingsApplyResult>('set_openai_server_config', { config });
}

export async function restartOpenAiServer(): Promise<OpenAiServerStatus> {
  return await invoke<OpenAiServerStatus>('restart_openai_server');
}

export async function sha256Base64NoPad(raw: string): Promise<string> {
  const bytes = new TextEncoder().encode(raw);
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  const arr = Array.from(new Uint8Array(digest));
  const base64 = btoa(String.fromCharCode(...arr));
  return base64.replace(/=+$/g, '');
}

export async function searchSettingsV2(query: string): Promise<SettingsSearchResult[]> {
  return await invoke<SettingsSearchResult[]>('search_settings_v2', { query });
}
