<script lang="ts">
  import { get } from 'svelte/store';
  import { onDestroy, onMount } from 'svelte';
  import SettingsRuntimeHardwarePanel from '$lib/components/settings/SettingsRuntimeHardwarePanel.svelte';
  import { hardwareService } from '$lib/services/hardware-service';
  import { chatState } from '$lib/stores/chat';
  import type { InferenceMetrics } from '$lib/types/performance';
  import type { PerformanceSettings } from '$lib/types/settings-v2';
  import type { HardwareSystemInfo, HardwareSystemUsage } from '$lib/types/hardware';

  interface ModelPlan {
    gpuLayers: number;
    maxContextLength: number;
    noOffloadKvCache: boolean;
    offloadMmproj: boolean;
    batchSize: number;
    mode: 'GPU' | 'Hybrid' | 'CPU' | 'Unsupported';
  }

  interface Props {
    value: PerformanceSettings;
    highlightedSettingId?: string | null;
    onChange: (next: PerformanceSettings) => void;
  }

  let { value, highlightedSettingId = null, onChange }: Props = $props();

  let planner = $state<ModelPlan | null>(null);
  let systemInfo = $state<HardwareSystemInfo | null>(null);
  let systemUsage = $state<HardwareSystemUsage | null>(null);
  let modelMaxLayers = $state(0);
  let modelSizeGb = $state<number | null>(null);
  let tokensPerSecond = $state<number | null>(null);
  let latencyMs = $state<number | null>(null);
  let isLoadingModel = $state(false);
  let isModelLoaded = $state(false);
  let loadingProgress = $state(0);
  let loadingStage = $state('');

  let inferenceUnlisten: (() => void) | null = null;
  let chatStateUnsubscribe: (() => void) | null = null;
  let hardwarePollHandle: ReturnType<typeof setInterval> | null = null;
  let lastModelPath = $state('');

  async function loadHardwareUsage() {
    try {
      const [info, usage] = await Promise.all([
        hardwareService.getSystemInfo(),
        hardwareService.getSystemUsage(),
      ]);
      systemInfo = info;
      systemUsage = usage;
    } catch (error) {
      console.warn('Failed to load hardware usage', error);
    }
  }

  async function refreshPlanner(modelPathOverride?: string) {
    const modelPath = modelPathOverride ?? get(chatState).modelPath;
    if (!modelPath) {
      planner = null;
      return;
    }
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      planner = await invoke<ModelPlan>('plugin:llamacpp|plan_model_load', {
        path: modelPath,
        memoryMode: value.memory_mode,
        mmprojPath: null,
        requestedCtx: Math.max(512, value.llama_runtime.ctx_size),
      });
    } catch (error) {
      console.warn('Failed to calculate model plan', error);
      planner = null;
    }
  }

  async function refreshModelMetadata(modelPathOverride?: string) {
    const modelPath = modelPathOverride ?? get(chatState).modelPath;
    if (!modelPath) {
      modelMaxLayers = 0;
      modelSizeGb = null;
      return;
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const [metadata, modelSizeBytes] = await Promise.all([
        modelPath.toLowerCase().endsWith('.gguf')
          ? invoke<{ block_count?: number }>('parse_gguf_metadata', { filePath: modelPath }).catch(() => null)
          : Promise.resolve(null),
        invoke<number>('plugin:llamacpp|get_model_size', { path: modelPath }).catch(() => 0),
      ]);

      const parsedBlockCount = Number(metadata?.block_count);
      if (Number.isFinite(parsedBlockCount) && parsedBlockCount > 0) {
        modelMaxLayers = Math.max(1, Math.round(parsedBlockCount + 1));
      } else {
        modelMaxLayers = Math.max(
          modelMaxLayers,
          Math.round(planner?.gpuLayers ?? 0),
          Math.round(value.llama_runtime.n_gpu_layers ?? 0),
        );
      }

      modelSizeGb =
        typeof modelSizeBytes === 'number' && Number.isFinite(modelSizeBytes) && modelSizeBytes > 0
          ? modelSizeBytes / (1024 ** 3)
          : null;
    } catch (error) {
      console.warn('Failed to read model metadata for hardware panel', error);
      modelMaxLayers = Math.max(
        modelMaxLayers,
        Math.round(planner?.gpuLayers ?? 0),
        Math.round(value.llama_runtime.n_gpu_layers ?? 0),
      );
      modelSizeGb = null;
    }
  }

  async function subscribeInferenceMetrics() {
    try {
      const { listen } = await import('@tauri-apps/api/event');
      inferenceUnlisten = await listen<InferenceMetrics>('inference_metrics', (event) => {
        tokensPerSecond = event.payload.tokens_per_second;
        latencyMs = event.payload.total_duration_ms;
      });
    } catch (error) {
      console.warn('Failed to subscribe inference metrics', error);
    }
  }

  onMount(() => {
    const initialChatState = get(chatState);
    lastModelPath = initialChatState.modelPath;
    isModelLoaded = initialChatState.isLoaded;
    isLoadingModel = initialChatState.isLoadingModel;
    loadingProgress = initialChatState.loadingProgress;
    loadingStage = initialChatState.loadingStage;

    void loadHardwareUsage();
    void refreshPlanner();
    void refreshModelMetadata();
    void subscribeInferenceMetrics();

    hardwarePollHandle = setInterval(() => {
      void loadHardwareUsage();
    }, 2500);

    chatStateUnsubscribe = chatState.subscribe((snapshot) => {
      isModelLoaded = snapshot.isLoaded;
      isLoadingModel = snapshot.isLoadingModel;
      loadingProgress = snapshot.loadingProgress;
      loadingStage = snapshot.loadingStage;
      if (snapshot.modelPath === lastModelPath) return;
      lastModelPath = snapshot.modelPath;
      void refreshPlanner(snapshot.modelPath);
      void refreshModelMetadata(snapshot.modelPath);
    });
  });

  onDestroy(() => {
    if (inferenceUnlisten) {
      inferenceUnlisten();
      inferenceUnlisten = null;
    }
    if (chatStateUnsubscribe) {
      chatStateUnsubscribe();
      chatStateUnsubscribe = null;
    }
    if (hardwarePollHandle) {
      clearInterval(hardwarePollHandle);
      hardwarePollHandle = null;
    }
  });

  $effect(() => {
    value.memory_mode;
    value.llama_runtime.ctx_size;
    void refreshPlanner();
  });
</script>

<SettingsRuntimeHardwarePanel
  value={value}
  planner={planner}
  highlightedSettingId={highlightedSettingId}
  systemInfo={systemInfo}
  systemUsage={systemUsage}
  modelMaxLayers={modelMaxLayers}
  modelSizeGb={modelSizeGb}
  tokensPerSecond={tokensPerSecond}
  latencyMs={latencyMs}
  isModelLoaded={isModelLoaded}
  isLoadingModel={isLoadingModel}
  loadingProgress={loadingProgress}
  loadingStage={loadingStage}
  {onChange}
/>
