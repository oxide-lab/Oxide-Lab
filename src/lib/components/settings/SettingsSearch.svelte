<script lang="ts">
  import { onMount } from 'svelte';
  import * as Command from '$lib/components/ui/command';
  import { t } from '$lib/i18n';
  import MagnifyingGlass from 'phosphor-svelte/lib/MagnifyingGlass';
  import X from 'phosphor-svelte/lib/X';
  import type { SettingsSectionId } from '$lib/types/settings-v2';

  interface SearchResult {
    id: string;
    section: SettingsSectionId;
    title: string;
    description: string;
    hiddenByMode?: boolean;
  }

  interface Props {
    query: string;
    results: SearchResult[];
    onQueryChange: (value: string) => void;
    onSelect: (result: SearchResult) => void;
    class?: string;
  }

  let { query, results, onQueryChange, onSelect, class: className = '' }: Props = $props();
  let rootEl = $state<HTMLElement | null>(null);
  let dismissed = $state(false);

  const showDropdown = $derived(query.trim().length > 0 && !dismissed);

  function handleInput(value: string) {
    dismissed = false;
    onQueryChange(value);
  }

  function clearQuery() {
    dismissed = false;
    onQueryChange('');
  }

  function handleDocumentPointerDown(event: PointerEvent) {
    if (event.button !== 0) return;
    const target = event.target as Node | null;
    if (!rootEl || !target) return;
    if (rootEl.contains(target)) return;
    dismissed = true;
  }

  onMount(() => {
    document.addEventListener('pointerdown', handleDocumentPointerDown);
    return () => {
      document.removeEventListener('pointerdown', handleDocumentPointerDown);
    };
  });
</script>

<div class={className} bind:this={rootEl}>
  <Command.Root shouldFilter={false} class="relative overflow-visible rounded-lg border bg-card">
    <Command.Input
      value={query}
      placeholder={$t('settings.v2.search.placeholder')}
      class="h-10 pr-10"
      data-settings-search-input="true"
      onfocus={() => (dismissed = false)}
      oninput={(event) => handleInput((event.currentTarget as HTMLInputElement).value)}
    />
    {#if query.trim().length > 0}
      <button
        type="button"
        class="absolute top-1/2 right-2 z-[1310] -translate-y-1/2 rounded-sm p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
        aria-label={$t('settings.v2.search.clear') || 'Clear search'}
        onclick={clearQuery}
      >
        <X class="size-3.5" />
      </button>
    {/if}
    {#if showDropdown}
      <Command.List
        class="absolute left-0 right-0 top-[calc(100%+4px)] z-[1300] max-h-80 overflow-auto rounded-md border bg-popover shadow-xl shadow-black/20"
      >
        <Command.Empty>{$t('settings.v2.search.empty')}</Command.Empty>
        <Command.Group heading={$t('settings.v2.search.results')}>
          {#each results.slice(0, 8) as row (row.id)}
            <Command.Item
              value={`${row.title} ${row.description}`}
              onSelect={() => {
                dismissed = true;
                onSelect(row);
              }}
            >
              <div class="flex w-full items-center gap-2">
                <MagnifyingGlass class="size-4 text-muted-foreground" />
                <div class="min-w-0">
                  <div class="truncate text-sm">{row.title}</div>
                  <div class="truncate text-xs text-muted-foreground">{row.description}</div>
                </div>
                {#if row.hiddenByMode}
                  <span class="ml-auto text-[10px] text-amber-600">{$t('settings.v2.search.developer_only')}</span>
                {/if}
              </div>
            </Command.Item>
          {/each}
        </Command.Group>
      </Command.List>
    {/if}
  </Command.Root>
</div>
