import type { AppSettingsV2, OpenAiServerStatus } from '$lib/types/settings-v2';

export function createSettingsV2(overrides?: Partial<AppSettingsV2>): AppSettingsV2 {
  const base: AppSettingsV2 = {
    schema_version: 2,
    general: {
      locale: 'en',
      theme: 'system',
      auto_update: true,
      launch_on_startup: false,
      expert_mode: false,
      developer_mode: false,
      search_history_enabled: false,
    },
    models_storage: {
      models_dir: null,
      cache_dir: null,
      model_selector_search: true,
    },
    performance: {
      manual_thread_limit: null,
      memory_mode: 'medium',
      llama_runtime: {
        server_path: null,
        selected_backend: null,
        n_gpu_layers: 50,
        threads: 0,
        threads_batch: 0,
        ctx_size: 4096,
        batch_size: 512,
        ubatch_size: 512,
        n_predict: 1024,
        flash_attn: 'auto',
        extra_env: {},
        embeddings_strategy: 'separate_session',
        scheduler: {
          keep_alive_secs: 300,
          max_loaded_models: 0,
          max_queue: 32,
          queue_wait_timeout_ms: 120000,
          vram_recovery_timeout_ms: 30000,
          vram_recovery_poll_ms: 500,
          vram_recovery_threshold: 0.95,
          expiration_tick_ms: 5000,
        },
      },
    },
    chat_presets: {
      default_preset_id: 'code',
      default_system_prompt: '',
      presets: [
        {
          id: 'code',
          name: 'Code',
          system_prompt: 'You are a coding assistant.',
          context: 8192,
          builtin: true,
          sampling: {
            temperature: 0.2,
            top_p: 0.9,
            top_k: 20,
            min_p: 0,
            repeat_penalty: 1.1,
            max_tokens: 1024,
            seed: null,
            stop_sequences: [],
          },
        },
      ],
    },
    privacy_data: {
      telemetry_enabled: false,
      crash_reports_enabled: false,
    },
    developer: {
      openai_server: {
        enabled: false,
        bind_host: '127.0.0.1',
        port: 11434,
        auth_required: false,
        api_keys_hashed: [],
        cors_mode: 'same_origin',
        cors_allowlist: [],
      },
    },
  };

  return {
    ...base,
    ...overrides,
    general: { ...base.general, ...(overrides?.general ?? {}) },
    models_storage: { ...base.models_storage, ...(overrides?.models_storage ?? {}) },
    performance: {
      ...base.performance,
      ...(overrides?.performance ?? {}),
      llama_runtime: {
        ...base.performance.llama_runtime,
        ...(overrides?.performance?.llama_runtime ?? {}),
      },
    },
    chat_presets: {
      ...base.chat_presets,
      ...(overrides?.chat_presets ?? {}),
      presets: overrides?.chat_presets?.presets ?? base.chat_presets.presets,
    },
    privacy_data: { ...base.privacy_data, ...(overrides?.privacy_data ?? {}) },
    developer: {
      ...base.developer,
      ...(overrides?.developer ?? {}),
      openai_server: {
        ...base.developer.openai_server,
        ...(overrides?.developer?.openai_server ?? {}),
      },
    },
  };
}

export function statusFromSettings(settings: AppSettingsV2): OpenAiServerStatus {
  const config = settings.developer.openai_server;
  return {
    running: config.enabled,
    enabled: config.enabled,
    bind_host: config.bind_host,
    port: config.port,
    endpoint: `http://${config.bind_host}:${config.port}/v1`,
    auth_required: config.auth_required,
    cors_mode: config.cors_mode,
    cors_allowlist: config.cors_allowlist,
    api_keys_count: config.api_keys_hashed.length,
    warnings: [],
  };
}
