<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { get } from 'svelte/store';
  import * as Card from '$lib/components/ui/card';
  import * as Select from '$lib/components/ui/select';
  import * as Tabs from '$lib/components/ui/tabs';
  import * as Collapsible from '$lib/components/ui/collapsible';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import { Input } from '$lib/components/ui/input';
  import { Textarea } from '$lib/components/ui/textarea';
  import { cn } from '$lib/utils';
  import SettingRow from '$lib/components/settings/SettingRow.svelte';
  import VramBudgetCard from '$lib/components/settings/VramBudgetCard.svelte';
  import { hardwareService } from '$lib/services/hardware-service';
  import { llamaBackendService } from '$lib/services/llama-backend-service';
  import { chatState } from '$lib/stores/chat';
  import { t } from '$lib/i18n';
  import type {
    ChatPreset,
    ChatPresetSettings,
    ChatSamplingSettings,
    PerformanceSettings,
  } from '$lib/types/settings-v2';
  import type { InferenceMetrics } from '$lib/types/performance';
  import CaretDown from 'phosphor-svelte/lib/CaretDown';
  import Minus from 'phosphor-svelte/lib/Minus';
  import Plus from 'phosphor-svelte/lib/Plus';

  interface ModelPlan {
    gpuLayers: number;
    maxContextLength: number;
    noOffloadKvCache: boolean;
    offloadMmproj: boolean;
    batchSize: number;
    mode: 'GPU' | 'Hybrid' | 'CPU' | 'Unsupported';
  }

  interface RuntimePreset {
    id: string;
    name: string;
    description: string;
    builtin: boolean;
    values: {
      temperature: number;
      top_p: number;
      top_k: number;
      repeat_penalty: number;
      max_tokens: number;
      seed: number | null;
      system_prompt: string;
      ctx_size: number;
      threads_batch: number;
      flash_attn: string;
    };
  }

  interface Props {
    value: PerformanceSettings;
    chatPresets: ChatPresetSettings;
    highlightedSettingId?: string | null;
    onChange: (next: PerformanceSettings) => void;
    onChangeChatPresets: (next: ChatPresetSettings) => void;
  }

  const RUNTIME_PRESET_STORAGE_KEY = 'settings.v2.runtime.custom_presets';
  const CONTEXT_OPTIONS = [2048, 4096, 8192, 16384, 32768];
  const MAX_TOKEN_OPTIONS = [256, 512, 1024, 2048, 4096];

  const DEFAULT_SAMPLING: ChatSamplingSettings = {
    temperature: 0.7,
    top_p: 0.9,
    top_k: 40,
    min_p: 0,
    repeat_penalty: 1.05,
    max_tokens: 1024,
    seed: null,
    stop_sequences: [],
  };

  const BUILTIN_RUNTIME_PRESETS: RuntimePreset[] = [
    {
      id: 'precise',
      name: 'Precise',
      description: 'Low randomness for factual and deterministic responses.',
      builtin: true,
      values: {
        temperature: 0.2,
        top_p: 0.3,
        top_k: 20,
        repeat_penalty: 1.1,
        max_tokens: 1024,
        seed: 42,
        system_prompt: 'Be precise, concise, and factual.',
        ctx_size: 4096,
        threads_batch: 0,
        flash_attn: 'auto',
      },
    },
    {
      id: 'balanced',
      name: 'Balanced',
      description: 'General-purpose profile for everyday chats.',
      builtin: true,
      values: {
        temperature: 0.7,
        top_p: 0.9,
        top_k: 40,
        repeat_penalty: 1.05,
        max_tokens: 1024,
        seed: null,
        system_prompt: 'You are a helpful assistant.',
        ctx_size: 8192,
        threads_batch: 0,
        flash_attn: 'auto',
      },
    },
    {
      id: 'creative',
      name: 'Creative',
      description: 'Higher diversity for ideation and storytelling.',
      builtin: true,
      values: {
        temperature: 0.9,
        top_p: 0.95,
        top_k: 80,
        repeat_penalty: 1,
        max_tokens: 1536,
        seed: null,
        system_prompt: 'Be imaginative and propose diverse ideas.',
        ctx_size: 8192,
        threads_batch: 0,
        flash_attn: 'auto',
      },
    },
    {
      id: 'coding',
      name: 'Coding',
      description: 'Deterministic profile for code generation and reviews.',
      builtin: true,
      values: {
        temperature: 0.1,
        top_p: 0.2,
        top_k: 20,
        repeat_penalty: 1.1,
        max_tokens: 1200,
        seed: 42,
        system_prompt: 'Act as a strict coding assistant. Prefer deterministic outputs.',
        ctx_size: 8192,
        threads_batch: 0,
        flash_attn: 'auto',
      },
    },
  ];

  let {
    value,
    chatPresets,
    highlightedSettingId = null,
    onChange,
    onChangeChatPresets,
  }: Props = $props();

  let mode = $state<'basic' | 'advanced'>('basic');
  let samplingGroupOpen = $state(true);
  let contextGroupOpen = $state(true);
  let runtimeGroupOpen = $state(true);
  let repetitionGroupOpen = $state(true);
  let promptExpanded = $state(false);

  let totalVramGb = $state(0);
  let usedVramGb = $state(0);
  let planner = $state<ModelPlan | null>(null);

  let customRuntimePresets = $state<RuntimePreset[]>([]);
  let selectedRuntimePresetId = $state<string>('balanced');
  let presetNotice = $state('');
  let backendNotice = $state('');
  let backendLoadError = $state<string | null>(null);
  let backendLoading = $state(false);
  let installedBackendStrings = $state<string[]>([]);
  let detectedCurrentBackend = $state<string | null>(null);
  let manualThreadLimitText = $state(value.manual_thread_limit?.toString() ?? '');
  let tokensPerSecond = $state<number | null>(null);
  let latencyMs = $state<number | null>(null);
  let inferenceUnlisten: (() => void) | null = null;

  const runtimePresets = $derived([...BUILTIN_RUNTIME_PRESETS, ...customRuntimePresets]);
  const selectedRuntimePreset = $derived(
    runtimePresets.find((preset) => preset.id === selectedRuntimePresetId) ?? null,
  );
  const selectedBackendString = $derived(
    value.llama_runtime.selected_backend ?? detectedCurrentBackend ?? installedBackendStrings[0] ?? '',
  );
  const effectivePresetId = $derived(
    chatPresets.presets.some((preset) => preset.id === chatPresets.default_preset_id)
      ? chatPresets.default_preset_id
      : (chatPresets.presets[0]?.id ?? ''),
  );
  const activePreset = $derived(
    chatPresets.presets.find((preset) => preset.id === effectivePresetId) ?? null,
  );
  const activeSampling = $derived(activePreset?.sampling ?? DEFAULT_SAMPLING);
  const modelContextLimit = $derived(planner?.maxContextLength ?? 8192);
  const isContextOverLimit = $derived(value.llama_runtime.ctx_size > modelContextLimit);
  const isCreativeWarning = $derived(activeSampling.temperature > 1 || activeSampling.top_p > 1);
  const selectedContextValue = $derived(
    CONTEXT_OPTIONS.includes(value.llama_runtime.ctx_size) ? String(value.llama_runtime.ctx_size) : 'custom',
  );
  const selectedMaxTokensValue = $derived(
    MAX_TOKEN_OPTIONS.includes(activeSampling.max_tokens) ? String(activeSampling.max_tokens) : 'custom',
  );
  const temperatureSliderValue = $derived(temperatureToSlider(activeSampling.temperature));
  const temperatureToneLabel = $derived(
    activeSampling.temperature <= 0.35
      ? 'Deterministic'
      : activeSampling.temperature <= 0.8
        ? 'Balanced'
        : 'Creative',
  );
  const temperatureToneClass = $derived(
    activeSampling.temperature <= 0.35
      ? 'text-sky-600 dark:text-sky-400'
      : activeSampling.temperature <= 0.8
        ? 'text-emerald-600 dark:text-emerald-400'
        : 'text-orange-600 dark:text-orange-400',
  );

  $effect(() => {
    manualThreadLimitText = value.manual_thread_limit?.toString() ?? '';
  });

  function clamp(valueToClamp: number, min: number, max: number): number {
    return Math.min(max, Math.max(min, valueToClamp));
  }

  function roundTo(valueToRound: number, digits: number): number {
    const factor = 10 ** digits;
    return Math.round(valueToRound * factor) / factor;
  }

  function sliderToTemperature(sliderValue: number): number {
    const normalized = clamp(sliderValue, 0, 100) / 100;
    if (normalized <= 0.7) {
      return roundTo(normalized / 0.7, 2);
    }
    return roundTo(1 + ((normalized - 0.7) / 0.3), 2);
  }

  function temperatureToSlider(temperature: number): number {
    const clamped = clamp(temperature, 0, 2);
    if (clamped <= 1) {
      return Math.round((clamped * 0.7) * 100);
    }
    return Math.round((0.7 + ((clamped - 1) * 0.3)) * 100);
  }

  function parseMaybeNumber(rawValue: string): number | null {
    const cleaned = rawValue.trim();
    if (cleaned.length === 0) return null;
    const parsed = Number(cleaned);
    return Number.isFinite(parsed) ? parsed : null;
  }

  function parseCustomRuntimePresets(rawValue: string | null): RuntimePreset[] {
    if (!rawValue) return [];
    try {
      const parsed = JSON.parse(rawValue) as unknown;
      if (!Array.isArray(parsed)) return [];
      return parsed.filter((item): item is RuntimePreset => {
        if (typeof item !== 'object' || item === null) return false;
        const candidate = item as Partial<RuntimePreset>;
        return (
          typeof candidate.id === 'string' &&
          typeof candidate.name === 'string' &&
          typeof candidate.description === 'string' &&
          typeof candidate.values === 'object' &&
          candidate.values !== null
        );
      });
    } catch {
      return [];
    }
  }

  function persistCustomRuntimePresets(next: RuntimePreset[]) {
    if (typeof window === 'undefined') return;
    localStorage.setItem(RUNTIME_PRESET_STORAGE_KEY, JSON.stringify(next));
  }

  function markPresetAsCustom() {
    if (selectedRuntimePresetId !== 'custom') {
      selectedRuntimePresetId = 'custom';
    }
  }
  function updateChatPresets(nextPresets: ChatPreset[]) {
    const normalizedDefault =
      nextPresets.some((preset) => preset.id === chatPresets.default_preset_id)
        ? chatPresets.default_preset_id
        : (nextPresets[0]?.id ?? chatPresets.default_preset_id);
    onChangeChatPresets({
      ...chatPresets,
      default_preset_id: normalizedDefault,
      presets: nextPresets,
    });
  }

  function updateActivePreset(mutator: (preset: ChatPreset) => ChatPreset) {
    if (!effectivePresetId) return;
    updateChatPresets(
      chatPresets.presets.map((preset) => (preset.id === effectivePresetId ? mutator(preset) : preset)),
    );
  }

  function updateSampling(patch: Partial<ChatSamplingSettings>) {
    markPresetAsCustom();
    updateActivePreset((preset) => ({
      ...preset,
      sampling: {
        ...preset.sampling,
        ...patch,
      },
    }));
  }

  function updateSystemPrompt(next: string) {
    markPresetAsCustom();
    updateActivePreset((preset) => ({
      ...preset,
      system_prompt: next,
    }));
  }

  function commitRuntime(patch: Partial<PerformanceSettings['llama_runtime']>) {
    markPresetAsCustom();
    onChange({
      ...value,
      llama_runtime: {
        ...value.llama_runtime,
        ...patch,
      },
    });
  }

  function backendPriority(backendString: string): number {
    const backend = backendString.split('/')[1]?.toLowerCase() ?? backendString.toLowerCase();
    if (backend.includes('cuda-13')) return 0;
    if (backend.includes('cuda-12')) return 1;
    if (backend.includes('cuda-11')) return 2;
    if (backend.includes('vulkan')) return 3;
    if (backend.includes('common_cpus') || backend.includes('cpu')) return 4;
    return 5;
  }

  function formatBackendLabel(backendString: string): string {
    const [version, backend] = backendString.split('/');
    if (!version || !backend) return backendString;
    return `${version} / ${backend}`;
  }

  async function loadInstalledBackends() {
    backendLoading = true;
    backendLoadError = null;
    try {
      const overview = await llamaBackendService.getInstalledOverview();
      const raw = [
        ...overview.installed.map((entry) => `${entry.version}/${entry.backend}`),
        ...(overview.currentBackend ? [overview.currentBackend] : []),
      ];
      const deduped = Array.from(new Set(raw)).sort((a, b) => {
        const priorityDelta = backendPriority(a) - backendPriority(b);
        if (priorityDelta !== 0) return priorityDelta;
        return a.localeCompare(b);
      });
      installedBackendStrings = deduped;
      detectedCurrentBackend = overview.currentBackend;
    } catch (error) {
      backendLoadError = error instanceof Error ? error.message : String(error);
    } finally {
      backendLoading = false;
    }
  }

  async function handleSelectBackend(nextBackend: string) {
    if (!nextBackend || nextBackend === value.llama_runtime.selected_backend) return;
    backendNotice = '';
    backendLoadError = null;
    backendLoading = true;
    try {
      const updated = await llamaBackendService.setSelectedBackend(nextBackend);
      commitRuntime({
        selected_backend: nextBackend,
        server_path: updated.serverPath,
      });
      backendNotice = `Selected llama.cpp backend: ${nextBackend}`;
    } catch (error) {
      backendLoadError = error instanceof Error ? error.message : String(error);
    } finally {
      backendLoading = false;
      await loadInstalledBackends();
    }
  }

  function setContextWindow(next: number) {
    const normalized = Math.max(512, Math.round(next));
    commitRuntime({ ctx_size: normalized });
    updateActivePreset((preset) => ({
      ...preset,
      context: normalized,
    }));
  }

  function getCurrentValues(): RuntimePreset['values'] {
    return {
      temperature: activeSampling.temperature,
      top_p: activeSampling.top_p,
      top_k: activeSampling.top_k,
      repeat_penalty: activeSampling.repeat_penalty,
      max_tokens: activeSampling.max_tokens,
      seed: activeSampling.seed,
      system_prompt: activePreset?.system_prompt ?? '',
      ctx_size: value.llama_runtime.ctx_size,
      threads_batch: value.llama_runtime.threads_batch,
      flash_attn: value.llama_runtime.flash_attn,
    };
  }

  function applyRuntimePreset(preset: RuntimePreset) {
    selectedRuntimePresetId = preset.id;
    presetNotice = preset.description;
    onChange({
      ...value,
      llama_runtime: {
        ...value.llama_runtime,
        ctx_size: preset.values.ctx_size,
        threads_batch: preset.values.threads_batch,
        flash_attn: preset.values.flash_attn || 'auto',
      },
    });
    updateActivePreset((chatPreset) => ({
      ...chatPreset,
      system_prompt: preset.values.system_prompt,
      context: preset.values.ctx_size,
      sampling: {
        ...chatPreset.sampling,
        temperature: preset.values.temperature,
        top_p: preset.values.top_p,
        top_k: preset.values.top_k,
        repeat_penalty: preset.values.repeat_penalty,
        max_tokens: preset.values.max_tokens,
        seed: preset.values.seed,
      },
    }));
  }

  function saveCurrentAsPreset() {
    if (typeof window === 'undefined') return;
    const suggestedName = `Custom ${customRuntimePresets.length + 1}`;
    const name = window.prompt('Preset name', suggestedName)?.trim();
    if (!name) return;
    const id = `runtime_custom_${Date.now()}`;
    const nextPreset: RuntimePreset = {
      id,
      name,
      description: 'Saved from current runtime configuration.',
      builtin: false,
      values: getCurrentValues(),
    };
    const nextCustomPresets = [...customRuntimePresets, nextPreset];
    customRuntimePresets = nextCustomPresets;
    persistCustomRuntimePresets(nextCustomPresets);
    selectedRuntimePresetId = id;
    presetNotice = `${name} saved.`;
  }

  function resetRuntimeDefaults() {
    presetNotice = 'Runtime settings reset to Balanced defaults.';
    onChange({
      ...value,
      manual_thread_limit: null,
      llama_runtime: {
        ...value.llama_runtime,
        ctx_size: 8192,
        threads_batch: 0,
        flash_attn: 'auto',
      },
    });
    const balancedPreset = BUILTIN_RUNTIME_PRESETS.find((preset) => preset.id === 'balanced');
    if (balancedPreset) {
      applyRuntimePreset(balancedPreset);
    }
  }

  async function loadHardwareUsage() {
    try {
      const [info, usage] = await Promise.all([
        hardwareService.getSystemInfo(),
        hardwareService.getSystemUsage(),
      ]);
      totalVramGb = info.gpus.reduce((acc, gpu) => acc + gpu.total_memory, 0) / 1024;
      usedVramGb = usage.gpus.reduce((acc, gpu) => acc + gpu.used_memory, 0) / 1024;
    } catch (error) {
      console.warn('Failed to load hardware usage', error);
    }
  }

  async function refreshPlanner(modelPathOverride?: string) {
    const modelPath = modelPathOverride ?? get(chatState).modelPath;
    const mmprojPath = get(chatState).mmprojPath;
    if (!modelPath) {
      planner = null;
      return;
    }
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      planner = await invoke<ModelPlan>('plugin:llamacpp|plan_model_load', {
        path: modelPath,
        memoryMode: value.memory_mode,
        mmprojPath: mmprojPath?.trim() ? mmprojPath.trim() : null,
        requestedCtx: Math.max(512, value.llama_runtime.ctx_size),
      });
    } catch (error) {
      console.warn('Failed to calculate model plan', error);
      planner = null;
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
    void loadHardwareUsage();
    void refreshPlanner();
    void subscribeInferenceMetrics();
    void loadInstalledBackends();
    if (typeof window !== 'undefined') {
      customRuntimePresets = parseCustomRuntimePresets(localStorage.getItem(RUNTIME_PRESET_STORAGE_KEY));
    }
  });

  onDestroy(() => {
    if (inferenceUnlisten) {
      inferenceUnlisten();
      inferenceUnlisten = null;
    }
  });

  $effect(() => {
    value.memory_mode;
    value.llama_runtime.ctx_size;
    void refreshPlanner();
  });
</script>

<div class="space-y-3">
  <VramBudgetCard
    usedGb={usedVramGb}
    totalGb={Math.max(1, totalVramGb)}
    predictedGb={Math.max(usedVramGb, (((planner?.gpuLayers ?? 0) / 100) * Math.max(totalVramGb, 1)))}
    recommendedGpuLayers={planner?.gpuLayers ?? null}
  />

  <Card.Root>
    <Card.Header>
      <Card.Title>{$t('settings.v2.sections.performance.title')}</Card.Title>
      <Card.Description>{$t('settings.v2.sections.performance.description')}</Card.Description>
    </Card.Header>

    <Card.Content class="space-y-4">
      <div class="rounded-lg border px-3 py-3 space-y-3">
        <div class="flex flex-wrap items-center gap-2">
          <Select.Root
            type="single"
            value={selectedRuntimePresetId}
            onValueChange={(next) => {
              const selectedId = next ?? selectedRuntimePresetId;
              selectedRuntimePresetId = selectedId;
              const preset = runtimePresets.find((item) => item.id === selectedId);
              if (preset) {
                applyRuntimePreset(preset);
              }
            }}
          >
            <Select.Trigger class="w-full sm:w-[260px]">
              {selectedRuntimePreset?.name ?? 'Custom'}
            </Select.Trigger>
            <Select.Content>
              {#each runtimePresets as preset (preset.id)}
                <Select.Item value={preset.id}>{preset.name}</Select.Item>
              {/each}
              <Select.Item value="custom">Custom</Select.Item>
            </Select.Content>
          </Select.Root>
          <Button variant="outline" size="sm" onclick={saveCurrentAsPreset}>Save current as preset</Button>
          <Button variant="outline" size="sm" onclick={resetRuntimeDefaults}>Revert to defaults</Button>
          {#if activePreset}
            <Badge variant="secondary">{`Editing: ${activePreset.name}`}</Badge>
          {/if}
        </div>

        {#if presetNotice}
          <p class="text-xs text-muted-foreground">{presetNotice}</p>
        {/if}

        <div class="flex flex-wrap items-center gap-2 text-xs">
          <Badge variant="secondary">
            {tokensPerSecond !== null ? `${tokensPerSecond.toFixed(2)} tok/s` : 'tok/s: n/a'}
          </Badge>
          <Badge variant="secondary">
            {latencyMs !== null ? `Latency: ${Math.round(latencyMs)} ms` : 'Latency: n/a'}
          </Badge>
          <Badge variant="outline">Model context limit: {modelContextLimit}</Badge>
        </div>
      </div>

      <Tabs.Root bind:value={mode}>
        <Tabs.List class="w-full sm:w-fit">
          <Tabs.Trigger value="basic" class="flex-1">Basic</Tabs.Trigger>
          <Tabs.Trigger value="advanced" class="flex-1">Advanced</Tabs.Trigger>
        </Tabs.List>

        <Tabs.Content value="basic" class="space-y-3">
          <SettingRow
            id="performance.temperature"
            title="Temperature"
            description="Lower values are more deterministic. Higher values are more creative."
            highlighted={highlightedSettingId === 'chat_presets.temperature'}
          >
            <div class="space-y-2">
              <input
                type="range"
                min="0"
                max="100"
                step="1"
                value={temperatureSliderValue}
                class="w-full accent-primary"
                oninput={(event) =>
                  updateSampling({
                    temperature: sliderToTemperature(Number((event.currentTarget as HTMLInputElement).value) || 0),
                  })}
              />
              <div class="flex items-center justify-between gap-2 text-xs text-muted-foreground">
                <span>Precise</span>
                <span class={cn('font-medium', temperatureToneClass)}>{temperatureToneLabel}</span>
                <span>Creative</span>
              </div>
              <Input
                type="number"
                min="0"
                max="2"
                step="0.01"
                value={activeSampling.temperature}
                oninput={(event) =>
                  updateSampling({
                    temperature: clamp(Number((event.currentTarget as HTMLInputElement).value) || 0, 0, 2),
                  })}
              />
            </div>
          </SettingRow>

          <SettingRow
            id="performance.max_tokens"
            title="Max tokens"
            description="Response length limit for generated output."
          >
            <div class="space-y-2">
              <div class="flex items-center gap-2">
                <Select.Root
                  type="single"
                  value={selectedMaxTokensValue}
                  onValueChange={(next) => {
                    if (!next || next === 'custom') return;
                    updateSampling({ max_tokens: Math.max(1, Number(next) || activeSampling.max_tokens) });
                  }}
                >
                  <Select.Trigger class="w-full sm:w-[160px]">{selectedMaxTokensValue}</Select.Trigger>
                  <Select.Content>
                    {#each MAX_TOKEN_OPTIONS as option (option)}
                      <Select.Item value={String(option)}>{option}</Select.Item>
                    {/each}
                    <Select.Item value="custom">Custom</Select.Item>
                  </Select.Content>
                </Select.Root>
                <Button
                  type="button"
                  variant="outline"
                  size="icon"
                  class="size-8"
                  onclick={() => updateSampling({ max_tokens: Math.max(1, activeSampling.max_tokens - 64) })}
                >
                  <Minus class="size-3.5" />
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="icon"
                  class="size-8"
                  onclick={() => updateSampling({ max_tokens: activeSampling.max_tokens + 64 })}
                >
                  <Plus class="size-3.5" />
                </Button>
              </div>
              <Input
                type="number"
                min="1"
                step="1"
                value={activeSampling.max_tokens}
                oninput={(event) =>
                  updateSampling({
                    max_tokens: Math.max(1, Number((event.currentTarget as HTMLInputElement).value) || 1),
                  })}
              />
            </div>
          </SettingRow>

          <SettingRow
            id="performance.system_prompt"
            title="System prompt"
            description="Default system prompt used by the default preset."
          >
            <Textarea
              value={activePreset?.system_prompt ?? ''}
              rows={promptExpanded ? 6 : 2}
              placeholder="You are a helpful assistant..."
              onfocus={() => (promptExpanded = true)}
              onblur={() => (promptExpanded = false)}
              oninput={(event) => updateSystemPrompt((event.currentTarget as HTMLTextAreaElement).value)}
            />
          </SettingRow>

          <SettingRow
            id="performance.ctx_size"
            title="Context window"
            description="Token context used at runtime. Mirrored into default preset context."
            highlighted={highlightedSettingId === 'performance.ctx_size'}
          >
            <div class="space-y-2">
              <div class="flex items-center gap-2">
                <Select.Root
                  type="single"
                  value={selectedContextValue}
                  onValueChange={(next) => {
                    if (!next || next === 'custom') return;
                    setContextWindow(Number(next) || value.llama_runtime.ctx_size);
                  }}
                >
                  <Select.Trigger class="w-full sm:w-[160px]">{selectedContextValue}</Select.Trigger>
                  <Select.Content>
                    {#each CONTEXT_OPTIONS as option (option)}
                      <Select.Item value={String(option)}>{option}</Select.Item>
                    {/each}
                    <Select.Item value="custom">Custom</Select.Item>
                  </Select.Content>
                </Select.Root>
                <Button
                  type="button"
                  variant="outline"
                  size="icon"
                  class="size-8"
                  onclick={() => setContextWindow(value.llama_runtime.ctx_size - 512)}
                >
                  <Minus class="size-3.5" />
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="icon"
                  class="size-8"
                  onclick={() => setContextWindow(value.llama_runtime.ctx_size + 512)}
                >
                  <Plus class="size-3.5" />
                </Button>
              </div>
              <Input
                type="number"
                min="512"
                step="1"
                value={value.llama_runtime.ctx_size}
                oninput={(event) =>
                  setContextWindow(Math.max(512, Number((event.currentTarget as HTMLInputElement).value) || 512))}
              />
              {#if isContextOverLimit}
                <p class="text-xs text-amber-600 dark:text-amber-400">
                  Current context exceeds model limit ({modelContextLimit}). Runtime may clamp this value.
                </p>
              {/if}
            </div>
          </SettingRow>
        </Tabs.Content>

        <Tabs.Content value="advanced" class="space-y-3">
          <Collapsible.Root bind:open={samplingGroupOpen} class="rounded-lg border">
            <Collapsible.Trigger class="flex w-full items-center justify-between px-3 py-2 text-sm font-medium">
              <span>Sampling</span>
              <CaretDown class={cn('size-4 transition-transform', samplingGroupOpen && 'rotate-180')} />
            </Collapsible.Trigger>
            <Collapsible.Content class="border-t px-3 py-3 space-y-3">
              <SettingRow id="performance.top_p" title="Top-p" description="Cumulative probability threshold.">
                <Input
                  type="number"
                  min="0"
                  max="2"
                  step="0.01"
                  value={activeSampling.top_p}
                  oninput={(event) =>
                    updateSampling({
                      top_p: clamp(Number((event.currentTarget as HTMLInputElement).value) || 0, 0, 2),
                    })}
                />
              </SettingRow>

              <SettingRow id="performance.top_k" title="Top-k" description="Candidate token shortlist size.">
                <Input
                  type="number"
                  min="1"
                  step="1"
                  value={activeSampling.top_k}
                  oninput={(event) =>
                    updateSampling({
                      top_k: Math.max(1, Number((event.currentTarget as HTMLInputElement).value) || 1),
                    })}
                />
              </SettingRow>

              <SettingRow
                id="performance.seed"
                title="Seed"
                description="Use a fixed seed for reproducible outputs. Leave empty for random."
              >
                <Input
                  type="number"
                  placeholder="random"
                  value={activeSampling.seed ?? ''}
                  oninput={(event) => {
                    const parsed = parseMaybeNumber((event.currentTarget as HTMLInputElement).value);
                    updateSampling({ seed: parsed === null ? null : Math.round(parsed) });
                  }}
                />
              </SettingRow>

              {#if isCreativeWarning}
                <p class="text-xs text-amber-600 dark:text-amber-400">
                  Values above 1.0 can increase artifacts and instability in output.
                </p>
              {/if}
            </Collapsible.Content>
          </Collapsible.Root>

          <Collapsible.Root bind:open={contextGroupOpen} class="rounded-lg border">
            <Collapsible.Trigger class="flex w-full items-center justify-between px-3 py-2 text-sm font-medium">
              <span>Context</span>
              <CaretDown class={cn('size-4 transition-transform', contextGroupOpen && 'rotate-180')} />
            </Collapsible.Trigger>
            <Collapsible.Content class="border-t px-3 py-3 space-y-3">
              <SettingRow
                id="performance.manual_thread_limit"
                title={$t('settings.v2.performance.manual_thread_limit.title')}
                description={$t('settings.v2.performance.manual_thread_limit.description')}
                highlighted={highlightedSettingId === 'performance.manual_thread_limit'}
              >
                <Input
                  value={manualThreadLimitText}
                  inputmode="numeric"
                  placeholder={$t('settings.v2.performance.auto')}
                  oninput={(event) => (manualThreadLimitText = (event.currentTarget as HTMLInputElement).value)}
                  onblur={() => {
                    markPresetAsCustom();
                    const parsed = parseMaybeNumber(manualThreadLimitText);
                    onChange({
                      ...value,
                      manual_thread_limit: parsed === null ? null : Math.max(1, Math.round(parsed)),
                    });
                  }}
                />
              </SettingRow>

              <SettingRow
                id="performance.ctx_size_advanced"
                title="Context size"
                description="Advanced context configuration for llama runtime."
              >
                <Input
                  type="number"
                  min="512"
                  step="1"
                  value={value.llama_runtime.ctx_size}
                  oninput={(event) =>
                    setContextWindow(Math.max(512, Number((event.currentTarget as HTMLInputElement).value) || 512))}
                />
              </SettingRow>
            </Collapsible.Content>
          </Collapsible.Root>

          <Collapsible.Root bind:open={runtimeGroupOpen} class="rounded-lg border">
            <Collapsible.Trigger class="flex w-full items-center justify-between px-3 py-2 text-sm font-medium">
              <span>Runtime Engine</span>
              <CaretDown class={cn('size-4 transition-transform', runtimeGroupOpen && 'rotate-180')} />
            </Collapsible.Trigger>
            <Collapsible.Content class="border-t px-3 py-3 space-y-3">
              <SettingRow
                id="performance.backend_binary"
                title="llama.cpp backend binary"
                description="Manual backend selection (CUDA/CPU/Vulkan)."
              >
                <div class="space-y-2">
                  <div class="flex flex-wrap items-center gap-2">
                    <Select.Root
                      type="single"
                      value={selectedBackendString}
                      onValueChange={(next) => {
                        if (!next) return;
                        void handleSelectBackend(next);
                      }}
                    >
                      <Select.Trigger class="w-full sm:w-[380px]">
                        {selectedBackendString ? formatBackendLabel(selectedBackendString) : 'No backend detected'}
                      </Select.Trigger>
                      <Select.Content>
                        {#each installedBackendStrings as backendString (backendString)}
                          <Select.Item value={backendString}>{formatBackendLabel(backendString)}</Select.Item>
                        {/each}
                      </Select.Content>
                    </Select.Root>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      disabled={backendLoading}
                      onclick={() => void loadInstalledBackends()}
                    >
                      Refresh
                    </Button>
                  </div>
                  <p class="text-xs text-muted-foreground">
                    Current binary path: {value.llama_runtime.server_path ?? 'auto-resolve from selected backend'}
                  </p>
                  {#if backendLoading}
                    <p class="text-xs text-muted-foreground">Refreshing backend list…</p>
                  {/if}
                  {#if backendNotice}
                    <p class="text-xs text-emerald-600 dark:text-emerald-400">{backendNotice}</p>
                  {/if}
                  {#if backendLoadError}
                    <p class="text-xs text-destructive">{backendLoadError}</p>
                  {/if}
                </div>
              </SettingRow>

              <SettingRow
                id="performance.threads_batch"
                title={$t('settings.v2.performance.threads_batch.title')}
                description={$t('settings.v2.performance.threads_batch.description')}
                highlighted={highlightedSettingId === 'performance.threads_batch'}
              >
                <Input
                  type="number"
                  min="0"
                  step="1"
                  value={value.llama_runtime.threads_batch}
                  oninput={(event) =>
                    commitRuntime({ threads_batch: Math.max(0, Number((event.currentTarget as HTMLInputElement).value) || 0) })}
                />
              </SettingRow>

              <SettingRow
                id="performance.ubatch_size"
                title={$t('settings.v2.performance.ubatch_size.title')}
                description={$t('settings.v2.performance.ubatch_size.description')}
                highlighted={highlightedSettingId === 'performance.ubatch_size'}
              >
                <Input
                  type="number"
                  min="1"
                  step="1"
                  value={value.llama_runtime.ubatch_size}
                  oninput={(event) =>
                    commitRuntime({ ubatch_size: Math.max(1, Number((event.currentTarget as HTMLInputElement).value) || 1) })}
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
            </Collapsible.Content>
          </Collapsible.Root>

          <Collapsible.Root bind:open={repetitionGroupOpen} class="rounded-lg border">
            <Collapsible.Trigger class="flex w-full items-center justify-between px-3 py-2 text-sm font-medium">
              <span>Repetition</span>
              <CaretDown class={cn('size-4 transition-transform', repetitionGroupOpen && 'rotate-180')} />
            </Collapsible.Trigger>
            <Collapsible.Content class="border-t px-3 py-3 space-y-3">
              <SettingRow
                id="performance.repeat_penalty"
                title="Repeat penalty"
                description="Penalizes repeated words and tokens."
              >
                <Input
                  type="number"
                  min="0.1"
                  max="2"
                  step="0.01"
                  value={activeSampling.repeat_penalty}
                  oninput={(event) =>
                    updateSampling({
                      repeat_penalty: clamp(Number((event.currentTarget as HTMLInputElement).value) || 1, 0.1, 2),
                    })}
                />
              </SettingRow>
            </Collapsible.Content>
          </Collapsible.Root>
        </Tabs.Content>
      </Tabs.Root>
    </Card.Content>
  </Card.Root>
</div>
