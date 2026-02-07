<script lang="ts">
  import * as Card from '$lib/components/ui/card';
  import * as Select from '$lib/components/ui/select';
  import * as Collapsible from '$lib/components/ui/collapsible';
  import * as Slider from '$lib/components/ui/slider';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Input } from '$lib/components/ui/input';
  import SettingRow from '$lib/components/settings/SettingRow.svelte';
  import { cn } from '$lib/utils';
  import type { PerformanceSettings } from '$lib/types/settings-v2';
  import type {
    HardwareSystemInfo,
    HardwareSystemUsage,
  } from '$lib/types/hardware';
  import {
    clamp,
    estimateLayerVramGb,
    estimateMaxLayersForVram,
    getRecommendedLayerRange,
    getUsageTone,
  } from '$lib/components/settings/hardware-utils';
  import CaretDown from 'phosphor-svelte/lib/CaretDown';

  interface ModelPlan {
    gpuLayers: number;
    maxContextLength: number;
    noOffloadKvCache: boolean;
    offloadMmproj: boolean;
    batchSize: number;
    mode: 'GPU' | 'Hybrid' | 'CPU' | 'Unsupported';
  }

  interface HardwareProfile {
    id: string;
    name: string;
    builtin: boolean;
    values: {
      n_gpu_layers: number;
      threads: number;
      batch_size: number;
      ubatch_size: number;
      memory_mode: PerformanceSettings['memory_mode'];
      split_across_gpus: boolean;
      load_in_ram: boolean;
      selected_gpu_uuid: string | null;
    };
  }

  interface Props {
    value: PerformanceSettings;
    planner: ModelPlan | null;
    highlightedSettingId?: string | null;
    systemInfo: HardwareSystemInfo | null;
    systemUsage: HardwareSystemUsage | null;
    modelMaxLayers: number;
    modelSizeGb: number | null;
    tokensPerSecond: number | null;
    latencyMs: number | null;
    isModelLoaded: boolean;
    isLoadingModel: boolean;
    loadingProgress: number;
    loadingStage: string;
    onChange: (next: PerformanceSettings) => void;
  }

  const CUSTOM_PROFILE_STORAGE_KEY = 'settings.v2.hardware.custom_profiles';

  let {
    value,
    planner,
    highlightedSettingId = null,
    systemInfo,
    systemUsage,
    modelMaxLayers,
    modelSizeGb,
    tokensPerSecond,
    latencyMs,
    isModelLoaded,
    isLoadingModel,
    loadingProgress,
    loadingStage,
    onChange,
  }: Props = $props();

  let advancedCpuOpen = $state(true);
  let selectedGpuUuid = $state<string>('');
  let selectedProfileId = $state<string>('max_performance');
  let customProfiles = $state<HardwareProfile[]>([]);
  let autoMessage = $state<string>('');
  let pendingReloadNotice = $state(false);
  let reloadActionMessage = $state<string>('');

  const maxCpuThreads = $derived(Math.max(1, (systemInfo?.cpu.core_count ?? 4) - 2));
  const splitAcrossGpus = $derived(value.llama_runtime.extra_env.OXIDE_GPU_SPLIT === '1');
  const loadInRam = $derived(value.llama_runtime.extra_env.OXIDE_MEMORY_MAPPING === 'ram');
  const maxLayers = $derived(
    Math.max(
      1,
      Math.round(modelMaxLayers || 0),
      Math.round(planner?.gpuLayers ?? 0),
      Math.round(value.llama_runtime.n_gpu_layers || 0),
    ),
  );
  const layerVramGb = $derived(
    estimateLayerVramGb({
      modelSizeGb,
      maxLayers,
    }),
  );
  const gpuRows = $derived(
    (systemInfo?.gpus ?? []).map((gpu, index) => {
      const usage = (systemUsage?.gpus ?? []).find((row) => row.uuid === gpu.uuid);
      return {
        index,
        gpu,
        usage,
      };
    }),
  );
  const selectedGpu = $derived(
    gpuRows.find((row) => row.gpu.uuid === selectedGpuUuid) ?? gpuRows[0] ?? null,
  );
  const pooledVramTotalGb = $derived(
    splitAcrossGpus
      ? gpuRows.reduce((sum, row) => sum + toGb(row.usage?.total_memory ?? row.gpu.total_memory), 0)
      : selectedGpu
        ? toGb(selectedGpu.usage?.total_memory ?? selectedGpu.gpu.total_memory)
        : 0,
  );
  const pooledVramUsedGb = $derived(
    splitAcrossGpus
      ? gpuRows.reduce((sum, row) => sum + toGb(row.usage?.used_memory ?? 0), 0)
      : selectedGpu
        ? toGb(selectedGpu.usage?.used_memory ?? 0)
        : 0,
  );
  const pooledVramFreeGb = $derived(Math.max(0, pooledVramTotalGb - pooledVramUsedGb));
  const vramUsagePercent = $derived(
    pooledVramTotalGb > 0 ? clamp((pooledVramUsedGb / pooledVramTotalGb) * 100, 0, 100) : 0,
  );
  const gpuTemperatureC = $derived.by(() => {
    const values = splitAcrossGpus
      ? gpuRows
          .map((row) => row.usage?.temperature_c)
          .filter((value): value is number => typeof value === 'number')
      : [selectedGpu?.usage?.temperature_c].filter((value): value is number => typeof value === 'number');
    if (values.length === 0) return null;
    return values.reduce((sum, value) => sum + value, 0) / values.length;
  });
  const gpuUtilizationPercent = $derived.by(() => {
    const values = splitAcrossGpus
      ? gpuRows
          .map((row) => row.usage?.utilization_percent)
          .filter((value): value is number => typeof value === 'number')
      : [selectedGpu?.usage?.utilization_percent].filter((value): value is number => typeof value === 'number');
    if (values.length === 0) return null;
    return values.reduce((sum, value) => sum + value, 0) / values.length;
  });
  const currentGpuLayers = $derived(clamp(value.llama_runtime.n_gpu_layers, 0, maxLayers));
  const estimatedVramGb = $derived(currentGpuLayers * layerVramGb);
  const estimatedFreeAfterGb = $derived(pooledVramTotalGb - estimatedVramGb);
  const recommendedLayers = $derived(
    clamp(
      planner?.gpuLayers ??
        estimateMaxLayersForVram({
          availableVramGb: pooledVramFreeGb,
          maxLayers,
          modelSizeGb,
          reserveVramGb: 1.5,
        }),
      0,
      maxLayers,
    ),
  );
  const recommendedRange = $derived(getRecommendedLayerRange(maxLayers, recommendedLayers));
  const ramUsagePercent = $derived(
    systemUsage && systemUsage.total_memory > 0
      ? clamp((systemUsage.used_memory / systemUsage.total_memory) * 100, 0, 100)
      : 0,
  );
  const availableRamGb = $derived(
    systemUsage ? toGb(Math.max(0, systemUsage.total_memory - systemUsage.used_memory)) : 0,
  );
  const totalRamGb = $derived(systemUsage ? toGb(systemUsage.total_memory) : 0);
  const cpuUsagePercent = $derived(clamp(systemUsage?.cpu ?? 0, 0, 100));
  const diskFreeBytes = $derived(systemUsage?.disk_free_bytes ?? 0);
  const diskTotalBytes = $derived(systemUsage?.disk_total_bytes ?? 0);
  const diskUsagePercent = $derived(
    diskTotalBytes > 0 ? clamp(((diskTotalBytes - diskFreeBytes) / diskTotalBytes) * 100, 0, 100) : 0,
  );
  const integratedGpuSelected = $derived(
    !!selectedGpu &&
      String(selectedGpu.gpu.vendor).toLowerCase().includes('intel') &&
      (systemInfo?.gpus.length ?? 0) > 0,
  );
  const hasDiscreteGpu = $derived(
    (systemInfo?.gpus ?? []).some((gpu) => !String(gpu.vendor).toLowerCase().includes('intel')),
  );
  const possibleFragmentation = $derived(
    pooledVramFreeGb >= 3 && planner?.mode === 'Unsupported' && pooledVramUsedGb > 0,
  );

  const builtinProfiles = $derived([
    {
      id: 'gaming',
      name: 'Gaming mode',
      builtin: true,
      values: {
        n_gpu_layers: Math.max(0, Math.floor(recommendedLayers * 0.6)),
        threads: Math.max(1, maxCpuThreads - 1),
        batch_size: Math.max(64, Math.min(value.llama_runtime.batch_size, 256)),
        ubatch_size: Math.max(64, Math.min(value.llama_runtime.ubatch_size, 256)),
        memory_mode: 'medium' as const,
        split_across_gpus: false,
        load_in_ram: false,
        selected_gpu_uuid: selectedGpu?.gpu.uuid ?? null,
      },
    },
    {
      id: 'max_performance',
      name: 'Max performance',
      builtin: true,
      values: {
        n_gpu_layers: recommendedLayers,
        threads: Math.max(1, maxCpuThreads),
        batch_size: Math.max(256, planner?.batchSize ?? value.llama_runtime.batch_size),
        ubatch_size: Math.max(256, planner?.batchSize ?? value.llama_runtime.ubatch_size),
        memory_mode: 'high' as const,
        split_across_gpus: (systemInfo?.gpus.length ?? 0) > 1 && recommendedLayers >= maxLayers,
        load_in_ram: false,
        selected_gpu_uuid: selectedGpu?.gpu.uuid ?? null,
      },
    },
    {
      id: 'power_saving',
      name: 'Power saving',
      builtin: true,
      values: {
        n_gpu_layers: 0,
        threads: Math.max(1, Math.floor(maxCpuThreads / 2)),
        batch_size: 64,
        ubatch_size: 64,
        memory_mode: 'low' as const,
        split_across_gpus: false,
        load_in_ram: true,
        selected_gpu_uuid: selectedGpu?.gpu.uuid ?? null,
      },
    },
  ]);
  const allProfiles = $derived([...builtinProfiles, ...customProfiles]);

  $effect(() => {
    if (!selectedGpuUuid && gpuRows.length > 0) {
      selectedGpuUuid = gpuRows[0].gpu.uuid;
    }
  });

  $effect(() => {
    const fromEnv = value.llama_runtime.extra_env.OXIDE_SELECTED_GPU_UUID;
    if (fromEnv && fromEnv !== selectedGpuUuid && gpuRows.some((row) => row.gpu.uuid === fromEnv)) {
      selectedGpuUuid = fromEnv;
    }
  });

  $effect(() => {
    if (!isModelLoaded) {
      pendingReloadNotice = false;
      reloadActionMessage = '';
    }
  });

  function toGb(mib: number): number {
    return mib / 1024;
  }

  function formatGb(gb: number): string {
    return `${gb.toFixed(1)} GB`;
  }

  function formatBytes(bytes: number): string {
    if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    let value = bytes;
    let unitIndex = 0;
    while (value >= 1024 && unitIndex < units.length - 1) {
      value /= 1024;
      unitIndex += 1;
    }
    return `${value.toFixed(unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
  }

  function getToneBarClass(percent: number): string {
    const tone = getUsageTone(percent);
    if (tone === 'danger') return 'bg-red-500';
    if (tone === 'warn') return 'bg-amber-500';
    return 'bg-emerald-500';
  }

  function getToneTextClass(percent: number): string {
    const tone = getUsageTone(percent);
    if (tone === 'danger') return 'text-red-600 dark:text-red-400';
    if (tone === 'warn') return 'text-amber-600 dark:text-amber-400';
    return 'text-emerald-600 dark:text-emerald-400';
  }

  function notifyDeferredApplyIfNeeded() {
    if (isModelLoaded && !isLoadingModel) {
      pendingReloadNotice = true;
    }
  }

  function commit(nextRuntimePatch: Partial<PerformanceSettings['llama_runtime']>, nextMemoryMode?: PerformanceSettings['memory_mode']) {
    notifyDeferredApplyIfNeeded();
    onChange({
      ...value,
      memory_mode: nextMemoryMode ?? value.memory_mode,
      llama_runtime: {
        ...value.llama_runtime,
        ...nextRuntimePatch,
      },
    });
  }

  function commitExtraEnv(mutator: (env: Record<string, string>) => void) {
    const nextEnv = { ...value.llama_runtime.extra_env };
    mutator(nextEnv);
    commit({ extra_env: nextEnv });
  }

  function applyGpuSelection(uuid: string) {
    selectedGpuUuid = uuid;
    const gpuIndex = (systemInfo?.gpus ?? []).findIndex((gpu) => gpu.uuid === uuid);
    commitExtraEnv((env) => {
      env.OXIDE_SELECTED_GPU_UUID = uuid;
      if (!splitAcrossGpus && gpuIndex >= 0) {
        env.CUDA_VISIBLE_DEVICES = String(gpuIndex);
        env.HIP_VISIBLE_DEVICES = String(gpuIndex);
      }
    });
  }

  function setSplitAcrossGpus(enabled: boolean) {
    const gpuIndex = (systemInfo?.gpus ?? []).findIndex((gpu) => gpu.uuid === selectedGpuUuid);
    commitExtraEnv((env) => {
      if (enabled) {
        env.OXIDE_GPU_SPLIT = '1';
        delete env.CUDA_VISIBLE_DEVICES;
        delete env.HIP_VISIBLE_DEVICES;
      } else {
        delete env.OXIDE_GPU_SPLIT;
        if (gpuIndex >= 0) {
          env.CUDA_VISIBLE_DEVICES = String(gpuIndex);
          env.HIP_VISIBLE_DEVICES = String(gpuIndex);
        }
      }
    });
  }

  function setLoadInRam(enabled: boolean) {
    commitExtraEnv((env) => {
      env.OXIDE_MEMORY_MAPPING = enabled ? 'ram' : 'mmap';
    });
  }

  async function reloadModelNow() {
    reloadActionMessage = '';
    if (typeof window === 'undefined') return;
    const bridge = (window as any).__oxide as
      | {
          reloadSelectedModel?: () => Promise<void> | void;
        }
      | undefined;

    if (!bridge?.reloadSelectedModel) {
      reloadActionMessage = 'Reload action is unavailable right now. Open chat view and reload the model there.';
      return;
    }

    try {
      await bridge.reloadSelectedModel();
      pendingReloadNotice = false;
      reloadActionMessage = 'Reload requested. Hardware changes will apply after model reload completes.';
    } catch {
      reloadActionMessage = 'Failed to trigger model reload. Please reload the current model from chat view.';
    }
  }

  function maximizeGpu() {
    const nextLayers = recommendedLayers;
    commit({ n_gpu_layers: nextLayers });
    autoMessage = `Maximize GPU: ${nextLayers}/${maxLayers} layers (~${(nextLayers * layerVramGb).toFixed(1)} GB VRAM).`;
  }

  function autoDetectOptimal() {
    const targetLayers = recommendedLayers;
    const targetThreads = Math.max(1, maxCpuThreads);
    const targetBatch = Math.max(64, planner?.batchSize ?? value.llama_runtime.batch_size);
    const targetUBatch = Math.max(64, planner?.batchSize ?? value.llama_runtime.ubatch_size);

    commit(
      {
        n_gpu_layers: targetLayers,
        threads: targetThreads,
        batch_size: targetBatch,
        ubatch_size: targetUBatch,
      },
      value.memory_mode,
    );

    const requiredGb = targetLayers * layerVramGb;
    if (planner?.mode === 'Unsupported' || requiredGb > pooledVramFreeGb + 0.25) {
      autoMessage =
        `Model requires ~${requiredGb.toFixed(1)} GB VRAM, available ${pooledVramFreeGb.toFixed(1)} GB. CPU offload will be used (typically 5-10x slower).`;
    } else {
      autoMessage =
        `Auto-detect optimal: load ${targetLayers}/${maxLayers} layers on GPU, rest on CPU (requires ~${requiredGb.toFixed(1)} GB VRAM).`;
    }
  }

  function applyProfile(profileId: string) {
    selectedProfileId = profileId;
    const profile = allProfiles.find((entry) => entry.id === profileId);
    if (!profile) return;

    const gpuId = profile.values.selected_gpu_uuid ?? selectedGpuUuid;
    if (gpuId) {
      selectedGpuUuid = gpuId;
    }

    const gpuIndex = (systemInfo?.gpus ?? []).findIndex((gpu) => gpu.uuid === gpuId);
    const nextEnv = { ...value.llama_runtime.extra_env };
    if (gpuId) nextEnv.OXIDE_SELECTED_GPU_UUID = gpuId;
    nextEnv.OXIDE_MEMORY_MAPPING = profile.values.load_in_ram ? 'ram' : 'mmap';
    if (profile.values.split_across_gpus) {
      nextEnv.OXIDE_GPU_SPLIT = '1';
      delete nextEnv.CUDA_VISIBLE_DEVICES;
      delete nextEnv.HIP_VISIBLE_DEVICES;
    } else {
      delete nextEnv.OXIDE_GPU_SPLIT;
      if (gpuIndex >= 0) {
        nextEnv.CUDA_VISIBLE_DEVICES = String(gpuIndex);
        nextEnv.HIP_VISIBLE_DEVICES = String(gpuIndex);
      }
    }

    commit(
      {
        n_gpu_layers: clamp(profile.values.n_gpu_layers, 0, maxLayers),
        threads: Math.max(1, profile.values.threads),
        batch_size: Math.max(1, profile.values.batch_size),
        ubatch_size: Math.max(1, profile.values.ubatch_size),
        extra_env: nextEnv,
      },
      profile.values.memory_mode,
    );

    autoMessage = `Profile applied: ${profile.name}.`;
  }

  function saveCustomProfile() {
    if (typeof window === 'undefined') return;
    const name = window.prompt('Custom hardware profile name', `Custom ${customProfiles.length + 1}`)?.trim();
    if (!name) return;

    const profile: HardwareProfile = {
      id: `custom_${Date.now()}`,
      name,
      builtin: false,
      values: {
        n_gpu_layers: currentGpuLayers,
        threads: Math.max(1, value.llama_runtime.threads || maxCpuThreads),
        batch_size: Math.max(1, value.llama_runtime.batch_size),
        ubatch_size: Math.max(1, value.llama_runtime.ubatch_size),
        memory_mode: value.memory_mode,
        split_across_gpus: splitAcrossGpus,
        load_in_ram: loadInRam,
        selected_gpu_uuid: selectedGpuUuid || null,
      },
    };

    const nextCustomProfiles = [...customProfiles, profile];
    customProfiles = nextCustomProfiles;
    selectedProfileId = profile.id;
    localStorage.setItem(CUSTOM_PROFILE_STORAGE_KEY, JSON.stringify(nextCustomProfiles));
    autoMessage = `Custom profile "${name}" saved.`;
  }

  if (typeof window !== 'undefined') {
    try {
      const raw = localStorage.getItem(CUSTOM_PROFILE_STORAGE_KEY);
      if (raw) {
        const parsed = JSON.parse(raw) as unknown;
        if (Array.isArray(parsed)) {
          customProfiles = parsed.filter((item): item is HardwareProfile => {
            if (typeof item !== 'object' || item === null) return false;
            const candidate = item as Partial<HardwareProfile>;
            return (
              typeof candidate.id === 'string' &&
              typeof candidate.name === 'string' &&
              typeof candidate.values === 'object' &&
              candidate.values !== null
            );
          });
        }
      }
    } catch {
      customProfiles = [];
    }
  }
</script>

<div class="space-y-3">
  <Card.Root>
    <Card.Header>
      <Card.Title>Hardware Dashboard</Card.Title>
      <Card.Description>Live system telemetry and runtime hardware controls.</Card.Description>
    </Card.Header>
    <Card.Content class="space-y-3">
      <div class="flex flex-wrap items-center gap-2 text-xs">
        <Badge variant="secondary">
          {tokensPerSecond !== null ? `${tokensPerSecond.toFixed(2)} tok/s` : 'tok/s: n/a'}
        </Badge>
        <Badge variant="secondary">
          {latencyMs !== null ? `Latency: ${Math.round(latencyMs)} ms` : 'Latency: n/a'}
        </Badge>
        <Badge variant="outline">
          {systemInfo ? `CPU threads available: ${maxCpuThreads}` : 'CPU threads: n/a'}
        </Badge>
      </div>

      {#if isLoadingModel}
        <div class="rounded-md border border-primary/25 bg-primary/5 p-2">
          <div class="mb-1 flex items-center justify-between text-xs">
            <span>
              {loadingStage ? `Loading stage: ${loadingStage}` : 'Loading model'}
            </span>
            <span>{Math.round(clamp(loadingProgress, 0, 100))}%</span>
          </div>
          <div class="h-2 overflow-hidden rounded bg-muted">
            <div
              class="h-full bg-primary transition-all"
              style={`width:${clamp(loadingProgress, 0, 100)}%`}
            ></div>
          </div>
        </div>
      {/if}

      {#if pendingReloadNotice}
        <div class="rounded-md border border-amber-500/35 bg-amber-500/10 p-2 text-xs text-amber-800 dark:text-amber-200 space-y-2">
          <p>Hardware changes will apply on the next model load. App restart is not required.</p>
          <div class="flex flex-wrap items-center gap-2">
            <Button type="button" size="sm" variant="outline" onclick={reloadModelNow}>Reload model now</Button>
            {#if reloadActionMessage}
              <span>{reloadActionMessage}</span>
            {/if}
          </div>
        </div>
      {/if}

      <div class="grid gap-3 lg:grid-cols-2">
        <div class="rounded-lg border p-3 space-y-2">
          <div class="flex items-center justify-between">
            <p class="text-sm font-medium">GPU</p>
            {#if splitAcrossGpus}<Badge variant="outline">Split mode</Badge>{/if}
          </div>
          {#if gpuRows.length === 0}
            <p class="text-xs text-amber-600 dark:text-amber-400">
              No GPU detected. Install CUDA/ROCm drivers and restart runtime.
            </p>
          {:else}
            <p class="text-xs text-muted-foreground">
              {splitAcrossGpus
                ? `All GPUs (${gpuRows.length})`
                : `${selectedGpu?.gpu.name ?? 'GPU'} (${selectedGpu?.gpu.vendor ?? 'Unknown'})`}
            </p>
            <div class="h-2 overflow-hidden rounded bg-muted">
              <div
                class={cn('h-full transition-all', getToneBarClass(vramUsagePercent))}
                style={`width:${vramUsagePercent}%`}
              ></div>
            </div>
            <div class="flex items-center justify-between text-xs">
              <span>{formatGb(pooledVramUsedGb)} / {formatGb(pooledVramTotalGb)}</span>
              <span class={getToneTextClass(vramUsagePercent)}>{vramUsagePercent.toFixed(0)}%</span>
            </div>
            <div class="text-[11px] text-muted-foreground">
              Temp:
              {gpuTemperatureC !== null ? `${gpuTemperatureC.toFixed(0)} C` : 'unavailable'}
              • Load:
              {gpuUtilizationPercent !== null ? `${gpuUtilizationPercent.toFixed(0)}%` : 'unavailable'}
            </div>
          {/if}
        </div>

        <div class="rounded-lg border p-3 space-y-2">
          <p class="text-sm font-medium">RAM</p>
          <div class="h-2 overflow-hidden rounded bg-muted">
            <div
              class={cn('h-full transition-all', getToneBarClass(ramUsagePercent))}
              style={`width:${ramUsagePercent}%`}
            ></div>
          </div>
          <div class="flex items-center justify-between text-xs">
            <span>{formatGb(availableRamGb)} available / {formatGb(totalRamGb)} total</span>
            <span class={getToneTextClass(ramUsagePercent)}>{ramUsagePercent.toFixed(0)}%</span>
          </div>
          <p class="text-xs text-muted-foreground">
            CPU usage: {cpuUsagePercent.toFixed(0)}% • CPU threads (for `n_threads`): {maxCpuThreads}
          </p>
          {#if diskTotalBytes > 0}
            <p class={cn('text-xs', getToneTextClass(diskUsagePercent))}>
              Disk free: {formatBytes(diskFreeBytes)} / {formatBytes(diskTotalBytes)}
            </p>
          {:else}
            <p class="text-xs text-muted-foreground">Disk free: unavailable</p>
          {/if}
        </div>
      </div>
    </Card.Content>
  </Card.Root>

  <Card.Root>
    <Card.Header>
      <Card.Title>GPU Configuration</Card.Title>
      <Card.Description>Offload, device selection, and automated GPU fit strategy.</Card.Description>
    </Card.Header>
    <Card.Content class="space-y-3">
      <SettingRow
        id="performance.hardware.gpu_offload"
        title="GPU Layers"
        description="Granular offload control. 0 = CPU only."
        highlighted={highlightedSettingId === 'performance.hardware.gpu_offload'}
      >
        <div class="space-y-2">
          <input
            type="range"
            class="w-full accent-primary"
            min="0"
            max={maxLayers}
            step="1"
            value={currentGpuLayers}
            title={`${currentGpuLayers}/${maxLayers} layers -> ~${estimatedVramGb.toFixed(1)} GB VRAM required`}
            oninput={(event) => {
              const next = clamp(Math.round(Number((event.currentTarget as HTMLInputElement).value) || 0), 0, maxLayers);
              commit({ n_gpu_layers: next });
            }}
          />

          <div class="text-xs text-muted-foreground">
            GPU Layers: {currentGpuLayers}/{maxLayers} ({Math.round((currentGpuLayers / Math.max(1, maxLayers)) * 100)}% on GPU) - ~{estimatedVramGb.toFixed(1)} GB VRAM
          </div>
          <div class="text-xs text-muted-foreground">
            Now: ~{estimatedVramGb.toFixed(1)} GB VRAM. Remaining: ~{estimatedFreeAfterGb.toFixed(1)} GB.
          </div>

          <div class="space-y-1">
            <div class="flex items-center justify-between text-[11px] text-muted-foreground">
              <span>Recommended range</span>
              <span>{recommendedRange.start}..{recommendedRange.end}</span>
            </div>
            <div class="relative h-2 overflow-hidden rounded bg-muted">
              <div
                class="absolute inset-y-0 bg-emerald-500/35"
                style={`left:${(recommendedRange.start / Math.max(1, maxLayers)) * 100}%; width:${((recommendedRange.end - recommendedRange.start) / Math.max(1, maxLayers)) * 100}%`}
              ></div>
              <div
                class="absolute inset-y-0 w-[2px] bg-primary"
                style={`left:${(currentGpuLayers / Math.max(1, maxLayers)) * 100}%`}
              ></div>
            </div>
          </div>
        </div>
      </SettingRow>

      <div class="flex flex-wrap items-center gap-2">
        <Button type="button" variant="outline" size="sm" onclick={maximizeGpu}>Maximize GPU</Button>
        <Button type="button" size="sm" onclick={autoDetectOptimal}>Auto-detect optimal</Button>
      </div>

      <SettingRow
        id="performance.hardware.gpu_selection"
        title="GPU Selection"
        description="Choose a primary GPU for runtime. Changes apply on next model load."
        highlighted={highlightedSettingId === 'performance.hardware.gpu_selection'}
      >
        <Select.Root
          type="single"
          value={selectedGpuUuid}
          onValueChange={(next) => {
            if (!next) return;
            applyGpuSelection(next);
          }}
        >
          <Select.Trigger class="w-full">
            {selectedGpu
              ? `${selectedGpu.gpu.name} - ${formatGb(toGb(selectedGpu.usage?.total_memory ?? selectedGpu.gpu.total_memory))}`
              : 'No GPU'}
          </Select.Trigger>
          <Select.Content>
            {#each gpuRows as row (row.gpu.uuid)}
              <Select.Item value={row.gpu.uuid}>
                {row.gpu.name} - {formatGb(toGb(row.usage?.total_memory ?? row.gpu.total_memory))}
              </Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
      </SettingRow>

      <SettingRow
        id="performance.hardware.split_gpus"
        title="Split Across GPUs"
        description="Enable when model does not fit a single GPU."
        highlighted={highlightedSettingId === 'performance.hardware.split_gpus'}
      >
        <div class="flex items-center gap-2">
          <Checkbox checked={splitAcrossGpus} onCheckedChange={(checked) => setSplitAcrossGpus(checked === true)} />
          <span class="text-sm">Distribute layers across all detected GPUs</span>
        </div>
      </SettingRow>

      {#if autoMessage}
        <p class="rounded-md border border-primary/25 bg-primary/5 p-2 text-xs">{autoMessage}</p>
      {/if}
      {#if planner?.mode === 'Unsupported'}
        <p class="rounded-md border border-destructive/30 bg-destructive/10 p-2 text-xs text-destructive">
          Model does not fit current memory budget. CPU offload fallback will be much slower.
        </p>
      {/if}
      {#if integratedGpuSelected && hasDiscreteGpu}
        <p class="rounded-md border border-amber-500/35 bg-amber-500/10 p-2 text-xs text-amber-700 dark:text-amber-300">
          Integrated GPU selected while a discrete GPU is available. For LLM, discrete GPU is recommended.
        </p>
      {/if}
      {#if possibleFragmentation}
        <p class="rounded-md border border-amber-500/35 bg-amber-500/10 p-2 text-xs text-amber-700 dark:text-amber-300">
          Possible VRAM fragmentation/reservation detected: free VRAM exists, but planner still reports unsupported mode.
        </p>
      {/if}
    </Card.Content>
  </Card.Root>

  <Card.Root>
    <Card.Header>
      <Card.Title>Hardware Profiles</Card.Title>
      <Card.Description>Quick presets for common runtime hardware scenarios.</Card.Description>
    </Card.Header>
    <Card.Content class="space-y-3">
      <div class="flex flex-wrap items-center gap-2">
        <Select.Root type="single" value={selectedProfileId} onValueChange={(next) => next && applyProfile(next)}>
          <Select.Trigger class="w-full sm:w-[260px]">
            {allProfiles.find((profile) => profile.id === selectedProfileId)?.name ?? 'Select profile'}
          </Select.Trigger>
          <Select.Content>
            {#each allProfiles as profile (profile.id)}
              <Select.Item value={profile.id}>{profile.name}</Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
        <Button type="button" variant="outline" size="sm" onclick={saveCustomProfile}>Save custom</Button>
      </div>
    </Card.Content>
  </Card.Root>

  <Card.Root>
    <Card.Header>
      <Card.Title>CPU & Memory</Card.Title>
      <Card.Description>Advanced CPU threads, model loading mode, and batch tuning.</Card.Description>
    </Card.Header>
    <Card.Content>
      <Collapsible.Root bind:open={advancedCpuOpen} class="rounded-lg border">
        <Collapsible.Trigger class="flex w-full items-center justify-between px-3 py-2 text-sm font-medium">
          <span>Advanced CPU settings</span>
          <CaretDown class={cn('size-4 transition-transform', advancedCpuOpen && 'rotate-180')} />
        </Collapsible.Trigger>
        <Collapsible.Content class="border-t p-3 space-y-3">
          <SettingRow
            id="performance.hardware.cpu_threads"
            title="CPU Threads"
            description="Keep resources for OS by default (1..cores-2)."
            highlighted={highlightedSettingId === 'performance.hardware.cpu_threads'}
          >
            <div class="space-y-2">
              <Slider.Root
                type="single"
                min={1}
                max={maxCpuThreads}
                step={1}
                value={Math.max(1, Math.min(maxCpuThreads, value.llama_runtime.threads || maxCpuThreads))}
                onValueChange={(next) => {
                  const valueAsNumber = Math.round(Number(next) || 1);
                  commit({ threads: clamp(valueAsNumber, 1, maxCpuThreads) });
                }}
              />
              <p class="text-xs text-muted-foreground">
                n_threads: {Math.max(1, Math.min(maxCpuThreads, value.llama_runtime.threads || maxCpuThreads))}
              </p>
            </div>
          </SettingRow>

          <SettingRow
            id="performance.hardware.memory_mapping"
            title="Memory Mapping"
            description="Load model in RAM vs memory-mapped I/O."
            highlighted={highlightedSettingId === 'performance.hardware.memory_mapping'}
          >
            <div class="flex items-center gap-2">
              <Checkbox checked={loadInRam} onCheckedChange={(checked) => setLoadInRam(checked === true)} />
              <span class="text-sm">{loadInRam ? 'Load model in RAM' : 'Memory-mapped I/O'}</span>
            </div>
          </SettingRow>

          <SettingRow
            id="performance.hardware.batch_size"
            title="Batch Size"
            description="Increase for throughput, decrease for latency."
            highlighted={highlightedSettingId === 'performance.hardware.batch_size'}
          >
            <Input
              type="number"
              min="1"
              step="1"
              value={value.llama_runtime.batch_size}
              oninput={(event) => {
                const next = Math.max(1, Math.round(Number((event.currentTarget as HTMLInputElement).value) || 1));
                commit({ batch_size: next });
              }}
            />
          </SettingRow>

          <SettingRow
            id="performance.hardware.memory_mode"
            title="Memory Mode"
            description="Planner strategy for memory pressure."
            highlighted={highlightedSettingId === 'performance.hardware.memory_mode'}
          >
            <Select.Root
              type="single"
              value={value.memory_mode}
              onValueChange={(next) => {
                const targetMode = (next ?? 'medium') as PerformanceSettings['memory_mode'];
                commit({}, targetMode);
              }}
            >
              <Select.Trigger class="w-full">{value.memory_mode}</Select.Trigger>
              <Select.Content>
                <Select.Item value="low">low</Select.Item>
                <Select.Item value="medium">medium</Select.Item>
                <Select.Item value="high">high</Select.Item>
              </Select.Content>
            </Select.Root>
          </SettingRow>
        </Collapsible.Content>
      </Collapsible.Root>
    </Card.Content>
  </Card.Root>
</div>
