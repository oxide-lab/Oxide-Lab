<script lang="ts">
  /**
   * Recommendations Panel
   *
   * Browse and download curated model cards from HuggingFace.
   * Uses model-cards store for state management.
   */
  import { onMount } from 'svelte';
  import * as Card from '$lib/components/ui/card';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import { Input } from '$lib/components/ui/input';
  import * as Select from '$lib/components/ui/select';
  import { Spinner } from '$lib/components/ui/spinner';
  import * as ScrollArea from '$lib/components/ui/scroll-area';
  import MagnifyingGlass from 'phosphor-svelte/lib/MagnifyingGlass';
  import DownloadSimple from 'phosphor-svelte/lib/DownloadSimple';
  import ArrowClockwise from 'phosphor-svelte/lib/ArrowClockwise';
  import Heart from 'phosphor-svelte/lib/Heart';
  import Cube from 'phosphor-svelte/lib/Cube';
  import ModelIcon from '$lib/components/ModelIcon.svelte';
  import FileArrowUp from 'phosphor-svelte/lib/FileArrowUp';
  import ArrowCounterClockwise from 'phosphor-svelte/lib/ArrowCounterClockwise';
  import ArrowSquareOut from 'phosphor-svelte/lib/ArrowSquareOut';
  import { t } from '$lib/i18n';
  import { folderPath, scanFolder } from '$lib/stores/local-models';
  import {
    filteredModelCards,
    importModelCards,
    loadModelCards,
    modelCardFilters,
    modelCardsError,
    modelCardsLoading,
    modelCardsVersion,
    resetModelCards,
    uniqueFamilies,
  } from '$lib/stores/model-cards';
  import {
    activeDownloads,
    downloadHistory,
    downloadsLoaded,
    ensureDownloadManager,
  } from '$lib/stores/download-manager';
  import { ModelCardsService } from '$lib/services/model-cards';
  import type { ModelCardSummary } from '$lib/types/model-cards';

  // State
  let selectedCard = $state<ModelCardSummary | null>(null);
  let downloadErrors: Record<string, string> = $state({});
  let downloadQueued: Record<string, boolean> = $state({});
  let selectedQuantizations: Record<string, string> = $state({});
  let lastProcessedHistoryId: string | null = $state(null);

  // Derived values
  let selectedCardQuantizations = $derived<string[]>(
    ((selectedCard as ModelCardSummary | null)?.gguf_quantizations ?? []) as string[],
  );

  onMount(() => {
    void loadModelCards();
    void ensureDownloadManager();
  });

  // ─────────────────────────────────────────────────────────────
  // Helpers
  // ─────────────────────────────────────────────────────────────

  function getDownloadId(card: ModelCardSummary, format: 'gguf') {
    return `model-card::${card.id}::${format}`;
  }

  function jobGroupKey(job: { group_id?: string | null; repo_id: string; filename: string }) {
    return job.group_id ?? `${job.repo_id}::${job.filename}`;
  }

  function setQuantization(cardId: string, quant: string) {
    selectedQuantizations = { ...selectedQuantizations, [cardId]: quant };
  }

  function isDownloading(card: ModelCardSummary, format: 'gguf') {
    const key = getDownloadId(card, format);
    if (downloadQueued[key]) {
      return true;
    }
    if ($downloadsLoaded) {
      return $activeDownloads.some((job) => jobGroupKey(job) === key);
    }
    return false;
  }

  function updateFilter(filter: Partial<{ searchText: string; family: string; format: 'gguf' | '' }>) {
    modelCardFilters.update((prev) => ({ ...prev, ...filter }));
  }

  // ─────────────────────────────────────────────────────────────
  // Actions
  // ─────────────────────────────────────────────────────────────

  async function handleDownload(card: ModelCardSummary, format: 'gguf') {
    if (!$folderPath) {
      alert($t('models.remote.selectFolderAlert'));
      return;
    }
    const downloadId = getDownloadId(card, format);
    downloadErrors = { ...downloadErrors, [downloadId]: '' };
    try {
      const quantization = selectedQuantizations[card.id];
      downloadQueued = { ...downloadQueued, [downloadId]: true };
      await ModelCardsService.downloadModelCardFormat(card.id, format, $folderPath, quantization);
    } catch (error) {
      downloadErrors = {
        ...downloadErrors,
        [downloadId]: error instanceof Error ? error.message : String(error),
      };
      const next = { ...downloadQueued };
      delete next[downloadId];
      downloadQueued = next;
    }
  }

  async function refreshCards() {
    await loadModelCards(true);
  }

  async function handleImportConfig() {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = (await open({
        filters: [{ name: 'JSON', extensions: ['json'] }],
        multiple: false,
      })) as string | string[] | undefined;
      const path = Array.isArray(selected) ? selected[0] : selected;
      if (typeof path === 'string' && path.length > 0) {
        await importModelCards(path);
      }
    } catch (error) {
      console.error('Import failed', error);
    }
  }

  async function handleResetConfig() {
    await resetModelCards();
  }

  function openOnHuggingFace(repoId: string) {
    window.open(`https://huggingface.co/${repoId}`, '_blank');
  }

  // ─────────────────────────────────────────────────────────────
  // Effects
  // ─────────────────────────────────────────────────────────────

  // Auto-select first card when list updates
  $effect(() => {
    if ($filteredModelCards.length && !selectedCard) {
      selectedCard = $filteredModelCards[0];
    }
  });

  // Re-select card if current selection is filtered out
  $effect(() => {
    if (selectedCard && !$filteredModelCards.find((card) => card.id === selectedCard?.id)) {
      selectedCard = $filteredModelCards[0] ?? null;
    }
  });

  // Auto-select first quantization for selected card
  $effect(() => {
    if (selectedCard && selectedCardQuantizations.length) {
      const current = selectedQuantizations[selectedCard.id];
      if (!current || !selectedCardQuantizations.includes(current)) {
        setQuantization(selectedCard.id, selectedCardQuantizations[0]);
      }
    }
  });

  // Handle download completion
  $effect(() => {
    if ($downloadHistory.length) {
      const latest = $downloadHistory[0];
      if (latest?.group_id && downloadQueued[latest.group_id]) {
        const next = { ...downloadQueued };
        delete next[latest.group_id];
        downloadQueued = next;
      }
      if (
        latest &&
        $folderPath &&
        latest.id !== lastProcessedHistoryId &&
        latest.status === 'completed' &&
        latest.destination_path?.startsWith($folderPath)
      ) {
        lastProcessedHistoryId = latest.id;
        scanFolder($folderPath, true);
      }
    }
  });
</script>

<div class="h-full flex flex-col gap-3 sm:gap-4">
  <!-- Search Bar -->
  <div class="flex flex-wrap items-center gap-2 sm:gap-3 p-2 sm:p-3 rounded-lg border bg-card">
    <!-- Search Input -->
    <div class="flex-1 min-w-[200px] relative">
      <MagnifyingGlass
        class="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground"
      />
      <Input
        type="search"
        placeholder={$t('models.remote.searchPlaceholder')}
        value={$modelCardFilters.searchText}
        class="pl-10"
        oninput={(e: Event) => updateFilter({ searchText: (e.currentTarget as HTMLInputElement).value })}
      />
    </div>

    <!-- Family Filter -->
    <Select.Root
      type="single"
      value={$modelCardFilters.family || ''}
      onValueChange={(v) => updateFilter({ family: v ?? '' })}
    >
      <Select.Trigger class="w-[160px]">
        {$modelCardFilters.family || $t('models.remote.allFamilies')}
      </Select.Trigger>
      <Select.Content>
        <Select.Item value="">{$t('models.remote.allFamilies')}</Select.Item>
        {#each $uniqueFamilies as family}
          <Select.Item value={family}>{family}</Select.Item>
        {/each}
      </Select.Content>
    </Select.Root>

    <!-- Format Filter -->
    <Select.Root
      type="single"
      value={$modelCardFilters.format || ''}
      onValueChange={(v) => updateFilter({ format: (v ?? '') as 'gguf' | '' })}
    >
      <Select.Trigger class="w-[140px]">
        {$modelCardFilters.format?.toUpperCase() || $t('models.remote.allFormats')}
      </Select.Trigger>
      <Select.Content>
        <Select.Item value="">{$t('models.remote.allFormats')}</Select.Item>
        <Select.Item value="gguf">GGUF</Select.Item>
      </Select.Content>
    </Select.Root>

    <!-- Refresh Button -->
    <Button variant="default" size="sm" onclick={refreshCards} disabled={$modelCardsLoading}>
      {#if $modelCardsLoading}
        <Spinner class="size-4 mr-2" />
      {:else}
        <ArrowClockwise class="size-4 mr-2" />
      {/if}
      {$modelCardsLoading ? $t('models.remote.refreshing') : $t('models.remote.refresh')}
    </Button>

    <!-- Config Actions -->
    <div class="flex items-center gap-2">
      <Button
        variant="outline"
        size="sm"
        onclick={handleImportConfig}
        disabled={$modelCardsLoading}
      >
        <FileArrowUp class="size-4 mr-1" />
        {$t('models.remote.importConfig')}
      </Button>
      <Button variant="outline" size="sm" onclick={handleResetConfig} disabled={$modelCardsLoading}>
        <ArrowCounterClockwise class="size-4 mr-1" />
        {$t('models.remote.resetConfig')}
      </Button>
      <Badge variant="secondary" class="text-xs">
        {$t('models.remote.version')}
        {$modelCardsVersion ?? '—'}
      </Badge>
    </div>
  </div>

  <!-- Error Banner -->
  {#if $modelCardsError}
    <div
      class="flex items-center justify-between gap-3 p-3 rounded-lg border border-destructive/30 bg-destructive/10"
    >
      <span class="text-sm text-destructive">{$modelCardsError}</span>
      <Button variant="outline" size="sm" onclick={refreshCards}>
        {$t('models.remote.retry')}
      </Button>
    </div>
  {/if}

  <!-- Results Section -->
  <div class="flex-1 min-h-0">
    {#if $modelCardsLoading}
      <div class="flex items-center justify-center py-12 gap-3">
        <Spinner class="size-6" />
        <span class="text-muted-foreground">{$t('models.remote.loading')}</span>
      </div>
    {:else if !$filteredModelCards.length}
      <Card.Root>
        <Card.Content class="py-12 text-center">
          <Cube class="size-12 mx-auto mb-4 text-muted-foreground" weight="light" />
          <p class="text-muted-foreground">{$t('models.remote.noResults')}</p>
        </Card.Content>
      </Card.Root>
    {:else}
      <div class="grid grid-cols-1 lg:grid-cols-[2fr_3fr] gap-4 h-full min-h-0">
        <!-- Models List -->
        <div class="border rounded-lg bg-card overflow-hidden flex flex-col min-h-0 overflow-y-auto custom-scrollbar">
          <ScrollArea.Root class="flex-1">
            <div class="p-2 space-y-2">
              {#each $filteredModelCards as card (card.id)}
                <button
                  type="button"
                  class="w-full p-3 rounded-lg text-left transition-colors {selectedCard?.id ===
                  card.id
                    ? 'bg-primary/20 border border-primary/40'
                    : 'bg-muted/30 hover:bg-muted/50 border border-transparent'}"
                  onclick={() => (selectedCard = card)}
                >
                  <div class="flex justify-between items-start gap-2">
                    <div class="min-w-0 flex-1">
                      <div class="flex items-center gap-2 mb-1">
                        <Cube class="size-4 text-muted-foreground flex-shrink-0" />
                        <strong class="block truncate">{card.name}</strong>
                      </div>
                      <span class="text-xs text-muted-foreground block truncate">
                        {card.family ?? '—'} · {card.hf_repo_id}
                      </span>
                      <div class="flex flex-wrap gap-1 mt-1">
                        {#each card.tags.slice(0, 3) as tag}
                          <Badge variant="outline" class="text-[10px] px-1.5 py-0">{tag}</Badge>
                        {/each}
                      </div>
                    </div>
                    {#if isDownloading(card, 'gguf')}
                      <Badge variant="secondary" class="text-xs flex-shrink-0">
                        <Spinner class="size-3 mr-1" />
                        {$t('models.remote.downloading')}
                      </Badge>
                    {/if}
                  </div>
                </button>
              {/each}
            </div>
          </ScrollArea.Root>
        </div>

        <!-- Model Detail -->
        <div class="border rounded-lg bg-card overflow-auto custom-scrollbar">
          {#if selectedCard}
            <div class="p-4 space-y-4">
              <!-- Header -->
              <div class="flex items-start justify-between gap-4">
                <div class="flex items-center gap-3">
                  <div
                    class="size-12 rounded-lg bg-muted/50 flex items-center justify-center overflow-hidden"
                  >
                    <ModelIcon family={selectedCard.family} size={32} />
                  </div>
                  <div>
                    <h3 class="font-semibold text-lg">
                      <button
                        type="button"
                        class="hover:underline inline-flex items-center gap-1"
                        onclick={() => openOnHuggingFace(selectedCard!.hf_repo_id)}
                      >
                        {selectedCard.name}
                        <ArrowSquareOut class="size-4" />
                      </button>
                    </h3>
                    <p class="text-sm text-muted-foreground">{selectedCard.hf_repo_id}</p>
                  </div>
                </div>
                <Badge variant="outline" class="flex items-center gap-1">
                  <Heart class="size-3" weight="bold" />
                  {selectedCard.tags.length || '—'}
                </Badge>
              </div>

              <!-- Description -->
              <p class="text-sm text-muted-foreground leading-relaxed">
                {selectedCard.description ?? $t('models.remote.noDescription')}
              </p>

              <!-- Sources -->
              {#if selectedCard.sources}
                <div class="text-xs text-muted-foreground space-y-1">
                  {#if selectedCard.sources.gguf}
                    <div>
                      {$t('models.remote.sources.gguf')}
                      {selectedCard.sources.gguf.repo_id}
                    </div>
                  {/if}
                </div>
              {/if}

              <!-- Tags -->
              <div class="flex flex-wrap gap-1.5">
                {#each selectedCard.tags as tag}
                  <Badge variant="secondary" class="text-xs">{tag}</Badge>
                {/each}
              </div>

              <!-- Download Section -->
              <div class="flex flex-wrap items-end gap-3 pt-2">
                <!-- Quantization Selector (GGUF only) -->
                {#if selectedCardQuantizations.length}
                  <div class="space-y-1">
                    <label class="text-xs text-muted-foreground" for="quant-select">
                      {$t('models.remote.quantization')}
                    </label>
                    <Select.Root
                      type="single"
                      value={selectedQuantizations[selectedCard.id] ??
                        selectedCardQuantizations[0] ??
                        ''}
                      onValueChange={(v) => {
                        if (selectedCard && v) setQuantization(selectedCard.id, v);
                      }}
                    >
                      <Select.Trigger class="w-[180px]" id="quant-select">
                        {selectedQuantizations[selectedCard.id] ?? selectedCardQuantizations[0]}
                      </Select.Trigger>
                      <Select.Content>
                        {#each selectedCardQuantizations as quant}
                          <Select.Item value={quant}>{quant}</Select.Item>
                        {/each}
                      </Select.Content>
                    </Select.Root>
                  </div>
                {/if}

                <!-- Download Buttons -->
                {#if selectedCard.has_gguf}
                  <div class="space-y-1">
                    <Button
                      variant="default"
                      size="sm"
                      onclick={() => handleDownload(selectedCard!, 'gguf')}
                      disabled={isDownloading(selectedCard!, 'gguf')}
                    >
                      {#if isDownloading(selectedCard, 'gguf')}
                        <Spinner class="size-4 mr-2" />
                      {:else}
                        <DownloadSimple class="size-4 mr-2" />
                      {/if}
                      GGUF
                    </Button>
                    {#if downloadErrors[getDownloadId(selectedCard!, 'gguf')]}
                      <p class="text-xs text-destructive">
                        {downloadErrors[getDownloadId(selectedCard!, 'gguf')]}
                      </p>
                    {/if}
                  </div>
                {/if}

              </div>

              <!-- Destination Folder -->
              <p class="text-xs text-muted-foreground pt-2">
                {#if $folderPath}
                  {$t('models.remote.modelsFolder')}
                  <code class="bg-muted px-1 py-0.5 rounded">{$folderPath}</code>
                {:else}
                  {$t('models.remote.folderNotSelected')}
                {/if}
              </p>
            </div>
          {:else}
            <div class="flex items-center justify-center h-full py-12">
              <p class="text-muted-foreground">{$t('models.remote.selectCard')}</p>
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</div>
