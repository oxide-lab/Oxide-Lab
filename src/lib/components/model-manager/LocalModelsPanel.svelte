<script lang="ts">
  import { goto } from '$app/navigation';
  import { get } from 'svelte/store';
  import { onDestroy, onMount, tick } from 'svelte';
  import * as Card from '$lib/components/ui/card';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import { Input } from '$lib/components/ui/input';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
  import * as Breadcrumb from '$lib/components/ui/breadcrumb';
  import { Label } from '$lib/components/ui/label';
  import * as Select from '$lib/components/ui/select';
  import * as Progress from '$lib/components/ui/progress';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Sheet from '$lib/components/ui/sheet';
  import * as Separator from '$lib/components/ui/separator';
  import * as Slider from '$lib/components/ui/slider';
  import * as Tabs from '$lib/components/ui/tabs';
  import { Textarea } from '$lib/components/ui/textarea';
  import FolderOpen from 'phosphor-svelte/lib/FolderOpen';
  import MagnifyingGlass from 'phosphor-svelte/lib/MagnifyingGlass';
  import Play from 'phosphor-svelte/lib/Play';
  import Trash from 'phosphor-svelte/lib/Trash';
  import DotsThree from 'phosphor-svelte/lib/DotsThree';
  import PencilSimple from 'phosphor-svelte/lib/PencilSimple';
  import Check from 'phosphor-svelte/lib/Check';
  import X from 'phosphor-svelte/lib/X';
  import CaretDown from 'phosphor-svelte/lib/CaretDown';
  import CaretUp from 'phosphor-svelte/lib/CaretUp';
  import Rows from 'phosphor-svelte/lib/Rows';
  import SquaresFour from 'phosphor-svelte/lib/SquaresFour';
  import Copy from 'phosphor-svelte/lib/Copy';
  import ArrowSquareOut from 'phosphor-svelte/lib/ArrowSquareOut';
  import CircleNotch from 'phosphor-svelte/lib/CircleNotch';
  import Info from 'phosphor-svelte/lib/Info';
  import DownloadSimple from 'phosphor-svelte/lib/DownloadSimple';
  import Cpu from 'phosphor-svelte/lib/Cpu';
  import ChatsCircle from 'phosphor-svelte/lib/ChatsCircle';
  import HardDrives from 'phosphor-svelte/lib/HardDrives';
  import { t } from '$lib/i18n';
  import {
    folderPath,
    models,
    filteredModels,
    filterOptions,
    sortOptions,
    selectedModel,
    isLoading,
    error,
    scanFolder,
    deleteModel,
    loadedModelIds,
    initLoadedModels,
    modelsCount,
    totalModelsSize,
  } from '$lib/stores/local-models';
  import { activePageTab } from '$lib/stores/page-tabs.svelte';
  import { LocalModelsService } from '$lib/services/local-models';
  import { hardwareService } from '$lib/services/hardware-service';
  import {
    areModelPathsEqual,
    isModelPathLoaded,
  } from '$lib/model-manager/model-identity';
  import type { ModelInfo, SortField, SortOrder, ValidationLevel } from '$lib/types/local-models';

  type ViewMode = 'list' | 'grid';
  type SortMode = 'recent' | 'name' | 'size' | 'architecture';
  type ModelStatus = 'loaded' | 'loading' | 'unloading' | 'available';
  type DetailsTab = 'info' | 'load' | 'inference';

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

  type OxideStateSnapshot = {
    currentModelPath?: string;
    isLoadingModel?: boolean;
    isUnloadingModel?: boolean;
    isLoaded?: boolean;
    busy?: boolean;
  };

  type ManagerRuntimeSnapshot = {
    ctx_limit_value?: number;
    use_custom_params?: boolean;
    temperature?: number;
    temperature_enabled?: boolean;
    top_k_value?: number;
    top_k_enabled?: boolean;
    top_p_value?: number;
    top_p_enabled?: boolean;
    min_p_value?: number;
    min_p_enabled?: boolean;
    repeat_penalty_value?: number;
    repeat_penalty_enabled?: boolean;
    max_new_tokens_value?: number;
    max_new_tokens_enabled?: boolean;
    seed_value?: number;
    seed_enabled?: boolean;
    stop_sequences_text?: string;
    reasoning_parse_enabled?: boolean;
    reasoning_start_tag?: string;
    reasoning_end_tag?: string;
    structured_output_enabled?: boolean;
    split_prompt?: boolean;
    verbose_prompt?: boolean;
    tracing?: boolean;
  };

  type RuntimeFlashAttnMode = 'auto' | 'on' | 'off';

  type LlamaRuntimeConfig = {
    server_path: string | null;
    selected_backend: string | null;
    n_gpu_layers: number;
    threads: number;
    threads_batch: number;
    ctx_size: number;
    batch_size: number;
    ubatch_size: number;
    n_predict: number;
    flash_attn: RuntimeFlashAttnMode | string;
    offload_mmproj: boolean;
    cpu_moe: boolean;
    n_cpu_moe: number;
    override_tensor_buffer_t: string;
    split_mode: string;
    main_gpu: number;
    n_parallel: number;
    cont_batching: boolean;
    mlock: boolean;
    no_kv_offload: boolean;
    cache_type_k: string;
    cache_type_v: string;
    defrag_thold: number;
    rope_scaling: string;
    rope_scale: number;
    rope_freq_base: number;
    rope_freq_scale: number;
    ctx_shift: boolean;
    extra_env: Record<string, string>;
    embeddings_strategy: 'separate_session';
    scheduler: {
      keep_alive_secs: number;
      max_loaded_models: number;
      max_queue: number;
      queue_wait_timeout_ms: number;
      vram_recovery_timeout_ms: number;
      vram_recovery_poll_ms: number;
      vram_recovery_threshold: number;
      expiration_tick_ms: number;
    };
  };

  type OxideBridge = {
    loadModelFromManager?: (args: {
      path: string;
      format: 'gguf';
      runtime?: ManagerRuntimePatch;
    }) => void;
    unloadGGUF?: () => Promise<void> | void;
    getState?: () => OxideStateSnapshot;
    getRuntimeConfig?: () => ManagerRuntimeSnapshot;
    setRuntimeConfig?: (patch: ManagerRuntimePatch) => void;
    getSystemPrompt?: () => string;
    setSystemPrompt?: (prompt: string) => Promise<void> | void;
  };

  const SORT_MODE_MAP: Record<SortMode, { field: SortField; order: SortOrder }> = {
    recent: { field: 'created_at', order: 'desc' },
    name: { field: 'name', order: 'asc' },
    size: { field: 'file_size', order: 'desc' },
    architecture: { field: 'architecture', order: 'asc' },
  };
  const DETAILS_SHEET_BREAKPOINT = 1024;
  const FLASH_ATTN_OPTIONS: RuntimeFlashAttnMode[] = ['auto', 'on', 'off'];
  const CACHE_TYPE_OPTIONS = ['f16', 'f32', 'q8_0', 'q6_k', 'q5_1', 'q5_0', 'q4_1', 'q4_0'];
  const DETAILS_TAB_ORDER: DetailsTab[] = ['info', 'load', 'inference'];

  const validationVariants: Record<
    ValidationLevel,
    'default' | 'secondary' | 'destructive' | 'outline'
  > = {
    ok: 'default',
    warning: 'secondary',
    error: 'destructive',
  };

  let metadataExpanded = $state(false);
  let editingModelPath = $state<string | null>(null);
  let editPublisher = $state('');
  let editName = $state('');
  let searchQuery = $state('');
  let viewMode = $state<ViewMode>('list');
  let sortMode = $state<SortMode>('name');
  let pendingLoadPath = $state<string | null>(null);
  let pendingUnloadPath = $state<string | null>(null);
  let isCompactViewport = $state(false);
  let detailsSheetOpen = $state(false);
  let detailsTab = $state<DetailsTab>('info');
  let deleteCandidate = $state<ModelInfo | null>(null);
  let deleteDialogOpen = $state(false);
  let runtimeCtxLimit = $state(4096);
  let runtimeUseCustomParams = $state(false);
  let runtimeTemperatureEnabled = $state(false);
  let runtimeTemperature = $state(0.8);
  let runtimeTopKEnabled = $state(false);
  let runtimeTopK = $state(40);
  let runtimeTopPEnabled = $state(false);
  let runtimeTopP = $state(0.9);
  let runtimeMinPEnabled = $state(false);
  let runtimeMinP = $state(0.05);
  let runtimeRepeatPenaltyEnabled = $state(false);
  let runtimeRepeatPenalty = $state(1.1);
  let runtimeMaxNewTokensEnabled = $state(false);
  let runtimeMaxNewTokens = $state(1024);
  let runtimeSeedEnabled = $state(false);
  let runtimeSeed = $state(42);
  let runtimeStopSequencesText = $state('');
  let runtimeReasoningParseEnabled = $state(true);
  let runtimeReasoningStartTag = $state('<think>');
  let runtimeReasoningEndTag = $state('</think>');
  let runtimeStructuredOutputEnabled = $state(false);
  let runtimeSplitPrompt = $state(false);
  let runtimeVerbosePrompt = $state(false);
  let runtimeTracing = $state(false);
  let runtimeSystemPrompt = $state('');
  let runtimeGpuLayers = $state(100);
  let runtimeThreads = $state(0);
  let runtimeThreadsBatch = $state(0);
  let runtimeBatchSize = $state(512);
  let runtimeUBatchSize = $state(512);
  let runtimeFlashAttn = $state<RuntimeFlashAttnMode>('auto');
  let runtimeMaxConcurrentPredictions = $state(1);
  let runtimeUnifiedKvCache = $state(true);
  let runtimeOffloadKvCache = $state(true);
  let runtimeKeepInMemory = $state(false);
  let runtimeTryMmap = $state(true);
  let runtimeCpuMoe = $state(false);
  let runtimeCpuMoeCount = $state(0);
  let runtimeRopeFreqBaseAuto = $state(true);
  let runtimeRopeFreqBase = $state(0);
  let runtimeRopeFreqScaleAuto = $state(true);
  let runtimeRopeFreqScale = $state(1);
  let runtimeCacheTypeK = $state('f16');
  let runtimeCacheTypeV = $state('f16');
  let runtimeCtxShift = $state(false);
  let runtimeSyncedModelPath = $state('');
  let applyingRuntimeConfig = $state(false);
  let savingSystemPrompt = $state(false);

  let diskTotalBytes = $state(0);
  let diskFreeBytes = $state(0);

  let _unlistenLoadProgress: (() => void) | null = null;
  let _pendingLoadTimer: ReturnType<typeof setTimeout> | null = null;
  let _pendingUnloadTimer: ReturnType<typeof setTimeout> | null = null;
  let _wasCompactViewport = false;

  const rowNodes = new Map<string, HTMLDivElement>();

  const totalModelsSizeLabel = $derived(LocalModelsService.formatFileSize($totalModelsSize));
  const selectedModelPath = $derived($selectedModel?.path ?? '');
  const modelsDiskShare = $derived(
    diskTotalBytes > 0 ? Math.min(100, Math.max(0, ($totalModelsSize / diskTotalBytes) * 100)) : 0,
  );

  function deriveSortMode(field: SortField, order: SortOrder): SortMode {
    if (field === 'created_at' && order === 'desc') return 'recent';
    if (field === 'file_size' && order === 'desc') return 'size';
    if (field === 'architecture' && order === 'asc') return 'architecture';
    return 'name';
  }

  $effect(() => {
    const synced = deriveSortMode($sortOptions.field, $sortOptions.order);
    if (sortMode !== synced) {
      sortMode = synced;
    }
  });

  $effect(() => {
    if (pendingLoadPath && isModelPathLoaded(pendingLoadPath, $loadedModelIds)) {
      pendingLoadPath = null;
    }
  });

  $effect(() => {
    if (pendingUnloadPath && !isModelPathLoaded(pendingUnloadPath, $loadedModelIds)) {
      pendingUnloadPath = null;
    }
  });

  $effect(() => {
    if (!deleteDialogOpen) {
      deleteCandidate = null;
    }
  });

  $effect(() => {
    if (!isCompactViewport || !selectedModelPath) {
      detailsSheetOpen = false;
    }
  });

  $effect(() => {
    if (isCompactViewport !== _wasCompactViewport) {
      if (isCompactViewport) {
        detailsSheetOpen = false;
      }
      _wasCompactViewport = isCompactViewport;
    }
  });

  $effect(() => {
    if (!selectedModelPath) {
      runtimeSyncedModelPath = '';
      detailsTab = 'info';
      return;
    }
    if (runtimeSyncedModelPath === selectedModelPath) return;
    runtimeSyncedModelPath = selectedModelPath;
    detailsTab = 'info';
    void syncRuntimeFromBridge();
  });

  function registerRow(node: HTMLDivElement, modelPath: string) {
    rowNodes.set(modelPath, node);

    return {
      update(nextPath: string) {
        if (nextPath === modelPath) return;
        rowNodes.delete(modelPath);
        modelPath = nextPath;
        rowNodes.set(modelPath, node);
      },
      destroy() {
        rowNodes.delete(modelPath);
      },
    };
  }

  async function refreshDiskUsage() {
    try {
      const usage = await hardwareService.getSystemUsage();
      diskTotalBytes = usage.disk_total_bytes || 0;
      diskFreeBytes = usage.disk_free_bytes || 0;
    } catch (err) {
      console.warn('Failed to refresh disk usage for model manager', err);
      diskTotalBytes = 0;
      diskFreeBytes = 0;
    }
  }

  function updateViewportMode() {
    if (typeof window === 'undefined') return;
    isCompactViewport = window.innerWidth < DETAILS_SHEET_BREAKPOINT;
  }

  function getPublisher(model: ModelInfo): string {
    if (model.source_repo_id) {
      return model.source_repo_id.split('/')[0] || ($t('models.local.unknownPublisher') || 'local');
    }
    return model.metadata?.author || ($t('models.local.unknownPublisher') || 'local');
  }

  function getModelStatus(model: ModelInfo): ModelStatus {
    if (pendingUnloadPath === model.path) return 'unloading';
    if (isCurrentLoadedModel(model)) return 'loaded';
    if (isModelPathLoaded(model.path, $loadedModelIds)) return 'loaded';
    if (pendingLoadPath === model.path) return 'loading';
    return 'available';
  }

  function statusDotClass(status: ModelStatus): string {
    if (status === 'loaded') return 'bg-emerald-400 shadow-[0_0_0_3px_rgba(16,185,129,0.16)]';
    if (status === 'unloading') return 'bg-amber-400 animate-pulse shadow-[0_0_0_3px_rgba(251,191,36,0.16)]';
    if (status === 'loading') return 'bg-amber-400 animate-pulse shadow-[0_0_0_3px_rgba(251,191,36,0.16)]';
    return 'bg-muted-foreground/50';
  }

  function statusLabel(status: ModelStatus): string {
    if (status === 'loaded') return $t('models.local.status.loaded') || 'Loaded';
    if (status === 'unloading') return $t('models.local.status.unloading') || 'Unloading';
    if (status === 'loading') return $t('models.local.status.loading') || 'Loading';
    return $t('models.local.status.available') || 'Available';
  }

  function clampNumber(value: number, min: number, max: number): number {
    return Math.min(max, Math.max(min, value));
  }

  function getParentPath(path: string): string {
    const idx = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
    return idx > 0 ? path.slice(0, idx) : path;
  }

  function getMaxContextForSelectedModel(): number {
    const modelContext = Number($selectedModel?.context_length ?? 0);
    if (Number.isFinite(modelContext) && modelContext > 0) {
      return Math.max(512, Math.floor(modelContext));
    }
    return 131072;
  }

  function getModelSupportsText(): string {
    const supports = String($selectedModel?.context_length ?? getMaxContextForSelectedModel());
    try {
      return $t('models.local.loadTab.modelSupports', { value: supports }) || `Model supports up to ${supports} tokens`;
    } catch {
      return `Model supports up to ${supports} tokens`;
    }
  }

  function getDetailsTabLabel(tab: DetailsTab): string {
    if (tab === 'load') return $t('models.local.tabs.load') || 'Load';
    if (tab === 'inference') return $t('models.local.tabs.inference') || 'Inference';
    return $t('models.local.tabs.info') || 'Info';
  }

  function getDetailsTabIcon(tab: DetailsTab) {
    if (tab === 'load') return DownloadSimple;
    if (tab === 'inference') return Cpu;
    return Info;
  }

  function buildRuntimePatch(): ManagerRuntimePatch {
    return {
      ctx_limit_value: Math.max(1, Math.floor(runtimeCtxLimit)),
      use_custom_params: runtimeUseCustomParams,
      temperature: clampNumber(runtimeTemperature, 0, 2),
      temperature_enabled: runtimeTemperatureEnabled,
      top_k_value: Math.max(1, Math.floor(runtimeTopK)),
      top_k_enabled: runtimeTopKEnabled,
      top_p_value: clampNumber(runtimeTopP, 0, 1),
      top_p_enabled: runtimeTopPEnabled,
      min_p_value: clampNumber(runtimeMinP, 0, 1),
      min_p_enabled: runtimeMinPEnabled,
      repeat_penalty_value: clampNumber(runtimeRepeatPenalty, 0.1, 2),
      repeat_penalty_enabled: runtimeRepeatPenaltyEnabled,
      max_new_tokens_value: Math.max(1, Math.floor(runtimeMaxNewTokens)),
      max_new_tokens_enabled: runtimeMaxNewTokensEnabled,
      seed_value: Math.max(0, Math.floor(runtimeSeed)),
      seed_enabled: runtimeSeedEnabled,
      stop_sequences_text: runtimeStopSequencesText,
      reasoning_parse_enabled: runtimeReasoningParseEnabled,
      reasoning_start_tag: runtimeReasoningStartTag || '<think>',
      reasoning_end_tag: runtimeReasoningEndTag || '</think>',
      structured_output_enabled: runtimeStructuredOutputEnabled,
      split_prompt: runtimeSplitPrompt,
      verbose_prompt: runtimeVerbosePrompt,
      tracing: runtimeTracing,
    };
  }

  function applyRuntimeSnapshot(snapshot: ManagerRuntimeSnapshot) {
    runtimeCtxLimit = Math.max(1, Math.floor(Number(snapshot.ctx_limit_value ?? runtimeCtxLimit)));
    runtimeUseCustomParams = Boolean(snapshot.use_custom_params ?? runtimeUseCustomParams);
    runtimeTemperature = clampNumber(Number(snapshot.temperature ?? runtimeTemperature), 0, 2);
    runtimeTemperatureEnabled = Boolean(snapshot.temperature_enabled ?? runtimeTemperatureEnabled);
    runtimeTopK = Math.max(1, Math.floor(Number(snapshot.top_k_value ?? runtimeTopK)));
    runtimeTopKEnabled = Boolean(snapshot.top_k_enabled ?? runtimeTopKEnabled);
    runtimeTopP = clampNumber(Number(snapshot.top_p_value ?? runtimeTopP), 0, 1);
    runtimeTopPEnabled = Boolean(snapshot.top_p_enabled ?? runtimeTopPEnabled);
    runtimeMinP = clampNumber(Number(snapshot.min_p_value ?? runtimeMinP), 0, 1);
    runtimeMinPEnabled = Boolean(snapshot.min_p_enabled ?? runtimeMinPEnabled);
    runtimeRepeatPenalty = clampNumber(
      Number(snapshot.repeat_penalty_value ?? runtimeRepeatPenalty),
      0.1,
      2,
    );
    runtimeRepeatPenaltyEnabled = Boolean(
      snapshot.repeat_penalty_enabled ?? runtimeRepeatPenaltyEnabled,
    );
    runtimeMaxNewTokens = Math.max(
      1,
      Math.floor(Number(snapshot.max_new_tokens_value ?? runtimeMaxNewTokens)),
    );
    runtimeMaxNewTokensEnabled = Boolean(
      snapshot.max_new_tokens_enabled ?? runtimeMaxNewTokensEnabled,
    );
    runtimeSeed = Math.max(0, Math.floor(Number(snapshot.seed_value ?? runtimeSeed)));
    runtimeSeedEnabled = Boolean(snapshot.seed_enabled ?? runtimeSeedEnabled);
    runtimeStopSequencesText = String(snapshot.stop_sequences_text ?? runtimeStopSequencesText);
    runtimeReasoningParseEnabled = Boolean(
      snapshot.reasoning_parse_enabled ?? runtimeReasoningParseEnabled,
    );
    runtimeReasoningStartTag = String(snapshot.reasoning_start_tag ?? runtimeReasoningStartTag);
    runtimeReasoningEndTag = String(snapshot.reasoning_end_tag ?? runtimeReasoningEndTag);
    runtimeStructuredOutputEnabled = Boolean(
      snapshot.structured_output_enabled ?? runtimeStructuredOutputEnabled,
    );
    runtimeSplitPrompt = Boolean(snapshot.split_prompt ?? runtimeSplitPrompt);
    runtimeVerbosePrompt = Boolean(snapshot.verbose_prompt ?? runtimeVerbosePrompt);
    runtimeTracing = Boolean(snapshot.tracing ?? runtimeTracing);
  }

  function normalizeFlashAttn(value: string | undefined): RuntimeFlashAttnMode {
    if (value === 'on' || value === 'off') return value;
    return 'auto';
  }

  function applyLoadRuntimeSnapshot(snapshot: LlamaRuntimeConfig) {
    runtimeGpuLayers = Math.max(0, Math.floor(Number(snapshot.n_gpu_layers ?? runtimeGpuLayers)));
    runtimeThreads = Math.max(0, Math.floor(Number(snapshot.threads ?? runtimeThreads)));
    runtimeThreadsBatch = Math.max(
      0,
      Math.floor(Number(snapshot.threads_batch ?? runtimeThreadsBatch)),
    );
    runtimeBatchSize = Math.max(1, Math.floor(Number(snapshot.batch_size ?? runtimeBatchSize)));
    runtimeUBatchSize = Math.max(1, Math.floor(Number(snapshot.ubatch_size ?? runtimeUBatchSize)));
    runtimeFlashAttn = normalizeFlashAttn(snapshot.flash_attn);
    runtimeMaxConcurrentPredictions = Math.max(
      1,
      Math.floor(Number(snapshot.n_parallel ?? runtimeMaxConcurrentPredictions)),
    );
    runtimeUnifiedKvCache = Boolean(snapshot.cont_batching ?? runtimeUnifiedKvCache);
    runtimeOffloadKvCache = !Boolean(snapshot.no_kv_offload ?? !runtimeOffloadKvCache);
    runtimeKeepInMemory = Boolean(snapshot.mlock ?? runtimeKeepInMemory);
    runtimeCpuMoe = Boolean(snapshot.cpu_moe ?? runtimeCpuMoe);
    runtimeCpuMoeCount = Math.max(0, Math.floor(Number(snapshot.n_cpu_moe ?? runtimeCpuMoeCount)));
    runtimeCacheTypeK = String(snapshot.cache_type_k ?? (runtimeCacheTypeK || 'f16'));
    runtimeCacheTypeV = String(snapshot.cache_type_v ?? (runtimeCacheTypeV || 'f16'));
    runtimeCtxShift = Boolean(snapshot.ctx_shift ?? runtimeCtxShift);
    const ctxSize = Number(snapshot.ctx_size ?? runtimeCtxLimit);
    if (Number.isFinite(ctxSize) && ctxSize > 0) {
      runtimeCtxLimit = Math.max(1, Math.floor(ctxSize));
    }

    const ropeFreqBase = Number(snapshot.rope_freq_base ?? 0);
    runtimeRopeFreqBaseAuto = !Number.isFinite(ropeFreqBase) || ropeFreqBase === 0;
    runtimeRopeFreqBase = runtimeRopeFreqBaseAuto ? 0 : ropeFreqBase;

    const ropeFreqScale = Number(snapshot.rope_freq_scale ?? 1);
    runtimeRopeFreqScaleAuto =
      !Number.isFinite(ropeFreqScale) || Math.abs(ropeFreqScale - 1) < 0.000_001;
    runtimeRopeFreqScale = runtimeRopeFreqScaleAuto ? 1 : ropeFreqScale;

    const mappingMode = String(snapshot.extra_env?.OXIDE_MEMORY_MAPPING ?? '').toLowerCase();
    runtimeTryMmap = mappingMode !== 'ram';
  }

  async function fetchBackendRuntimeConfig(): Promise<LlamaRuntimeConfig | null> {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return await invoke<LlamaRuntimeConfig>('get_llama_runtime_config');
    } catch (err) {
      console.warn('Failed to read runtime config', err);
      return null;
    }
  }

  async function syncLoadRuntimeFromBackend() {
    const snapshot = await fetchBackendRuntimeConfig();
    if (!snapshot) return;
    applyLoadRuntimeSnapshot(snapshot);
  }

  async function persistLoadRuntimeConfig() {
    const snapshot = await fetchBackendRuntimeConfig();
    if (!snapshot) return;

    const nextExtraEnv = { ...(snapshot.extra_env ?? {}) };
    nextExtraEnv.OXIDE_MEMORY_MAPPING = runtimeTryMmap ? 'mmap' : 'ram';

    const nextConfig: LlamaRuntimeConfig = {
      ...snapshot,
      n_gpu_layers: Math.max(0, Math.floor(runtimeGpuLayers)),
      threads: Math.max(0, Math.floor(runtimeThreads)),
      threads_batch: Math.max(0, Math.floor(runtimeThreadsBatch)),
      ctx_size: Math.max(1, Math.floor(runtimeCtxLimit)),
      batch_size: Math.max(1, Math.floor(runtimeBatchSize)),
      ubatch_size: Math.max(1, Math.floor(runtimeUBatchSize)),
      flash_attn: runtimeFlashAttn,
      n_parallel: Math.max(1, Math.floor(runtimeMaxConcurrentPredictions)),
      cont_batching: runtimeUnifiedKvCache,
      cpu_moe: runtimeCpuMoe,
      n_cpu_moe: runtimeCpuMoe ? Math.max(0, Math.floor(runtimeCpuMoeCount)) : 0,
      mlock: runtimeKeepInMemory,
      no_kv_offload: !runtimeOffloadKvCache,
      cache_type_k: runtimeCacheTypeK || 'f16',
      cache_type_v: runtimeCacheTypeV || 'f16',
      ctx_shift: runtimeCtxShift,
      rope_freq_base: runtimeRopeFreqBaseAuto ? 0 : runtimeRopeFreqBase,
      rope_freq_scale: runtimeRopeFreqScaleAuto ? 1 : runtimeRopeFreqScale,
      extra_env: nextExtraEnv,
    };

    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('set_llama_runtime_config', { config: nextConfig });
  }

  async function syncRuntimeFromBridge() {
    const bridge = getBridge();
    if (bridge) {
      const runtimeSnapshot = bridge.getRuntimeConfig?.();
      if (runtimeSnapshot) {
        applyRuntimeSnapshot(runtimeSnapshot);
      }

      const prompt = bridge.getSystemPrompt?.();
      if (typeof prompt === 'string') {
        runtimeSystemPrompt = prompt;
      }
    }

    await syncLoadRuntimeFromBackend();
  }

  async function applyRuntimeToBridge() {
    const bridge = getBridge();
    if (!bridge?.setRuntimeConfig) return;

    applyingRuntimeConfig = true;
    try {
      bridge.setRuntimeConfig(buildRuntimePatch());
    } finally {
      applyingRuntimeConfig = false;
    }
  }

  async function applyLoadSettings() {
    applyingRuntimeConfig = true;
    try {
      await persistLoadRuntimeConfig();
      await applyRuntimeToBridge();
    } catch (err) {
      console.error('Failed to apply load runtime settings', err);
    } finally {
      applyingRuntimeConfig = false;
    }
  }

  async function applyInferenceSettings() {
    applyingRuntimeConfig = true;
    try {
      await persistLoadRuntimeConfig();
      await applyRuntimeToBridge();
    } catch (err) {
      console.error('Failed to apply inference runtime settings', err);
    } finally {
      applyingRuntimeConfig = false;
    }
  }

  async function saveSystemPromptToBridge() {
    const bridge = getBridge();
    if (!bridge?.setSystemPrompt) return;

    savingSystemPrompt = true;
    try {
      await bridge.setSystemPrompt(runtimeSystemPrompt);
    } catch (err) {
      console.error('Failed to save system prompt', err);
    } finally {
      savingSystemPrompt = false;
    }
  }

  function updateFilter(
    partial: Parameters<typeof filterOptions.update>[0] extends (prev: infer P) => unknown
      ? Partial<P>
      : never,
  ) {
    filterOptions.update((prev) => ({
      ...prev,
      ...partial,
    }));
  }

  function startEditing(model: ModelInfo, e: Event) {
    e.stopPropagation();
    editingModelPath = model.path;
    editPublisher = model.metadata?.author ?? model.source_repo_id?.split('/')[0] ?? 'local';
    editName = model.name;
  }

  function cancelEditing() {
    editingModelPath = null;
  }

  async function saveEditing(model: ModelInfo) {
    try {
      await LocalModelsService.updateModelMetadata(
        model.path,
        editName || null,
        editPublisher || null,
      );
      models.update(($models) =>
        $models.map((entry) => {
          if (entry.path !== model.path) return entry;
          return {
            ...entry,
            name: editName || entry.name,
            metadata: {
              ...entry.metadata,
              author: editPublisher || entry.metadata?.author,
            },
          };
        }),
      );
    } catch (err) {
      console.error('Failed to save model metadata', err);
    } finally {
      editingModelPath = null;
    }
  }

  function handleSearchInput(e: Event) {
    const value = (e.target as HTMLInputElement).value;
    searchQuery = value;
    updateFilter({ searchText: value });
  }

  function handleSortModeChange(value: string) {
    const next = (value as SortMode) || 'name';
    const mapped = SORT_MODE_MAP[next] || SORT_MODE_MAP.name;
    sortMode = next;
    sortOptions.set(mapped);
  }

  async function handleSelectFolder() {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = (await open({
        directory: true,
        multiple: false,
        recursive: false,
      })) as string | string[] | undefined;
      const path = Array.isArray(selected) ? selected[0] : selected;
      if (typeof path === 'string' && path.length > 0) {
        folderPath.set(path);
        await scanFolder(path, true);
        await refreshDiskUsage();
      }
    } catch (err) {
      console.error('Failed to select models folder', err);
    }
  }

  async function handleRescan() {
    if (!$folderPath) return;
    await scanFolder($folderPath, true);
    await refreshDiskUsage();
  }

  function requestDelete(model: ModelInfo, e?: Event) {
    e?.stopPropagation();
    deleteCandidate = model;
    deleteDialogOpen = true;
  }

  async function confirmDelete() {
    if (!deleteCandidate) return;
    const target = deleteCandidate;
    deleteDialogOpen = false;

    try {
      await deleteModel(target.path);
    } catch (err) {
      console.error('Failed to delete model', err);
    }
  }

  function getBridge(): OxideBridge | null {
    const candidate = (window as unknown as { __oxide?: OxideBridge }).__oxide;
    return candidate || null;
  }

  function getCurrentLoadedModelPath(): string | null {
    const bridge = getBridge();
    const path = bridge?.getState?.().currentModelPath;
    return typeof path === 'string' && path.length > 0 ? path : null;
  }

  function isCurrentLoadedModel(model: ModelInfo): boolean {
    const bridge = getBridge();
    const state = bridge?.getState?.();
    if (!state?.isLoaded) return false;
    return areModelPathsEqual(state.currentModelPath, model.path);
  }

  function canUnloadModel(model: ModelInfo): boolean {
    const bridge = getBridge();
    if (!bridge?.unloadGGUF) return false;

    const currentPath = getCurrentLoadedModelPath();
    if (currentPath) {
      return areModelPathsEqual(currentPath, model.path);
    }

    return $loadedModelIds.length === 1 && isModelPathLoaded(model.path, $loadedModelIds);
  }

  async function refreshLoadedModelsSnapshot() {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const ids = await invoke<string[]>('get_loaded_models');
      loadedModelIds.set(ids || []);
    } catch (err) {
      console.warn('Failed to refresh loaded models snapshot', err);
    }
  }

  function schedulePendingLoadReset(path: string) {
    if (_pendingLoadTimer) {
      clearTimeout(_pendingLoadTimer);
      _pendingLoadTimer = null;
    }

    _pendingLoadTimer = setTimeout(() => {
      if (pendingLoadPath === path && !isModelPathLoaded(path, $loadedModelIds)) {
        pendingLoadPath = null;
      }
      _pendingLoadTimer = null;
    }, 8000);
  }

  function schedulePendingUnloadReset(path: string) {
    if (_pendingUnloadTimer) {
      clearTimeout(_pendingUnloadTimer);
      _pendingUnloadTimer = null;
    }

    _pendingUnloadTimer = setTimeout(() => {
      if (pendingUnloadPath === path) {
        pendingUnloadPath = null;
      }
      _pendingUnloadTimer = null;
    }, 8000);
  }

  async function loadModel(model: ModelInfo, e?: Event, runtime?: ManagerRuntimePatch) {
    e?.stopPropagation();
    const bridge = getBridge();
    if (!bridge?.loadModelFromManager) return;

    try {
      await persistLoadRuntimeConfig();
    } catch (err) {
      console.warn('Failed to persist load runtime config before loading model', err);
    }

    selectedModel.set(model);
    pendingLoadPath = model.path;
    bridge.loadModelFromManager({
      path: model.path,
      format: 'gguf',
      runtime,
    });
    schedulePendingLoadReset(model.path);
    void refreshLoadedModelsSnapshot();
  }

  async function unloadModel(model: ModelInfo, e?: Event) {
    e?.stopPropagation();
    const bridge = getBridge();
    if (!bridge?.unloadGGUF || !canUnloadModel(model)) return;

    pendingUnloadPath = model.path;
    schedulePendingUnloadReset(model.path);

    try {
      await bridge.unloadGGUF();
    } catch (err) {
      console.error('Failed to unload model', err);
    } finally {
      await refreshLoadedModelsSnapshot();
    }
  }

  async function loadSelectedModel() {
    const model = get(selectedModel);
    if (!model) return;
    const status = getModelStatus(model);
    if (status === 'loaded' && canUnloadModel(model)) {
      void unloadModel(model);
      return;
    }
    const runtimePatch = buildRuntimePatch();
    const bridge = getBridge();
    bridge?.setRuntimeConfig?.(runtimePatch);
    await loadModel(model, undefined, runtimePatch);
  }

  async function useModelInNewChat() {
    if (!$selectedModel) return;
    await loadSelectedModel();
    await goto('/');
  }

  function toggleModelSelection(model: ModelInfo) {
    if ($selectedModel?.path !== model.path) selectedModel.set(model);
    if (isCompactViewport) detailsSheetOpen = true;
  }

  async function copyModelPath(model: ModelInfo, e?: Event) {
    e?.stopPropagation();
    try {
      await navigator.clipboard.writeText(model.path);
    } catch (err) {
      console.error('Failed to copy model path', err);
    }
  }

  async function openModelLocation(model: ModelInfo, e?: Event) {
    e?.stopPropagation();
    try {
      const { openPath } = await import('@tauri-apps/plugin-opener');
      await openPath(getParentPath(model.path));
    } catch (err) {
      console.error('Failed to open model location', err);
    }
  }

  function openCatalog() {
    activePageTab.set('remote');
  }

  async function scrollSelectedRowIntoView() {
    await tick();
    const selectedPath = get(selectedModel)?.path;
    if (!selectedPath) return;
    rowNodes.get(selectedPath)?.scrollIntoView({ block: 'nearest' });
  }

  function handleListKeyboard(event: KeyboardEvent) {
    if (!$filteredModels.length || editingModelPath) return;

    const currentPath = $selectedModel?.path;
    const currentIndex = currentPath
      ? $filteredModels.findIndex((model) => model.path === currentPath)
      : -1;

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      const nextIndex = Math.min(currentIndex + 1, $filteredModels.length - 1);
      selectedModel.set($filteredModels[nextIndex]);
      if (isCompactViewport) detailsSheetOpen = true;
      void scrollSelectedRowIntoView();
      return;
    }

    if (event.key === 'ArrowUp') {
      event.preventDefault();
      const nextIndex = currentIndex <= 0 ? 0 : currentIndex - 1;
      selectedModel.set($filteredModels[nextIndex]);
      if (isCompactViewport) detailsSheetOpen = true;
      void scrollSelectedRowIntoView();
      return;
    }

    if (event.key === 'Enter') {
      event.preventDefault();
      if ($selectedModel) {
        const status = getModelStatus($selectedModel);
        if (status === 'loaded' && canUnloadModel($selectedModel)) {
          void unloadModel($selectedModel);
        } else {
          void loadModel($selectedModel);
        }
      }
      return;
    }

    if (event.key === 'Delete' && $selectedModel) {
      event.preventDefault();
      requestDelete($selectedModel);
    }
  }

  function summaryText(): string {
    const modelsLabel = $t('models.local.summary.models') || 'models';
    return `${$modelsCount} ${modelsLabel} · ${totalModelsSizeLabel}`;
  }

  onMount(async () => {
    updateViewportMode();
    window.addEventListener('resize', updateViewportMode);
    await syncRuntimeFromBridge();

    if ($folderPath) {
      await scanFolder($folderPath);
    }

    await refreshDiskUsage();
    _unlistenLoadProgress = await initLoadedModels();
  });

  onDestroy(() => {
    window.removeEventListener('resize', updateViewportMode);

    if (_unlistenLoadProgress) {
      _unlistenLoadProgress();
      _unlistenLoadProgress = null;
    }

    if (_pendingLoadTimer) {
      clearTimeout(_pendingLoadTimer);
      _pendingLoadTimer = null;
    }

    if (_pendingUnloadTimer) {
      clearTimeout(_pendingUnloadTimer);
      _pendingUnloadTimer = null;
    }
  });
</script>

{#snippet ModelDetailsPanelContent()}
  {#if $selectedModel}
    {@const selectedStatus = getModelStatus($selectedModel)}
    <Card.Header class="pb-2 shrink-0 space-y-3">
      <div class="flex items-start justify-between gap-3">
        <div class="min-w-0">
          <Card.Title class="truncate text-lg">{$selectedModel.name}</Card.Title>
          <div class="mt-1 flex items-center gap-2 text-xs text-muted-foreground">
            <span class={`size-2 rounded-full ${statusDotClass(selectedStatus)}`}></span>
            <span>{statusLabel(selectedStatus)}</span>
          </div>
        </div>
        <div class="flex items-center gap-1">
          <Badge variant="outline" class="uppercase text-[10px]">{$selectedModel.format}</Badge>
        </div>
      </div>

      <div class="grid grid-cols-2 gap-2">
        <Button variant="outline" size="sm" onclick={useModelInNewChat}>
          <ChatsCircle class="size-4 mr-1.5" />
          {$t('models.local.details.useInNewChat') || 'Use in New Chat'}
        </Button>
        <Button
          size="sm"
          variant={selectedStatus === 'loaded' && canUnloadModel($selectedModel) ? 'outline' : selectedStatus === 'loaded' ? 'secondary' : 'default'}
          disabled={selectedStatus === 'loading' || selectedStatus === 'unloading' || (selectedStatus === 'loaded' && !canUnloadModel($selectedModel))}
          onclick={loadSelectedModel}
        >
          {#if selectedStatus === 'loading' || selectedStatus === 'unloading'}
            <CircleNotch class="size-4 mr-1 animate-spin" />
            {selectedStatus === 'unloading'
              ? $t('models.local.status.unloading') || 'Unloading'
              : $t('models.local.status.loading') || 'Loading'}
          {:else if selectedStatus === 'loaded'}
            {#if canUnloadModel($selectedModel)}
              <X class="size-4 mr-1" />
              {$t('models.local.unload') || 'Unload'}
            {:else}
              <Check class="size-4 mr-1" />
              {$t('models.local.status.loaded') || 'Loaded'}
            {/if}
          {:else}
            <Play class="size-4 mr-1" />
            {$t('models.local.details.loadModel') || 'Load Model'}
          {/if}
        </Button>
      </div>

      {#if isCompactViewport}
        {@const CurrentDetailsTabIcon = getDetailsTabIcon(detailsTab)}
        <div data-no-drag>
          <Breadcrumb.Root>
            <Breadcrumb.List>
              <Breadcrumb.Item>
                <span class="text-xs font-semibold text-foreground/90">
                  {$t('models.local.title') || 'Local Models'}
                </span>
              </Breadcrumb.Item>
              <Breadcrumb.Separator />
              <Breadcrumb.Item>
                <DropdownMenu.Root>
                  <DropdownMenu.Trigger>
                    {#snippet child({ props })}
                      <Button {...props} variant="ghost" size="sm" class="h-7 gap-1 px-2">
                        <CurrentDetailsTabIcon class="size-3.5" />
                        {getDetailsTabLabel(detailsTab)}
                        <CaretDown class="size-3" />
                      </Button>
                    {/snippet}
                  </DropdownMenu.Trigger>
                  <DropdownMenu.Content align="start" sideOffset={6} class="w-56 z-[1400]">
                    {#each DETAILS_TAB_ORDER as tab (tab)}
                      {@const TabIcon = getDetailsTabIcon(tab)}
                      <DropdownMenu.Item onSelect={() => (detailsTab = tab)}>
                        <TabIcon class="size-4" />
                        {getDetailsTabLabel(tab)}
                      </DropdownMenu.Item>
                    {/each}
                  </DropdownMenu.Content>
                </DropdownMenu.Root>
              </Breadcrumb.Item>
            </Breadcrumb.List>
          </Breadcrumb.Root>
        </div>
      {:else}
        <Tabs.Root bind:value={detailsTab}>
          <Tabs.List class="grid h-9 w-full grid-cols-3 rounded-lg bg-muted/35 p-0.5">
            <Tabs.Trigger value="info" class="text-xs">
              <Info class="size-3.5 mr-1" />
              {$t('models.local.tabs.info') || 'Info'}
            </Tabs.Trigger>
            <Tabs.Trigger value="load" class="text-xs">
              <DownloadSimple class="size-3.5 mr-1" />
              {$t('models.local.tabs.load') || 'Load'}
            </Tabs.Trigger>
            <Tabs.Trigger value="inference" class="text-xs">
              <Cpu class="size-3.5 mr-1" />
              {$t('models.local.tabs.inference') || 'Inference'}
            </Tabs.Trigger>
          </Tabs.List>
        </Tabs.Root>
      {/if}
    </Card.Header>

    <Tabs.Root bind:value={detailsTab} class="flex min-h-0 flex-1 flex-col">
      <Tabs.Content value="info" class="min-h-0 flex-1 overflow-y-auto custom-scrollbar data-[state=inactive]:hidden">
        <Card.Content class="space-y-4">
          <div class="space-y-2">
            <h4 class="text-sm font-medium">{$t('models.local.details.modelInformation') || 'Model Information'}</h4>
            <dl class="grid grid-cols-1 gap-2 text-sm">
              <div class="rounded-md border bg-muted/20 p-2">
                <dt class="text-xs text-muted-foreground">{$t('models.local.details.path')}</dt>
                <dd class="mt-1 break-all font-mono text-xs">{$selectedModel.path}</dd>
              </div>
              <div class="grid grid-cols-2 gap-2">
                <div class="rounded-md border bg-muted/20 p-2">
                  <dt class="text-xs text-muted-foreground">{$t('models.local.details.size')}</dt>
                  <dd class="mt-1">{LocalModelsService.formatFileSize($selectedModel.file_size)}</dd>
                </div>
                <div class="rounded-md border bg-muted/20 p-2">
                  <dt class="text-xs text-muted-foreground">{$t('models.local.details.date')}</dt>
                  <dd class="mt-1">{LocalModelsService.formatDate($selectedModel.created_at)}</dd>
                </div>
                <div class="rounded-md border bg-muted/20 p-2">
                  <dt class="text-xs text-muted-foreground">{$t('models.local.details.architecture')}</dt>
                  <dd class="mt-1">{$selectedModel.architecture ?? '—'}</dd>
                </div>
                <div class="rounded-md border bg-muted/20 p-2">
                  <dt class="text-xs text-muted-foreground">{$t('models.local.details.detected')}</dt>
                  <dd class="mt-1">{$selectedModel.detected_architecture ?? '—'}</dd>
                </div>
                <div class="rounded-md border bg-muted/20 p-2">
                  <dt class="text-xs text-muted-foreground">{$t('models.local.table.quant')}</dt>
                  <dd class="mt-1">{$selectedModel.quantization ?? '—'}</dd>
                </div>
                <div class="rounded-md border bg-muted/20 p-2">
                  <dt class="text-xs text-muted-foreground">{$t('models.local.details.context')}</dt>
                  <dd class="mt-1">{$selectedModel.context_length ?? '—'}</dd>
                </div>
              </div>
            </dl>
          </div>

          <div class="space-y-2">
            <h4 class="text-sm font-medium">{$t('models.local.details.validation')}</h4>
            <Badge variant={validationVariants[$selectedModel.validation_status?.level ?? 'warning']}>
              {$t(
                `models.local.details.${($selectedModel.validation_status?.level ?? 'warning') === 'ok' ? 'valid' : ($selectedModel.validation_status?.level ?? 'warning')}`,
              )}
            </Badge>

            {#if ($selectedModel.validation_status?.messages?.length ?? 0) > 0}
              <ul class="list-disc space-y-1 pl-4 text-xs text-muted-foreground">
                {#each $selectedModel.validation_status?.messages ?? [] as message}
                  <li>{message}</li>
                {/each}
              </ul>
            {/if}
          </div>

          {#if $selectedModel.metadata}
            <div class="space-y-2">
              <div class="flex items-center justify-between">
                <h4 class="text-sm font-medium">{$t('models.local.details.metadata.title')}</h4>
                <Button
                  variant="ghost"
                  size="sm"
                  class="h-7 px-2 text-xs"
                  onclick={() => (metadataExpanded = !metadataExpanded)}
                >
                  {metadataExpanded
                    ? $t('models.local.details.metadata.hide')
                    : $t('models.local.details.metadata.showAll')}
                  {#if metadataExpanded}
                    <CaretUp class="size-3 ml-1" />
                  {:else}
                    <CaretDown class="size-3 ml-1" />
                  {/if}
                </Button>
              </div>

              <dl class="grid grid-cols-2 gap-2 rounded-md border bg-muted/20 p-2 text-sm">
                <div>
                  <dt class="text-xs text-muted-foreground">
                    {$t('models.local.details.metadata.formatVersion')}
                  </dt>
                  <dd class="font-semibold">{$selectedModel.metadata.format_version ?? '—'}</dd>
                </div>
                <div>
                  <dt class="text-xs text-muted-foreground">
                    {$t('models.local.details.metadata.tensorCount')}
                  </dt>
                  <dd class="font-semibold">{$selectedModel.metadata.tensor_count ?? '—'}</dd>
                </div>
                <div>
                  <dt class="text-xs text-muted-foreground">
                    {$t('models.local.details.metadata.alignment')}
                  </dt>
                  <dd class="font-semibold">{$selectedModel.metadata.alignment ?? '—'}</dd>
                </div>
                <div>
                  <dt class="text-xs text-muted-foreground">
                    {$t('models.local.details.metadata.tokenCount')}
                  </dt>
                  <dd class="font-semibold">
                    {$selectedModel.vocab_size ?? $selectedModel.metadata.tokenizer_tokens?.length ?? '—'}
                  </dd>
                </div>
              </dl>

              {#if metadataExpanded && $selectedModel.metadata.custom_metadata?.length}
                <div class="overflow-hidden rounded-md border">
                  <table class="w-full text-xs">
                    <thead class="bg-muted/40">
                      <tr>
                        <th class="px-2 py-1 text-left">Key</th>
                        <th class="px-2 py-1 text-left">Value</th>
                      </tr>
                    </thead>
                    <tbody>
                      {#each $selectedModel.metadata.custom_metadata as entry (entry.key)}
                        <tr class="border-t border-border/50 align-top">
                          <td class="px-2 py-1 font-mono">{entry.key}</td>
                          <td class="px-2 py-1">
                            <pre class="whitespace-pre-wrap break-all text-[10px]">{JSON.stringify(
                              entry.value,
                              null,
                              2,
                            )}</pre>
                          </td>
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                </div>
              {/if}
            </div>
          {/if}
        </Card.Content>
      </Tabs.Content>

      <Tabs.Content value="load" class="min-h-0 flex-1 overflow-y-auto custom-scrollbar data-[state=inactive]:hidden">
        <Card.Content class="space-y-4">
          <div class="space-y-3 rounded-md border bg-muted/20 p-3">
            <div class="flex items-center justify-between gap-2">
              <h4 class="text-sm font-medium">{$t('models.local.loadTab.contextAndOffload') || 'Context and Offload'}</h4>
              <Input
                type="number"
                min="512"
                max={getMaxContextForSelectedModel()}
                step="128"
                class="h-8 w-28 text-right"
                value={runtimeCtxLimit}
                oninput={(event) => {
                  const next = Number((event.currentTarget as HTMLInputElement).value) || 512;
                  runtimeCtxLimit = Math.floor(clampNumber(next, 512, getMaxContextForSelectedModel()));
                }}
              />
            </div>

            <p class="text-xs text-muted-foreground">{getModelSupportsText()}</p>

            <Slider.Root
              type="single"
              min={512}
              max={getMaxContextForSelectedModel()}
              step={128}
              value={runtimeCtxLimit}
              onValueChange={(value) => {
                const next = Number(value ?? runtimeCtxLimit);
                runtimeCtxLimit = Math.floor(clampNumber(next, 512, getMaxContextForSelectedModel()));
              }}
            />

            <Separator.Root class="my-2" />

            <div class="grid grid-cols-[auto_1fr_auto] items-center gap-2">
              <span class="text-sm">GPU Offload</span>
              <Slider.Root
                type="single"
                min={0}
                max={200}
                step={1}
                value={runtimeGpuLayers}
                onValueChange={(value) => {
                  runtimeGpuLayers = Math.max(0, Math.floor(Number(value ?? runtimeGpuLayers)));
                }}
              />
              <Input
                type="number"
                min="0"
                step="1"
                class="h-8 w-24 text-right"
                value={runtimeGpuLayers}
                oninput={(event) => {
                  runtimeGpuLayers = Math.max(
                    0,
                    Math.floor(Number((event.currentTarget as HTMLInputElement).value) || 0),
                  );
                }}
              />
            </div>
          </div>

          <div class="space-y-3 rounded-md border bg-muted/20 p-3">
            <h4 class="text-sm font-medium">{$t('models.local.loadTab.advanced') || 'Advanced'}</h4>
            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <span class="text-sm">CPU Thread Pool Size</span>
              <Input
                type="number"
                min="0"
                step="1"
                class="h-8 w-24 text-right"
                value={runtimeThreads}
                oninput={(event) => {
                  runtimeThreads = Math.max(
                    0,
                    Math.floor(Number((event.currentTarget as HTMLInputElement).value) || 0),
                  );
                }}
              />
            </div>

            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <span class="text-sm">Threads Batch</span>
              <Input
                type="number"
                min="0"
                step="1"
                class="h-8 w-24 text-right"
                value={runtimeThreadsBatch}
                oninput={(event) => {
                  runtimeThreadsBatch = Math.max(
                    0,
                    Math.floor(Number((event.currentTarget as HTMLInputElement).value) || 0),
                  );
                }}
              />
            </div>

            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <span class="text-sm">Evaluation Batch Size</span>
              <Input
                type="number"
                min="1"
                step="1"
                class="h-8 w-24 text-right"
                value={runtimeBatchSize}
                oninput={(event) => {
                  runtimeBatchSize = Math.max(
                    1,
                    Math.floor(Number((event.currentTarget as HTMLInputElement).value) || 1),
                  );
                }}
              />
            </div>

            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <span class="text-sm">Micro Batch Size</span>
              <Input
                type="number"
                min="1"
                step="1"
                class="h-8 w-24 text-right"
                value={runtimeUBatchSize}
                oninput={(event) => {
                  runtimeUBatchSize = Math.max(
                    1,
                    Math.floor(Number((event.currentTarget as HTMLInputElement).value) || 1),
                  );
                }}
              />
            </div>

            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <span class="text-sm">Flash Attention</span>
              <Select.Root
                type="single"
                value={runtimeFlashAttn}
                onValueChange={(next) => (runtimeFlashAttn = normalizeFlashAttn(next))}
              >
                <Select.Trigger class="h-8 w-24 text-xs uppercase">{runtimeFlashAttn}</Select.Trigger>
                <Select.Content>
                  {#each FLASH_ATTN_OPTIONS as option}
                    <Select.Item value={option}>{option}</Select.Item>
                  {/each}
                </Select.Content>
              </Select.Root>
            </div>

            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <div class="space-y-1">
                <span class="text-sm">Max Concurrent Predictions</span>
                <p class="text-[11px] text-muted-foreground">Experimental</p>
              </div>
              <Input
                type="number"
                min="1"
                step="1"
                class="h-8 w-24 text-right"
                value={runtimeMaxConcurrentPredictions}
                oninput={(event) => {
                  runtimeMaxConcurrentPredictions = Math.max(
                    1,
                    Math.floor(Number((event.currentTarget as HTMLInputElement).value) || 1),
                  );
                }}
              />
            </div>

            <label class="flex items-center justify-between gap-2 text-sm">
              <div class="space-y-1">
                <span>Unified KV Cache</span>
                <p class="text-[11px] text-muted-foreground">Experimental</p>
              </div>
              <Checkbox
                checked={runtimeUnifiedKvCache}
                onCheckedChange={(v) => (runtimeUnifiedKvCache = v === true)}
              />
            </label>

            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <span class="text-sm">K Cache Quantization</span>
              <Select.Root
                type="single"
                value={runtimeCacheTypeK}
                onValueChange={(next) => (runtimeCacheTypeK = next || 'f16')}
              >
                <Select.Trigger class="h-8 w-24 text-xs uppercase">{runtimeCacheTypeK}</Select.Trigger>
                <Select.Content>
                  {#each CACHE_TYPE_OPTIONS as option}
                    <Select.Item value={option}>{option}</Select.Item>
                  {/each}
                </Select.Content>
              </Select.Root>
            </div>

            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <span class="text-sm">V Cache Quantization</span>
              <Select.Root
                type="single"
                value={runtimeCacheTypeV}
                onValueChange={(next) => (runtimeCacheTypeV = next || 'f16')}
              >
                <Select.Trigger class="h-8 w-24 text-xs uppercase">{runtimeCacheTypeV}</Select.Trigger>
                <Select.Content>
                  {#each CACHE_TYPE_OPTIONS as option}
                    <Select.Item value={option}>{option}</Select.Item>
                  {/each}
                </Select.Content>
              </Select.Root>
            </div>

            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <div class="space-y-1">
                <span class="text-sm">RoPE Frequency Base</span>
                <label class="flex items-center gap-2 text-xs text-muted-foreground">
                  <Checkbox
                    checked={runtimeRopeFreqBaseAuto}
                    onCheckedChange={(v) => (runtimeRopeFreqBaseAuto = v === true)}
                  />
                  <span>Auto</span>
                </label>
              </div>
              <Input
                type="number"
                step="0.01"
                class="h-8 w-24 text-right"
                value={runtimeRopeFreqBase}
                disabled={runtimeRopeFreqBaseAuto}
                oninput={(event) => {
                  runtimeRopeFreqBase = Number((event.currentTarget as HTMLInputElement).value) || 0;
                }}
              />
            </div>

            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <div class="space-y-1">
                <span class="text-sm">RoPE Frequency Scale</span>
                <label class="flex items-center gap-2 text-xs text-muted-foreground">
                  <Checkbox
                    checked={runtimeRopeFreqScaleAuto}
                    onCheckedChange={(v) => (runtimeRopeFreqScaleAuto = v === true)}
                  />
                  <span>Auto</span>
                </label>
              </div>
              <Input
                type="number"
                step="0.01"
                class="h-8 w-24 text-right"
                value={runtimeRopeFreqScale}
                disabled={runtimeRopeFreqScaleAuto}
                oninput={(event) => {
                  runtimeRopeFreqScale =
                    Number((event.currentTarget as HTMLInputElement).value) || 1;
                }}
              />
            </div>

            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <span class="text-sm">Number of Experts</span>
              <div class="flex items-center gap-2">
                <Checkbox
                  checked={runtimeCpuMoe}
                  onCheckedChange={(v) => (runtimeCpuMoe = v === true)}
                />
                <Input
                  type="number"
                  min="0"
                  step="1"
                  class="h-8 w-20 text-right"
                  value={runtimeCpuMoeCount}
                  disabled={!runtimeCpuMoe}
                  oninput={(event) => {
                    runtimeCpuMoeCount = Math.max(
                      0,
                      Math.floor(Number((event.currentTarget as HTMLInputElement).value) || 0),
                    );
                  }}
                />
              </div>
            </div>

            <label class="flex items-center justify-between gap-2 text-sm">
              <span>Offload KV Cache to GPU</span>
              <Checkbox
                checked={runtimeOffloadKvCache}
                onCheckedChange={(v) => (runtimeOffloadKvCache = v === true)}
              />
            </label>
            <label class="flex items-center justify-between gap-2 text-sm">
              <span>Keep Model in Memory</span>
              <Checkbox
                checked={runtimeKeepInMemory}
                onCheckedChange={(v) => (runtimeKeepInMemory = v === true)}
              />
            </label>
            <label class="flex items-center justify-between gap-2 text-sm">
              <span>Try mmap()</span>
              <Checkbox checked={runtimeTryMmap} onCheckedChange={(v) => (runtimeTryMmap = v === true)} />
            </label>
            <label class="flex items-center justify-between gap-2 text-sm">
              <span>Context Shift</span>
              <Checkbox checked={runtimeCtxShift} onCheckedChange={(v) => (runtimeCtxShift = v === true)} />
            </label>

            <Separator.Root />

            <label class="flex items-center gap-2 text-sm">
              <Checkbox checked={runtimeSplitPrompt} onCheckedChange={(v) => (runtimeSplitPrompt = v === true)} />
              <span>{$t('models.local.loadTab.splitPrompt') || 'Split prompt'}</span>
            </label>
            <label class="flex items-center gap-2 text-sm">
              <Checkbox checked={runtimeVerbosePrompt} onCheckedChange={(v) => (runtimeVerbosePrompt = v === true)} />
              <span>{$t('models.local.loadTab.verbosePrompt') || 'Verbose prompt'}</span>
            </label>
            <label class="flex items-center gap-2 text-sm">
              <Checkbox checked={runtimeTracing} onCheckedChange={(v) => (runtimeTracing = v === true)} />
              <span>{$t('models.local.loadTab.chromeTracing') || 'Chrome tracing'}</span>
            </label>
          </div>

          <Button size="sm" class="w-full" onclick={applyLoadSettings} disabled={applyingRuntimeConfig}>
            {#if applyingRuntimeConfig}
              <CircleNotch class="size-4 mr-1 animate-spin" />
            {/if}
            {$t('models.local.loadTab.apply') || 'Apply load settings'}
          </Button>
        </Card.Content>
      </Tabs.Content>

      <Tabs.Content value="inference" class="min-h-0 flex-1 overflow-y-auto custom-scrollbar data-[state=inactive]:hidden">
        <Card.Content class="space-y-4">
          <div class="space-y-2">
            <div class="flex items-center justify-between gap-2">
              <h4 class="text-sm font-medium">{$t('models.local.inferenceTab.systemPrompt') || 'System Prompt'}</h4>
              <Button
                size="sm"
                variant="outline"
                onclick={saveSystemPromptToBridge}
                disabled={savingSystemPrompt}
              >
                {#if savingSystemPrompt}
                  <CircleNotch class="size-4 mr-1 animate-spin" />
                {/if}
                {$t('models.local.inferenceTab.savePrompt') || 'Save'}
              </Button>
            </div>
            <Textarea
              rows={5}
              placeholder={$t('models.local.inferenceTab.systemPromptPlaceholder') || 'Enter system prompt'}
              value={runtimeSystemPrompt}
              oninput={(event) => (runtimeSystemPrompt = (event.currentTarget as HTMLTextAreaElement).value)}
            />
          </div>

          <div class="space-y-3 rounded-md border bg-muted/20 p-3">
            <label class="flex items-center justify-between gap-2 text-sm">
              <span>{$t('models.local.inferenceTab.useCustom') || 'Use custom sampling parameters'}</span>
              <Checkbox
                checked={runtimeUseCustomParams}
                onCheckedChange={(v) => (runtimeUseCustomParams = v === true)}
              />
            </label>

            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <label class="inline-flex items-center gap-2 text-sm">
                <Checkbox
                  checked={runtimeMaxNewTokensEnabled}
                  onCheckedChange={(v) => (runtimeMaxNewTokensEnabled = v === true)}
                />
                <span>Limit response length</span>
              </label>
              <Input
                type="number"
                min="1"
                step="1"
                class="h-8 w-24 text-right"
                value={runtimeMaxNewTokens}
                disabled={!runtimeMaxNewTokensEnabled}
                oninput={(event) => {
                  runtimeMaxNewTokens = Math.max(
                    1,
                    Math.floor(Number((event.currentTarget as HTMLInputElement).value) || 1),
                  );
                }}
              />
            </div>

            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <label class="inline-flex items-center gap-2 text-sm">
                <Checkbox checked={runtimeSeedEnabled} onCheckedChange={(v) => (runtimeSeedEnabled = v === true)} />
                <span>Seed</span>
              </label>
              <Input
                type="number"
                min="0"
                step="1"
                class="h-8 w-24 text-right"
                value={runtimeSeed}
                disabled={!runtimeSeedEnabled}
                oninput={(event) => {
                  runtimeSeed = Math.max(
                    0,
                    Math.floor(Number((event.currentTarget as HTMLInputElement).value) || 0),
                  );
                }}
              />
            </div>

            <div class="space-y-2">
              <Label class="text-sm">Stop Strings</Label>
              <Textarea
                rows={2}
                placeholder="One sequence per line"
                value={runtimeStopSequencesText}
                oninput={(event) =>
                  (runtimeStopSequencesText = (event.currentTarget as HTMLTextAreaElement).value)}
              />
            </div>

            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <span class="text-sm">CPU Threads</span>
              <Input
                type="number"
                min="0"
                step="1"
                class="h-8 w-24 text-right"
                value={runtimeThreads}
                oninput={(event) => {
                  runtimeThreads = Math.max(
                    0,
                    Math.floor(Number((event.currentTarget as HTMLInputElement).value) || 0),
                  );
                }}
              />
            </div>

            <div class="space-y-3">
              <div class="space-y-2">
                <div class="grid grid-cols-[auto_1fr_auto] items-center gap-2">
                  <label class="inline-flex items-center gap-2 text-sm">
                    <Checkbox
                      checked={runtimeTemperatureEnabled}
                      onCheckedChange={(v) => (runtimeTemperatureEnabled = v === true)}
                      disabled={!runtimeUseCustomParams}
                    />
                    <span>{$t('models.local.inferenceTab.temperature') || 'Temperature'}</span>
                  </label>
                  <Slider.Root
                    type="single"
                    min={0}
                    max={2}
                    step={0.01}
                    value={runtimeTemperature}
                    onValueChange={(value) => {
                      runtimeTemperature = clampNumber(Number(value ?? runtimeTemperature), 0, 2);
                    }}
                    disabled={!runtimeUseCustomParams || !runtimeTemperatureEnabled}
                  />
                  <Input
                    type="number"
                    min="0"
                    max="2"
                    step="0.01"
                    class="h-8 w-24 text-right"
                    value={runtimeTemperature}
                    disabled={!runtimeUseCustomParams || !runtimeTemperatureEnabled}
                    oninput={(event) => {
                      const next = Number((event.currentTarget as HTMLInputElement).value) || 0;
                      runtimeTemperature = clampNumber(next, 0, 2);
                    }}
                  />
                </div>
              </div>

              <div class="grid grid-cols-[auto_1fr_auto] items-center gap-2">
                <label class="inline-flex items-center gap-2 text-sm">
                  <Checkbox
                    checked={runtimeTopPEnabled}
                    onCheckedChange={(v) => (runtimeTopPEnabled = v === true)}
                    disabled={!runtimeUseCustomParams}
                  />
                  <span>{$t('models.local.inferenceTab.topP') || 'Top-P'}</span>
                </label>
                <Slider.Root
                  type="single"
                  min={0}
                  max={1}
                  step={0.01}
                  value={runtimeTopP}
                  onValueChange={(value) => {
                    runtimeTopP = clampNumber(Number(value ?? runtimeTopP), 0, 1);
                  }}
                  disabled={!runtimeUseCustomParams || !runtimeTopPEnabled}
                />
                <Input
                  type="number"
                  min="0"
                  max="1"
                  step="0.01"
                  class="h-8 w-24 text-right"
                  value={runtimeTopP}
                  disabled={!runtimeUseCustomParams || !runtimeTopPEnabled}
                  oninput={(event) => {
                    const next = Number((event.currentTarget as HTMLInputElement).value) || 0;
                    runtimeTopP = clampNumber(next, 0, 1);
                  }}
                />
              </div>

              <div class="grid grid-cols-[auto_1fr_auto] items-center gap-2">
                <label class="inline-flex items-center gap-2 text-sm">
                  <Checkbox
                    checked={runtimeTopKEnabled}
                    onCheckedChange={(v) => (runtimeTopKEnabled = v === true)}
                    disabled={!runtimeUseCustomParams}
                  />
                  <span>{$t('models.local.inferenceTab.topK') || 'Top-K'}</span>
                </label>
                <Slider.Root
                  type="single"
                  min={1}
                  max={200}
                  step={1}
                  value={runtimeTopK}
                  onValueChange={(value) => {
                    runtimeTopK = Math.max(1, Math.floor(Number(value ?? runtimeTopK)));
                  }}
                  disabled={!runtimeUseCustomParams || !runtimeTopKEnabled}
                />
                <Input
                  type="number"
                  min="1"
                  step="1"
                  class="h-8 w-24 text-right"
                  value={runtimeTopK}
                  disabled={!runtimeUseCustomParams || !runtimeTopKEnabled}
                  oninput={(event) => {
                    const next = Number((event.currentTarget as HTMLInputElement).value) || 1;
                    runtimeTopK = Math.max(1, Math.floor(next));
                  }}
                />
              </div>

              <div class="grid grid-cols-[auto_1fr_auto] items-center gap-2">
                <label class="inline-flex items-center gap-2 text-sm">
                  <Checkbox
                    checked={runtimeRepeatPenaltyEnabled}
                    onCheckedChange={(v) => (runtimeRepeatPenaltyEnabled = v === true)}
                    disabled={!runtimeUseCustomParams}
                  />
                  <span>{$t('models.local.inferenceTab.repeatPenalty') || 'Repeat penalty'}</span>
                </label>
                <Slider.Root
                  type="single"
                  min={0.1}
                  max={2}
                  step={0.01}
                  value={runtimeRepeatPenalty}
                  onValueChange={(value) => {
                    runtimeRepeatPenalty = clampNumber(
                      Number(value ?? runtimeRepeatPenalty),
                      0.1,
                      2,
                    );
                  }}
                  disabled={!runtimeUseCustomParams || !runtimeRepeatPenaltyEnabled}
                />
                <Input
                  type="number"
                  min="0.1"
                  max="2"
                  step="0.01"
                  class="h-8 w-24 text-right"
                  value={runtimeRepeatPenalty}
                  disabled={!runtimeUseCustomParams || !runtimeRepeatPenaltyEnabled}
                  oninput={(event) => {
                    const next = Number((event.currentTarget as HTMLInputElement).value) || 1.1;
                    runtimeRepeatPenalty = clampNumber(next, 0.1, 2);
                  }}
                />
              </div>

              <div class="grid grid-cols-[auto_1fr_auto] items-center gap-2">
                <label class="inline-flex items-center gap-2 text-sm">
                  <Checkbox
                    checked={runtimeMinPEnabled}
                    onCheckedChange={(v) => (runtimeMinPEnabled = v === true)}
                    disabled={!runtimeUseCustomParams}
                  />
                  <span>{$t('models.local.inferenceTab.minP') || 'Min-P'}</span>
                </label>
                <Slider.Root
                  type="single"
                  min={0}
                  max={1}
                  step={0.01}
                  value={runtimeMinP}
                  onValueChange={(value) => {
                    runtimeMinP = clampNumber(Number(value ?? runtimeMinP), 0, 1);
                  }}
                  disabled={!runtimeUseCustomParams || !runtimeMinPEnabled}
                />
                <Input
                  type="number"
                  min="0"
                  max="1"
                  step="0.01"
                  class="h-8 w-24 text-right"
                  value={runtimeMinP}
                  disabled={!runtimeUseCustomParams || !runtimeMinPEnabled}
                  oninput={(event) => {
                    const next = Number((event.currentTarget as HTMLInputElement).value) || 0;
                    runtimeMinP = clampNumber(next, 0, 1);
                  }}
                />
              </div>
            </div>
          </div>

          <div class="space-y-3 rounded-md border bg-muted/20 p-3">
            <h4 class="text-sm font-medium">Structured Output</h4>
            <label class="flex items-center justify-between gap-2 text-sm">
              <span>Enable JSON mode</span>
              <Checkbox
                checked={runtimeStructuredOutputEnabled}
                onCheckedChange={(v) => (runtimeStructuredOutputEnabled = v === true)}
              />
            </label>
          </div>

          <div class="space-y-3 rounded-md border bg-muted/20 p-3">
            <h4 class="text-sm font-medium">Reasoning Parsing</h4>
            <label class="flex items-center justify-between gap-2 text-sm">
              <span>Reasoning section parsing</span>
              <Checkbox
                checked={runtimeReasoningParseEnabled}
                onCheckedChange={(v) => (runtimeReasoningParseEnabled = v === true)}
              />
            </label>
            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <span class="text-sm">Start String</span>
              <Input
                type="text"
                class="h-8 w-32 text-right"
                value={runtimeReasoningStartTag}
                disabled={!runtimeReasoningParseEnabled}
                oninput={(event) =>
                  (runtimeReasoningStartTag = (event.currentTarget as HTMLInputElement).value)}
              />
            </div>
            <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <span class="text-sm">End String</span>
              <Input
                type="text"
                class="h-8 w-32 text-right"
                value={runtimeReasoningEndTag}
                disabled={!runtimeReasoningParseEnabled}
                oninput={(event) =>
                  (runtimeReasoningEndTag = (event.currentTarget as HTMLInputElement).value)}
              />
            </div>
          </div>

          <Button size="sm" class="w-full" onclick={applyInferenceSettings} disabled={applyingRuntimeConfig}>
            {#if applyingRuntimeConfig}
              <CircleNotch class="size-4 mr-1 animate-spin" />
            {/if}
            {$t('models.local.inferenceTab.apply') || 'Apply inference settings'}
          </Button>

          <Button variant="destructive" size="sm" class="w-full" onclick={() => requestDelete($selectedModel!)}>
            <Trash class="size-4 mr-1" />
            {$t('models.local.details.delete')}
          </Button>
        </Card.Content>
      </Tabs.Content>
    </Tabs.Root>

  {:else}
    <Card.Header class="pb-3 shrink-0">
      <Card.Title class="text-lg">{$t('models.local.details.placeholderTitle') || 'Model details'}</Card.Title>
    </Card.Header>
    <Card.Content class="flex flex-1 items-center justify-center text-center">
      <p class="max-w-[240px] text-sm text-muted-foreground">
        {$t('models.local.details.placeholderSubtitle') || 'Select a model in the list to see full details.'}
      </p>
    </Card.Content>
  {/if}
{/snippet}

<div class="h-full flex flex-col gap-3 sm:gap-4">
  <div class="rounded-xl border bg-card px-3 py-3 sm:px-4 sm:py-4">
    <div class="flex flex-wrap items-center gap-2 sm:gap-3">
      <div class="flex min-w-0 flex-1 items-center gap-2">
        <span class="text-xs text-muted-foreground whitespace-nowrap">{$t('models.local.folderLabel')}</span>
        <div
          class="flex-1 min-w-0 rounded-md border bg-muted/30 px-2 py-1 font-mono text-xs truncate"
          title={$folderPath || $t('models.local.folderNotSelected')}
        >
          {$folderPath || $t('models.local.notSelected')}
        </div>

        <DropdownMenu.Root>
          <DropdownMenu.Trigger>
            {#snippet child({ props })}
              <Button {...props} variant="ghost" size="icon" aria-label={$t('models.local.folderActions')}>
                <DotsThree class="size-5" weight="bold" />
              </Button>
            {/snippet}
          </DropdownMenu.Trigger>
          <DropdownMenu.Content align="end">
            <DropdownMenu.Item onclick={handleSelectFolder}>
              <FolderOpen class="size-4 mr-2" />
              {$t('models.local.menu.selectFolder')}
            </DropdownMenu.Item>
            <DropdownMenu.Item onclick={handleRescan} disabled={!$folderPath}>
              {$t('models.local.menu.rescan')}
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Root>
      </div>

      <div class="relative w-full sm:w-[260px]">
        <MagnifyingGlass class="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
        <Input
          type="search"
          placeholder={$t('models.local.searchPlaceholder')}
          value={searchQuery}
          class="pl-10"
          oninput={handleSearchInput}
        />
      </div>

      <Select.Root type="single" value={sortMode} onValueChange={handleSortModeChange}>
        <Select.Trigger class="w-[170px]">
          {$t(`models.local.sort.${sortMode}`) || sortMode}
        </Select.Trigger>
        <Select.Content>
          <Select.Item value="recent">{$t('models.local.sort.recent') || 'Recently added'}</Select.Item>
          <Select.Item value="name">{$t('models.local.sort.name') || 'Name'}</Select.Item>
          <Select.Item value="size">{$t('models.local.sort.size') || 'Size'}</Select.Item>
          <Select.Item value="architecture"
            >{$t('models.local.sort.architecture') || 'Architecture'}</Select.Item
          >
        </Select.Content>
      </Select.Root>

      <div class="flex items-center rounded-md border bg-muted/30 p-0.5">
        <Button
          variant={viewMode === 'list' ? 'secondary' : 'ghost'}
          size="sm"
          class="h-8 px-2"
          aria-label={$t('models.local.view.list') || 'List view'}
          onclick={() => (viewMode = 'list')}
        >
          <Rows class="size-4" />
        </Button>
        <Button
          variant={viewMode === 'grid' ? 'secondary' : 'ghost'}
          size="sm"
          class="h-8 px-2"
          aria-label={$t('models.local.view.grid') || 'Grid view'}
          onclick={() => (viewMode = 'grid')}
        >
          <SquaresFour class="size-4" />
        </Button>
      </div>
    </div>

    <div class="mt-3 flex flex-wrap items-center justify-between gap-3">
      <span class="text-xs text-muted-foreground">{summaryText()}</span>

      <div class="w-full max-w-[360px] min-w-[220px] space-y-1">
        <div class="flex items-center justify-between text-[11px] text-muted-foreground">
          <div class="inline-flex items-center gap-1.5">
            <HardDrives class="size-3.5" />
            <span>{$t('models.local.storage.label') || 'Models storage'}</span>
          </div>
          <span>{modelsDiskShare.toFixed(1)}%</span>
        </div>
        <Progress.Root value={modelsDiskShare} max={100} class="h-1.5" />
        <p class="text-[11px] text-muted-foreground">
          {totalModelsSizeLabel}
          {#if diskTotalBytes > 0}
            / {LocalModelsService.formatFileSize(diskTotalBytes)}
          {:else}
            / —
          {/if}
        </p>
      </div>
    </div>
  </div>

  {#if $error}
    <div class="flex items-center justify-between gap-3 rounded-lg border border-destructive/30 bg-destructive/10 p-3">
      <span class="text-sm text-destructive">{$error}</span>
      <Button variant="outline" size="sm" onclick={handleRescan}>
        {$t('models.local.errors.retry')}
      </Button>
    </div>
  {/if}

  <div
    class={`relative flex-1 min-h-0 grid gap-0 ${isCompactViewport ? 'grid-cols-1' : 'grid-cols-[minmax(0,70%)_8px_minmax(0,30%)]'}`}
  >
    <Card.Root class="flex min-h-0 min-w-0 flex-col overflow-hidden">
      {#if $isLoading}
        <div class="flex flex-1 items-center justify-center gap-3 py-12">
          <CircleNotch class="size-5 animate-spin text-muted-foreground" />
          <span class="text-muted-foreground">{$t('common.loading') || 'Loading...'}</span>
        </div>
      {:else if !$models.length}
        <div class="flex flex-1 flex-col items-center justify-center gap-3 px-6 text-center">
          <FolderOpen class="size-12 text-muted-foreground/70" weight="light" />
          <p class="text-base font-medium">{$t('models.local.empty.title') || 'No local models yet'}</p>
          <p class="text-sm text-muted-foreground max-w-[420px]">
            {$t('models.local.empty.subtitle') || 'Select a folder or open the catalog to download your first model.'}
          </p>
          <div class="flex flex-wrap items-center justify-center gap-2 pt-1">
            <Button variant="outline" onclick={handleSelectFolder}>
              <FolderOpen class="size-4 mr-2" />
              {$t('models.local.selectFolder')}
            </Button>
            <Button onclick={openCatalog}>
              {$t('models.local.empty.catalogCta') || 'Open catalog'}
            </Button>
          </div>
        </div>
      {:else if !$filteredModels.length}
        <div class="flex flex-1 flex-col items-center justify-center gap-3 px-6 text-center">
          <MagnifyingGlass class="size-10 text-muted-foreground/70" weight="light" />
          <p class="text-base font-medium">{$t('models.local.empty.noResults') || 'Nothing found'}</p>
          <p class="text-sm text-muted-foreground">{$t('models.local.empty.tryAnotherQuery') || 'Try a different search query.'}</p>
        </div>
      {:else if viewMode === 'list'}
        <div
          class="flex-1 min-h-0 overflow-y-auto custom-scrollbar p-2 sm:p-3"
          role="listbox"
          aria-label={$t('models.local.aria.modelsList') || 'Local models list'}
          tabindex="0"
          onkeydown={handleListKeyboard}
        >
          <div class="space-y-2">
            {#each $filteredModels as model (model.path)}
              {@const status = getModelStatus(model)}
              {@const isSelected = $selectedModel?.path === model.path}
              <div class="rounded-xl border bg-card/60 {isSelected ? 'border-primary/50' : ''}">
                <div
                  use:registerRow={model.path}
                  role="button"
                  tabindex="0"
                  aria-pressed={isSelected}
                  class="group grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 px-3 py-3 sm:px-4 {isSelected
                    ? 'bg-primary/[0.08]'
                    : 'hover:bg-muted/35'}"
                  onclick={() => toggleModelSelection(model)}
                  onkeydown={(event) => {
                    if (event.key === 'Enter' || event.key === ' ') {
                      event.preventDefault();
                      toggleModelSelection(model);
                    }
                  }}
                >
                  <div class="min-w-0 space-y-1.5">
                    <div class="flex min-w-0 items-center gap-2">
                      <span class={`size-2 rounded-full ${statusDotClass(status)}`}></span>
                      <p class="truncate text-sm font-medium" title={model.name}>{model.name}</p>
                    </div>

                    <div class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
                      <span class="truncate max-w-[220px]">{getPublisher(model)}</span>
                      <span class="hidden sm:inline">|</span>
                      <span class="hidden sm:inline">{statusLabel(status)}</span>
                      <span class="hidden md:inline">|</span>
                      <span class="hidden md:inline"
                        >{$t('models.local.table.architecture')}: {model.architecture ?? '—'}</span
                      >
                      <span class="hidden md:inline">|</span>
                      <span class="hidden md:inline"
                        >{$t('models.local.table.quant')}: {model.quantization ?? '—'}</span
                      >
                    </div>
                  </div>

                  <div class="flex items-center gap-1.5 sm:gap-2">
                    <span class="hidden sm:inline text-xs text-muted-foreground whitespace-nowrap">
                      {LocalModelsService.formatFileSize(model.file_size)}
                    </span>

                    <Button
                      size="sm"
                      variant={status === 'loaded' && canUnloadModel(model) ? 'outline' : status === 'loaded' ? 'secondary' : 'default'}
                      class="h-8 px-2.5"
                      disabled={status === 'loading' || status === 'unloading' || (status === 'loaded' && !canUnloadModel(model))}
                      onclick={(event) => (status === 'loaded' ? unloadModel(model, event) : loadModel(model, event))}
                    >
                      {#if status === 'loading' || status === 'unloading'}
                        <CircleNotch class="size-3.5 mr-1.5 animate-spin" />
                        {status === 'unloading'
                          ? $t('models.local.status.unloading') || 'Unloading'
                          : $t('models.local.status.loading') || 'Loading'}
                      {:else if status === 'loaded'}
                        {#if canUnloadModel(model)}
                          <X class="size-3.5 mr-1.5" />
                          {$t('models.local.unload') || 'Unload'}
                        {:else}
                          <Check class="size-3.5 mr-1.5" />
                          {$t('models.local.status.loaded') || 'Loaded'}
                        {/if}
                      {:else}
                        <Play class="size-3.5 mr-1.5" />
                        {$t('models.local.load') || 'Load'}
                      {/if}
                    </Button>

                    <DropdownMenu.Root>
                      <DropdownMenu.Trigger>
                        {#snippet child({ props })}
                          <Button
                            {...props}
                            variant="ghost"
                            size="icon"
                            class="h-8 w-8"
                            onclick={(event) => event.stopPropagation()}
                            aria-label={$t('models.local.rowMenu') || 'Model actions'}
                          >
                            <DotsThree class="size-4" weight="bold" />
                          </Button>
                        {/snippet}
                      </DropdownMenu.Trigger>
                      <DropdownMenu.Content align="end">
                        {#if status === 'loaded'}
                          <DropdownMenu.Item
                            onclick={(event) => unloadModel(model, event)}
                            disabled={!canUnloadModel(model)}
                          >
                            <X class="size-4 mr-2" />
                            {$t('models.local.unload') || 'Unload'}
                          </DropdownMenu.Item>
                        {:else}
                          <DropdownMenu.Item onclick={(event) => loadModel(model, event)}>
                            <Play class="size-4 mr-2" />
                            {$t('models.local.load') || 'Load'}
                          </DropdownMenu.Item>
                        {/if}
                        <DropdownMenu.Item onclick={(event) => startEditing(model, event)}>
                          <PencilSimple class="size-4 mr-2" />
                          {$t('models.local.actions.edit') || 'Edit'}
                        </DropdownMenu.Item>
                        <DropdownMenu.Item onclick={(event) => copyModelPath(model, event)}>
                          <Copy class="size-4 mr-2" />
                          {$t('models.local.actions.copyPath') || 'Copy path'}
                        </DropdownMenu.Item>
                        <DropdownMenu.Item onclick={(event) => openModelLocation(model, event)}>
                          <ArrowSquareOut class="size-4 mr-2" />
                          {$t('models.local.actions.openFolder') || 'Open folder'}
                        </DropdownMenu.Item>
                        <DropdownMenu.Separator />
                        <DropdownMenu.Item class="text-destructive" onclick={(event) => requestDelete(model, event)}>
                          <Trash class="size-4 mr-2" />
                          {$t('models.local.details.delete')}
                        </DropdownMenu.Item>
                      </DropdownMenu.Content>
                    </DropdownMenu.Root>
                  </div>
                </div>

                {#if editingModelPath === model.path}
                  <div class="border-t border-border/50 bg-muted/25 px-3 py-3 sm:px-4">
                    <div class="grid grid-cols-1 gap-3 sm:grid-cols-3 sm:items-end">
                      <div class="space-y-1">
                        <Label class="text-xs">{$t('models.local.details.edit.publisher')}</Label>
                        <Input
                          type="text"
                          placeholder={$t('models.local.details.edit.publisherPlaceholder')}
                          bind:value={editPublisher}
                          onclick={(event: Event) => event.stopPropagation()}
                        />
                      </div>

                      <div class="space-y-1">
                        <Label class="text-xs">{$t('models.local.details.edit.name')}</Label>
                        <Input
                          type="text"
                          placeholder={$t('models.local.details.edit.namePlaceholder')}
                          bind:value={editName}
                          onclick={(event: Event) => event.stopPropagation()}
                        />
                      </div>

                      <div class="flex justify-end gap-2">
                        <Button size="sm" onclick={() => saveEditing(model)}>
                          <Check class="size-4 mr-1" />
                          {$t('models.local.details.edit.save')}
                        </Button>
                        <Button variant="outline" size="sm" onclick={cancelEditing}>
                          <X class="size-4 mr-1" />
                          {$t('models.local.details.edit.cancel')}
                        </Button>
                      </div>
                    </div>
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        </div>
      {:else}
        <div class="flex-1 min-h-0 overflow-y-auto custom-scrollbar p-3">
          <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3">
            {#each $filteredModels as model (model.path)}
              {@const status = getModelStatus(model)}
              {@const isSelected = $selectedModel?.path === model.path}
              <div
                role="button"
                tabindex="0"
                class="rounded-xl border bg-card p-3 transition-colors {isSelected
                  ? 'border-primary/50 bg-primary/[0.08]'
                  : 'hover:bg-muted/35'}"
                onclick={() => toggleModelSelection(model)}
                onkeydown={(event) => {
                  if (event.key === 'Enter' || event.key === ' ') {
                    event.preventDefault();
                    toggleModelSelection(model);
                  }
                }}
              >
                <div class="flex items-start justify-between gap-2">
                  <div class="min-w-0">
                    <div class="flex items-center gap-2">
                      <span class={`size-2 rounded-full ${statusDotClass(status)}`}></span>
                      <p class="truncate text-sm font-medium" title={model.name}>{model.name}</p>
                    </div>
                    <p class="mt-1 truncate text-xs text-muted-foreground">{getPublisher(model)}</p>
                  </div>
                  <Badge variant="outline" class="uppercase text-[10px]">{model.format}</Badge>
                </div>

                <div class="mt-3 space-y-1.5 text-xs text-muted-foreground">
                  <div class="flex items-center justify-between">
                    <span>{$t('models.local.table.architecture')}</span>
                    <span class="text-foreground/85">{model.architecture ?? '—'}</span>
                  </div>
                  <div class="flex items-center justify-between">
                    <span>{$t('models.local.table.quant')}</span>
                    <span class="text-foreground/85">{model.quantization ?? '—'}</span>
                  </div>
                  <div class="flex items-center justify-between">
                    <span>{$t('models.local.table.size')}</span>
                    <span class="text-foreground/85">{LocalModelsService.formatFileSize(model.file_size)}</span>
                  </div>
                </div>

                <div class="mt-3 flex items-center justify-between gap-2">
                  <span class="text-xs text-muted-foreground">{statusLabel(status)}</span>
                  <div class="flex items-center gap-1.5">
                    <Button
                      size="sm"
                      variant={status === 'loaded' && canUnloadModel(model) ? 'outline' : status === 'loaded' ? 'secondary' : 'default'}
                      class="h-8 px-2.5"
                      disabled={status === 'loading' || status === 'unloading' || (status === 'loaded' && !canUnloadModel(model))}
                      onclick={(event) => (status === 'loaded' ? unloadModel(model, event) : loadModel(model, event))}
                    >
                      {#if status === 'loading' || status === 'unloading'}
                        <CircleNotch class="size-3.5 animate-spin" />
                      {:else if status === 'loaded'}
                        {#if canUnloadModel(model)}
                          <X class="size-3.5" />
                        {:else}
                          <Check class="size-3.5" />
                        {/if}
                      {:else}
                        <Play class="size-3.5" />
                      {/if}
                    </Button>

                    <DropdownMenu.Root>
                      <DropdownMenu.Trigger>
                        {#snippet child({ props })}
                          <Button
                            {...props}
                            variant="ghost"
                            size="icon"
                            class="h-8 w-8"
                            onclick={(event) => event.stopPropagation()}
                            aria-label={$t('models.local.rowMenu') || 'Model actions'}
                          >
                            <DotsThree class="size-4" />
                          </Button>
                        {/snippet}
                      </DropdownMenu.Trigger>
                      <DropdownMenu.Content align="end">
                        {#if status === 'loaded'}
                          <DropdownMenu.Item
                            onclick={(event) => unloadModel(model, event)}
                            disabled={!canUnloadModel(model)}
                          >
                            <X class="size-4 mr-2" />
                            {$t('models.local.unload') || 'Unload'}
                          </DropdownMenu.Item>
                        {:else}
                          <DropdownMenu.Item onclick={(event) => loadModel(model, event)}>
                            <Play class="size-4 mr-2" />
                            {$t('models.local.load') || 'Load'}
                          </DropdownMenu.Item>
                        {/if}
                        <DropdownMenu.Item onclick={(event) => startEditing(model, event)}>
                          <PencilSimple class="size-4 mr-2" />
                          {$t('models.local.actions.edit') || 'Edit'}
                        </DropdownMenu.Item>
                        <DropdownMenu.Item onclick={(event) => copyModelPath(model, event)}>
                          <Copy class="size-4 mr-2" />
                          {$t('models.local.actions.copyPath') || 'Copy path'}
                        </DropdownMenu.Item>
                        <DropdownMenu.Item onclick={(event) => openModelLocation(model, event)}>
                          <ArrowSquareOut class="size-4 mr-2" />
                          {$t('models.local.actions.openFolder') || 'Open folder'}
                        </DropdownMenu.Item>
                        <DropdownMenu.Separator />
                        <DropdownMenu.Item class="text-destructive" onclick={(event) => requestDelete(model, event)}>
                          <Trash class="size-4 mr-2" />
                          {$t('models.local.details.delete')}
                        </DropdownMenu.Item>
                      </DropdownMenu.Content>
                    </DropdownMenu.Root>
                  </div>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </Card.Root>

    <div class={`${isCompactViewport ? 'hidden' : 'flex'} min-h-0 items-stretch justify-center`}>
      <Separator.Root orientation="vertical" class="h-full" />
    </div>

    <Card.Root class={`${isCompactViewport ? 'hidden' : 'flex'} min-h-0 min-w-0 flex-col overflow-hidden`}>
      {@render ModelDetailsPanelContent()}
    </Card.Root>

    {#if isCompactViewport && $selectedModel}
      <Sheet.Root bind:open={detailsSheetOpen} onOpenChange={(open) => (detailsSheetOpen = open)}>
        <Sheet.Content
          side="right"
          portalProps={{ to: '.app-body' }}
          overlayClass="!absolute !inset-0 !z-[120] bg-black/40"
          class="model-details-sheet-content !absolute !inset-y-2 !right-2 !left-auto !z-[121] !w-[min(92vw,460px)] !max-w-[calc(100%-1rem)] !h-[calc(100%-1rem)] !rounded-xl p-0"
          trapFocus={false}
          preventScroll={false}
          onInteractOutside={() => (detailsSheetOpen = false)}
          onEscapeKeydown={() => (detailsSheetOpen = false)}
        >
          <Card.Root class="flex h-full min-h-0 flex-col overflow-hidden rounded-none border-0">
            {@render ModelDetailsPanelContent()}
          </Card.Root>
        </Sheet.Content>
      </Sheet.Root>
    {/if}
  </div>
</div>

<Dialog.Root bind:open={deleteDialogOpen}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>{$t('models.local.deleteDialog.title') || 'Delete model?'}</Dialog.Title>
      {#if deleteCandidate}
        <Dialog.Description>
          {$t('models.local.deleteDialog.description', {
            name: deleteCandidate.name,
            size: LocalModelsService.formatFileSize(deleteCandidate.file_size),
          }) ||
            `Delete "${deleteCandidate.name}" and free ${LocalModelsService.formatFileSize(deleteCandidate.file_size)}?`}
        </Dialog.Description>
      {/if}
    </Dialog.Header>

    <Dialog.Footer class="mt-4">
      <Button variant="outline" onclick={() => (deleteDialogOpen = false)}>
        {$t('models.local.deleteDialog.cancel') || 'Cancel'}
      </Button>
      <Button variant="destructive" onclick={confirmDelete}>
        {$t('models.local.deleteDialog.confirm') || 'Delete'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
