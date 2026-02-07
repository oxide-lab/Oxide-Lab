<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import * as Card from '$lib/components/ui/card';
  import * as Select from '$lib/components/ui/select';
  import { Input } from '$lib/components/ui/input';
  import SettingRow from '$lib/components/settings/SettingRow.svelte';
  import VramBudgetCard from '$lib/components/settings/VramBudgetCard.svelte';
  import { hardwareService } from '$lib/services/hardware-service';
  import { chatState } from '$lib/stores/chat';
  import { t } from '$lib/i18n';
  import type { PerformanceSettings } from '$lib/types/settings-v2';

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
    expertMode?: boolean;
    onChange: (next: PerformanceSettings) => void;
  }

  let { value, highlightedSettingId = null, expertMode = false, onChange }: Props = $props();

  let totalVramGb = $state(0);
  let usedVramGb = $state(0);
  let planner = $state<ModelPlan | null>(null);

  let threadLimitText = $state(value.manual_thread_limit?.toString() ?? '');
  let nGpuLayersText = $state(value.llama_runtime.n_gpu_layers.toString());
  let threadsText = $state(value.llama_runtime.threads.toString());
  let threadsBatchText = $state(value.llama_runtime.threads_batch.toString());
  let ctxSizeText = $state(value.llama_runtime.ctx_size.toString());
  let batchSizeText = $state(value.llama_runtime.batch_size.toString());
  let ubatchSizeText = $state(value.llama_runtime.ubatch_size.toString());

  $effect(() => {
    threadLimitText = value.manual_thread_limit?.toString() ?? '';
    nGpuLayersText = value.llama_runtime.n_gpu_layers.toString();
    threadsText = value.llama_runtime.threads.toString();
    threadsBatchText = value.llama_runtime.threads_batch.toString();
    ctxSizeText = value.llama_runtime.ctx_size.toString();
    batchSizeText = value.llama_runtime.batch_size.toString();
    ubatchSizeText = value.llama_runtime.ubatch_size.toString();
  });

  async function loadHardwareUsage() {
    try {
      const [info, usage] = await Promise.all([
        hardwareService.getSystemInfo(),
        hardwareService.getSystemUsage(),
      ]);
      totalVramGb = info.gpus.reduce((acc, gpu) => acc + gpu.total_memory, 0) / 1024;
      usedVramGb = usage.gpus.reduce((acc, gpu) => acc + gpu.used_memory, 0) / 1024;
    } catch (e) {
      console.warn('Failed to load hardware usage', e);
    }
  }

  async function refreshPlanner() {
    const modelPath = get(chatState).modelPath;
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
    } catch (e) {
      console.warn('Failed to calculate model plan', e);
      planner = null;
    }
  }

  function commitRuntime(patch: Partial<PerformanceSettings['llama_runtime']>) {
    onChange({
      ...value,
      llama_runtime: {
        ...value.llama_runtime,
        ...patch,
      },
    });
  }

  onMount(() => {
    void loadHardwareUsage();
    void refreshPlanner();
  });

  $effect(() => {
    void refreshPlanner();
  });
</script>

<div class="space-y-3">
  <VramBudgetCard
    usedGb={usedVramGb}
    totalGb={Math.max(1, totalVramGb)}
    predictedGb={Math.max(usedVramGb, (planner?.gpuLayers ?? 0) / 100 * Math.max(totalVramGb, 1))}
    recommendedGpuLayers={planner?.gpuLayers ?? null}
  />

  <Card.Root>
    <Card.Header>
      <Card.Title>{$t('settings.v2.sections.performance.title')}</Card.Title>
      <Card.Description>{$t('settings.v2.sections.performance.description')}</Card.Description>
    </Card.Header>
    <Card.Content class="space-y-3">
      <SettingRow
        id="performance.manual_thread_limit"
        title={$t('settings.v2.performance.manual_thread_limit.title')}
        description={$t('settings.v2.performance.manual_thread_limit.description')}
        highlighted={highlightedSettingId === 'performance.manual_thread_limit'}
      >
        <Input
          bind:value={threadLimitText}
          inputmode="numeric"
          placeholder={$t('settings.v2.performance.auto')}
          onblur={() =>
            onChange({
              ...value,
              manual_thread_limit: threadLimitText.trim() ? Math.max(1, Number(threadLimitText)) : null,
            })}
        />
      </SettingRow>

      <SettingRow
        id="performance.n_gpu_layers"
        title={$t('settings.v2.performance.n_gpu_layers.title')}
        description={$t('settings.v2.performance.n_gpu_layers.description')}
        highlighted={highlightedSettingId === 'performance.n_gpu_layers'}
      >
        <Input
          bind:value={nGpuLayersText}
          inputmode="numeric"
          onblur={() => commitRuntime({ n_gpu_layers: Math.max(0, Number(nGpuLayersText) || 0) })}
        />
      </SettingRow>

      <SettingRow
        id="performance.ctx_size"
        title={$t('settings.v2.performance.ctx_size.title')}
        description={$t('settings.v2.performance.ctx_size.description')}
        highlighted={highlightedSettingId === 'performance.ctx_size'}
      >
        <Input
          bind:value={ctxSizeText}
          inputmode="numeric"
          onblur={() => commitRuntime({ ctx_size: Math.max(512, Number(ctxSizeText) || 4096) })}
        />
      </SettingRow>

      <SettingRow
        id="performance.batch_size"
        title={$t('settings.v2.performance.batch_size.title')}
        description={$t('settings.v2.performance.batch_size.description')}
        highlighted={highlightedSettingId === 'performance.batch_size'}
      >
        <Input
          bind:value={batchSizeText}
          inputmode="numeric"
          onblur={() => commitRuntime({ batch_size: Math.max(1, Number(batchSizeText) || 512) })}
        />
      </SettingRow>

      <SettingRow
        id="performance.ubatch_size"
        title={$t('settings.v2.performance.ubatch_size.title')}
        description={$t('settings.v2.performance.ubatch_size.description')}
        highlighted={highlightedSettingId === 'performance.ubatch_size'}
      >
        <Input
          bind:value={ubatchSizeText}
          inputmode="numeric"
          onblur={() => commitRuntime({ ubatch_size: Math.max(1, Number(ubatchSizeText) || 512) })}
        />
      </SettingRow>

      {#if expertMode}
        <SettingRow
          id="performance.threads"
          title={$t('settings.v2.performance.threads.title')}
          description={$t('settings.v2.performance.threads.description')}
          highlighted={highlightedSettingId === 'performance.threads'}
        >
          <Input
            bind:value={threadsText}
            inputmode="numeric"
            onblur={() => commitRuntime({ threads: Number(threadsText) || 0 })}
          />
        </SettingRow>

        <SettingRow
          id="performance.threads_batch"
          title={$t('settings.v2.performance.threads_batch.title')}
          description={$t('settings.v2.performance.threads_batch.description')}
          highlighted={highlightedSettingId === 'performance.threads_batch'}
        >
          <Input
            bind:value={threadsBatchText}
            inputmode="numeric"
            onblur={() => commitRuntime({ threads_batch: Number(threadsBatchText) || 0 })}
          />
        </SettingRow>

        <SettingRow
          id="performance.flash_attn"
          title={$t('settings.v2.performance.flash_attn.title')}
          description={$t('settings.v2.performance.flash_attn.description')}
          highlighted={highlightedSettingId === 'performance.flash_attn'}
        >
          <Select.Root
            type="single"
            value={value.llama_runtime.flash_attn}
            onValueChange={(next) => commitRuntime({ flash_attn: next ?? 'auto' })}
          >
            <Select.Trigger class="w-full">{value.llama_runtime.flash_attn}</Select.Trigger>
            <Select.Content>
              <Select.Item value="auto">{$t('settings.v2.performance.flash_attn.auto')}</Select.Item>
              <Select.Item value="on">{$t('settings.v2.performance.flash_attn.on')}</Select.Item>
              <Select.Item value="off">{$t('settings.v2.performance.flash_attn.off')}</Select.Item>
            </Select.Content>
          </Select.Root>
        </SettingRow>
      {/if}

      <SettingRow
        id="performance.memory_mode"
        title={$t('settings.v2.performance.memory_mode.title')}
        description={$t('settings.v2.performance.memory_mode.description')}
        highlighted={highlightedSettingId === 'performance.memory_mode'}
      >
        <Select.Root
          type="single"
          value={value.memory_mode}
          onValueChange={(next) =>
            onChange({ ...value, memory_mode: (next ?? 'medium') as PerformanceSettings['memory_mode'] })}
        >
          <Select.Trigger class="w-full">{value.memory_mode}</Select.Trigger>
          <Select.Content>
            <Select.Item value="low">{$t('settings.v2.performance.memory_mode.low')}</Select.Item>
            <Select.Item value="medium">{$t('settings.v2.performance.memory_mode.medium')}</Select.Item>
            <Select.Item value="high">{$t('settings.v2.performance.memory_mode.high')}</Select.Item>
          </Select.Content>
        </Select.Root>
      </SettingRow>
    </Card.Content>
  </Card.Root>
</div>
