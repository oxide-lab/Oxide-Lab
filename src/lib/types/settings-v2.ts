import type { LlamaRuntimeConfig } from '$lib/services/llama-backend-service';

export type CorsMode = 'same_origin' | 'allowlist' | 'any';

export interface OpenAiServerConfig {
  enabled: boolean;
  bind_host: '127.0.0.1' | '0.0.0.0';
  port: number;
  auth_required: boolean;
  api_keys_hashed: string[];
  cors_mode: CorsMode;
  cors_allowlist: string[];
}

export interface ChatSamplingSettings {
  temperature: number;
  top_p: number;
  top_k: number;
  min_p: number;
  repeat_penalty: number;
  max_tokens: number;
  seed: number | null;
  stop_sequences: string[];
}

export interface ChatPreset {
  id: string;
  name: string;
  system_prompt: string;
  sampling: ChatSamplingSettings;
  context: number;
  builtin: boolean;
}

export interface ChatPresetSettings {
  default_preset_id: string;
  presets: ChatPreset[];
  default_system_prompt: string;
}

export interface GeneralSettings {
  locale: 'en' | 'ru' | 'pt-BR';
  theme: 'light' | 'dark' | 'system';
  auto_update: boolean;
  launch_on_startup: boolean;
  expert_mode: boolean;
  developer_mode: boolean;
  search_history_enabled: boolean;
}

export interface ModelsStorageSettings {
  models_dir: string | null;
  cache_dir: string | null;
  model_selector_search: boolean;
}

export interface PerformanceSettings {
  manual_thread_limit: number | null;
  llama_runtime: LlamaRuntimeConfig;
  memory_mode: 'low' | 'medium' | 'high';
}

export interface PrivacyDataSettings {
  telemetry_enabled: boolean;
  crash_reports_enabled: boolean;
}

export interface DeveloperSettings {
  openai_server: OpenAiServerConfig;
}

export interface AppSettingsV2 {
  schema_version: number;
  general: GeneralSettings;
  models_storage: ModelsStorageSettings;
  performance: PerformanceSettings;
  chat_presets: ChatPresetSettings;
  privacy_data: PrivacyDataSettings;
  developer: DeveloperSettings;
}

export interface AppSettingsPatch {
  general?: GeneralSettings;
  models_storage?: ModelsStorageSettings;
  performance?: PerformanceSettings;
  chat_presets?: ChatPresetSettings;
  privacy_data?: PrivacyDataSettings;
  developer?: DeveloperSettings;
}

export type SettingsScope =
  | 'all'
  | 'general'
  | 'models_storage'
  | 'performance'
  | 'chat_presets'
  | 'privacy_data'
  | 'developer';

export interface SettingsApplyResult {
  applied: boolean;
  requires_restart: boolean;
  warnings: string[];
  settings: AppSettingsV2;
}

export interface DataLocations {
  app_data_dir: string;
  profile_dir: string;
  settings_file: string;
  settings_backup_file: string;
  chat_db: string;
  legacy_thread_limit_file: string;
  legacy_runtime_file: string;
  legacy_experimental_file: string;
}

export interface ExportResult {
  success: boolean;
  target_path: string;
  exported_files: string[];
}

export type ClearDataScope = 'all' | 'chats' | 'downloads' | 'settings';

export interface ClearDataResult {
  success: boolean;
  cleared_files: string[];
}

export interface OpenAiServerStatus {
  running: boolean;
  enabled: boolean;
  bind_host: string;
  port: number;
  endpoint: string;
  auth_required: boolean;
  cors_mode: CorsMode;
  cors_allowlist: string[];
  api_keys_count: number;
  warnings: string[];
}

export type SettingsSectionId =
  | 'general'
  | 'models_storage'
  | 'performance'
  | 'chat_presets'
  | 'privacy_data'
  | 'developer'
  | 'about';

export interface SettingsSearchItem {
  id: string;
  section: SettingsSectionId;
  title: string;
  description: string;
  keywords: string[];
  synonyms: string[];
  devOnly?: boolean;
  expertOnly?: boolean;
}

export interface SettingsSearchResult {
  id: string;
  section: SettingsSectionId;
  title: string;
  description: string;
  hiddenByMode: boolean;
}
