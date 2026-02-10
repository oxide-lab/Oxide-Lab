<script lang="ts">
  /**
   * Loader Panel Component
   *
   * Model loading settings and status display in a Sheet panel.
   */
  import { t } from '$lib/i18n';
  import { Label } from '$lib/components/ui/label';
  import { Badge } from '$lib/components/ui/badge';
  import { Progress } from '$lib/components/ui/progress';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Input } from '$lib/components/ui/input';
  import { Button } from '$lib/components/ui/button';
  import * as Select from '$lib/components/ui/select';
  import { cn } from '../../utils';
  import Cpu from 'phosphor-svelte/lib/Cpu';
  import GpuCard from 'phosphor-svelte/lib/GraphicsCard';
  import Check from 'phosphor-svelte/lib/Check';
  import Sparkle from 'phosphor-svelte/lib/Sparkle';

  interface Props {
    format?: 'gguf' | 'hub_gguf';
    modelPath?: string;
    repoId?: string;
    revision?: string;
    hubGgufFilename?: string;
    mmprojPath?: string;
    ctx_limit_value?: number;
    isLoadingModel?: boolean;
    isUnloadingModel?: boolean;
    isCancelling?: boolean;
    loadingStage?: string;
    loadingProgress?: number;
    unloadingProgress?: number;
    errorText?: string;
    busy?: boolean;
    isLoaded?: boolean;
    use_gpu?: boolean;
    cuda_available?: boolean;
    cuda_build?: boolean;
    avx?: boolean;
    neon?: boolean;
    simd128?: boolean;
    f16c?: boolean;
    split_prompt?: boolean;
    verbose_prompt?: boolean;
    tracing?: boolean;
    use_custom_params?: boolean;
    temperature?: number;
    temperature_enabled?: boolean;
    top_k_enabled?: boolean;
    top_k_value?: number;
    top_p_enabled?: boolean;
    top_p_value?: number;
    min_p_enabled?: boolean;
    min_p_value?: number;
    repeat_penalty_enabled?: boolean;
    repeat_penalty_value?: number;
    presets?: { id: string; name: string }[];
    selectedPresetId?: string | null;
    onPresetSelect?: (presetId: string) => void;
    onPresetApply?: () => void;
    onDeviceToggle?: (enabled: boolean) => void;
    class?: string;
  }

  let {
    format = $bindable('gguf'),
    modelPath = $bindable(''),
    repoId = $bindable(''),
    revision = $bindable(''),
    hubGgufFilename = $bindable(''),
    mmprojPath = $bindable(''),
    ctx_limit_value = $bindable(4096),
    isLoadingModel = $bindable(false),
    isUnloadingModel = $bindable(false),
    isCancelling = $bindable(false),
    loadingStage = $bindable(''),
    loadingProgress = $bindable(0),
    unloadingProgress = $bindable(0),
    errorText = $bindable(''),
    busy = $bindable(false),
    isLoaded = $bindable(false),
    use_gpu = $bindable(false),
    cuda_available = $bindable(false),
    cuda_build = $bindable(false),
    avx = $bindable(false),
    neon = $bindable(false),
    simd128 = $bindable(false),
    f16c = $bindable(false),
    split_prompt = $bindable(false),
    verbose_prompt = $bindable(false),
    tracing = $bindable(false),
    use_custom_params = $bindable(false),
    temperature = $bindable(0.8),
    temperature_enabled = $bindable(false),
    top_k_enabled = $bindable(false),
    top_k_value = $bindable(40),
    top_p_enabled = $bindable(false),
    top_p_value = $bindable(0.9),
    min_p_enabled = $bindable(false),
    min_p_value = $bindable(0.05),
    repeat_penalty_enabled = $bindable(false),
    repeat_penalty_value = $bindable(1.1),
    presets = [],
    selectedPresetId = $bindable(null),
    onPresetSelect,
    onPresetApply,
    onDeviceToggle,
    class: className = '',
  }: Props = $props();

  function setDevice(enabled: boolean) {
    if (onDeviceToggle) {
      onDeviceToggle(enabled);
    } else {
      use_gpu = enabled;
    }
  }

  const contextOptions = [2048, 4096, 8192, 16384, 32768];

  function formatLoadingStage(stage: string): string {
    if (!stage) return '';
    const key = `chat.loading.stages.${stage}`;
    const localized = $t(key);
    if (localized && localized !== key) {
      return localized;
    }
    const fallback = $t('chat.loading.stages.default', { stage });
    return fallback || stage;
  }

</script>

<section class={cn('loader-panel space-y-6', className)}>
  {#if presets.length > 0}
    <div class="space-y-3">
      <div class="flex items-center justify-between">
        <Label class="text-sm font-medium flex items-center gap-1">
          <Sparkle class="size-4" />
          {$t('settings.v2.chat_presets.preset_label')}
        </Label>
        <Button
          variant="outline"
          size="sm"
          onclick={() => onPresetApply?.()}
          disabled={!selectedPresetId}
        >
          {$t('settings.v2.chat_presets.actions.apply')}
        </Button>
      </div>
      <Select.Root
        type="single"
        value={selectedPresetId ?? undefined}
        onValueChange={(next) => {
          if (!next) return;
          selectedPresetId = next;
          onPresetSelect?.(next);
        }}
      >
        <Select.Trigger class="w-full">
          {presets.find((preset) => preset.id === selectedPresetId)?.name ?? $t('settings.v2.chat_presets.select')}
        </Select.Trigger>
        <Select.Content>
          {#each presets as preset (preset.id)}
            <Select.Item value={preset.id}>{preset.name}</Select.Item>
          {/each}
        </Select.Content>
      </Select.Root>
    </div>
  {/if}

  <!-- Device Selector -->
  <div class="space-y-3">
    <Label class="text-sm font-medium">{$t('common.loader.device') || 'Device'}</Label>
    <div class="flex gap-2">
      <Button
        variant="outline"
        class={cn(
          'flex-1 flex items-center justify-center gap-2 p-3 rounded-lg border transition-all',
          !use_gpu
            ? 'border-primary bg-primary/10 text-primary'
            : 'border-border hover:border-muted-foreground',
        )}
        onclick={() => setDevice(false)}
      >
        <Cpu class="size-5" />
        <span>CPU</span>
        {#if !use_gpu}
          <Check class="size-4 ml-auto" />
        {/if}
      </Button>
      <Button
        variant="outline"
        class={cn(
          'flex-1 flex items-center justify-center gap-2 p-3 rounded-lg border transition-all',
          !cuda_available && !cuda_build && 'opacity-50 cursor-not-allowed',
          use_gpu
            ? 'border-primary bg-primary/10 text-primary'
            : 'border-border hover:border-muted-foreground',
        )}
        onclick={() => setDevice(true)}
        disabled={!cuda_available && !cuda_build}
      >
        <GpuCard class="size-5" />
        <span>GPU</span>
        {#if use_gpu}
          <Check class="size-4 ml-auto" />
        {/if}
      </Button>
    </div>
    {#if !cuda_available && !cuda_build}
      <p class="text-xs text-muted-foreground">
        {$t('common.loader.gpuNotAvailable') || 'GPU not available (CUDA not detected)'}
      </p>
    {/if}
  </div>

  <!-- Context Length -->
  <div class="space-y-3">
    <Label class="text-sm font-medium"
      >{$t('common.loader.contextLength') || 'Context Length'}</Label
    >
    <div class="flex flex-wrap gap-2">
      {#each contextOptions as option}
        <Button
          variant="outline"
          size="sm"
          class={cn(
            'transition-all',
            ctx_limit_value === option
              ? 'border-primary bg-primary/10 text-primary font-medium'
              : 'border-border hover:border-muted-foreground',
          )}
          onclick={() => {
            ctx_limit_value = option;
          }}
        >
          {option.toLocaleString()}
        </Button>
      {/each}
    </div>
  </div>

  <div class="space-y-2">
    <Label class="text-sm font-medium">MMProj Path (optional)</Label>
    <Input
      type="text"
      placeholder="C:\\models\\mmproj.gguf"
      bind:value={mmprojPath}
      disabled={busy || isLoadingModel}
    />
    <p class="text-xs text-muted-foreground">Required for most vision GGUF models.</p>
  </div>

  <!-- CPU Features -->
  <div class="space-y-3">
    <Label class="text-sm font-medium">{$t('common.loader.cpuFeatures') || 'CPU Features'}</Label>
    <div class="flex flex-wrap gap-2">
      <Badge variant={avx ? 'default' : 'outline'} class={cn(!avx && 'opacity-50')}>AVX</Badge>
      <Badge variant={neon ? 'default' : 'outline'} class={cn(!neon && 'opacity-50')}>NEON</Badge>
      <Badge variant={simd128 ? 'default' : 'outline'} class={cn(!simd128 && 'opacity-50')}>
        SIMD128
      </Badge>
      <Badge variant={f16c ? 'default' : 'outline'} class={cn(!f16c && 'opacity-50')}>F16C</Badge>
    </div>
  </div>

  <!-- Advanced Options -->
  <div class="space-y-3">
    <Label class="text-sm font-medium"
      >{$t('common.loader.advancedOptions') || 'Advanced Options'}</Label
    >
    <div class="space-y-3">
      <div class="flex items-center gap-2">
        <Checkbox id="split-prompt" bind:checked={split_prompt} />
        <Label for="split-prompt" class="text-sm cursor-pointer">
          {$t('common.loader.splitPrompt') || 'Split prompt'}
        </Label>
      </div>
      <div class="flex items-center gap-2">
        <Checkbox id="verbose-prompt" bind:checked={verbose_prompt} />
        <Label for="verbose-prompt" class="text-sm cursor-pointer">
          {$t('common.loader.verbosePrompt') || 'Verbose prompt'}
        </Label>
      </div>
      <div class="flex items-center gap-2">
        <Checkbox id="tracing" bind:checked={tracing} />
        <Label for="tracing" class="text-sm cursor-pointer">
          {$t('common.loader.chromeTracing') || 'Chrome tracing'}
        </Label>
      </div>
    </div>
  </div>

  <!-- Sampling Parameters -->
  <div class="space-y-3">
    <div class="flex items-center justify-between">
      <Label class="text-sm font-medium">
        {$t('common.loader.sampling.title') || 'Sampling Parameters'}
      </Label>
      <div class="flex items-center gap-2">
        <Checkbox id="use-custom-params" bind:checked={use_custom_params} />
        <Label for="use-custom-params" class="text-xs cursor-pointer text-muted-foreground">
          {$t('common.loader.sampling.useCustom') || 'Use custom parameters'}
        </Label>
      </div>
    </div>

    <div class="grid grid-cols-1 gap-3">
      <div class="grid grid-cols-[auto_1fr] items-center gap-3">
        <div class="flex items-center gap-2">
          <Checkbox id="temperature-enabled" bind:checked={temperature_enabled} disabled={!use_custom_params} />
          <Label for="temperature-enabled" class="text-sm cursor-pointer">
            {$t('common.loader.sampling.temperature') || 'Temperature'}
          </Label>
        </div>
        <Input type="number" step="0.01" min="0" max="2" bind:value={temperature} disabled={!use_custom_params || !temperature_enabled} />
      </div>

      <div class="grid grid-cols-[auto_1fr] items-center gap-3">
        <div class="flex items-center gap-2">
          <Checkbox id="top-k-enabled" bind:checked={top_k_enabled} disabled={!use_custom_params} />
          <Label for="top-k-enabled" class="text-sm cursor-pointer">
            {$t('common.loader.sampling.topK') || 'Top-K'}
          </Label>
        </div>
        <Input type="number" step="1" min="1" bind:value={top_k_value} disabled={!use_custom_params || !top_k_enabled} />
      </div>

      <div class="grid grid-cols-[auto_1fr] items-center gap-3">
        <div class="flex items-center gap-2">
          <Checkbox id="top-p-enabled" bind:checked={top_p_enabled} disabled={!use_custom_params} />
          <Label for="top-p-enabled" class="text-sm cursor-pointer">
            {$t('common.loader.sampling.topP') || 'Top-P'}
          </Label>
        </div>
        <Input type="number" step="0.01" min="0" max="1" bind:value={top_p_value} disabled={!use_custom_params || !top_p_enabled} />
      </div>

      <div class="grid grid-cols-[auto_1fr] items-center gap-3">
        <div class="flex items-center gap-2">
          <Checkbox id="min-p-enabled" bind:checked={min_p_enabled} disabled={!use_custom_params} />
          <Label for="min-p-enabled" class="text-sm cursor-pointer">
            {$t('common.loader.sampling.minP') || 'Min-P'}
          </Label>
        </div>
        <Input type="number" step="0.01" min="0" max="1" bind:value={min_p_value} disabled={!use_custom_params || !min_p_enabled} />
      </div>

      <div class="grid grid-cols-[auto_1fr] items-center gap-3">
        <div class="flex items-center gap-2">
          <Checkbox id="repeat-penalty-enabled" bind:checked={repeat_penalty_enabled} disabled={!use_custom_params} />
          <Label for="repeat-penalty-enabled" class="text-sm cursor-pointer">
            {$t('common.loader.sampling.repeatPenalty') || 'Repeat penalty'}
          </Label>
        </div>
        <Input type="number" step="0.01" min="0.1" max="2" bind:value={repeat_penalty_value} disabled={!use_custom_params || !repeat_penalty_enabled} />
      </div>
    </div>
  </div>

  <!-- Loading Status -->
  {#if isLoadingModel || isUnloadingModel}
    <div class="space-y-3 p-4 rounded-lg border bg-muted/50">
      <div class="flex items-center justify-between">
        <span class="text-sm font-medium">
          {#if isLoadingModel}
            {$t('common.loader.loading') || 'Loading model...'}
          {:else}
            {$t('common.loader.unloading') || 'Unloading model...'}
          {/if}
        </span>
        {#if loadingStage}
          <Badge variant="outline">{formatLoadingStage(loadingStage)}</Badge>
        {/if}
      </div>
      <Progress value={isLoadingModel ? loadingProgress : unloadingProgress} class="h-2" />
      {#if isCancelling}
        <p class="text-xs text-muted-foreground">
          {$t('common.loader.cancelling') || 'Cancelling...'}
        </p>
      {/if}
    </div>
  {/if}

  <!-- Error Display -->
  {#if errorText}
    <div class="p-4 rounded-lg border border-destructive/50 bg-destructive/10">
      <p class="text-sm text-destructive">{errorText}</p>
    </div>
  {/if}

  <!-- Model Info -->
  {#if isLoaded && modelPath}
    <div class="p-4 rounded-lg border bg-muted/30">
      <div class="flex items-center gap-2 mb-2">
        <Badge variant="default">{$t('common.loader.loaded') || 'Loaded'}</Badge>
      </div>
      <p class="text-xs text-muted-foreground truncate">{modelPath}</p>
    </div>
  {/if}
</section>
