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
    ChatMessages,
    ChatComposer,
    LoaderPanel,
    McpPermissionModal,
    PreviewPanel,
  } from '$lib/chat/components';
  import type { McpPermissionDecision } from '$lib/chat/types';
  import { createChatController } from '$lib/chat/controller';
  import type { ChatControllerState } from '$lib/chat/controller/types';
  import { chatState, chatUiMounted, type ChatPersistedState } from '$lib/stores/chat';
  import { currentSession } from '$lib/stores/chat-history';
  import { showChatHistory } from '$lib/stores/sidebar';
  import { isPreviewOpen } from '$lib/stores/html-preview';
  import { performanceService } from '$lib/services/performance-service';
  import { inferenceMetricsStore } from '$lib/stores/inference-metrics';
  import type { InferenceMetrics } from '$lib/types/performance';
  import { settingsV2Store } from '$lib/stores/settings-v2';
  import { clearMcpPendingPermission, mcpPendingPermission } from '$lib/stores/mcp-tooling';
  import type { ChatPreset } from '$lib/types/settings-v2';

  type OxideApplyPresetEventDetail = {
    presetId: string;
    source?: 'settings' | 'chat';
  };

  const savedState = get(chatState);

  let ui = $state({
    isLoaderPanelVisible: false,
  });

  let chat: ChatControllerState = $state({
    ...savedState,
    messages: Array.isArray(savedState.messages) ? savedState.messages : [],
    supports_text: true,
    supports_image: false,
    supports_audio: false,
    supports_video: false,
  });

  const controller = createChatController({ state: chat });

  const sendMessage = controller.handleSend;
  const stopGenerate = controller.stopGenerate;

  let isChatHistoryVisible = $derived(!!get(showChatHistory));
  let hasMessages = $derived((chat.messages?.length ?? 0) > 0);
  let canStopGeneration = $derived(chat.busy && chat.isLoaded);
  let showComposer = $derived(chat.isLoaded || hasMessages);
  let presetOptions = $derived(
    ($settingsV2Store?.chat_presets.presets ?? []).map((preset) => ({
      id: preset.id,
      name: preset.name,
    })),
  );
  let retrievalLocalBetaEnabled = $derived(
    Boolean($settingsV2Store?.web_rag.local_rag.beta_enabled),
  );
  let mcpFeatureEnabled = $derived(Boolean($settingsV2Store?.web_rag.mcp.enabled));
  let insertPromptListener: ((event: Event) => void) | null = null;
  let applyPresetListener: ((event: Event) => void) | null = null;

  $effect(() => {
    if (!retrievalLocalBetaEnabled) {
      chat.retrieval_local_enabled = false;
    }
    if (!mcpFeatureEnabled) {
      chat.mcp_enabled = false;
    }
  });

  function toPersistedChatState(state: ChatControllerState): ChatPersistedState {
    return {
      modelPath: state.modelPath,
      repoId: state.repoId,
      revision: state.revision,
      hubGgufFilename: state.hubGgufFilename,
      mmprojPath: state.mmprojPath,
      format: state.format,
      pendingModelPath: state.pendingModelPath,
      pendingFormat: state.pendingFormat,
      prompt: state.prompt,
      messages: state.messages,
      busy: state.busy,
      isLoaded: state.isLoaded,
      errorText: state.errorText,
      isLoadingModel: state.isLoadingModel,
      loadingProgress: state.loadingProgress,
      loadingStage: state.loadingStage,
      isCancelling: state.isCancelling,
      isUnloadingModel: state.isUnloadingModel,
      unloadingProgress: state.unloadingProgress,
      temperature: state.temperature,
      temperature_enabled: state.temperature_enabled,
      top_k_enabled: state.top_k_enabled,
      top_k_value: state.top_k_value,
      top_p_enabled: state.top_p_enabled,
      top_p_value: state.top_p_value,
      min_p_enabled: state.min_p_enabled,
      min_p_value: state.min_p_value,
      repeat_penalty_enabled: state.repeat_penalty_enabled,
      repeat_penalty_value: state.repeat_penalty_value,
      max_new_tokens_enabled: state.max_new_tokens_enabled,
      max_new_tokens_value: state.max_new_tokens_value,
      seed_enabled: state.seed_enabled,
      seed_value: state.seed_value,
      stop_sequences_text: state.stop_sequences_text,
      reasoning_parse_enabled: state.reasoning_parse_enabled,
      reasoning_start_tag: state.reasoning_start_tag,
      reasoning_end_tag: state.reasoning_end_tag,
      structured_output_enabled: state.structured_output_enabled,
      ctx_limit_value: state.ctx_limit_value,
      use_custom_params: state.use_custom_params,
      use_gpu: state.use_gpu,
      cuda_available: state.cuda_available,
      cuda_build: state.cuda_build,
      current_device: state.current_device,
      avx: state.avx,
      neon: state.neon,
      simd128: state.simd128,
      f16c: state.f16c,
      split_prompt: state.split_prompt,
      verbose_prompt: state.verbose_prompt,
      tracing: state.tracing,
      retrieval_url_enabled: state.retrieval_url_enabled,
      retrieval_urls: state.retrieval_urls,
      retrieval_local_enabled: state.retrieval_local_enabled,
      mcp_enabled: state.mcp_enabled,
      preset_id: state.preset_id,
    };
  }

  // Single source of truth is local `chat`; shared store is a one-way projection.
  $effect(() => {
    chatState.set(toPersistedChatState(chat));
  });

  function toggleLoaderPanelVisibility() {
    ui.isLoaderPanelVisible = !ui.isLoaderPanelVisible;
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
    if (!chat.preset_id) return;
    applyPresetById(chat.preset_id);
  }

  function applyPresetValues(preset: ChatPreset) {
    chat.preset_id = preset.id;
    chat.use_custom_params = true;
    chat.temperature = preset.sampling.temperature;
    chat.temperature_enabled = true;
    chat.top_k_enabled = true;
    chat.top_k_value = preset.sampling.top_k;
    chat.top_p_enabled = true;
    chat.top_p_value = preset.sampling.top_p;
    chat.min_p_enabled = true;
    chat.min_p_value = preset.sampling.min_p;
    chat.repeat_penalty_enabled = true;
    chat.repeat_penalty_value = preset.sampling.repeat_penalty;
    chat.max_new_tokens_enabled = true;
    chat.max_new_tokens_value = Math.max(1, Math.floor(preset.sampling.max_tokens));
    chat.seed_enabled = preset.sampling.seed !== null;
    chat.seed_value = preset.sampling.seed ?? 42;
    chat.stop_sequences_text = (preset.sampling.stop_sequences ?? []).join('\n');
    chat.ctx_limit_value = preset.context;
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
      chat.ctx_limit_value = Math.max(1, Math.floor(patch.ctx_limit_value));
    }
    if (typeof patch.use_custom_params === 'boolean') chat.use_custom_params = patch.use_custom_params;
    if (typeof patch.temperature === 'number' && Number.isFinite(patch.temperature)) {
      chat.temperature = clampNumber(patch.temperature, 0, 2);
    }
    if (typeof patch.temperature_enabled === 'boolean') {
      chat.temperature_enabled = patch.temperature_enabled;
    }
    if (typeof patch.top_k_value === 'number' && Number.isFinite(patch.top_k_value)) {
      chat.top_k_value = Math.max(1, Math.floor(patch.top_k_value));
    }
    if (typeof patch.top_k_enabled === 'boolean') chat.top_k_enabled = patch.top_k_enabled;
    if (typeof patch.top_p_value === 'number' && Number.isFinite(patch.top_p_value)) {
      chat.top_p_value = clampNumber(patch.top_p_value, 0, 1);
    }
    if (typeof patch.top_p_enabled === 'boolean') chat.top_p_enabled = patch.top_p_enabled;
    if (typeof patch.min_p_value === 'number' && Number.isFinite(patch.min_p_value)) {
      chat.min_p_value = clampNumber(patch.min_p_value, 0, 1);
    }
    if (typeof patch.min_p_enabled === 'boolean') chat.min_p_enabled = patch.min_p_enabled;
    if (
      typeof patch.repeat_penalty_value === 'number' &&
      Number.isFinite(patch.repeat_penalty_value)
    ) {
      chat.repeat_penalty_value = clampNumber(patch.repeat_penalty_value, 0.1, 2);
    }
    if (typeof patch.repeat_penalty_enabled === 'boolean') {
      chat.repeat_penalty_enabled = patch.repeat_penalty_enabled;
    }
    if (
      typeof patch.max_new_tokens_value === 'number' &&
      Number.isFinite(patch.max_new_tokens_value)
    ) {
      chat.max_new_tokens_value = Math.max(1, Math.floor(patch.max_new_tokens_value));
    }
    if (typeof patch.max_new_tokens_enabled === 'boolean') {
      chat.max_new_tokens_enabled = patch.max_new_tokens_enabled;
    }
    if (typeof patch.seed_value === 'number' && Number.isFinite(patch.seed_value)) {
      chat.seed_value = Math.max(0, Math.floor(patch.seed_value));
    }
    if (typeof patch.seed_enabled === 'boolean') {
      chat.seed_enabled = patch.seed_enabled;
    }
    if (typeof patch.stop_sequences_text === 'string') {
      chat.stop_sequences_text = patch.stop_sequences_text;
    }
    if (typeof patch.reasoning_parse_enabled === 'boolean') {
      chat.reasoning_parse_enabled = patch.reasoning_parse_enabled;
    }
    if (typeof patch.reasoning_start_tag === 'string') {
      chat.reasoning_start_tag = patch.reasoning_start_tag;
    }
    if (typeof patch.reasoning_end_tag === 'string') {
      chat.reasoning_end_tag = patch.reasoning_end_tag;
    }
    if (typeof patch.structured_output_enabled === 'boolean') {
      chat.structured_output_enabled = patch.structured_output_enabled;
    }
    if (typeof patch.split_prompt === 'boolean') chat.split_prompt = patch.split_prompt;
    if (typeof patch.verbose_prompt === 'boolean') chat.verbose_prompt = patch.verbose_prompt;
    if (typeof patch.tracing === 'boolean') chat.tracing = patch.tracing;
  }

  function getManagerRuntimeConfig() {
    return {
      ctx_limit_value: chat.ctx_limit_value,
      use_custom_params: chat.use_custom_params,
      temperature: chat.temperature,
      temperature_enabled: chat.temperature_enabled,
      top_k_value: chat.top_k_value,
      top_k_enabled: chat.top_k_enabled,
      top_p_value: chat.top_p_value,
      top_p_enabled: chat.top_p_enabled,
      min_p_value: chat.min_p_value,
      min_p_enabled: chat.min_p_enabled,
      repeat_penalty_value: chat.repeat_penalty_value,
      repeat_penalty_enabled: chat.repeat_penalty_enabled,
      max_new_tokens_value: chat.max_new_tokens_value,
      max_new_tokens_enabled: chat.max_new_tokens_enabled,
      seed_value: chat.seed_value,
      seed_enabled: chat.seed_enabled,
      stop_sequences_text: chat.stop_sequences_text,
      reasoning_parse_enabled: chat.reasoning_parse_enabled,
      reasoning_start_tag: chat.reasoning_start_tag,
      reasoning_end_tag: chat.reasoning_end_tag,
      structured_output_enabled: chat.structured_output_enabled,
      split_prompt: chat.split_prompt,
      verbose_prompt: chat.verbose_prompt,
      tracing: chat.tracing,
      preset_id: chat.preset_id,
    };
  }

  function getActiveSystemPrompt(): string {
    const snapshot = settingsV2Store.getSnapshot();
    if (!snapshot) return '';
    const activePresetId = chat.preset_id ?? snapshot.chat_presets.default_preset_id;
    const preset = snapshot.chat_presets.presets.find((item) => item.id === activePresetId);
    return preset?.system_prompt ?? snapshot.chat_presets.default_system_prompt ?? '';
  }

  async function setActiveSystemPrompt(nextPrompt: string) {
    const snapshot = settingsV2Store.getSnapshot();
    if (!snapshot) return;

    const activePresetId = chat.preset_id ?? snapshot.chat_presets.default_preset_id;
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

  function loadModelFromManager(args: LoadModelFromManagerArgs) {
    if (!args?.path) return;
    applyManagerRuntimePatch(args.runtime);
    chat.pendingModelPath = args.path;
    chat.pendingFormat = args.format;

    if (chat.isLoaded || chat.isLoadingModel) {
      return;
    }

    chat.format = args.format;
    chat.modelPath = args.path;
    chat.repoId = '';
    chat.revision = '';
    chat.hubGgufFilename = '';
    chat.mmprojPath = '';
    chat.pendingModelPath = '';
    chat.pendingFormat = 'gguf';
    void controller.loadGGUF?.();
  }

  async function reloadSelectedModel() {
    if (!chat.pendingModelPath || chat.pendingModelPath === chat.modelPath) return;

    try {
      await stopGenerate();
    } catch {
      /* ignore */
    }

    await controller.unloadGGUF();
    chat.format = chat.pendingFormat;
    chat.modelPath = chat.pendingModelPath;
    chat.repoId = '';
    chat.revision = '';
    chat.hubGgufFilename = '';
    chat.mmprojPath = '';
    chat.pendingModelPath = '';
    chat.pendingFormat = 'gguf';
    void controller.loadGGUF?.();
  }

  function registerOxideBridge() {
    if (typeof window === 'undefined') return;
    window.__oxide = {
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
        currentModelPath: chat.modelPath,
        currentFormat: chat.format,
        modelPath: chat.modelPath,
        format: chat.format,
        isLoaded: chat.isLoaded,
        isLoadingModel: chat.isLoadingModel,
        isUnloadingModel: chat.isUnloadingModel,
        isCancelling: chat.isCancelling,
        loadingStage: chat.loadingStage,
        loadingProgress: chat.loadingProgress,
        unloadingProgress: chat.unloadingProgress,
        busy: chat.busy,
        pendingModelPath: chat.pendingModelPath,
        pendingFormat: chat.pendingFormat,
        ctx_limit_value: chat.ctx_limit_value,
        use_custom_params: chat.use_custom_params,
        temperature: chat.temperature,
        temperature_enabled: chat.temperature_enabled,
        top_k_value: chat.top_k_value,
        top_k_enabled: chat.top_k_enabled,
        top_p_value: chat.top_p_value,
        top_p_enabled: chat.top_p_enabled,
        min_p_value: chat.min_p_value,
        min_p_enabled: chat.min_p_enabled,
        repeat_penalty_value: chat.repeat_penalty_value,
        repeat_penalty_enabled: chat.repeat_penalty_enabled,
        max_new_tokens_value: chat.max_new_tokens_value,
        max_new_tokens_enabled: chat.max_new_tokens_enabled,
        seed_value: chat.seed_value,
        seed_enabled: chat.seed_enabled,
        stop_sequences_text: chat.stop_sequences_text,
        reasoning_parse_enabled: chat.reasoning_parse_enabled,
        reasoning_start_tag: chat.reasoning_start_tag,
        reasoning_end_tag: chat.reasoning_end_tag,
        structured_output_enabled: chat.structured_output_enabled,
        split_prompt: chat.split_prompt,
        verbose_prompt: chat.verbose_prompt,
        tracing: chat.tracing,
      }),
    };
  }

  onMount(async () => {
    chatUiMounted.set(true);
    registerOxideBridge();

    try {
      const session = get(currentSession);
      if (session) {
        chat.messages = session.messages;
      }
    } catch {
      /* ignore */
    }

    try {
      if (!settingsV2Store.getSnapshot()) {
        await settingsV2Store.load();
      }
      const snapshot = settingsV2Store.getSnapshot();
      if (snapshot) {
        const fromSettings = localStorage.getItem('chat.quickPreset');
        const targetPresetId = fromSettings || chat.preset_id || snapshot.chat_presets.default_preset_id;
        const shouldApplyDefaults =
          !chat.preset_id && !chat.use_custom_params && chat.messages.length === 0 && Boolean(targetPresetId);
        if (fromSettings || shouldApplyDefaults) {
          applyPresetById(targetPresetId, fromSettings ? 'settings' : 'chat');
        }
      }
    } catch (err) {
      console.warn('Failed to sync chat defaults from settings:', err);
    }

    try {
      await controller.ensureStreamListener();
    } catch (err) {
      console.warn('Failed to initialize stream listener:', err);
    }

    await performanceService.setupEventListeners(
      undefined,
      (inferenceMetrics: InferenceMetrics) => {
        setTimeout(() => {
          const lastAssistantIndex = chat.messages.findLastIndex((m) => m.role === 'assistant');
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
      chat.prompt = chat.prompt ? `${chat.prompt}\n\n${text}` : text;
    };
    applyPresetListener = (event: Event) => {
      const detail = (event as CustomEvent<OxideApplyPresetEventDetail>).detail;
      const presetId = detail?.presetId?.trim();
      if (!presetId) return;
      applyPresetById(presetId, detail?.source ?? 'settings');
    };
    window.addEventListener('oxide:insert-prompt', insertPromptListener);
    window.addEventListener('oxide:apply-preset', applyPresetListener);
  });

  onDestroy(() => {
    chatUiMounted.set(false);
    clearMcpPendingPermission();
    if (insertPromptListener) {
      window.removeEventListener('oxide:insert-prompt', insertPromptListener);
      insertPromptListener = null;
    }
    if (applyPresetListener) {
      window.removeEventListener('oxide:apply-preset', applyPresetListener);
      applyPresetListener = null;
    }
    if (window.__oxide) {
      delete window.__oxide;
    }

    controller.destroy();
    performanceService.cleanup();
  });

  let lastSessionId: string | null = null;
  $effect(() => {
    if ($currentSession && $currentSession.id !== lastSessionId) {
      if (chat.busy) {
        void stopGenerate();
      }
      controller.resetStreamState?.();
      chat.messages = [...$currentSession.messages];
      lastSessionId = $currentSession.id;
      inferenceMetricsStore.clear();
    }
  });
</script>

<main class="flex flex-col h-full overflow-hidden bg-background">
  <PaneGroup
    direction="horizontal"
    class="flex flex-row flex-1 min-h-0 items-stretch h-full bg-background"
  >
    <Pane defaultSize={$isPreviewOpen ? 60 : 100} minSize={30}>
      <section class="flex-1 min-w-0 flex flex-col relative h-full">
        <ChatMessages
          bind:messages={chat.messages}
          isLoaded={chat.isLoaded}
          onRegenerate={(index) => controller.handleRegenerate(index)}
          onEdit={(index, content) => controller.handleEdit(index, content)}
        />

        {#if showComposer}
          <ChatComposer
            bind:prompt={chat.prompt}
            busy={chat.busy}
            isLoaded={chat.isLoaded}
            canStop={canStopGeneration}
            retrievalUrlEnabled={chat.retrieval_url_enabled}
            retrievalUrls={chat.retrieval_urls}
            retrievalLocalEnabled={chat.retrieval_local_enabled}
            mcpEnabled={chat.mcp_enabled}
            supports_text={chat.supports_text}
            supports_image={chat.supports_image}
            supports_audio={chat.supports_audio}
            supports_video={chat.supports_video}
            isLoaderPanelVisible={ui.isLoaderPanelVisible}
            {isChatHistoryVisible}
            {hasMessages}
            onRetrievalUrlToggle={(enabled) => (chat.retrieval_url_enabled = enabled)}
            onRetrievalUrlsChange={(urls) => (chat.retrieval_urls = urls)}
            onRetrievalLocalToggle={(enabled) => (chat.retrieval_local_enabled = enabled)}
            onMcpToggle={(enabled) => (chat.mcp_enabled = enabled)}
            onSend={sendMessage}
            onStop={stopGenerate}
            onToggleLoaderPanel={toggleLoaderPanelVisibility}
            onToggleChatHistory={toggleChatHistoryVisibility}
          />
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

  <Sheet.Root bind:open={ui.isLoaderPanelVisible}>
    <Sheet.Content side="right" class="w-full sm:max-w-[450px] p-0">
      <Sheet.Header class="p-4 pb-2">
        <Sheet.Title>Model Settings</Sheet.Title>
      </Sheet.Header>
      <div class="flex-1 overflow-y-auto p-4 pt-0 custom-scrollbar">
        <LoaderPanel
          bind:format={chat.format}
          bind:modelPath={chat.modelPath}
          bind:repoId={chat.repoId}
          bind:revision={chat.revision}
          bind:hubGgufFilename={chat.hubGgufFilename}
          bind:ctx_limit_value={chat.ctx_limit_value}
          bind:isLoadingModel={chat.isLoadingModel}
          bind:isUnloadingModel={chat.isUnloadingModel}
          bind:isCancelling={chat.isCancelling}
          bind:loadingStage={chat.loadingStage}
          bind:loadingProgress={chat.loadingProgress}
          bind:unloadingProgress={chat.unloadingProgress}
          bind:errorText={chat.errorText}
          bind:busy={chat.busy}
          bind:isLoaded={chat.isLoaded}
          bind:avx={chat.avx}
          bind:neon={chat.neon}
          bind:simd128={chat.simd128}
          bind:f16c={chat.f16c}
          bind:split_prompt={chat.split_prompt}
          bind:verbose_prompt={chat.verbose_prompt}
          bind:tracing={chat.tracing}
          bind:use_custom_params={chat.use_custom_params}
          bind:temperature={chat.temperature}
          bind:temperature_enabled={chat.temperature_enabled}
          bind:top_k_enabled={chat.top_k_enabled}
          bind:top_k_value={chat.top_k_value}
          bind:top_p_enabled={chat.top_p_enabled}
          bind:top_p_value={chat.top_p_value}
          bind:min_p_enabled={chat.min_p_enabled}
          bind:min_p_value={chat.min_p_value}
          bind:repeat_penalty_enabled={chat.repeat_penalty_enabled}
          bind:repeat_penalty_value={chat.repeat_penalty_value}
          bind:selectedPresetId={chat.preset_id}
          presets={presetOptions}
          onPresetSelect={(presetId) => (chat.preset_id = presetId)}
          onPresetApply={applySelectedPreset}
        />
      </div>
    </Sheet.Content>
  </Sheet.Root>
</main>
<McpPermissionModal request={$mcpPendingPermission} onDecision={resolveMcpPermission} />

<style>
  :global(.pane-resizer) {
    width: 6px;
    background: transparent;
    cursor: col-resize;
    transition: background-color 0.2s ease;
  }

  :global(.pane-resizer:hover),
  :global(.pane-resizer[data-state='dragging']) {
    background: var(--border);
  }

  :global(.pane-resizer[data-state='dragging']) {
    background: var(--primary);
  }
</style>
