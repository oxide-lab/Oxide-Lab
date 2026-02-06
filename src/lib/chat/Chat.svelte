<script lang="ts">
  /**
   * Chat Component
   *
   * Main chat interface using ai-elements components and chat controller.
   */
  import { onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import * as Sheet from '$lib/components/ui/sheet';
  import { PaneGroup, Pane, PaneResizer } from 'paneforge';
  import {
    Conversation,
    ConversationContent,
    ConversationScrollButton,
  } from '$lib/components/ai-elements/conversation';
  import { MessageList, Composer, LoaderPanel, PreviewPanel } from '$lib/chat/components';
  import type { ChatMessage } from '$lib/chat/types';
  import { createChatController } from '$lib/chat/controller';
  import { chatState, chatUiMounted, getDefaultChatState } from '$lib/stores/chat';
  import { currentSession } from '$lib/stores/chat-history';
  import { showChatHistory } from '$lib/stores/sidebar';
  import { htmlPreviewStore, isPreviewOpen } from '$lib/stores/html-preview';
  import { performanceService } from '$lib/services/performance-service';
  import { inferenceMetricsStore } from '$lib/stores/inference-metrics';
  import type { InferenceMetrics } from '$lib/types/performance';


  // State
  // Initial State from Store (Synchronous)
  const savedState = get(chatState);

  let isLoaderPanelVisible = $state(false);
  let modelPath = $state(savedState.modelPath);
  let repoId = $state<string>(savedState.repoId);
  let revision = $state<string>(savedState.revision);
  let hubGgufFilename = $state<string>(savedState.hubGgufFilename);
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
  let ctx_limit_value = $state<number>(savedState.ctx_limit_value);
  let use_custom_params = $state<boolean>(savedState.use_custom_params);
  let split_prompt = $state<boolean>(savedState.split_prompt);
  let verbose_prompt = $state<boolean>(savedState.verbose_prompt);
  let tracing = $state<boolean>(savedState.tracing);

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

  // Keep shared chatState in sync so header and other views get instant truth
  // Note: isLoaded and busy are NOT synced here - they are managed directly in actions.ts
  $effect(() => {
    chatState.update((s) => ({
      ...s,
      modelPath,
      repoId,
      revision,
      hubGgufFilename,
      format,
      pendingModelPath,
      pendingFormat,
      // busy and isLoaded are managed in actions.ts - don't overwrite!
      isLoadingModel,
      isUnloadingModel,
      isCancelling,
      loadingStage,
      loadingProgress,
      unloadingProgress,
    }));
  });

  function toggleLoaderPanelVisibility() {
    isLoaderPanelVisible = !isLoaderPanelVisible;
  }

  function toggleChatHistoryVisibility() {
    showChatHistory.update((value) => !value);
  }

  /**
   * Load a model from the Model Manager or header picker
   */
  function loadModelFromManager(args: { path: string; format: 'gguf' }) {
    if (!args?.path) return;
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
      loadGGUF: controller.loadGGUF,
      unloadGGUF: controller.unloadGGUF,
      cancelLoading: controller.cancelLoading,
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

  });

  onDestroy(() => {
    chatUiMounted.set(false);

    // Persist state
    chatState.set({
      modelPath,
      repoId,
      revision,
      hubGgufFilename,
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
              {isLoaderPanelVisible}
              {isChatHistoryVisible}
              {hasMessages}
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
            bind:use_gpu
            bind:cuda_available
            bind:cuda_build
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
            onDeviceToggle={(val) => controller.setDeviceByToggle(val)}
          />
        </div>
      </Sheet.Content>
    </Sheet.Root>
</main>

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


