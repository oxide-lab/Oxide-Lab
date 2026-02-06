<script lang="ts">
  /**
   * Local Models Panel
   *
   * Full-featured panel for managing locally stored GGUF/safetensors models.
   * Features: table view, detail sidebar, inline editing, GGUF metadata viewer.
   */
  import { get } from 'svelte/store';
  import { onMount } from 'svelte';
  import * as Card from '$lib/components/ui/card';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import { Input } from '$lib/components/ui/input';
  import { Spinner } from '$lib/components/ui/spinner';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
  import * as ScrollArea from '$lib/components/ui/scroll-area';
  import { Label } from '$lib/components/ui/label';
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
  import { t } from '$lib/i18n';
  import {
    folderPath,
    models,
    filteredModels,
    filterOptions,
    selectedModel,
    isLoading,
    error,
    scanFolder,
    deleteModel,
  } from '$lib/stores/local-models';
  import { LocalModelsService } from '$lib/services/local-models';
  import type { ModelInfo, ValidationLevel } from '$lib/types/local-models';

  // ─────────────────────────────────────────────────────────────
  // State
  // ─────────────────────────────────────────────────────────────

  let metadataExpanded = $state(false);
  let editingModelPath = $state<string | null>(null);
  let editPublisher = $state('');
  let editName = $state('');
  let searchQuery = $state('');

  // Validation badge variants
  const validationVariants: Record<
    ValidationLevel,
    'default' | 'secondary' | 'destructive' | 'outline'
  > = {
    ok: 'default',
    warning: 'secondary',
    error: 'destructive',
  };

  // ─────────────────────────────────────────────────────────────
  // Editing functions
  // ─────────────────────────────────────────────────────────────

  function startEditing(model: ModelInfo, e: Event) {
    e.stopPropagation();
    editingModelPath = model.path;
    editPublisher = model.metadata?.author ?? model.source_repo_id?.split('/')[0] ?? 'local';
    editName = model.format === 'safetensors' ? (model.source_repo_name ?? model.name) : model.name;
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
          const updated = { ...entry };
          updated.metadata = {
            ...entry.metadata,
            author: editPublisher || entry.metadata?.author,
          };
          if (entry.format === 'safetensors') {
            updated.source_repo_name = editName || entry.source_repo_name;
          } else {
            updated.name = editName || entry.name;
          }
          return updated;
        }),
      );
    } catch (err) {
      console.error('Failed to save metadata', err);
    } finally {
      editingModelPath = null;
    }
  }

  // ─────────────────────────────────────────────────────────────
  // Actions
  // ─────────────────────────────────────────────────────────────

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
      }
    } catch (err) {
      console.error('Failed to select folder', err);
    }
  }

  async function handleRescan() {
    if ($folderPath) {
      await scanFolder($folderPath, true);
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

  function handleSearchInput(e: Event) {
    const value = (e.target as HTMLInputElement).value;
    searchQuery = value;
    updateFilter({ searchText: value });
  }

  async function handleDelete(model: ModelInfo) {
    const confirmMessage = $t('models.local.details.deleteConfirm').replace('{name}', model.name);
    const confirmed = confirm(confirmMessage);
    if (!confirmed) return;
    await deleteModel(model.path);
  }

  function loadSelectedModel() {
    const model = get(selectedModel);
    if (!model) return;
    const ox = (
      window as unknown as {
        __oxide?: { loadModelFromManager?: (args: { path: string; format: string }) => void };
      }
    ).__oxide;
    if (!ox?.loadModelFromManager) return;
    ox.loadModelFromManager({
      path: model.path,
      format: 'gguf',
    });
  }

  function toggleModelSelection(model: ModelInfo) {
    if ($selectedModel?.path === model.path) {
      selectedModel.set(null);
    } else {
      selectedModel.set(model);
    }
  }

  // ─────────────────────────────────────────────────────────────
  // Lifecycle
  // ─────────────────────────────────────────────────────────────

  onMount(async () => {
    if ($folderPath) {
      await scanFolder($folderPath);
    }
  });
</script>

<div class="h-full flex flex-col gap-3 sm:gap-4">
  <!-- Controls Bar -->
  <div class="flex flex-wrap items-center gap-2 sm:gap-3 p-2 sm:p-3 rounded-lg border bg-card">
    <!-- Folder Path -->
    <div class="flex items-center gap-2 min-w-0 flex-1">
      <span class="text-xs text-muted-foreground whitespace-nowrap"
        >{$t('models.local.folderLabel')}</span
      >
      <div
        class="flex-1 min-w-0 px-2 py-1 rounded border bg-muted/30 font-mono text-xs truncate"
        title={$folderPath || $t('models.local.folderNotSelected')}
      >
        {$folderPath || $t('models.local.notSelected')}
      </div>

      <!-- Menu -->
      <DropdownMenu.Root>
        <DropdownMenu.Trigger>
          {#snippet child({ props })}
            <Button {...props} variant="ghost" size="icon">
              <DotsThree class="size-5" weight="bold" />
            </Button>
          {/snippet}
        </DropdownMenu.Trigger>
        <DropdownMenu.Content>
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

    <!-- Search -->
    <div class="relative min-w-[200px]">
      <MagnifyingGlass
        class="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground"
      />
      <Input
        type="search"
        placeholder={$t('models.local.searchPlaceholder')}
        value={searchQuery}
        class="pl-10"
        oninput={handleSearchInput}
      />
    </div>

  </div>

  <!-- Error Banner -->
  {#if $error}
    <div
      class="flex items-center justify-between gap-3 p-3 rounded-lg border border-destructive/30 bg-destructive/10"
    >
      <span class="text-sm text-destructive">{$error}</span>
      <Button variant="outline" size="sm" onclick={handleRescan}>
        {$t('models.local.errors.retry')}
      </Button>
    </div>
  {/if}

  <!-- Main Content -->
  <div
    class="flex-1 min-h-0 grid gap-4"
    class:grid-cols-1={!$selectedModel}
    class:lg:grid-cols-[1fr_360px]={$selectedModel}
  >
    <!-- Models Table -->
    <div class="border rounded-lg bg-card overflow-hidden flex flex-col min-h-0">
      {#if $isLoading}
        <div class="flex items-center justify-center py-12 gap-3 flex-1">
          <Spinner class="size-6" />
          <span class="text-muted-foreground">{$t('common.loading') || 'Loading...'}</span>
        </div>
      {:else if !$filteredModels.length}
        <div class="flex flex-col items-center justify-center py-12 gap-4 flex-1">
          <FolderOpen class="size-12 text-muted-foreground" weight="light" />
          <p class="text-muted-foreground">{$t('models.local.noModels')}</p>
          {#if !$models.length}
            <Button variant="outline" onclick={handleSelectFolder}>
              <FolderOpen class="size-4 mr-2" />
              {$t('models.local.selectFolder')}
            </Button>
          {/if}
        </div>
      {:else}
        <div class="flex-1 min-h-0 overflow-x-auto custom-scrollbar">
          <div class="min-w-[800px] flex flex-col h-full">
            <!-- Fixed Header -->
            <table class="text-sm table-fixed w-full shrink-0">
              <colgroup>
                <col class="w-[12%]" />
                <col class="w-[10%]" />
                <col class="w-[10%]" />
                <col class="w-[28%]" />
                <col class="w-[12%]" />
                <col class="w-[12%]" />
                <col class="w-[10%]" />
              </colgroup>
              <thead>
                <tr class="bg-muted">
                  <th class="text-left px-3 py-2 font-medium">{$t('models.local.table.architecture')}</th>
                  <th class="text-left px-3 py-2 font-medium">{$t('models.local.table.parameters')}</th>
                  <th class="text-left px-3 py-2 font-medium">{$t('models.local.table.publisher')}</th>
                  <th class="text-left px-3 py-2 font-medium">{$t('models.local.table.modelName')}</th>
                  <th class="text-left px-3 py-2 font-medium">{$t('models.local.table.quant')}</th>
                  <th class="text-left px-3 py-2 font-medium">{$t('models.local.table.size')}</th>
                  <th class="text-left px-3 py-2 font-medium">{$t('models.local.table.format')}</th>
                </tr>
              </thead>
            </table>
            <!-- Scrollable Body -->
            <div class="flex-1 min-h-0 overflow-y-auto custom-scrollbar">
              <table class="text-sm table-fixed w-full">
              <colgroup>
                <col class="w-[12%]" />
                <col class="w-[10%]" />
                <col class="w-[10%]" />
                <col class="w-[28%]" />
                <col class="w-[12%]" />
                <col class="w-[12%]" />
                <col class="w-[10%]" />
              </colgroup>
              <tbody>
                {#each $filteredModels as model (model.path)}
                  {@const isSelected = $selectedModel?.path === model.path}
                  <tr
                    class="border-b border-border/50 cursor-pointer transition-colors hover:bg-primary/5 {isSelected
                      ? 'bg-primary/10'
                      : ''}"
                    onclick={() => toggleModelSelection(model)}
                  >
                    <td class="px-3 py-2">{model.architecture ?? '—'}</td>
                    <td class="px-3 py-2">{model.parameter_count ?? '—'}</td>
                    <td class="px-3 py-2">
                      <div class="flex items-center gap-1">
                        <button
                          type="button"
                          class="p-1 rounded hover:bg-muted/50 opacity-60 hover:opacity-100 transition-opacity"
                          onclick={(e) => startEditing(model, e)}
                          aria-label={$t('models.local.details.edit.ariaLabel')}
                        >
                          <PencilSimple class="size-3.5" />
                        </button>
                        <span>
                          {#if model.format === 'safetensors'}
                            {model.metadata?.author ?? '—'}
                          {:else if model.source_repo_id}
                            {model.source_repo_id.split('/')[0]}
                          {:else}
                            {model.metadata?.author ?? '—'}
                          {/if}
                        </span>
                      </div>
                    </td>
                    <td class="px-3 py-2">
                      <div class="flex items-center gap-1">
                        <button
                          type="button"
                          class="p-1 rounded hover:bg-muted/50 opacity-60 hover:opacity-100 transition-opacity"
                          onclick={(e) => startEditing(model, e)}
                          aria-label={$t('models.local.details.edit.ariaLabel')}
                        >
                          <PencilSimple class="size-3.5" />
                        </button>
                        <strong class="truncate max-w-[200px]" title={model.name}>
                          {#if model.format === 'safetensors'}
                            {model.source_repo_name ?? '—'}
                          {:else}
                            {model.name}
                          {/if}
                        </strong>
                      </div>
                    </td>
                    <td class="px-3 py-2">
                      {#if model.format === 'safetensors'}
                        {model.source_quantization ?? '—'}
                      {:else}
                        {model.quantization ?? '—'}
                      {/if}
                    </td>
                    <td class="px-3 py-2">{LocalModelsService.formatFileSize(model.file_size)}</td>
                    <td class="px-3 py-2">
                      <Badge variant="outline" class="uppercase text-[10px]">{model.format}</Badge>
                    </td>
                  </tr>

                  <!-- Edit Row -->
                  {#if editingModelPath === model.path}
                    <tr class="bg-muted/30 border-b border-border/50">
                      <td colspan="7" class="px-3 py-3">
                        <div class="grid grid-cols-1 sm:grid-cols-3 gap-3 items-end">
                          <div class="space-y-1">
                            <Label class="text-xs"
                              >{$t('models.local.details.edit.publisher')}</Label
                            >
                            <Input
                              type="text"
                              placeholder={$t('models.local.details.edit.publisherPlaceholder')}
                              bind:value={editPublisher}
                              onclick={(e: Event) => e.stopPropagation()}
                            />
                          </div>
                          <div class="space-y-1">
                            <Label class="text-xs">{$t('models.local.details.edit.name')}</Label>
                            <Input
                              type="text"
                              placeholder={$t('models.local.details.edit.namePlaceholder')}
                              bind:value={editName}
                              onclick={(e: Event) => e.stopPropagation()}
                            />
                          </div>
                          <div class="flex gap-2 justify-end">
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
                      </td>
                    </tr>
                  {/if}
                {/each}
              </tbody>
            </table>
          </div>
        </div>
      </div>
      {/if}
    </div>

    <!-- Model Details Sidebar -->
    {#if $selectedModel}
      <Card.Root class="flex flex-col min-h-0 overflow-hidden">
        <Card.Header class="pb-2 shrink-0">
          <Card.Title class="text-lg truncate">{$selectedModel.name}</Card.Title>
          <div class="flex gap-2 pt-2">
            <Button variant="destructive" size="sm" onclick={() => handleDelete($selectedModel!)}>
              <Trash class="size-4 mr-1" />
              {$t('models.local.details.delete')}
            </Button>
            <Button size="sm" onclick={loadSelectedModel}>
              <Play class="size-4 mr-1" />
              {$t('models.local.details.loadToChat')}
            </Button>
          </div>
        </Card.Header>

        <div class="flex-1 min-h-0 overflow-y-auto custom-scrollbar">
          <Card.Content class="space-y-4">
            <!-- Properties Grid -->
            <dl class="grid grid-cols-2 gap-2 text-sm">
              <div>
                <dt class="text-xs text-muted-foreground">{$t('models.local.details.path')}</dt>
                <dd class="font-mono text-xs break-all">{$selectedModel.path}</dd>
              </div>
              <div>
                <dt class="text-xs text-muted-foreground">{$t('models.local.details.size')}</dt>
                <dd>{LocalModelsService.formatFileSize($selectedModel.file_size)}</dd>
              </div>
              <div>
                <dt class="text-xs text-muted-foreground">{$t('models.local.details.date')}</dt>
                <dd>{LocalModelsService.formatDate($selectedModel.created_at)}</dd>
              </div>
              <div>
                <dt class="text-xs text-muted-foreground">
                  {$t('models.local.details.architecture')}
                </dt>
                <dd>{$selectedModel.architecture ?? '—'}</dd>
              </div>
              <div>
                <dt class="text-xs text-muted-foreground">{$t('models.local.details.format')}</dt>
                <dd class="uppercase">{$selectedModel.format}</dd>
              </div>
              <div>
                <dt class="text-xs text-muted-foreground">{$t('models.local.details.detected')}</dt>
                <dd>{$selectedModel.detected_architecture ?? '—'}</dd>
              </div>
              <div>
                <dt class="text-xs text-muted-foreground">{$t('models.local.details.context')}</dt>
                <dd>{$selectedModel.context_length ?? '—'}</dd>
              </div>
            </dl>

            <!-- Validation Status -->
            <div class="space-y-2">
              <h4 class="text-sm font-medium">{$t('models.local.details.validation')}</h4>
              <Badge variant={validationVariants[$selectedModel.validation_status.level]}>
                {$t(
                  `models.local.details.${$selectedModel.validation_status.level === 'ok' ? 'valid' : $selectedModel.validation_status.level}`,
                )}
              </Badge>
              {#if $selectedModel.validation_status.messages.length}
                <ul class="text-xs text-muted-foreground list-disc pl-4 space-y-1">
                  {#each $selectedModel.validation_status.messages as message}
                    <li>{message}</li>
                  {/each}
                </ul>
              {/if}
            </div>

            <!-- GGUF Metadata -->
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

                <dl class="grid grid-cols-2 gap-2 text-sm p-2 rounded border bg-muted/30">
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
                      {$selectedModel.vocab_size ??
                        $selectedModel.metadata.tokenizer_tokens?.length ??
                        '—'}
                    </dd>
                  </div>
                </dl>

                {#if metadataExpanded && $selectedModel.metadata.custom_metadata?.length}
                  <div class="border rounded overflow-hidden">
                    <table class="w-full text-xs">
                      <thead class="bg-muted/50">
                        <tr>
                          <th class="text-left px-2 py-1">Key</th>
                          <th class="text-left px-2 py-1">Value</th>
                        </tr>
                      </thead>
                      <tbody>
                        {#each $selectedModel.metadata.custom_metadata as entry (entry.key)}
                          <tr class="border-t border-border/50">
                            <td class="px-2 py-1 font-mono">{entry.key}</td>
                            <td class="px-2 py-1">
                              <pre
                                class="whitespace-pre-wrap break-all text-[10px]">{JSON.stringify(
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
        </div>
      </Card.Root>
    {/if}
  </div>
</div>
