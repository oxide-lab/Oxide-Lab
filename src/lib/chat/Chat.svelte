<script lang="ts">
  /**
   * Chat Component
   *
   * Main chat interface using ai-elements components and chat controller.
   */
  import { onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import { invoke } from '@tauri-apps/api/core';
  import * as Sheet from '$lib/components/ui/sheet';
  import { PaneGroup, Pane, PaneResizer } from 'paneforge';
  import {
    Conversation,
    ConversationContent,
    ConversationScrollButton,
  } from '$lib/components/ai-elements/conversation';
  import {
    MessageList,
    Composer,
    LoaderPanel,
    McpPermissionModal,
    PreviewPanel,
  } from '$lib/chat/components';
  import type { ChatMessage, McpPermissionDecision } from '$lib/chat/types';
  import { createChatController } from '$lib/chat/controller';
  import { chatState, chatUiMounted, getDefaultChatState } from '$lib/stores/chat';
  import { currentSession } from '$lib/stores/chat-history';
  import { showChatHistory } from '$lib/stores/sidebar';
  import { htmlPreviewStore, isPreviewOpen } from '$lib/stores/html-preview';
  import { performanceService } from '$lib/services/performance-service';
  import { inferenceMetricsStore } from '$lib/stores/inference-metrics';
  import type { InferenceMetrics } from '$lib/types/performance';
  import { settingsV2Store } from '$lib/stores/settings-v2';
  import {
    clearMcpPendingPermission,
    mcpPendingPermission,
  } from '$lib/stores/mcp-tooling';
  import type { ChatPreset } from '$lib/types/settings-v2';


  // State
  // Initial State from Store (Synchronous)
  const savedState = get(chatState);

  let isLoaderPanelVisible = $state(false);
  let modelPath = $state(savedState.modelPath);
  let repoId = $state<string>(savedState.repoId);
  let revision = $state<string>(savedState.revision);
  let hubGgufFilename = $state<string>(savedState.hubGgufFilename);
  let mmprojPath = $state<string>(savedState.mmprojPath ?? '');
  let prompt = $state(savedState.prompt);
  let messages = $state<ChatMessage[]>(Array.isArray(savedState.messages) ? savedState.messages : []);
  let busy = $state(savedState.busy);
  let format = $state<'gguf' | 'hub_gguf'>(savedState.format);
  let pendingModelPath = $state(savedState.pendingModelPath);
  let pendingFormat = $state<'gguf' | 'hub_gguf'>(savedState.pendingFormat);
  let isLoaded = $state(savedState.isLoaded);
  let errorText = $state(savedState.errorText);

  // Device state
  let use_gpu = $state<boolean>(savedState.use_gpu);
  let cuda_available = $state<boolean>(savedState.cuda_available);
  let cuda_build = $state<boolean>(savedState.cuda_build);
  let current_device = $state<string>(savedState.current_device);
  let avx = $state<boolean>(savedState.avx);
  let neon = $state<boolean>(savedState.neon);
  let simd128 = $state<boolean>(savedState.simd128);
  let f16c = $state<boolean>(savedState.f16c);

  // Modality support
  let supports_text = $state<boolean>(true);
  let supports_image = $state<boolean>(false);
  let supports_audio = $state<boolean>(false);
  let supports_video = $state<boolean>(false);

  // Loading state
  let isLoadingModel = $state(savedState.isLoadingModel);
  let loadingProgress = $state(savedState.loadingProgress);
  let loadingStage = $state(savedState.loadingStage);
  let isCancelling = $state(savedState.isCancelling);
  let isUnloadingModel = $state(savedState.isUnloadingModel);
  let unloadingProgress = $state(savedState.unloadingProgress);

  // Inference params
  let temperature = $state<number>(savedState.temperature);
  let temperature_enabled = $state(savedState.temperature_enabled);
  let top_k_enabled = $state(savedState.top_k_enabled);
  let top_k_value = $state<number>(savedState.top_k_value);
  let top_p_enabled = $state(savedState.top_p_enabled);
  let top_p_value = $state<number>(savedState.top_p_value);
  let min_p_enabled = $state(savedState.min_p_enabled);
  let min_p_value = $state<number>(savedState.min_p_value);
  let repeat_penalty_enabled = $state(savedState.repeat_penalty_enabled);
  let repeat_penalty_value = $state<number>(savedState.repeat_penalty_value);
  let max_new_tokens_enabled = $state(savedState.max_new_tokens_enabled ?? false);
  let max_new_tokens_value = $state<number>(savedState.max_new_tokens_value ?? 1024);
  let seed_enabled = $state(savedState.seed_enabled ?? false);
  let seed_value = $state<number>(savedState.seed_value ?? 42);
  let stop_sequences_text = $state(savedState.stop_sequences_text ?? '');
  let reasoning_parse_enabled = $state(savedState.reasoning_parse_enabled ?? true);
  let reasoning_start_tag = $state(savedState.reasoning_start_tag ?? '<think>');
  let reasoning_end_tag = $state(savedState.reasoning_end_tag ?? '</think>');
  let structured_output_enabled = $state(savedState.structured_output_enabled ?? false);
  let ctx_limit_value = $state<number>(savedState.ctx_limit_value);
  let use_custom_params = $state<boolean>(savedState.use_custom_params);
  let split_prompt = $state<boolean>(savedState.split_prompt);
  let verbose_prompt = $state<boolean>(savedState.verbose_prompt);
  let tracing = $state<boolean>(savedState.tracing);
  let retrieval_url_enabled = $state<boolean>(savedState.retrieval_url_enabled ?? false);
  let retrieval_urls = $state<string[]>(savedState.retrieval_urls ?? []);
  let retrieval_local_enabled = $state<boolean>(savedState.retrieval_local_enabled ?? false);
  let mcp_enabled = $state<boolean>(savedState.mcp_enabled ?? false);
  let preset_id = $state<string | null>(savedState.preset_id ?? null);

  // Create controller with context
  const controller = createChatController({
    get modelPath() {
      return modelPath;
    },
    set modelPath(v) {
      modelPath = v;
    },
    get format() {
      return format;
    },
    set format(v) {
      format = v;
    },
    get repoId() {
      return repoId;
    },
    set repoId(v) {
      repoId = v;
    },
    get revision() {
      return revision;
    },
    set revision(v) {
      revision = v;
    },
    get hubGgufFilename() {
      return hubGgufFilename;
    },
    set hubGgufFilename(v) {
      hubGgufFilename = v;
    },
    get mmprojPath() {
      return mmprojPath;
    },
    set mmprojPath(v) {
      mmprojPath = v;
    },
    get prompt() {
      return prompt;
    },
    set prompt(v) {
      prompt = v;
    },
    get messages() {
      return messages;
    },
    set messages(v) {
      messages = v;
    },
    get busy() {
      return busy;
    },
    set busy(v) {
      busy = v;
    },
    get isLoaded() {
      return isLoaded;
    },
    set isLoaded(v) {
      isLoaded = v;
    },
    get errorText() {
      return errorText;
    },
    set errorText(v) {
      errorText = v;
    },
    get isLoadingModel() {
      return isLoadingModel;
    },
    set isLoadingModel(v) {
      isLoadingModel = v;
    },
    get loadingProgress() {
      return loadingProgress;
    },
    set loadingProgress(v) {
      loadingProgress = v;
    },
    get loadingStage() {
      return loadingStage;
    },
    set loadingStage(v) {
      loadingStage = v;
    },
    get isCancelling() {
      return isCancelling;
    },
    set isCancelling(v) {
      isCancelling = v;
    },
    get isUnloadingModel() {
      return isUnloadingModel;
    },
    set isUnloadingModel(v) {
      isUnloadingModel = v;
    },
    get unloadingProgress() {
      return unloadingProgress;
    },
    set unloadingProgress(v) {
      unloadingProgress = v;
    },
    get temperature() {
      return temperature;
    },
    set temperature(v) {
      temperature = v;
    },
    get temperature_enabled() {
      return temperature_enabled;
    },
    set temperature_enabled(v) {
      temperature_enabled = v;
    },
    get top_k_enabled() {
      return top_k_enabled;
    },
    set top_k_enabled(v) {
      top_k_enabled = v;
    },
    get top_k_value() {
      return top_k_value;
    },
    set top_k_value(v) {
      top_k_value = v;
    },
    get top_p_enabled() {
      return top_p_enabled;
    },
    set top_p_enabled(v) {
      top_p_enabled = v;
    },
    get top_p_value() {
      return top_p_value;
    },
    set top_p_value(v) {
      top_p_value = v;
    },
    get min_p_enabled() {
      return min_p_enabled;
    },
    set min_p_enabled(v) {
      min_p_enabled = v;
    },
    get min_p_value() {
      return min_p_value;
    },
    set min_p_value(v) {
      min_p_value = v;
    },
    get repeat_penalty_enabled() {
      return repeat_penalty_enabled;
    },
    set repeat_penalty_enabled(v) {
      repeat_penalty_enabled = v;
    },
    get repeat_penalty_value() {
      return repeat_penalty_value;
    },
    set repeat_penalty_value(v) {
      repeat_penalty_value = v;
    },
    get max_new_tokens_enabled() {
      return max_new_tokens_enabled;
    },
    set max_new_tokens_enabled(v) {
      max_new_tokens_enabled = v;
    },
    get max_new_tokens_value() {
      return max_new_tokens_value;
    },
    set max_new_tokens_value(v) {
      max_new_tokens_value = v;
    },
    get seed_enabled() {
      return seed_enabled;
    },
    set seed_enabled(v) {
      seed_enabled = v;
    },
    get seed_value() {
      return seed_value;
    },
    set seed_value(v) {
      seed_value = v;
    },
    get stop_sequences_text() {
      return stop_sequences_text;
    },
    set stop_sequences_text(v) {
      stop_sequences_text = v;
    },
    get reasoning_parse_enabled() {
      return reasoning_parse_enabled;
    },
    set reasoning_parse_enabled(v) {
      reasoning_parse_enabled = v;
    },
    get reasoning_start_tag() {
      return reasoning_start_tag;
    },
    set reasoning_start_tag(v) {
      reasoning_start_tag = v;
    },
    get reasoning_end_tag() {
      return reasoning_end_tag;
    },
    set reasoning_end_tag(v) {
      reasoning_end_tag = v;
    },
    get structured_output_enabled() {
      return structured_output_enabled;
    },
    set structured_output_enabled(v) {
      structured_output_enabled = v;
    },
    get ctx_limit_value() {
      return ctx_limit_value;
    },
    set ctx_limit_value(v) {
      ctx_limit_value = v;
    },
    get use_custom_params() {
      return use_custom_params;
    },
    set use_custom_params(v) {
      use_custom_params = v;
    },
    get use_gpu() {
      return use_gpu;
    },
    set use_gpu(v) {
      use_gpu = v;
    },
    get cuda_available() {
      return cuda_available;
    },
    set cuda_available(v) {
      cuda_available = v;
    },
    get cuda_build() {
      return cuda_build;
    },
    set cuda_build(v) {
      cuda_build = v;
    },
    get current_device() {
      return current_device;
    },
    set current_device(v) {
      current_device = v;
    },
    get avx() {
      return avx;
    },
    set avx(v) {
      avx = v;
    },
    get neon() {
      return neon;
    },
    set neon(v) {
      neon = v;
    },
    get simd128() {
      return simd128;
    },
    set simd128(v) {
      simd128 = v;
    },
    get f16c() {
      return f16c;
    },
    set f16c(v) {
      f16c = v;
    },
    get supports_text() {
      return supports_text;
    },
    set supports_text(v) {
      supports_text = v;
    },
    get supports_image() {
      return supports_image;
    },
    set supports_image(v) {
      supports_image = v;
    },
    get supports_audio() {
      return supports_audio;
    },
    set supports_audio(v) {
      supports_audio = v;
    },
    get supports_video() {
      return supports_video;
    },
    set supports_video(v) {
      supports_video = v;
    },
    get split_prompt() {
      return split_prompt;
    },
    set split_prompt(v) {
      split_prompt = v;
    },
    get verbose_prompt() {
      return verbose_prompt;
    },
    set verbose_prompt(v) {
      verbose_prompt = v;
    },
    get tracing() {
      return tracing;
    },
    set tracing(v) {
      tracing = v;
    },
    get retrieval_url_enabled() {
      return retrieval_url_enabled;
    },
    set retrieval_url_enabled(v) {
      retrieval_url_enabled = v;
    },
    get retrieval_urls() {
      return retrieval_urls;
    },
    set retrieval_urls(v) {
      retrieval_urls = v;
    },
    get retrieval_local_enabled() {
      return retrieval_local_enabled;
    },
    set retrieval_local_enabled(v) {
      retrieval_local_enabled = v;
    },
    get mcp_enabled() {
      return mcp_enabled;
    },
    set mcp_enabled(v) {
      mcp_enabled = v;
    },
  });

  // Controller actions
  const sendMessage = controller.handleSend;
  const stopGenerate = controller.stopGenerate;


  // Derived values
  let isChatHistoryVisible = $derived(!!get(showChatHistory));
  let hasMessages = $derived((messages?.length ?? 0) > 0);
  // Use $chatState for proper reactivity in Svelte 5 (getter/setter pattern doesn't trigger reactive updates)
  let canStopGeneration = $derived($chatState.busy && $chatState.isLoaded);
  // Use $chatState.isLoaded because store subscriptions are properly tracked by Svelte 5
  let showComposer = $derived($chatState.isLoaded || hasMessages);
  let presetOptions = $derived(
    ($settingsV2Store?.chat_presets.presets ?? []).map((preset) => ({
      id: preset.id,
      name: preset.name,
    })),
  );
  let retrievalLocalBetaEnabled = $derived(Boolean($settingsV2Store?.web_rag.local_rag.beta_enabled));
  let mcpFeatureEnabled = $derived(Boolean($settingsV2Store?.web_rag.mcp.enabled));
  let insertPromptListener: ((event: Event) => void) | null = null;

  // Keep shared chatState in sync so header and other views get instant truth.
  // busy/isLoaded are managed in actions.ts and intentionally not overwritten here.
  // Model fields are synced separately so retrieval/tool toggles never mutate model selection state.
  $effect(() => {
    chatState.update((s) => ({
      ...s,
      modelPath,
      repoId,
      revision,
      hubGgufFilename,
      mmprojPath,
      format,
      pendingModelPath,
      pendingFormat,
      isLoadingModel,
      isUnloadingModel,
      isCancelling,
      loadingStage,
      loadingProgress,
      unloadingProgress,
      preset_id,
    }));
  });

  $effect(() => {
    chatState.update((s) => ({
      ...s,
      temperature,
      temperature_enabled,
      top_k_enabled,
      top_k_value,
      top_p_enabled,
      top_p_value,
      min_p_enabled,
      min_p_value,
      repeat_penalty_enabled,
      repeat_penalty_value,
      max_new_tokens_enabled,
      max_new_tokens_value,
      seed_enabled,
      seed_value,
      stop_sequences_text,
      reasoning_parse_enabled,
      reasoning_start_tag,
      reasoning_end_tag,
      structured_output_enabled,
      ctx_limit_value,
      use_custom_params,
      split_prompt,
      verbose_prompt,
      tracing,
      retrieval_url_enabled,
      retrieval_urls,
      retrieval_local_enabled,
      mcp_enabled,
    }));
  });

  $effect(() => {
    if (!retrievalLocalBetaEnabled) {
      retrieval_local_enabled = false;
    }
    if (!mcpFeatureEnabled) {
      mcp_enabled = false;
    }
  });

  function toggleLoaderPanelVisibility() {
    isLoaderPanelVisible = !isLoaderPanelVisible;
  }

  function toggleChatHistoryVisibility() {
    showChatHistory.update((value) => !value);
  }

  async function resolveMcpPermission(requestId: string, decision: McpPermissionDecision) {
    if (!requestId) return;
    try {
      await invoke('mcp_resolve_tool_permission', {
        requestId,
        decision,
      });
    } catch (err) {
      console.warn('Failed to resolve MCP permission request:', err);
    } finally {
      clearMcpPendingPermission(requestId);
    }
  }

  function applySelectedPreset() {
    if (!preset_id) return;
    applyPresetById(preset_id);
  }

  function applyPresetValues(preset: ChatPreset) {
    preset_id = preset.id;
    use_custom_params = true;
    temperature = preset.sampling.temperature;
    temperature_enabled = true;
    top_k_enabled = true;
    top_k_value = preset.sampling.top_k;
    top_p_enabled = true;
    top_p_value = preset.sampling.top_p;
    min_p_enabled = true;
    min_p_value = preset.sampling.min_p;
    repeat_penalty_enabled = true;
    repeat_penalty_value = preset.sampling.repeat_penalty;
    max_new_tokens_enabled = true;
    max_new_tokens_value = Math.max(1, Math.floor(preset.sampling.max_tokens));
    seed_enabled = preset.sampling.seed !== null;
    seed_value = preset.sampling.seed ?? 42;
    stop_sequences_text = (preset.sampling.stop_sequences ?? []).join('\n');
    ctx_limit_value = preset.context;
  }

  function applyPresetById(presetId: string, source: 'settings' | 'chat' = 'chat') {
    const snapshot = settingsV2Store.getSnapshot();
    const preset = snapshot?.chat_presets.presets.find((item) => item.id === presetId);
    if (!preset) return;
    applyPresetValues(preset);
    if (source === 'settings') {
      localStorage.removeItem('chat.quickPreset');
    }
  }

  type ManagerRuntimePatch = Partial<{
    ctx_limit_value: number;
    use_custom_params: boolean;
    temperature: number;
    temperature_enabled: boolean;
    top_k_value: number;
    top_k_enabled: boolean;
    top_p_value: number;
    top_p_enabled: boolean;
    min_p_value: number;
    min_p_enabled: boolean;
    repeat_penalty_value: number;
    repeat_penalty_enabled: boolean;
    max_new_tokens_value: number;
    max_new_tokens_enabled: boolean;
    seed_value: number;
    seed_enabled: boolean;
    stop_sequences_text: string;
    reasoning_parse_enabled: boolean;
    reasoning_start_tag: string;
    reasoning_end_tag: string;
    structured_output_enabled: boolean;
    split_prompt: boolean;
    verbose_prompt: boolean;
    tracing: boolean;
  }>;

  type LoadModelFromManagerArgs = {
    path: string;
    format: 'gguf';
    runtime?: ManagerRuntimePatch;
  };

  function clampNumber(value: number, min: number, max: number): number {
    return Math.min(max, Math.max(min, value));
  }

  function applyManagerRuntimePatch(patch?: ManagerRuntimePatch) {
    if (!patch) return;

    if (typeof patch.ctx_limit_value === 'number' && Number.isFinite(patch.ctx_limit_value)) {
      ctx_limit_value = Math.max(1, Math.floor(patch.ctx_limit_value));
    }
    if (typeof patch.use_custom_params === 'boolean') use_custom_params = patch.use_custom_params;
    if (typeof patch.temperature === 'number' && Number.isFinite(patch.temperature)) {
      temperature = clampNumber(patch.temperature, 0, 2);
    }
    if (typeof patch.temperature_enabled === 'boolean') {
      temperature_enabled = patch.temperature_enabled;
    }
    if (typeof patch.top_k_value === 'number' && Number.isFinite(patch.top_k_value)) {
      top_k_value = Math.max(1, Math.floor(patch.top_k_value));
    }
    if (typeof patch.top_k_enabled === 'boolean') top_k_enabled = patch.top_k_enabled;
    if (typeof patch.top_p_value === 'number' && Number.isFinite(patch.top_p_value)) {
      top_p_value = clampNumber(patch.top_p_value, 0, 1);
    }
    if (typeof patch.top_p_enabled === 'boolean') top_p_enabled = patch.top_p_enabled;
    if (typeof patch.min_p_value === 'number' && Number.isFinite(patch.min_p_value)) {
      min_p_value = clampNumber(patch.min_p_value, 0, 1);
    }
    if (typeof patch.min_p_enabled === 'boolean') min_p_enabled = patch.min_p_enabled;
    if (
      typeof patch.repeat_penalty_value === 'number' &&
      Number.isFinite(patch.repeat_penalty_value)
    ) {
      repeat_penalty_value = clampNumber(patch.repeat_penalty_value, 0.1, 2);
    }
    if (typeof patch.repeat_penalty_enabled === 'boolean') {
      repeat_penalty_enabled = patch.repeat_penalty_enabled;
    }
    if (
      typeof patch.max_new_tokens_value === 'number' &&
      Number.isFinite(patch.max_new_tokens_value)
    ) {
      max_new_tokens_value = Math.max(1, Math.floor(patch.max_new_tokens_value));
    }
    if (typeof patch.max_new_tokens_enabled === 'boolean') {
      max_new_tokens_enabled = patch.max_new_tokens_enabled;
    }
    if (typeof patch.seed_value === 'number' && Number.isFinite(patch.seed_value)) {
      seed_value = Math.max(0, Math.floor(patch.seed_value));
    }
    if (typeof patch.seed_enabled === 'boolean') {
      seed_enabled = patch.seed_enabled;
    }
    if (typeof patch.stop_sequences_text === 'string') {
      stop_sequences_text = patch.stop_sequences_text;
    }
    if (typeof patch.reasoning_parse_enabled === 'boolean') {
      reasoning_parse_enabled = patch.reasoning_parse_enabled;
    }
    if (typeof patch.reasoning_start_tag === 'string') {
      reasoning_start_tag = patch.reasoning_start_tag;
    }
    if (typeof patch.reasoning_end_tag === 'string') {
      reasoning_end_tag = patch.reasoning_end_tag;
    }
    if (typeof patch.structured_output_enabled === 'boolean') {
      structured_output_enabled = patch.structured_output_enabled;
    }
    if (typeof patch.split_prompt === 'boolean') split_prompt = patch.split_prompt;
    if (typeof patch.verbose_prompt === 'boolean') verbose_prompt = patch.verbose_prompt;
    if (typeof patch.tracing === 'boolean') tracing = patch.tracing;

    chatState.update((s) => ({
      ...s,
      ctx_limit_value,
      use_custom_params,
      temperature,
      temperature_enabled,
      top_k_value,
      top_k_enabled,
      top_p_value,
      top_p_enabled,
      min_p_value,
      min_p_enabled,
      repeat_penalty_value,
      repeat_penalty_enabled,
      max_new_tokens_value,
      max_new_tokens_enabled,
      seed_value,
      seed_enabled,
      stop_sequences_text,
      reasoning_parse_enabled,
      reasoning_start_tag,
      reasoning_end_tag,
      structured_output_enabled,
      split_prompt,
      verbose_prompt,
      tracing,
    }));
  }

  function getManagerRuntimeConfig() {
    return {
      ctx_limit_value,
      use_custom_params,
      temperature,
      temperature_enabled,
      top_k_value,
      top_k_enabled,
      top_p_value,
      top_p_enabled,
      min_p_value,
      min_p_enabled,
      repeat_penalty_value,
      repeat_penalty_enabled,
      max_new_tokens_value,
      max_new_tokens_enabled,
      seed_value,
      seed_enabled,
      stop_sequences_text,
      reasoning_parse_enabled,
      reasoning_start_tag,
      reasoning_end_tag,
      structured_output_enabled,
      split_prompt,
      verbose_prompt,
      tracing,
      preset_id,
    };
  }

  function getActiveSystemPrompt(): string {
    const snapshot = settingsV2Store.getSnapshot();
    if (!snapshot) return '';
    const activePresetId = preset_id ?? snapshot.chat_presets.default_preset_id;
    const preset = snapshot.chat_presets.presets.find((item) => item.id === activePresetId);
    return preset?.system_prompt ?? snapshot.chat_presets.default_system_prompt ?? '';
  }

  async function setActiveSystemPrompt(nextPrompt: string) {
    const snapshot = settingsV2Store.getSnapshot();
    if (!snapshot) return;

    const activePresetId = preset_id ?? snapshot.chat_presets.default_preset_id;
    let found = false;
    const nextPresets = snapshot.chat_presets.presets.map((item) => {
      if (item.id !== activePresetId) return item;
      found = true;
      return {
        ...item,
        system_prompt: nextPrompt,
      };
    });

    const nextChatPresets = {
      ...snapshot.chat_presets,
      presets: nextPresets,
      default_system_prompt: nextPrompt,
    };

    if (!found) return;
    await settingsV2Store.updateSection('chat_presets', nextChatPresets);
  }

  /**
   * Load a model from the Model Manager or header picker
   */
  function loadModelFromManager(args: LoadModelFromManagerArgs) {
    if (!args?.path) return;
    applyManagerRuntimePatch(args.runtime);
    pendingModelPath = args.path;
    pendingFormat = args.format;

    // If already loaded or loading, just update pending state.
    // Use shared store state because local runes state may lag in edge cases.
    if ($chatState.isLoaded || $chatState.isLoadingModel || isLoaded || isLoadingModel) {
      return;
    }

    // Set model info and trigger load
    format = args.format;
    modelPath = args.path;
    repoId = '';
    revision = '';
    hubGgufFilename = '';
    mmprojPath = '';
    pendingModelPath = '';
    pendingFormat = 'gguf';
    void controller.loadGGUF?.();
  }

  /**
   * Reload model with pending path (after unloading current)
   */
  async function reloadSelectedModel() {
    if (!pendingModelPath || pendingModelPath === modelPath) return;

    try {
      await stopGenerate();
    } catch {
      /* ignore */
    }

    await controller.unloadGGUF();
    format = pendingFormat;
    modelPath = pendingModelPath;
    repoId = '';
    revision = '';
    hubGgufFilename = '';
    mmprojPath = '';
    pendingModelPath = '';
    pendingFormat = 'gguf';
    void controller.loadGGUF?.();
  }

  // Expose controller API to window for header and Model Manager integration
  if (typeof window !== 'undefined') {
    (window as any).__oxide = {
      pickModel: controller.pickModel,
      loadModelFromManager,
      reloadSelectedModel,
      applyPresetById,
      loadGGUF: controller.loadGGUF,
      unloadGGUF: controller.unloadGGUF,
      cancelLoading: controller.cancelLoading,
      getRuntimeConfig: getManagerRuntimeConfig,
      setRuntimeConfig: applyManagerRuntimePatch,
      getSystemPrompt: getActiveSystemPrompt,
      setSystemPrompt: setActiveSystemPrompt,
      getState: () => ({
        currentModelPath: modelPath,
        currentFormat: format,
        modelPath,
        format,
        isLoaded,
        isLoadingModel,
        isUnloadingModel,
        isCancelling,
        loadingStage,
        loadingProgress,
        unloadingProgress,
        busy,
        pendingModelPath,
        pendingFormat,
        ctx_limit_value,
        use_custom_params,
        temperature,
        temperature_enabled,
        top_k_value,
        top_k_enabled,
        top_p_value,
        top_p_enabled,
        min_p_value,
        min_p_enabled,
        repeat_penalty_value,
        repeat_penalty_enabled,
        max_new_tokens_value,
        max_new_tokens_enabled,
        seed_value,
        seed_enabled,
        stop_sequences_text,
        reasoning_parse_enabled,
        reasoning_start_tag,
        reasoning_end_tag,
        structured_output_enabled,
        split_prompt,
        verbose_prompt,
        tracing,
      }),
    };
  }

  // Mount/Unmount
  onMount(async () => {
    chatUiMounted.set(true);

    chatUiMounted.set(true);

    // Initial session messages sync
    try {
      const session = get(currentSession);
      if (session) {
          messages = session.messages;
      }
    } catch { /* ignore */ }

    try {
      if (!settingsV2Store.getSnapshot()) {
        await settingsV2Store.load();
      }
      const snapshot = settingsV2Store.getSnapshot();
      if (snapshot) {
        const fromSettings = localStorage.getItem('chat.quickPreset');
        const targetPresetId = fromSettings || preset_id || snapshot.chat_presets.default_preset_id;
        const shouldApplyDefaults =
          !preset_id &&
          !use_custom_params &&
          messages.length === 0 &&
          Boolean(targetPresetId);
        if (fromSettings || shouldApplyDefaults) {
          applyPresetById(targetPresetId, fromSettings ? 'settings' : 'chat');
        }
      }
    } catch (err) {
      console.warn('Failed to sync chat defaults from settings:', err);
    }

    // Initialize stream listener
    try {
      await controller.ensureStreamListener();
    } catch (err) {
      console.warn('Failed to initialize stream listener:', err);
    }

    // Setup performance metrics listener
    await performanceService.setupEventListeners(
      undefined,
      (inferenceMetrics: InferenceMetrics) => {
        setTimeout(() => {
          const lastAssistantIndex = messages.findLastIndex((m) => m.role === 'assistant');
          if (lastAssistantIndex !== -1) {
            inferenceMetricsStore.setMetrics(lastAssistantIndex, inferenceMetrics);
          }
        }, 150);
      },
    );

    insertPromptListener = (event: Event) => {
      const detail = (event as CustomEvent<{ text?: string }>).detail;
      const text = detail?.text?.trim();
      if (!text) return;
      prompt = prompt ? `${prompt}\n\n${text}` : text;
    };
    window.addEventListener('oxide:insert-prompt', insertPromptListener);

  });

  onDestroy(() => {
    chatUiMounted.set(false);
    clearMcpPendingPermission();
    if (insertPromptListener) {
      window.removeEventListener('oxide:insert-prompt', insertPromptListener);
      insertPromptListener = null;
    }

    // Persist state
    chatState.set({
      modelPath,
      repoId,
      revision,
      hubGgufFilename,
      mmprojPath,
      format,
      pendingModelPath,
      pendingFormat,
      prompt,
      messages,
      busy,
      isLoaded,
      errorText,
      isLoadingModel,
      loadingProgress,
      loadingStage,
      isCancelling,
      isUnloadingModel,
      unloadingProgress,
      temperature,
      temperature_enabled,
      top_k_enabled,
      top_k_value,
      top_p_enabled,
      top_p_value,
      min_p_enabled,
      min_p_value,
      repeat_penalty_enabled,
      repeat_penalty_value,
      max_new_tokens_enabled,
      max_new_tokens_value,
      seed_enabled,
      seed_value,
      stop_sequences_text,
      reasoning_parse_enabled,
      reasoning_start_tag,
      reasoning_end_tag,
      structured_output_enabled,
      ctx_limit_value,
      use_custom_params,
      use_gpu,
      cuda_available,
      cuda_build,
      current_device,
      avx,
      neon,
      simd128,
      f16c,
      split_prompt,
      verbose_prompt,
      tracing,
      retrieval_url_enabled,
      retrieval_urls,
      retrieval_local_enabled,
      mcp_enabled,
      preset_id,
    });

    controller.destroy();
    performanceService.cleanup();
  });

  // Sync session changes
  let lastSessionId: string | null = null;
  $effect(() => {
    if ($currentSession && $currentSession.id !== lastSessionId) {
      messages = [...$currentSession.messages];
      lastSessionId = $currentSession.id;
      inferenceMetricsStore.clear();
    }
  });
</script>

<main class="flex flex-col h-full overflow-hidden bg-background">
  <PaneGroup direction="horizontal" class="flex flex-row flex-1 min-h-0 items-stretch h-full bg-background">
    <Pane defaultSize={$isPreviewOpen ? 60 : 100} minSize={30}>
      <section class="flex-1 min-w-0 flex flex-col relative h-full">
        <!-- Messages area with scroll -->
        <div class="messages-area flex-1 min-h-0 relative overflow-hidden">
          <Conversation class="h-full w-full">
            {#if hasMessages}
              <ConversationContent class="messages-content pb-16">
                <MessageList
                  bind:messages
                  showModelNotice={false}
                  onRegenerate={(index) => controller.handleRegenerate(index)}
                  onEdit={(index, content) => controller.handleEdit(index, content)}
                />
              </ConversationContent>
              <ConversationScrollButton />
            {:else}
              <MessageList
                bind:messages
                showModelNotice={!$chatState.isLoaded && messages.length === 0}
                onRegenerate={(index) => controller.handleRegenerate(index)}
                onEdit={(index, content) => controller.handleEdit(index, content)}
              />
            {/if}
          </Conversation>
        </div>

        <!-- Composer at bottom or centered when no messages -->
        {#if showComposer}
          <div
            class="composer-area shrink-0 relative z-10 px-3 sm:px-4 lg:px-6 pb-3 sm:pb-4 bg-background"
            class:composer-centered={!hasMessages}
          >
            <Composer
              bind:prompt
              {busy}
              isLoaded={$chatState.isLoaded}
              canStop={canStopGeneration}
              retrievalUrlEnabled={retrieval_url_enabled}
              retrievalUrls={retrieval_urls}
              retrievalLocalEnabled={retrieval_local_enabled}
              mcpEnabled={mcp_enabled}
              {supports_text}
              {supports_image}
              {supports_audio}
              {supports_video}
              {isLoaderPanelVisible}
              {isChatHistoryVisible}
              {hasMessages}
              onRetrievalUrlToggle={(enabled) => (retrieval_url_enabled = enabled)}
              onRetrievalUrlsChange={(urls) => (retrieval_urls = urls)}
              onRetrievalLocalToggle={(enabled) => (retrieval_local_enabled = enabled)}
              onMcpToggle={(enabled) => (mcp_enabled = enabled)}
              onSend={sendMessage}
              onStop={stopGenerate}
              onToggleLoaderPanel={toggleLoaderPanelVisibility}
              onToggleChatHistory={toggleChatHistoryVisibility}
            />
          </div>
        {/if}
      </section>
    </Pane>

    {#if $isPreviewOpen}
      <PaneResizer class="pane-resizer" />
      <Pane defaultSize={40} minSize={20}>
        <PreviewPanel class="h-full" />
      </Pane>
    {/if}
  </PaneGroup>

    <!-- Loader Panel Sheet -->
    <Sheet.Root bind:open={isLoaderPanelVisible}>
      <Sheet.Content side="right" class="w-full sm:max-w-[450px] p-0">
        <Sheet.Header class="p-4 pb-2">
          <Sheet.Title>Model Settings</Sheet.Title>
        </Sheet.Header>
        <div class="flex-1 overflow-y-auto p-4 pt-0 custom-scrollbar">
          <LoaderPanel
            bind:format
            bind:modelPath
            bind:repoId
            bind:revision
            bind:hubGgufFilename
            bind:ctx_limit_value
            bind:isLoadingModel
            bind:isUnloadingModel
            bind:isCancelling
            bind:loadingStage
            bind:loadingProgress
            bind:unloadingProgress
            bind:errorText
            bind:busy
            bind:isLoaded
            bind:avx
            bind:neon
            bind:simd128
            bind:f16c
            bind:split_prompt
            bind:verbose_prompt
            bind:tracing
            bind:use_custom_params
            bind:temperature
            bind:temperature_enabled
            bind:top_k_enabled
            bind:top_k_value
            bind:top_p_enabled
            bind:top_p_value
            bind:min_p_enabled
            bind:min_p_value
            bind:repeat_penalty_enabled
            bind:repeat_penalty_value
            bind:selectedPresetId={preset_id}
            presets={presetOptions}
            onPresetSelect={(presetId) => (preset_id = presetId)}
            onPresetApply={applySelectedPreset}
          />
        </div>
      </Sheet.Content>
    </Sheet.Root>
</main>
<McpPermissionModal request={$mcpPendingPermission} onDecision={resolveMcpPermission} />

<style>
  /* ===== Gradient Overlays (CSS Only - Complex Effects) ===== */
  
  /* Gradient fade overlay at top */
  .messages-area::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 20px; /* Leave space for scrollbar */
    height: 50px;
    background: linear-gradient(to top, transparent, var(--background));
    pointer-events: none;
    z-index: 1;
  }

  /* Gradient fade overlay at bottom */
  .messages-area::after {
    content: '';
    position: absolute;
    bottom: 0;
    left: 0;
    right: 20px; /* Leave space for scrollbar */
    height: 100px;
    background: linear-gradient(to bottom, transparent, var(--background));
    pointer-events: none;
    z-index: 1;
  }

  /* ===== Composer Centered State (CSS for transform) ===== */
  .composer-centered {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 100%;
    max-width: 48rem;
    padding: 1rem;
    background: transparent;
  }

  /* ===== PaneResizer Styles ===== */
  :global(.pane-resizer) {
    width: 6px;
    background: transparent;
    cursor: col-resize;
    transition: background-color 0.2s ease;
  }

  :global(.pane-resizer:hover),
  :global(.pane-resizer[data-state="dragging"]) {
    background: var(--border);
  }

  :global(.pane-resizer[data-state="dragging"]) {
    background: var(--primary);
  }
</style>


