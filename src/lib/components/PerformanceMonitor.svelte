<script lang="ts">
  /**
   * Performance Monitor Component
   * 
   * Real-time display of system performance metrics (CPU, GPU, memory, inference stats).
   */
  import { onMount, onDestroy } from 'svelte';
  import * as Card from '$lib/components/ui/card';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import { Spinner } from '$lib/components/ui/spinner';
  import ChartBar from 'phosphor-svelte/lib/ChartBar';
  import ArrowClockwise from 'phosphor-svelte/lib/ArrowClockwise';
  import Trash from 'phosphor-svelte/lib/Trash';
  import Cpu from 'phosphor-svelte/lib/Cpu';
  import HardDrive from 'phosphor-svelte/lib/HardDrive';
  import Lightning from 'phosphor-svelte/lib/Lightning';
  import Clock from 'phosphor-svelte/lib/Clock';
  import TrendUp from 'phosphor-svelte/lib/TrendUp';
  import Warning from 'phosphor-svelte/lib/Warning';
  import GpuCard from 'phosphor-svelte/lib/GraphicsCard';
  import { performanceService } from '$lib/services/performance-service';
  import { hardwareService } from '$lib/services/hardware-service';
  import type {
    PerformanceSummary,
    SystemUsage,
    ModelLoadMetrics,
    InferenceMetrics,
    StartupMetrics,
  } from '$lib/types/performance';
  import type { HardwareSystemInfo, HardwareSystemUsage } from '$lib/types/hardware';
  import { t } from '$lib/i18n';

  // State
  let summary = $state<PerformanceSummary | null>(null);
  let systemUsage = $state<SystemUsage | null>(null);
  let hardwareInfo = $state<HardwareSystemInfo | null>(null);
  let hardwareUsage = $state<HardwareSystemUsage | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let autoRefresh = $state(true);
  let refreshInterval: ReturnType<typeof setInterval> | null = null;

  function gpuUsageByUuid(uuid: string) {
    return hardwareUsage?.gpus?.find((gpu) => gpu.uuid === uuid);
  }

  function formatPercent(value: number) {
    return `${value.toFixed(1)}%`;
  }

  async function loadSummary() {
    loading = true;
    error = null;
    try {
      const [summaryData, systemUsageData, hardwareUsageData, hardwareInfoData] = await Promise.allSettled([
        performanceService.getPerformanceSummary(),
        performanceService.getSystemUsage(),
        hardwareService.getSystemUsage(),
        hardwareInfo ? Promise.resolve(hardwareInfo) : hardwareService.getSystemInfo(),
      ]);

      if (summaryData.status === 'fulfilled') {
        summary = summaryData.value;
      }
      if (systemUsageData.status === 'fulfilled') {
        systemUsage = systemUsageData.value;
      }
      if (hardwareUsageData.status === 'fulfilled') {
        hardwareUsage = hardwareUsageData.value;
      }
      if (hardwareInfoData.status === 'fulfilled') {
        hardwareInfo = hardwareInfoData.value;
      }

      if (
        summaryData.status === 'rejected' &&
        systemUsageData.status === 'rejected' &&
        hardwareUsageData.status === 'rejected' &&
        hardwareInfoData.status === 'rejected'
      ) {
        error = $t('settings.performance.loadError') || 'Failed to load performance data';
      }
    } catch (e) {
      error = e instanceof Error ? e.message : 'Unknown error';
    } finally {
      loading = false;
    }
  }

  async function clearMetrics() {
    try {
      await performanceService.clearMetrics();
      summary = null;
      await loadSummary();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to clear metrics';
    }
  }

  function startAutoRefresh() {
    if (!refreshInterval) {
      refreshInterval = setInterval(() => {
        loadSummary();
      }, 2000);
    }
  }

  function stopAutoRefresh() {
    if (refreshInterval) {
      clearInterval(refreshInterval);
      refreshInterval = null;
    }
  }

  onMount(async () => {
    await loadSummary();
    if (autoRefresh) {
      startAutoRefresh();
    }

    // Setup event listeners
    await performanceService.setupEventListeners(
      (modelLoadMetrics: ModelLoadMetrics) => {
        loadSummary();
      },
      (inferenceMetrics: InferenceMetrics) => {
        loadSummary();
      },
      (startupMetrics: StartupMetrics) => {
        loadSummary();
      },
    );
  });

  onDestroy(() => {
    performanceService.cleanup();
    stopAutoRefresh();
  });
</script>

<div class="space-y-4">
  <!-- Header -->
  <div class="flex items-center justify-between flex-wrap gap-2">
    <div class="flex items-center gap-2">
      <ChartBar class="size-5" />
      <span class="font-semibold">{$t('settings.performance.monitor') || 'Performance Monitor'}</span>
      {#if autoRefresh}
        <Badge variant="secondary" class="gap-1.5">
          <span class="size-2 rounded-full bg-green-500 animate-pulse"></span>
          {$t('settings.performance.realtime') || 'Real-time'}
        </Badge>
      {/if}
    </div>
    <div class="flex gap-2">
      <Button variant="outline" size="sm" onclick={loadSummary} disabled={loading}>
        {#if loading}
          <Spinner class="size-4 mr-1" />
        {:else}
          <ArrowClockwise class="size-4 mr-1" />
        {/if}
        {$t('common.refresh') || 'Refresh'}
      </Button>
      <Button variant="outline" size="sm" onclick={clearMetrics}>
        <Trash class="size-4 mr-1" />
        {$t('common.clear') || 'Clear'}
      </Button>
    </div>
  </div>

  <!-- Error -->
  {#if error}
    <div class="flex items-center gap-2 p-3 rounded-lg border border-destructive/30 bg-destructive/10 text-sm">
      <Warning class="size-4 text-destructive" />
      <span>{error}</span>
    </div>
  {/if}

  <!-- Metrics Grid -->
  {#if summary || systemUsage}
    <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      <!-- CPU Usage -->
      {#if systemUsage?.cpu_usage_percent !== undefined}
        <Card.Root>
          <Card.Content class="pt-4">
            <div class="flex items-start gap-3">
              <div class="p-2 rounded-lg bg-primary/10">
                <Cpu class="size-6 text-primary" />
              </div>
              <div>
                <p class="text-sm text-muted-foreground">{$t('settings.performance.cpuUsage') || 'CPU Usage'}</p>
                <p class="text-2xl font-bold text-primary">{systemUsage.cpu_usage_percent.toFixed(1)}%</p>
              </div>
            </div>
          </Card.Content>
        </Card.Root>
      {/if}

      <!-- Memory -->
      {#if summary?.current_memory_mb}
        <Card.Root>
          <Card.Content class="pt-4">
            <div class="flex items-start gap-3">
              <div class="p-2 rounded-lg bg-amber-500/10">
                <HardDrive class="size-6 text-amber-500" />
              </div>
              <div>
                <p class="text-sm text-muted-foreground">{$t('settings.performance.memory') || 'Memory'}</p>
                <p class="text-2xl font-bold text-amber-500">{performanceService.formatMemory(summary.current_memory_mb)}</p>
              </div>
            </div>
          </Card.Content>
        </Card.Root>
      {/if}

      <!-- Generation Speed -->
      {#if summary?.last_inference}
        <Card.Root>
          <Card.Content class="pt-4">
            <div class="flex items-start gap-3">
              <div class="p-2 rounded-lg bg-green-500/10">
                <Lightning class="size-6 text-green-500" />
              </div>
              <div>
                <p class="text-sm text-muted-foreground">{$t('settings.performance.speed') || 'Speed'}</p>
                <p class="text-2xl font-bold text-green-500">{performanceService.formatSpeed(summary.last_inference.tokens_per_second)}</p>
                <p class="text-xs text-muted-foreground">{summary.last_inference.generated_tokens} tokens</p>
              </div>
            </div>
          </Card.Content>
        </Card.Root>
      {/if}

      <!-- Inference Time -->
      {#if summary?.last_inference}
        <Card.Root>
          <Card.Content class="pt-4">
            <div class="flex items-start gap-3">
              <div class="p-2 rounded-lg bg-purple-500/10">
                <Clock class="size-6 text-purple-500" />
              </div>
              <div>
                <p class="text-sm text-muted-foreground">{$t('settings.performance.inferenceTime') || 'Inference Time'}</p>
                <p class="text-2xl font-bold text-purple-500">{performanceService.formatDuration(summary.last_inference.total_duration_ms)}</p>
              </div>
            </div>
          </Card.Content>
        </Card.Root>
      {/if}

      <!-- Average Speed -->
      {#if summary && summary.total_generated_tokens > 0}
        <Card.Root>
          <Card.Content class="pt-4">
            <div class="flex items-start gap-3">
              <div class="p-2 rounded-lg bg-blue-500/10">
                <TrendUp class="size-6 text-blue-500" />
              </div>
              <div>
                <p class="text-sm text-muted-foreground">{$t('settings.performance.avgSpeed') || 'Avg Speed'}</p>
                <p class="text-2xl font-bold text-blue-500">{performanceService.formatSpeed(summary.average_tokens_per_second)}</p>
                <p class="text-xs text-muted-foreground">{summary.total_generated_tokens} total tokens</p>
              </div>
            </div>
          </Card.Content>
        </Card.Root>
      {/if}

      <!-- Model Load Time -->
      {#if summary?.last_model_load}
        <Card.Root>
          <Card.Content class="pt-4">
            <div class="flex items-start gap-3">
              <div class="p-2 rounded-lg bg-orange-500/10">
                <Clock class="size-6 text-orange-500" />
              </div>
              <div>
                <p class="text-sm text-muted-foreground">{$t('settings.performance.modelLoad') || 'Model Load'}</p>
                <p class="text-2xl font-bold text-orange-500">{performanceService.formatDuration(summary.last_model_load.total_duration_ms)}</p>
                <p class="text-xs text-muted-foreground">Size: {performanceService.formatMemory(summary.last_model_load.model_size_mb)}</p>
              </div>
            </div>
          </Card.Content>
        </Card.Root>
      {/if}
    </div>

    {#if hardwareInfo}
      <Card.Root>
        <Card.Content class="pt-4">
          <div class="space-y-2">
            <h3 class="font-semibold text-sm">
              {$t('settings.performance.hardware.title') || 'Hardware'}
            </h3>
            <div class="grid gap-2 sm:grid-cols-2">
              <div class="text-sm">
                <span class="text-muted-foreground">
                  {$t('settings.performance.hardware.os') || 'OS'}:
                </span>
                <span class="ml-2">{hardwareInfo.os_name} ({hardwareInfo.os_type})</span>
              </div>
              <div class="text-sm">
                <span class="text-muted-foreground">
                  {$t('settings.performance.hardware.cpu') || 'CPU'}:
                </span>
                <span class="ml-2">{hardwareInfo.cpu.name}</span>
              </div>
              <div class="text-sm">
                <span class="text-muted-foreground">
                  {$t('settings.performance.hardware.cores') || 'Cores'}:
                </span>
                <span class="ml-2">{hardwareInfo.cpu.core_count}</span>
              </div>
              <div class="text-sm">
                <span class="text-muted-foreground">
                  {$t('settings.performance.hardware.arch') || 'Architecture'}:
                </span>
                <span class="ml-2">{hardwareInfo.cpu.arch}</span>
              </div>
              <div class="text-sm sm:col-span-2">
                <span class="text-muted-foreground">
                  {$t('settings.performance.hardware.ram') || 'RAM'}:
                </span>
                <span class="ml-2">{performanceService.formatMemory(hardwareInfo.total_memory)}</span>
              </div>
            </div>
          </div>
        </Card.Content>
      </Card.Root>
    {/if}

    {#if hardwareInfo?.gpus?.length}
      <Card.Root>
        <Card.Content class="pt-4 space-y-3">
          <h3 class="font-semibold text-sm flex items-center gap-2">
            <GpuCard class="size-4" />
            {$t('settings.performance.hardware.gpus') || 'GPUs'}
          </h3>
          {#each hardwareInfo.gpus as gpu}
            {@const usage = gpuUsageByUuid(gpu.uuid)}
            {@const usagePercent = usage && usage.total_memory > 0 ? (usage.used_memory / usage.total_memory) * 100 : null}
            <div class="rounded border p-3 text-sm">
              <div class="font-medium">{gpu.name}</div>
              <div class="text-muted-foreground">{gpu.vendor}</div>
              <div class="mt-1">
                {#if usage}
                  {$t('settings.performance.hardware.gpuMemory') || 'Memory'}:
                  {performanceService.formatMemory(usage.used_memory)} / {performanceService.formatMemory(usage.total_memory)}
                  {#if usagePercent !== null}
                    ({formatPercent(usagePercent)})
                  {/if}
                {:else}
                  {$t('settings.performance.hardware.gpuMemory') || 'Memory'}:
                  {performanceService.formatMemory(gpu.total_memory)}
                {/if}
              </div>
            </div>
          {/each}
        </Card.Content>
      </Card.Root>
    {/if}
  {:else if !loading}
    <div class="text-center py-8 text-muted-foreground">
      <ChartBar class="size-12 mx-auto mb-3 opacity-30" />
      <p>{$t('settings.performance.noData') || 'No performance data available'}</p>
    </div>
  {/if}
</div>
