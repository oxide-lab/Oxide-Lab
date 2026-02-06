<script lang="ts">
  import { onMount, tick } from 'svelte';
  import * as Card from '$lib/components/ui/card';
  import { Button } from '$lib/components/ui/button';
  import { Label } from '$lib/components/ui/label';
  import { Badge } from '$lib/components/ui/badge';
  import { Spinner } from '$lib/components/ui/spinner';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import Gear from 'phosphor-svelte/lib/Gear';
  import Globe from 'phosphor-svelte/lib/Globe';
  import Flask from 'phosphor-svelte/lib/Flask';
  import Info from 'phosphor-svelte/lib/Info';
  import Check from 'phosphor-svelte/lib/Check';
  import MagnifyingGlass from 'phosphor-svelte/lib/MagnifyingGlass';
  import ChartBar from 'phosphor-svelte/lib/ChartBar';
  import Lightning from 'phosphor-svelte/lib/Lightning';
  import Warning from 'phosphor-svelte/lib/Warning';
  import { t, locale, setLocale, loadTranslations, type SupportedLocale } from '$lib/i18n';
  import { experimentalFeatures } from '$lib/stores/experimental-features.svelte';
  import { modelSelectorSearchEnabled } from '$lib/stores/ui-preferences';
  import PerformanceMonitor from '$lib/components/PerformanceMonitor.svelte';

  const hardwareConcurrency = typeof navigator !== 'undefined' ? navigator.hardwareConcurrency || 4 : 4;
  let threadLimit = $state<number | null>(null);
  let threadSliderValue = $state(hardwareConcurrency);
  let threadLimitLoading = $state(true);
  let threadLimitError = $state<string | null>(null);

  let experimentalEnabled = $state(false);
  let modelSearchEnabled = $state(true);

  let prefixCacheEnabled = $state(true);
  let prefixCacheMaxEntries = $state(32);
  let prefixCacheLoading = $state(true);
  let prefixCacheStats = $state({ hits: 0, misses: 0, entries: 0 });

  const languages: { value: SupportedLocale; label: string }[] = [
    { value: 'en', label: 'English' },
    { value: 'ru', label: 'Русский' },
    { value: 'pt-BR', label: 'Português (Brasil)' },
  ];

  async function loadThreadLimit() {
    threadLimitLoading = true;
    threadLimitError = null;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const saved = await invoke<number | null>('get_rayon_thread_limit');
      threadLimit = saved;
      threadSliderValue = saved ?? hardwareConcurrency;
    } catch (err) {
      threadLimitError = `Failed to load thread limit: ${err}`;
      console.error(err);
    } finally {
      threadLimitLoading = false;
    }
  }

  async function applyThreadLimit(limit: number | null) {
    threadLimitLoading = true;
    threadLimitError = null;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('set_rayon_thread_limit', { limit });
      threadLimit = limit;
      threadSliderValue = limit ?? hardwareConcurrency;
    } catch (err) {
      threadLimitError = `Failed to save thread limit: ${err}`;
      console.error(err);
    } finally {
      threadLimitLoading = false;
    }
  }

  async function handleExperimentalToggle(enabled: boolean) {
    try {
      await experimentalFeatures.setEnabled(enabled);
      experimentalEnabled = enabled;
      await tick();
    } catch (err) {
      console.error('Failed to toggle experimental features:', err);
      experimentalEnabled = experimentalFeatures.enabled;
    }
  }

  function handleModelSearchToggle(enabled: boolean) {
    modelSelectorSearchEnabled.set(enabled);
  }

  async function handleLanguageChange(lang: SupportedLocale) {
    await setLocale(lang);
    await loadTranslations(lang);
  }

  async function loadPrefixCacheInfo() {
    prefixCacheLoading = true;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const info = await invoke<{ enabled: boolean; max_entries: number; stats: { hits: number; misses: number; entries: number } }>('get_prefix_cache_info');
      prefixCacheEnabled = info.enabled;
      prefixCacheMaxEntries = info.max_entries || 32;
      prefixCacheStats = info.stats;
    } catch (err) {
      console.error('Failed to load prefix cache info:', err);
    } finally {
      prefixCacheLoading = false;
    }
  }

  async function handlePrefixCacheToggle(enabled: boolean) {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('set_prefix_cache_enabled', { enabled, maxEntries: prefixCacheMaxEntries });
      prefixCacheEnabled = enabled;
    } catch (err) {
      console.error('Failed to toggle prefix cache:', err);
    }
  }

  async function handlePrefixCacheMaxEntriesChange(maxEntries: number) {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('set_prefix_cache_enabled', { enabled: prefixCacheEnabled, maxEntries });
      prefixCacheMaxEntries = maxEntries;
    } catch (err) {
      console.error('Failed to update prefix cache max entries:', err);
    }
  }

  async function handleClearPrefixCache() {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('clear_prefix_cache');
      await loadPrefixCacheInfo();
    } catch (err) {
      console.error('Failed to clear prefix cache:', err);
    }
  }

  onMount(async () => {
    await Promise.all([
      loadThreadLimit(),
      loadPrefixCacheInfo(),
    ]);
  });

  $effect(() => {
    if (experimentalFeatures.initialized) {
      experimentalEnabled = experimentalFeatures.enabled;
    }
  });

  $effect(() => {
    const unsubscribe = modelSelectorSearchEnabled.subscribe((value) => {
      modelSearchEnabled = value;
    });
    return unsubscribe;
  });
</script>

<div class="h-full overflow-auto p-3 sm:p-4 lg:p-6 custom-scrollbar">
  <div class="max-w-xl sm:max-w-2xl lg:max-w-3xl mx-auto space-y-4 sm:space-y-6">
    <h1 class="text-xl sm:text-2xl font-bold">{$t('settings.title')}</h1>

    <Card.Root>
      <Card.Header>
        <Card.Title class="flex items-center gap-2">
          <Gear class="size-5" />
          {$t('settings.threads.title') || 'Thread Limit'}
        </Card.Title>
        <Card.Description>{$t('settings.threads.description') || 'Control CPU thread usage'}</Card.Description>
      </Card.Header>
      <Card.Content>
        {#if threadLimitLoading}
          <div class="flex justify-center py-4"><Spinner class="size-6" /></div>
        {:else}
          <div class="space-y-4">
            <div class="flex items-center justify-between">
              <Label>{$t('settings.threads.maxThreads') || 'Max threads'}: {threadSliderValue}</Label>
              <Badge variant="outline">{$t('settings.threads.available') || 'Available'}: {hardwareConcurrency}</Badge>
            </div>
            <input
              type="range"
              min="1"
              max={hardwareConcurrency}
              bind:value={threadSliderValue}
              onchange={(e) => applyThreadLimit(parseInt((e.target as HTMLInputElement).value))}
              class="w-full accent-primary"
            />
            <div class="flex items-center justify-between text-sm">
              <Button variant="ghost" size="sm" disabled={threadLimit === null} onclick={() => applyThreadLimit(null)}>
                {$t('settings.threads.useSystem') || 'Use system default'}
              </Button>
              <span class="text-muted-foreground">
                {threadLimit === null ? $t('settings.threads.automatic') || 'Automatic' : $t('settings.threads.manual') || 'Manual'}
              </span>
            </div>
          </div>
        {/if}
        {#if threadLimitError}
          <div class="mt-3 text-sm text-destructive flex items-center gap-2">
            <Warning class="size-4" />{threadLimitError}
          </div>
        {/if}
      </Card.Content>
    </Card.Root>

    <Card.Root>
      <Card.Header>
        <Card.Title class="flex items-center gap-2">
          <MagnifyingGlass class="size-5" />
          {$t('settings.modelSelector.title') || 'Model Selector'}
        </Card.Title>
        <Card.Description>{$t('settings.modelSelector.description') || 'Configure model picker behavior'}</Card.Description>
      </Card.Header>
      <Card.Content>
        <label class="flex items-center gap-3 cursor-pointer">
          <Checkbox checked={modelSearchEnabled} onCheckedChange={(checked: boolean) => handleModelSearchToggle(checked)} />
          <span>{$t('settings.modelSelector.enableSearch') || 'Enable search in model picker'}</span>
        </label>
      </Card.Content>
    </Card.Root>

    <Card.Root>
      <Card.Header>
        <Card.Title class="flex items-center gap-2">
          <Lightning class="size-5" />
          {$t('settings.prefixCache.title') || 'Prefix Cache'}
        </Card.Title>
        <Card.Description>{$t('settings.prefixCache.description') || 'Reuse KV cache for faster multi-turn conversations'}</Card.Description>
      </Card.Header>
      <Card.Content class="space-y-4">
        {#if prefixCacheLoading}
          <div class="flex justify-center py-4"><Spinner class="size-6" /></div>
        {:else}
          <label class="flex items-center gap-3 cursor-pointer">
            <Checkbox checked={prefixCacheEnabled} onCheckedChange={(checked: boolean) => handlePrefixCacheToggle(checked)} />
            <span>{$t('settings.prefixCache.enable') || 'Enable prefix caching'}</span>
          </label>

          {#if prefixCacheEnabled}
            <div class="space-y-2">
              <Label>{$t('settings.prefixCache.maxEntries') || 'Max cache entries'}: {prefixCacheMaxEntries}</Label>
              <input
                type="range"
                min="8"
                max="128"
                step="8"
                bind:value={prefixCacheMaxEntries}
                onchange={() => handlePrefixCacheMaxEntriesChange(prefixCacheMaxEntries)}
                class="w-full accent-primary"
              />
            </div>

            <div class="flex items-center justify-between p-3 rounded bg-muted/30">
              <div class="text-sm space-y-1">
                <div>{$t('settings.prefixCache.stats.hits') || 'Hits'}: <span class="font-medium text-green-600">{prefixCacheStats.hits}</span></div>
                <div>{$t('settings.prefixCache.stats.misses') || 'Misses'}: <span class="font-medium text-orange-600">{prefixCacheStats.misses}</span></div>
                <div>{$t('settings.prefixCache.stats.entries') || 'Cached entries'}: <span class="font-medium">{prefixCacheStats.entries}</span></div>
              </div>
              <Button variant="outline" size="sm" onclick={handleClearPrefixCache}>
                {$t('settings.prefixCache.clear') || 'Clear cache'}
              </Button>
            </div>
          {/if}
        {/if}
      </Card.Content>
    </Card.Root>

    <Card.Root>
      <Card.Header>
        <Card.Title class="flex items-center gap-2">
          <Flask class="size-5" />
          {$t('settings.experimental.title') || 'Experimental Features'}
        </Card.Title>
      </Card.Header>
      <Card.Content>
        <label class="flex items-center gap-3 cursor-pointer">
          <Checkbox checked={experimentalEnabled} onCheckedChange={(checked: boolean) => handleExperimentalToggle(checked)} />
          <span>{$t('settings.experimental.enable') || 'Enable experimental features'}</span>
        </label>
        <div class="mt-4 flex items-start gap-2 text-sm text-muted-foreground">
          <Info class="size-4 mt-0.5 flex-shrink-0" />
          <p>{$t('settings.experimental.warning') || 'These features may be unstable'}</p>
        </div>
      </Card.Content>
    </Card.Root>

    <Card.Root>
      <Card.Header>
        <Card.Title class="flex items-center gap-2">
          <ChartBar class="size-5" />
          {$t('settings.performance.title') || 'Performance'}
        </Card.Title>
      </Card.Header>
      <Card.Content>
        <PerformanceMonitor />
      </Card.Content>
    </Card.Root>

    <Card.Root>
      <Card.Header>
        <Card.Title class="flex items-center gap-2">
          <Globe class="size-5" />
          {$t('settings.language.title') || 'Language'}
        </Card.Title>
      </Card.Header>
      <Card.Content>
        <div class="flex gap-2 flex-wrap">
          {#each languages as lang}
            <Button variant={$locale === lang.value ? 'default' : 'outline'} size="sm" class="gap-2" onclick={() => handleLanguageChange(lang.value)}>
              {#if $locale === lang.value}
                <Check class="size-4" />
              {/if}
              {lang.label}
            </Button>
          {/each}
        </div>
      </Card.Content>
    </Card.Root>

    <Card.Root>
      <Card.Header>
        <Card.Title class="flex items-center gap-2">
          <Info class="size-5" />
          {$t('about.title') || 'About'}
        </Card.Title>
      </Card.Header>
      <Card.Content>
        <div class="space-y-2 text-sm">
          <div class="flex justify-between">
            <span class="text-muted-foreground">{$t('about.version') || 'Version'}</span>
            <span>0.13.1</span>
          </div>
          <div class="flex justify-between">
            <span class="text-muted-foreground">{$t('about.license') || 'License'}</span>
            <span>Apache-2.0</span>
          </div>
        </div>
      </Card.Content>
    </Card.Root>
  </div>
</div>
