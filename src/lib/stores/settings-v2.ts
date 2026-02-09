import { derived, get, writable } from 'svelte/store';
import type {
  AppSettingsPatch,
  AppSettingsV2,
  OpenAiServerConfig,
  SettingsApplyResult,
  SettingsSectionId,
  SettingsScope,
} from '$lib/types/settings-v2';
import {
  getAppSettingsV2,
  getOpenAiServerStatus,
  patchAppSettingsV2,
  resetAppSettingsV2,
  setOpenAiServerConfig,
} from '$lib/services/settings-v2';

const settings = writable<AppSettingsV2 | null>(null);
const sessionBaseline = writable<AppSettingsV2 | null>(null);
const loading = writable(false);
const error = writable<string | null>(null);
const warnings = writable<string[]>([]);
const openAiStatus = writable<Awaited<ReturnType<typeof getOpenAiServerStatus>> | null>(null);
const HARDWARE_ENV_KEYS = new Set([
  'CUDA_VISIBLE_DEVICES',
  'HIP_VISIBLE_DEVICES',
  'OXIDE_SELECTED_GPU_UUID',
  'OXIDE_MEMORY_MAPPING',
  'OXIDE_GPU_SPLIT',
]);

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function countDiffs(left: unknown, right: unknown): number {
  if (Array.isArray(left) && Array.isArray(right)) {
    if (left.length !== right.length) return 1;
    return left.reduce((acc, item, index) => acc + countDiffs(item, right[index]), 0);
  }
  if (isObject(left) && isObject(right)) {
    const keys = new Set([...Object.keys(left), ...Object.keys(right)]);
    let count = 0;
    for (const key of keys) {
      count += countDiffs(left[key], right[key]);
    }
    return count;
  }
  return Object.is(left, right) ? 0 : 1;
}

function filterExtraEnv(
  env: Record<string, string>,
  predicate: (key: string) => boolean,
): Record<string, string> {
  const entries = Object.entries(env).filter(([key]) => predicate(key));
  return Object.fromEntries(entries);
}

function projectHardwareSettings(performance: AppSettingsV2['performance']) {
  const runtime = performance.llama_runtime;
  const extraEnv = runtime.extra_env ?? {};
  return {
    memory_mode: performance.memory_mode,
    llama_runtime: {
      n_gpu_layers: runtime.n_gpu_layers,
      threads: runtime.threads,
      batch_size: runtime.batch_size,
      extra_env: filterExtraEnv(extraEnv, (key) => HARDWARE_ENV_KEYS.has(key)),
    },
  };
}

function projectRuntimeSettings(performance: AppSettingsV2['performance']) {
  const runtime = performance.llama_runtime;
  const extraEnv = runtime.extra_env ?? {};
  const runtimeWithoutHardware = Object.fromEntries(
    Object.entries(runtime).filter(
      ([key]) => key !== 'n_gpu_layers' && key !== 'threads' && key !== 'batch_size' && key !== 'extra_env',
    ),
  );
  return {
    manual_thread_limit: performance.manual_thread_limit,
    llama_runtime: {
      ...runtimeWithoutHardware,
      extra_env: filterExtraEnv(extraEnv, (key) => !HARDWARE_ENV_KEYS.has(key)),
    },
  };
}

async function runFrontendMigration(snapshot: AppSettingsV2) {
  const migrationKey = 'settings_v2_frontend_migrated';
  if (localStorage.getItem(migrationKey) === '1') {
    return;
  }

  const patch: AppSettingsPatch = {};
  const modelSearch = localStorage.getItem('ui.modelSelectorSearch');
  if (modelSearch === 'true' || modelSearch === 'false') {
    patch.models_storage = {
      ...snapshot.models_storage,
      model_selector_search: modelSearch === 'true',
    };
  }

  const localModelsPath = localStorage.getItem('local_models_folder_path');
  if (localModelsPath) {
    patch.models_storage = {
      ...(patch.models_storage ?? snapshot.models_storage),
      models_dir: localModelsPath,
    };
  }

  if (patch.models_storage) {
    const result = await patchAppSettingsV2(patch);
    settings.set(result.settings);
    warnings.set(result.warnings);
  }

  localStorage.removeItem('ui.modelSelectorSearch');
  localStorage.removeItem('local_models_folder_path');
  localStorage.setItem(migrationKey, '1');
}

async function refreshOpenAiStatus() {
  try {
    openAiStatus.set(await getOpenAiServerStatus());
  } catch (e) {
    console.warn('Failed to get OpenAI server status', e);
  }
}

async function load() {
  loading.set(true);
  error.set(null);
  try {
    const snapshot = await getAppSettingsV2();
    settings.set(snapshot);
    sessionBaseline.set(structuredClone(snapshot));
    warnings.set([]);
    await runFrontendMigration(snapshot);
    await refreshOpenAiStatus();
  } catch (e) {
    error.set(String(e));
  } finally {
    loading.set(false);
  }
}

async function patch(patchData: AppSettingsPatch): Promise<SettingsApplyResult> {
  const result = await patchAppSettingsV2(patchData);
  settings.set(result.settings);
  warnings.set(result.warnings);
  await refreshOpenAiStatus();
  return result;
}

async function updateSection<K extends keyof AppSettingsPatch>(
  key: K,
  value: NonNullable<AppSettingsPatch[K]>,
) {
  await patch({ [key]: value } as AppSettingsPatch);
}

async function updateOpenAiConfig(config: OpenAiServerConfig) {
  const result = await setOpenAiServerConfig(config);
  settings.set(result.settings);
  warnings.set(result.warnings);
  await refreshOpenAiStatus();
  return result;
}

async function reset(scope?: SettingsScope) {
  const next = await resetAppSettingsV2(scope);
  settings.set(next);
  warnings.set([]);
  await refreshOpenAiStatus();
}

const dirtyBySection = derived([settings, sessionBaseline], ([$settings, $baseline]) => {
  if (!$settings || !$baseline) {
    return {
      general: 0,
      models_storage: 0,
      performance: 0,
      hardware: 0,
      chat_presets: 0,
      web_rag: 0,
      privacy_data: 0,
      developer: 0,
      about: 0,
    } satisfies Record<SettingsSectionId, number>;
  }

  return {
    general: countDiffs($settings.general, $baseline.general),
    models_storage: countDiffs($settings.models_storage, $baseline.models_storage),
    performance: countDiffs(
      projectRuntimeSettings($settings.performance),
      projectRuntimeSettings($baseline.performance),
    ),
    hardware: countDiffs(
      projectHardwareSettings($settings.performance),
      projectHardwareSettings($baseline.performance),
    ),
    chat_presets: countDiffs($settings.chat_presets, $baseline.chat_presets),
    web_rag: countDiffs($settings.web_rag, $baseline.web_rag),
    privacy_data: countDiffs($settings.privacy_data, $baseline.privacy_data),
    developer: countDiffs($settings.developer, $baseline.developer),
    about: 0,
  } satisfies Record<SettingsSectionId, number>;
});

const expertMode = derived(settings, ($settings) => $settings?.general.expert_mode ?? false);
const developerMode = derived(settings, ($settings) => $settings?.general.developer_mode ?? false);

const hasDirtyChanges = derived(dirtyBySection, ($dirty) =>
  Object.values($dirty).some((count) => count > 0),
);

export const settingsV2Store = {
  subscribe: settings.subscribe,
  loading: { subscribe: loading.subscribe },
  error: { subscribe: error.subscribe },
  warnings: { subscribe: warnings.subscribe },
  openAiStatus: { subscribe: openAiStatus.subscribe },
  dirtyBySection,
  expertMode,
  developerMode,
  hasDirtyChanges,
  load,
  patch,
  updateSection,
  updateOpenAiConfig,
  reset,
  getSnapshot: () => get(settings),
};
