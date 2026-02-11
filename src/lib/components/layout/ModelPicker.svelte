<script lang="ts">
  import { chatState } from '$lib/stores/chat';
  import { models, loadedModelIds } from '$lib/stores/local-models';
  import { areModelPathsEqual, isModelPathLoaded } from '$lib/model-manager/model-identity';
  import { t } from '$lib/i18n';

  import * as Popover from '$lib/components/ui/popover';
  import { Button } from '$lib/components/ui/button';
  import { Spinner } from '$lib/components/ui/spinner';
  import * as Command from '$lib/components/ui/command';
  import { CaretDown, CheckCircle as Check, Repeat, UploadSimple } from 'phosphor-svelte';
  import { cn } from '$lib/utils';
  import type { ModelInfo } from '$lib/types/local-models';

  // Derived state (using $ prefix for stores)
  let quickModels = $derived(
    $models.filter((m) => Boolean(m.source_repo_name?.trim() || m.name?.trim())),
  );
  let currentModelPath = $derived($chatState.modelPath);
  let pendingModelPath = $derived($chatState.pendingModelPath);
  let isModelLoading = $derived($chatState.isLoadingModel);

  let isCurrentModelLoaded = $derived(
    $chatState.isLoaded || isModelPathLoaded(currentModelPath, $loadedModelIds),
  );
  let selectedModelPath = $derived(pendingModelPath || currentModelPath);
  let selectedModel = $derived(
    quickModels.find((m) => areModelPathsEqual(m.path, selectedModelPath)),
  );

  let currentDisplayName = $derived(formatModelLabel(selectedModel));
  let isReloadAvailable = $derived(
    isCurrentModelLoaded && pendingModelPath && pendingModelPath !== currentModelPath,
  );
  let canUnloadCurrentModel = $derived(
    $chatState.isLoaded &&
      !$chatState.busy &&
      !$chatState.isLoadingModel &&
      !$chatState.isUnloadingModel,
  );

  let isPickerOpen = $state(false);
  let isUnloadActionRunning = $state(false);

  function formatModelLabel(model: ModelInfo | null | undefined) {
    if (!model) return $t('common.model.selectModel') || 'Select model';
    const publisher = model.metadata?.author ?? model.source_repo_id?.split('/')[0] ?? 'local';
    const title = model.name ?? model.source_repo_name ?? 'Unnamed';
    return `${publisher}/${title}`;
  }

  function handleSelectModel(model: ModelInfo) {
    const ox = window.__oxide;
    if (ox?.loadModelFromManager) {
      ox.loadModelFromManager({ path: model.path, format: 'gguf' });
    }
    isPickerOpen = false;
  }

  function handleReloadModel() {
    window.__oxide?.reloadSelectedModel?.();
  }

  async function handleUnloadAndClearCache() {
    if (isUnloadActionRunning || !canUnloadCurrentModel) return;
    const ox = window.__oxide;
    if (!ox?.unloadGGUF) return;

    isUnloadActionRunning = true;
    try {
      await ox.unloadGGUF();
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('clear_prefix_cache');
    } finally {
      isUnloadActionRunning = false;
    }
  }
</script>

<div class="flex items-center gap-2">
  <Popover.Root bind:open={isPickerOpen}>
    <Popover.Trigger data-no-drag>
      {#snippet child({ props })}
        <Button
          {...props}
          variant="ghost"
          class={cn('model-combobox-trigger', isPickerOpen && 'model-combobox-trigger--active')}
          role="combobox"
          type="button"
        >
          <span class="model-combobox-body">
            {#if isModelLoading}
              <Spinner size={14} class="model-combobox-spinner" />
            {/if}
            <span class="model-combobox-label">{currentDisplayName}</span>
          </span>
          <CaretDown size={14} />
        </Button>
      {/snippet}
    </Popover.Trigger>
    <Popover.Content class="model-combobox-content" side="bottom" align="start">
      <Command.Root>
        <Command.Input
          class="model-combobox-input"
          placeholder={($t('common.model.selectModel') || 'Select model') + '...'}
        />
        <Command.List class="model-combobox-list custom-scrollbar">
          <Command.Empty class="model-combobox-empty">
            {$t('common.model.noModelsFound') || 'No models found'}
          </Command.Empty>
          <Command.Group>
            {#each quickModels as model (model.path)}
              <Command.Item
                value={formatModelLabel(model)}
                onSelect={() => handleSelectModel(model)}
                class="model-combobox-item"
              >
                <Check
                  size={14}
                  weight="bold"
                  class={cn(
                    'model-combobox-check',
                    !areModelPathsEqual(model.path, selectedModelPath) &&
                      'model-combobox-check--hidden',
                  )}
                />
                <div class="model-combobox-item-body">
                  <span class="model-combobox-item-name">{formatModelLabel(model)}</span>
                  <span class="model-combobox-item-meta"
                    >{model.architecture ?? $t('common.unknownArch')}</span
                  >
                </div>
                {#if areModelPathsEqual(model.path, currentModelPath) && isCurrentModelLoaded}
                  <span class="model-combobox-item-badge">{$t('common.model.current')}</span>
                {:else if isModelPathLoaded(model.path, $loadedModelIds)}
                  <span class="model-combobox-item-badge">{$t('common.loader.loaded')}</span>
                {/if}
              </Command.Item>
            {/each}
          </Command.Group>
        </Command.List>
      </Command.Root>
    </Popover.Content>
  </Popover.Root>

  {#if isReloadAvailable}
    <button
      type="button"
      class="model-reload-btn"
      onclick={handleReloadModel}
      title={$t('common.model.reloadModel')}
    >
      <Repeat size={16} weight="bold" />
    </button>
  {:else if isCurrentModelLoaded}
    <button
      type="button"
      class="model-reload-btn model-unload-btn"
      onclick={handleUnloadAndClearCache}
      disabled={isUnloadActionRunning || !canUnloadCurrentModel}
      title="Unload model and clear cache"
    >
      {#if isUnloadActionRunning}
        <Spinner size={14} />
      {:else}
        <UploadSimple size={16} weight="bold" />
      {/if}
    </button>
  {/if}
</div>

<style>
  :global(.model-combobox-trigger) {
    min-width: 14rem;
    justify-content: center;
    gap: 0.5rem;
    padding: 0.35rem 0.75rem;
    background: var(--background);
    color: var(--foreground);
    border: 1px solid transparent;
    border-radius: 0.5rem;
    -webkit-app-region: no-drag;
    transition: all 0.2s ease;
  }

  :global(.model-combobox-trigger:hover),
  :global(.model-combobox-trigger--active) {
    background: var(--accent);
    border-color: var(--border);
  }

  :global(.model-combobox-label) {
    font-size: 0.95rem;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  :global(.model-combobox-body) {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    flex: 1;
    min-width: 0;
  }

  :global(.model-combobox-spinner) {
    opacity: 0.85;
    flex: 0 0 auto;
  }

  :global(.model-combobox-content) {
    width: 20rem;
    padding: 0.5rem;
    background: var(--popover);
    border: 1px solid var(--border);
    border-radius: 0.75rem;
    z-index: 1200;
    -webkit-app-region: no-drag;
  }

  :global(.model-combobox-input) {
    width: 100%;
    margin-bottom: 0.5rem;
  }

  :global(.model-combobox-list) {
    max-height: 20rem;
    overflow-y: auto;
  }

  :global(.model-combobox-empty) {
    padding: 0.75rem;
    font-size: 0.85rem;
    color: var(--muted-foreground);
  }

  :global(.model-combobox-item) {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.6rem 0.5rem;
    border-radius: 0.5rem;
    cursor: pointer;
  }

  :global(.model-combobox-item:hover) {
    background: var(--accent);
  }

  :global(.model-combobox-check) {
    color: var(--primary);
    flex-shrink: 0;
  }

  :global(.model-combobox-check--hidden) {
    opacity: 0;
  }

  :global(.model-combobox-item-body) {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  :global(.model-combobox-item-name) {
    font-weight: 600;
    font-size: 0.9rem;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  :global(.model-combobox-item-meta) {
    color: var(--muted-foreground);
    font-size: 0.75rem;
  }

  :global(.model-combobox-item-badge) {
    font-size: 0.7rem;
    padding: 0.2rem 0.5rem;
    border-radius: 9999px;
    background: color-mix(in srgb, var(--primary) 18%, transparent);
    color: var(--primary);
    flex-shrink: 0;
  }

  .model-reload-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 2rem;
    height: 2rem;
    border-radius: 0.5rem;
    border: 1px solid var(--border);
    background: transparent;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .model-reload-btn:hover {
    background: var(--accent);
    border-color: var(--primary);
  }

  .model-reload-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .model-unload-btn:hover:not(:disabled) {
    border-color: color-mix(in srgb, var(--destructive) 75%, var(--border));
    background: color-mix(in srgb, var(--destructive) 10%, transparent);
  }
</style>
