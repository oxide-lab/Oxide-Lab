<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { t } from '$lib/i18n';
  import * as Dialog from '$lib/components/ui/dialog';
  import { Button } from '$lib/components/ui/button';
  import { Spinner } from '$lib/components/ui/spinner';
  import {
    SettingsShell,
    SettingsSidebar,
    SettingsSearch,
    SettingsSectionAbout,
    SettingsSectionChatPresets,
    SettingsSectionDeveloper,
    SettingsSectionGeneral,
    SettingsSectionModelsStorage,
    SettingsSectionPerformance,
    SettingsSectionPrivacyData,
  } from '$lib/components/settings';
  import { settingsV2Store } from '$lib/stores/settings-v2';
  import { settingsSearchStore } from '$lib/stores/settings-search';
  import {
    clearUserData,
    exportUserData,
    getDataLocations,
    restartOpenAiServer,
    sha256Base64NoPad,
  } from '$lib/services/settings-v2';
  import type {
    ClearDataScope,
    DataLocations,
    SettingsSectionId,
  } from '$lib/types/settings-v2';
  import SlidersHorizontal from 'phosphor-svelte/lib/SlidersHorizontal';
  import Cube from 'phosphor-svelte/lib/Cube';
  import Gauge from 'phosphor-svelte/lib/Gauge';
  import ChatsCircle from 'phosphor-svelte/lib/ChatsCircle';
  import Shield from 'phosphor-svelte/lib/Shield';
  import Code from 'phosphor-svelte/lib/Code';
  import Info from 'phosphor-svelte/lib/Info';

  const settings = settingsV2Store;
  const loading = settingsV2Store.loading;
  const settingsError = settingsV2Store.error;
  const warnings = settingsV2Store.warnings;
  const dirtyBySection = settingsV2Store.dirtyBySection;
  const openAiStatus = settingsV2Store.openAiStatus;

  const searchResults = settingsSearchStore.results;

  let activeSection = $state<SettingsSectionId>('general');
  let highlightedSettingId = $state<string | null>(null);
  let searchQuery = $state('');
  let dataLocations = $state<DataLocations | null>(null);
  let appVersion = $state('0.15.0');
  let transientNotice = $state<string | null>(null);
  let clearDialogOpen = $state(false);
  let pendingClearScope = $state<ClearDataScope | null>(null);
  let viewportWidth = $state(1024);
  let highlightResetHandle: ReturnType<typeof setTimeout> | null = null;

  const allSections = [
    { id: 'general', label: 'settings.v2.sections.general.title', icon: SlidersHorizontal },
    { id: 'models_storage', label: 'settings.v2.sections.models_storage.title', icon: Cube },
    { id: 'performance', label: 'settings.v2.sections.performance.title', icon: Gauge },
    { id: 'chat_presets', label: 'settings.v2.sections.chat_presets.title', icon: ChatsCircle },
    { id: 'privacy_data', label: 'settings.v2.sections.privacy_data.title', icon: Shield },
    { id: 'developer', label: 'settings.v2.sections.developer.title', icon: Code },
    { id: 'about', label: 'settings.v2.sections.about.title', icon: Info },
  ] as const;

  const visibleSections = allSections;

  function syncSettingsQuery(section: SettingsSectionId, settingId?: string | null) {
    if (page.url.pathname !== '/settings') return;
    const nextParams = new URLSearchParams(page.url.searchParams);
    nextParams.set('section', section);
    if (settingId) {
      nextParams.set('setting', settingId);
    } else {
      nextParams.delete('setting');
    }

    const current = page.url.searchParams.toString();
    const next = nextParams.toString();
    if (current === next) return;

    void goto(`/settings?${next}`, {
      replaceState: true,
      noScroll: true,
      keepFocus: true,
    });
  }

  function focusSearchInput() {
    const node = document.querySelector<HTMLInputElement>('[data-settings-search-input="true"]');
    node?.focus();
    node?.select();
  }

  function jumpToSetting(section: SettingsSectionId, settingId?: string | null) {
    activeSection = section;
    highlightedSettingId = settingId ?? null;
    syncSettingsQuery(section, settingId);
    scheduleHighlightReset(settingId ?? null);
    tick().then(() => {
      if (!settingId) return;
      const node = document.getElementById(settingId);
      node?.scrollIntoView({ behavior: 'smooth', block: 'center' });
    });
  }

  function scheduleHighlightReset(settingId: string | null) {
    if (highlightResetHandle) {
      clearTimeout(highlightResetHandle);
      highlightResetHandle = null;
    }
    if (!settingId) return;
    highlightResetHandle = setTimeout(() => {
      highlightedSettingId = null;
      syncSettingsQuery(activeSection, null);
      highlightResetHandle = null;
    }, 1800);
  }

  async function loadDataLocations() {
    try {
      dataLocations = await getDataLocations();
    } catch (e) {
      console.warn('Failed to load data locations', e);
      dataLocations = null;
    }
  }

  async function loadAppVersion() {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const info = await invoke<{ version: string }>('get_app_info');
      appVersion = info.version;
    } catch {
      appVersion = '0.15.0';
    }
  }

  async function addApiKey(raw: string) {
    const snapshot = settings.getSnapshot();
    if (!snapshot) return;
    const hashed = await sha256Base64NoPad(raw);
    if (snapshot.developer.openai_server.api_keys_hashed.includes(hashed)) {
      return;
    }
    await settings.updateOpenAiConfig({
      ...snapshot.developer.openai_server,
      api_keys_hashed: [...snapshot.developer.openai_server.api_keys_hashed, hashed],
    });
  }

  async function handleExportData() {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const selected = await open({
      directory: true,
      multiple: false,
      title: $t('settings.v2.privacy_data.export_dialog_title'),
    });
    const path = Array.isArray(selected) ? selected[0] : selected;
    if (typeof path === 'string' && path.length > 0) {
      await exportUserData(path);
    }
  }

  function handleClearData(scope: ClearDataScope) {
    pendingClearScope = scope;
    clearDialogOpen = true;
  }

  async function confirmClearData() {
    if (!pendingClearScope) return;
    await clearUserData(pendingClearScope);
    await settings.load();
    await loadDataLocations();
    clearDialogOpen = false;
    pendingClearScope = null;
  }

  function handleSearchSelect(result: {
    id: string;
    section: SettingsSectionId;
    title: string;
    description: string;
  }) {
    jumpToSetting(result.section, result.id);
    const snapshot = settings.getSnapshot();
    settingsSearchStore.pushHistory(result.title, snapshot?.general.search_history_enabled === true);
    settingsSearchStore.clear();
    searchQuery = '';
  }

  function onKeyDown(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'f') {
      event.preventDefault();
      focusSearchInput();
      return;
    }

    if (event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) return;
    if (event.key.length !== 1) return;
    const target = event.target as HTMLElement | null;
    if (target && ['INPUT', 'TEXTAREA'].includes(target.tagName)) return;
    focusSearchInput();
  }

  function handleViewportResize() {
    viewportWidth = window.innerWidth;
  }

  $effect(() => {
    settingsSearchStore.setQuery(searchQuery);
  });

  $effect(() => {
    if ($settings && !$settings.general.search_history_enabled) {
      settingsSearchStore.clearHistory();
    }
  });

  $effect(() => {
    if (page.url.pathname !== '/settings') return;
    const section = page.url.searchParams.get('section') as SettingsSectionId | null;
    const setting = page.url.searchParams.get('setting');
    if (!section || !allSections.some((item) => item.id === section)) return;

    const nextSettingId = setting ?? null;
    if (activeSection === section && highlightedSettingId === nextSettingId) return;

    activeSection = section;
    highlightedSettingId = nextSettingId;
    scheduleHighlightReset(nextSettingId);
    if (!nextSettingId) return;
    tick().then(() => {
      const node = document.getElementById(nextSettingId);
      node?.scrollIntoView({ behavior: 'smooth', block: 'center' });
    });
  });

  onMount(() => {
    const run = async () => {
      await settings.load();
      await Promise.all([loadDataLocations(), loadAppVersion()]);

      const section = page.url.searchParams.get('section') as SettingsSectionId | null;
      const setting = page.url.searchParams.get('setting');
      if (section && allSections.some((item) => item.id === section)) {
        jumpToSetting(section, setting);
      }
    };
    void run();

    handleViewportResize();
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('resize', handleViewportResize);
    return () => {
      if (highlightResetHandle) {
        clearTimeout(highlightResetHandle);
        highlightResetHandle = null;
      }
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('resize', handleViewportResize);
    };
  });
</script>

<div class="h-full overflow-y-hidden overflow-x-hidden py-4" style="padding-inline: clamp(16px, 1.6vw, 18px);">
  <div class="mx-auto h-full w-full max-w-[1024px]">
    <SettingsShell showSidebar={viewportWidth >= 1024}>
      {#snippet header()}
        <div class="space-y-3">
          <SettingsSearch
            query={searchQuery}
            results={$searchResults}
            onQueryChange={(value) => (searchQuery = value)}
            onSelect={handleSearchSelect}
          />

          {#if $warnings.length > 0}
            <div class="rounded-md border border-amber-500/40 bg-amber-500/10 p-2 text-xs text-amber-700">
              {#each $warnings as warning (warning)}
                <p>{warning}</p>
              {/each}
            </div>
          {/if}
          {#if transientNotice}
            <div class="rounded-md border border-amber-500/40 bg-amber-500/10 p-2 text-xs text-amber-700">
              <p>{transientNotice}</p>
            </div>
          {/if}
        </div>
      {/snippet}

      {#snippet sidebar()}
        {#if viewportWidth >= 1024}
          <SettingsSidebar
            sections={visibleSections.map((section) => ({ ...section, label: $t(section.label) })) as any}
            activeSection={activeSection}
            dirtyCounts={$dirtyBySection}
            onSelect={(id) => jumpToSetting(id)}
          />
        {/if}
      {/snippet}

      {#snippet content()}
        <div class="space-y-3 pr-1">
          {#if $loading}
            <div class="flex h-40 items-center justify-center">
              <Spinner class="size-6" />
            </div>
          {:else if $settingsError}
            <div class="rounded-md border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">
              {$settingsError}
            </div>
          {:else if $settings}
            {#if activeSection === 'general'}
              <SettingsSectionGeneral
                value={$settings.general}
                highlightedSettingId={highlightedSettingId}
                onChange={(next) => settings.updateSection('general', next)}
              />
            {/if}

            {#if activeSection === 'models_storage'}
              <SettingsSectionModelsStorage
                value={$settings.models_storage}
                highlightedSettingId={highlightedSettingId}
                onChange={(next) => settings.updateSection('models_storage', next)}
              />
            {/if}

            {#if activeSection === 'performance'}
              <SettingsSectionPerformance
                value={$settings.performance}
                highlightedSettingId={highlightedSettingId}
                expertMode={true}
                onChange={(next) => settings.updateSection('performance', next)}
              />
            {/if}

            {#if activeSection === 'chat_presets'}
              <SettingsSectionChatPresets
                value={$settings.chat_presets}
                highlightedSettingId={highlightedSettingId}
                onChange={(next) => settings.updateSection('chat_presets', next)}
                onApplyPresetToChat={(presetId) => {
                  localStorage.setItem('chat.quickPreset', presetId);
                  const oxide = (window as any).__oxide;
                  if (oxide?.applyPresetById) {
                    oxide.applyPresetById(presetId, 'settings');
                  } else {
                    transientNotice = $t('settings.v2.chat_presets.apply_later_notice');
                  }
                }}
              />
            {/if}

            {#if activeSection === 'privacy_data'}
              <SettingsSectionPrivacyData
                value={$settings.privacy_data}
                locations={dataLocations}
                highlightedSettingId={highlightedSettingId}
                onChange={(next) => settings.updateSection('privacy_data', next)}
                onExportData={handleExportData}
                onClearData={handleClearData}
              />
            {/if}

            {#if activeSection === 'developer'}
              <SettingsSectionDeveloper
                value={$settings.developer.openai_server}
                status={$openAiStatus}
                highlightedSettingId={highlightedSettingId}
                onChange={(next) => settings.updateOpenAiConfig(next)}
                onAddApiKey={addApiKey}
                onRestart={() => void restartOpenAiServer()}
              />
            {/if}

            {#if activeSection === 'about'}
              <SettingsSectionAbout appVersion={appVersion} />
            {/if}
          {/if}
        </div>
      {/snippet}
    </SettingsShell>
  </div>

  <Dialog.Root bind:open={clearDialogOpen}>
    <Dialog.Content class="sm:max-w-md">
      <Dialog.Header>
        <Dialog.Title>{$t('settings.v2.privacy_data.clear_title')}</Dialog.Title>
        <Dialog.Description>
          {$t('settings.v2.privacy_data.clear_confirm', { scope: pendingClearScope ?? 'all' })}
        </Dialog.Description>
      </Dialog.Header>
      <Dialog.Footer>
        <Button variant="outline" onclick={() => (clearDialogOpen = false)}>
          {$t('settings.v2.common.cancel')}
        </Button>
        <Button variant="destructive" onclick={confirmClearData}>
          {$t('settings.v2.common.confirm')}
        </Button>
      </Dialog.Footer>
    </Dialog.Content>
  </Dialog.Root>
</div>
