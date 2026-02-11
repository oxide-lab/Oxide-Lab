<script lang="ts">
  /**
   * Root Layout
   *
   * Main application shell with sidebar, header with model picker, and window controls.
   */
  import '../app.css';
  import 'highlight.js/styles/github-dark.css';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { onMount, tick } from 'svelte';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { derived } from 'svelte/store';

  // Phosphor Icons
  import Minus from 'phosphor-svelte/lib/Minus';
  import ArrowsIn from 'phosphor-svelte/lib/ArrowsIn';
  import ArrowsOut from 'phosphor-svelte/lib/ArrowsOut';
  import X from 'phosphor-svelte/lib/X';
  import CaretDown from 'phosphor-svelte/lib/CaretDown';
  import Check from 'phosphor-svelte/lib/CheckCircle';
  import GithubLogo from 'phosphor-svelte/lib/GithubLogo';
  import Repeat from 'phosphor-svelte/lib/Repeat';
  import UploadSimple from 'phosphor-svelte/lib/UploadSimple';
  import SlidersHorizontal from 'phosphor-svelte/lib/SlidersHorizontal';
  import Cube from 'phosphor-svelte/lib/Cube';
  import Gauge from 'phosphor-svelte/lib/Gauge';
  import Cpu from 'phosphor-svelte/lib/Cpu';
  import ChatsCircle from 'phosphor-svelte/lib/ChatsCircle';
  import Globe from 'phosphor-svelte/lib/Globe';
  import Shield from 'phosphor-svelte/lib/Shield';
  import Code from 'phosphor-svelte/lib/Code';
  import Info from 'phosphor-svelte/lib/Info';

  // UI Components
  import AppSidebar from '$lib/components/app-sidebar.svelte';
  import * as SidebarUI from '$lib/components/ui/sidebar/index';
  import * as Breadcrumb from '$lib/components/ui/breadcrumb';
  import { Button } from '$lib/components/ui/button';
  import { Spinner } from '$lib/components/ui/spinner';
  import * as Popover from '$lib/components/ui/popover';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
  import * as Command from '$lib/components/ui/command';
  import * as Tabs from '$lib/components/ui/tabs';
  import DownloadManagerModal from '$lib/components/DownloadManagerModal.svelte';
  import AppUpdaterPrompt from '$lib/components/updater/AppUpdaterPrompt.svelte';

  // Core
  import { cn } from '../lib/utils';
  import { t, locale, initI18n } from '$lib/i18n';
  import { experimentalFeatures } from '$lib/stores/experimental-features.svelte';
  import { pageTabsList, activePageTab } from '$lib/stores/page-tabs.svelte';
  import type { TabId } from '$lib/stores/page-tabs.svelte';
  import { chatState } from '$lib/stores/chat';
  import {
    folderPath,
    initLoadedModels,
    loadedModelIds,
    models,
    scanFolder,
  } from '$lib/stores/local-models';
  import {
    areModelPathsEqual,
    isModelPathLoaded,
    normalizeModelIdentifier,
  } from '$lib/model-manager/model-identity';
  import type { ModelInfo } from '$lib/types/local-models';
  import { settingsV2Store } from '$lib/stores/settings-v2';
  import { settingsSearchStore } from '$lib/stores/settings-search';
  import { appUpdaterStore, getUpdateCheckIntervalMs } from '$lib/stores/app-updater';
  import type { SettingsSectionId } from '$lib/types/settings-v2';

  // Pages for mount-all pattern
  import Chat from '$lib/chat/Chat.svelte';

  // Guard for translations — avoid calling $t before locale is initialized
  let currentLocale = $derived($locale);

  const { children } = $props();

  let isMaximized = $state(false);
  let isSidebarOpen = $state(false);
  let isModelPickerOpen = $state(false);
  let comboboxTrigger = $state<HTMLButtonElement | null>(null);
  let showDownloadManager = $state(false);
  let showAbout = $state(false);
  let showCommandPalette = $state(false);
  let appVersion = $state('0.15.0');
  let modalElement = $state<HTMLDivElement | null>(null);
  let viewportWidth = $state(1024);
  let isUnloadActionRunning = $state(false);

  const appWindow = getCurrentWindow();

  // Derived stores for model picker
  const quickModels = derived(models, ($models) =>
    $models.filter((model: ModelInfo) =>
      Boolean(model.source_repo_name?.trim() || model.name?.trim()),
    ),
  );
  const currentModelPath = derived(chatState, ($chatState) => $chatState.modelPath);
  const pendingModelPath = derived(chatState, ($chatState) => $chatState.pendingModelPath);
  const isModelLoading = derived(chatState, ($chatState) => $chatState.isLoadingModel);
  const modelLoadingStage = derived(chatState, ($chatState) => $chatState.loadingStage);
  const isCurrentModelLoaded = derived(
    [chatState, loadedModelIds, currentModelPath],
    ([$chatState, $loadedModelIds, $currentModelPath]) =>
      Boolean($chatState.isLoaded || isModelPathLoaded($currentModelPath, $loadedModelIds)),
  );
  const selectedModelPath = derived(
    [pendingModelPath, currentModelPath],
    ([$pending, $current]) => $pending || $current,
  );
  const selectedModel = derived(
    [quickModels, selectedModelPath],
    ([$quickModels, $selectedModelPath]) =>
      $quickModels.find((model: ModelInfo) => areModelPathsEqual(model.path, $selectedModelPath)),
  );
  const currentDisplayName = derived(selectedModel, ($selectedModel) =>
    formatModelLabel($selectedModel),
  );
  const isReloadAvailable = derived(
    [pendingModelPath, currentModelPath, isCurrentModelLoaded],
    ([$pending, $current, $isCurrentModelLoaded]) =>
      Boolean($isCurrentModelLoaded && $pending && $pending !== $current),
  );
  const canUnloadCurrentModel = derived(chatState, ($chatState) =>
    Boolean(
      $chatState.isLoaded &&
        !$chatState.busy &&
        !$chatState.isLoadingModel &&
        !$chatState.isUnloadingModel,
    ),
  );
  const settingsSnapshot = settingsV2Store;

  const settingsSectionLabelMap: Record<SettingsSectionId, string> = {
    general: 'settings.v2.sections.general.title',
    models_storage: 'settings.v2.sections.models_storage.title',
    performance: 'settings.v2.sections.performance.title',
    hardware: 'settings.v2.sections.hardware.title',
    chat_presets: 'settings.v2.sections.chat_presets.title',
    web_rag: 'settings.v2.sections.web_rag.title',
    privacy_data: 'settings.v2.sections.privacy_data.title',
    developer: 'settings.v2.sections.developer.title',
    about: 'settings.v2.sections.about.title',
  };
  const settingsSectionOrder: SettingsSectionId[] = [
    'general',
    'models_storage',
    'chat_presets',
    'web_rag',
    'privacy_data',
    'developer',
    'about',
    'performance',
    'hardware',
  ];
  const settingsSectionIconMap: Record<SettingsSectionId, any> = {
    general: SlidersHorizontal,
    models_storage: Cube,
    performance: Gauge,
    hardware: Cpu,
    chat_presets: ChatsCircle,
    web_rag: Globe,
    privacy_data: Shield,
    developer: Code,
    about: Info,
  };
  const modelsTabIconMap: Record<string, any> = {
    local: Cube,
    remote: Globe,
    recommendations: ChatsCircle,
  };

  // Load local models on demand to keep startup responsive.
  $effect(() => {
    if (!isModelPickerOpen || !$folderPath || $models.length > 0) return;
    void scanFolder($folderPath).catch((err) => console.warn('Failed to scan local models', err));
  });

  $effect(() => {
    if (page.url.pathname !== '/settings') return;
    if ($settingsSnapshot) return;
    void settingsV2Store.load();
  });

  // Redirect experimental pages when features disabled
  $effect(() => {
    if (experimentalFeatures.initialized && !experimentalFeatures.enabled) {
      // TODO: Add experimental routes when implemented (e.g., '/api', '/performance')
      const experimentalPaths: string[] = [];
      if (experimentalPaths.includes(page.url.pathname)) {
        goto('/');
      }
    }
  });

  function currentHeaderTabValue(): TabId | '' {
    return $activePageTab || $pageTabsList[0]?.id || '';
  }

  function handleHeaderTabsChange(nextValue: string) {
    if (!nextValue || nextValue === $activePageTab) return;
    activePageTab.set(nextValue as TabId);
  }

  function getCurrentModelsTab() {
    const currentId = currentHeaderTabValue();
    return $pageTabsList.find((tab) => tab.id === currentId) ?? $pageTabsList[0] ?? null;
  }

  function formatModelLabel(model: ModelInfo | null | undefined) {
    if (!model)
      return currentLocale ? $t('common.model.selectModel') || 'Select model' : 'Select model';
    const publisher = model.metadata?.author ?? model.source_repo_id?.split('/')[0] ?? 'local';
    const title = model.name ?? model.source_repo_name ?? 'Unnamed';
    return `${publisher}/${title}`;
  }

  function getSettingsSectionFromQuery(): SettingsSectionId {
    const section = page.url.searchParams.get('section') as SettingsSectionId | null;
    if (!section || !(section in settingsSectionLabelMap)) return 'general';
    return section;
  }

  function handleReloadModel() {
    // TODO: Integrate with backend - this exposes the reload function
    const ox = (window as any).__oxide;
    if (ox?.reloadSelectedModel) {
      ox.reloadSelectedModel();
    }
  }

  async function handleUnloadAndClearCache() {
    if (isUnloadActionRunning || !$canUnloadCurrentModel) return;
    const ox = (window as any).__oxide;
    if (!ox?.unloadGGUF) {
      console.warn('Model unloading is not ready yet');
      return;
    }

    isUnloadActionRunning = true;
    try {
      await ox.unloadGGUF();
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('clear_prefix_cache');
    } catch (error) {
      console.warn('Failed to unload model and clear cache:', error);
    } finally {
      isUnloadActionRunning = false;
    }
  }

  function closeModelPicker() {
    isModelPickerOpen = false;
    void tick().then(() => comboboxTrigger?.focus());
  }

  function handleSelectModel(model: ModelInfo) {
    // TODO: Integrate with backend
    const ox = (window as any).__oxide;
    if (!ox?.loadModelFromManager) {
      console.warn('Model loading not ready yet');
      closeModelPicker();
      return;
    }
    ox.loadModelFromManager({
      path: model.path,
      format: 'gguf',
    });
    closeModelPicker();
  }

  async function toggleMaximize() {
    if (await appWindow.isMaximized()) {
      await appWindow.unmaximize();
      isMaximized = false;
    } else {
      await appWindow.maximize();
      isMaximized = true;
    }
  }

  function toggleAbout() {
    showAbout = !showAbout;
    if (showAbout) {
      void loadAppVersion();
    }
  }

  function openSettingsShortcut() {
    void goto('/settings');
    showCommandPalette = false;
  }

  function openSettingsSectionShortcut(section: string, settingId?: string) {
    const params = new URLSearchParams({ section });
    if (settingId) params.set('setting', settingId);
    void goto(`/settings?${params.toString()}`);
    showCommandPalette = false;
  }

  function openModelsShortcut() {
    void goto('/models');
    showCommandPalette = false;
  }

  function openApiShortcut() {
    void goto('/api');
    showCommandPalette = false;
  }

  function handleGlobalShortcut(event: KeyboardEvent) {
    const isMeta = event.metaKey || event.ctrlKey;
    if (!isMeta) return;
    const key = event.key.toLowerCase();
    if (key === ',') {
      event.preventDefault();
      openSettingsShortcut();
      return;
    }
    if (key === 'k') {
      event.preventDefault();
      showCommandPalette = !showCommandPalette;
    }
  }

  function handleViewportResize() {
    viewportWidth = window.innerWidth;
  }

  function handleBackdropKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' || event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      toggleAbout();
    }
  }

  async function loadAppVersion() {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const appInfo = (await invoke('get_app_info')) as { version: string };
      appVersion = appInfo.version;
    } catch (error) {
      console.warn('Failed to get app version:', error);
      appVersion = '0.15.0';
    }
  }

  // Focus trap for About modal
  $effect(() => {
    if (!showAbout || !modalElement) return;

    const node = modalElement;
    const focusableElements = Array.from(
      node.querySelectorAll<HTMLElement>(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
      ),
    );
    const firstElement = focusableElements[0];
    const lastElement = focusableElements[focusableElements.length - 1];

    const handleKeydown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        toggleAbout();
        return;
      }

      if (event.key !== 'Tab' || focusableElements.length === 0) return;

      const activeElement = document.activeElement as HTMLElement | null;

      if (event.shiftKey) {
        if (activeElement === firstElement) {
          event.preventDefault();
          lastElement?.focus();
        }
      } else if (activeElement === lastElement) {
        event.preventDefault();
        firstElement?.focus();
      }
    };

    node.addEventListener('keydown', handleKeydown);
    firstElement?.focus() ?? node.focus();

    return () => node.removeEventListener('keydown', handleKeydown);
  });

  async function startDragging(event: MouseEvent) {
    const target = event.target as HTMLElement;
    if (target.closest('button, input, textarea, select, a, [data-no-drag]')) {
      event.stopPropagation();
      return;
    }
    await appWindow.startDragging();
  }

  let unlistenFn: UnlistenFn | null = null;
  let unlistenLoadedModels: (() => void) | null = null;
  let updaterCheckInterval: ReturnType<typeof setInterval> | null = null;

  onMount(async () => {
    // Initialize i18n
    initI18n(page.url.pathname);

    // Load experimental features
    void experimentalFeatures.loadState();
    unlistenLoadedModels = await initLoadedModels();

    // Initialize backend connections (download manager, model cards, performance listeners)
    const { initializeBackend } = await import('$lib/services/backend');
    void initializeBackend().catch((err) => console.warn('Backend initialization failed:', err));

    if (!import.meta.env.DEV) {
      void appUpdaterStore.checkForUpdate({
        userInitiated: false,
        suppressErrors: true,
      });

      updaterCheckInterval = setInterval(() => {
        void appUpdaterStore.checkForUpdate({
          userInitiated: false,
          suppressErrors: true,
        });
      }, getUpdateCheckIntervalMs());
    }

    // Setup window state
    isMaximized = await appWindow.isMaximized();
    handleViewportResize();
    unlistenFn = await appWindow.onResized(() => {
      appWindow.isMaximized().then((maximized: boolean) => {
        isMaximized = maximized;
      });
      handleViewportResize();
    });

    const { listen } = await import('@tauri-apps/api/event');
    const { toast, Toaster } = await import('svelte-sonner');

    const unlistenUnload = await listen<string>('model_unloaded', (event) => {
      console.log('Model unloaded automatically:', event.payload);
      const unloadedId = normalizeModelIdentifier(event.payload);
      if (unloadedId) {
        loadedModelIds.update((ids) =>
          ids.filter((id) => normalizeModelIdentifier(id) !== unloadedId),
        );
      }

      chatState.update((s) => ({
        ...s,
        isLoaded: false,
        errorText: '',
        // Clear loading state too just in case
        isLoadingModel: false,
        loadingStage: '',
      }));

      toast.info($t('common.model.unloaded') || 'Model unloaded due to inactivity', {
        description: event.payload,
      });
    });

    const unlistenSchedulerSnapshot = await listen<{
      loaded_models: string[];
      queue_len: number;
      inflight: number;
      timestamp: number;
    }>('scheduler_snapshot', (event) => {
      (window as any).__oxideSchedulerSnapshot = event.payload;
      const nextLoaded = Array.isArray(event.payload?.loaded_models) ? event.payload.loaded_models : [];
      loadedModelIds.set(nextLoaded);
    });

    const unlistenQueueWait = await listen<{ waited_ms: number; queue_position?: number }>(
      'scheduler_queue_wait',
      (event) => {
        const waited = Math.round((event.payload?.waited_ms ?? 0) / 1000);
        const pos = event.payload?.queue_position;
        toast.info($t('common.loader.loading') || 'Request queued', {
          description:
            pos != null
              ? `Queue position: ${pos}, waited ${waited}s`
              : `Waited ${waited}s in scheduler queue`,
        });
      },
    );

    // Merge unlisten functions
    const originalUnlisten = unlistenFn;
    window.addEventListener('keydown', handleGlobalShortcut);
    window.addEventListener('resize', handleViewportResize);
    unlistenFn = () => {
      originalUnlisten();
      unlistenUnload();
      unlistenSchedulerSnapshot();
      unlistenQueueWait();
      window.removeEventListener('keydown', handleGlobalShortcut);
      window.removeEventListener('resize', handleViewportResize);
    };
  });

  import { onDestroy } from 'svelte';
  onDestroy(() => {
    if (unlistenFn) unlistenFn();
    if (unlistenLoadedModels) unlistenLoadedModels();
    if (updaterCheckInterval) {
      clearInterval(updaterCheckInterval);
      updaterCheckInterval = null;
    }

    // Cleanup backend connections
    import('$lib/services/backend').then(({ cleanupBackend }) => {
      cleanupBackend();
    });
  });
</script>

<SidebarUI.Provider bind:open={isSidebarOpen}>
  <AppSidebar onOpenDownloads={() => (showDownloadManager = true)} />

  <SidebarUI.Inset class="app-shell">
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="app-header-wrapper" onmousedown={startDragging}>
      <header class="flex items-center justify-between px-2 h-14 bg-background">
        <div class="flex-1 flex items-center justify-start gap-2 sm:gap-4">
          {#if page.url.pathname === '/settings'}
            <div class="settings-breadcrumbs" data-no-drag>
              <Breadcrumb.Root>
                <Breadcrumb.List>
                  <Breadcrumb.Item>
                    <Breadcrumb.Link
                      href="/settings?section=general"
                      class="text-sm font-semibold text-foreground"
                      onclick={(event) => {
                        event.preventDefault();
                        openSettingsSectionShortcut('general');
                      }}
                    >
                      {$t('settings.v2.page.title')}
                    </Breadcrumb.Link>
                  </Breadcrumb.Item>

                  {#if viewportWidth < 1024}
                    {@const CurrentSectionIcon =
                      settingsSectionIconMap[getSettingsSectionFromQuery()]}
                    <Breadcrumb.Separator />
                    <Breadcrumb.Item>
                      <DropdownMenu.Root>
                        <DropdownMenu.Trigger>
                          {#snippet child({ props })}
                            <Button {...props} variant="ghost" size="sm" class="h-7 gap-1 px-2">
                              <CurrentSectionIcon class="size-3.5" />
                              {$t(settingsSectionLabelMap[getSettingsSectionFromQuery()])}
                              <CaretDown size={12} />
                            </Button>
                          {/snippet}
                        </DropdownMenu.Trigger>
                        <DropdownMenu.Content align="start" sideOffset={6} class="w-56 z-[1400]">
                          {#each settingsSectionOrder as sectionId (sectionId)}
                            {#if sectionId === 'performance'}
                              <DropdownMenu.Separator />
                            {/if}
                            {@const SectionIcon = settingsSectionIconMap[sectionId]}
                            <DropdownMenu.Item
                              onSelect={() => openSettingsSectionShortcut(sectionId)}
                            >
                              <SectionIcon class="size-4" />
                              {$t(settingsSectionLabelMap[sectionId])}
                            </DropdownMenu.Item>
                          {/each}
                        </DropdownMenu.Content>
                      </DropdownMenu.Root>
                    </Breadcrumb.Item>
                  {/if}
                </Breadcrumb.List>
              </Breadcrumb.Root>
            </div>
          {/if}

          {#if page.url.pathname === '/'}
            <Popover.Root bind:open={isModelPickerOpen}>
              <Popover.Trigger bind:ref={comboboxTrigger} data-no-drag>
                {#snippet child({ props })}
                  <Button
                    {...props}
                    variant="ghost"
                    class={cn(
                      'model-combobox-trigger',
                      isModelPickerOpen && 'model-combobox-trigger--active',
                    )}
                    role="combobox"
                    aria-expanded={isModelPickerOpen}
                    aria-haspopup="listbox"
                    type="button"
                  >
                    <span class="model-combobox-body">
                      {#if $isModelLoading}
                        <Spinner size={14} class="model-combobox-spinner" />
                      {/if}
                      <span class="model-combobox-label">{$currentDisplayName}</span>
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
                    autofocus
                  />
                  <Command.List class="model-combobox-list custom-scrollbar">
                    <Command.Empty class="model-combobox-empty">
                      {$t('common.model.noModelsFound') || 'No models found'}
                    </Command.Empty>
                    <Command.Group>
                      {#each $quickModels as model (model.path)}
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
                              !areModelPathsEqual(model.path, $selectedModelPath) &&
                                'model-combobox-check--hidden',
                            )}
                          />
                          <div class="model-combobox-item-body">
                            <span class="model-combobox-item-name">
                              {formatModelLabel(model)}
                            </span>
                            <span class="model-combobox-item-meta">
                              {model.architecture ?? ($t('common.unknownArch') || 'Unknown')}
                            </span>
                          </div>
                          {@const isCurrentModelPath = areModelPathsEqual(model.path, $currentModelPath)}
                          {@const isCurrentAndLoaded = isCurrentModelPath && $isCurrentModelLoaded}
                          {@const isLoadedModel = isCurrentAndLoaded || isModelPathLoaded(model.path, $loadedModelIds)}
                          {#if isCurrentAndLoaded}
                            <span class="model-combobox-item-badge">
                              {$t('common.model.current') || 'Current'}
                            </span>
                          {:else if isLoadedModel}
                            <span class="model-combobox-item-badge">
                              {$t('common.loader.loaded') || 'Loaded'}
                            </span>
                          {/if}
                        </Command.Item>
                      {/each}
                    </Command.Group>
                  </Command.List>
                </Command.Root>
              </Popover.Content>
            </Popover.Root>

            {#if $isReloadAvailable}
              <button
                type="button"
                class="model-reload-btn"
                onclick={handleReloadModel}
                aria-label={$t('common.model.reloadModel') || 'Reload model'}
                title={$t('common.model.reloadModel') || 'Reload model'}
              >
                <Repeat size={16} weight="bold" />
              </button>
            {:else if $isCurrentModelLoaded}
              <button
                type="button"
                class="model-reload-btn model-unload-btn"
                onclick={() => void handleUnloadAndClearCache()}
                disabled={isUnloadActionRunning || !$canUnloadCurrentModel}
                aria-label="Unload model and clear cache"
                title="Unload model and clear cache"
              >
                {#if isUnloadActionRunning}
                  <Spinner size={14} />
                {:else}
                  <UploadSimple size={16} weight="bold" />
                {/if}
              </button>
            {/if}
          {/if}

          <!-- Page Tabs (for /models) -->
          {#if page.url.pathname === '/models' && $pageTabsList.length > 0}
            {#if viewportWidth < 1024}
              {@const currentModelsTab = getCurrentModelsTab()}
              <div class="settings-breadcrumbs" data-no-drag>
                <Breadcrumb.Root>
                  <Breadcrumb.List>
                    <Breadcrumb.Item>
                      <Breadcrumb.Link href="/models" class="text-sm font-semibold text-foreground">
                        {$t('models.title') || 'Models'}
                      </Breadcrumb.Link>
                    </Breadcrumb.Item>

                    {#if currentModelsTab}
                      {@const CurrentModelsTabIcon =
                        modelsTabIconMap[currentModelsTab.id] ?? Cube}
                      <Breadcrumb.Separator />
                      <Breadcrumb.Item>
                        <DropdownMenu.Root>
                          <DropdownMenu.Trigger>
                            {#snippet child({ props })}
                              <Button {...props} variant="ghost" size="sm" class="h-7 gap-1 px-2">
                                <CurrentModelsTabIcon class="size-3.5" />
                                {currentModelsTab.label}
                                <CaretDown size={12} />
                              </Button>
                            {/snippet}
                          </DropdownMenu.Trigger>
                          <DropdownMenu.Content align="start" sideOffset={6} class="w-56 z-[1400]">
                            {#each $pageTabsList as tab (tab.id)}
                              {@const ModelsTabIcon = modelsTabIconMap[tab.id] ?? Cube}
                              <DropdownMenu.Item onSelect={() => handleHeaderTabsChange(tab.id)}>
                                <ModelsTabIcon class="size-4" />
                                {tab.label}
                              </DropdownMenu.Item>
                            {/each}
                          </DropdownMenu.Content>
                        </DropdownMenu.Root>
                      </Breadcrumb.Item>
                    {/if}
                  </Breadcrumb.List>
                </Breadcrumb.Root>
              </div>
            {:else}
              <div class="page-tabs" data-no-drag>
                <Tabs.Root
                  value={currentHeaderTabValue()}
                  class="page-tabs-root"
                  onValueChange={handleHeaderTabsChange}
                >
                  <Tabs.List class="page-tabs-list" aria-label="Page tabs">
                    {#each $pageTabsList as tab}
                      <Tabs.Trigger class="page-tab" value={tab.id}>
                        {tab.label}
                      </Tabs.Trigger>
                    {/each}
                  </Tabs.List>
                </Tabs.Root>
              </div>
            {/if}
          {/if}
        </div>

        <div class="flex items-center gap-2">
          <div class="flex items-center gap-1">
            <Button
              variant="ghost"
              size="icon"
              class="win-btn"
              onclick={() => appWindow.minimize()}
            >
              <Minus size={16} weight="bold" />
            </Button>
            <Button variant="ghost" size="icon" class="win-btn" onclick={toggleMaximize}>
              {#if isMaximized}
                <ArrowsIn size={16} weight="bold" />
              {:else}
                <ArrowsOut size={16} weight="bold" />
              {/if}
            </Button>
            <Button
              variant="ghost"
              size="icon"
              class="win-btn win-btn-close"
              onclick={() => appWindow.close()}
            >
              <X size={16} weight="bold" />
            </Button>
          </div>
        </div>
      </header>
    </div>

    <div class="app-body">
      <main
        class="app-main"
        class:models-compact={page.url.pathname === '/models'}
        class:settings-page={page.url.pathname === '/settings'}
      >
        <div class="view-switch">
          <!-- Chat page always mounted -->
          <div class="page-container" class:active={page.url.pathname === '/'}>
            <Chat />
          </div>

          <!-- Other pages rendered via slot -->
          {#if page.url.pathname !== '/'}
            <div class="page-container active custom-scrollbar">
              {@render children()}
            </div>
          {/if}
        </div>
      </main>
    </div>
  </SidebarUI.Inset>

  <!-- About Modal -->
  {#if showAbout}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      bind:this={modalElement}
      class="about-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="about-title"
      tabindex="-1"
      onclick={(event) => {
        if (event.target === event.currentTarget) toggleAbout();
      }}
      onkeydown={handleBackdropKeydown}
    >
      <div class="about-content" role="document">
        <h2 id="about-title">{$t('about.title') || 'About'}</h2>
        <div class="about-info">
          <p>
            <strong>Oxide Lab</strong> — {$t('about.description') ||
              'Local AI inference application'}
          </p>
          <p>
            <strong>{$t('about.technologies') || 'Technologies'}:</strong> Tauri, Svelte, llama.cpp process-host
          </p>
          <p><strong>{$t('about.version') || 'Version'}:</strong> {appVersion}</p>
        </div>
        <div class="about-actions">
          <button
            class="github-btn"
            onclick={() => window.open('https://github.com/FerrisMind/Oxide-Lab', '_blank')}
            aria-label="GitHub"
          >
            <GithubLogo size={16} />
            GitHub
          </button>
          <button
            class="close-btn"
            onclick={(e) => {
              e.stopPropagation();
              toggleAbout();
            }}
            aria-label={$t('common.buttons.close') || 'Close'}
          >
            {$t('common.buttons.close') || 'Close'}
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Download Manager Modal -->
  {#if showDownloadManager}
    <DownloadManagerModal onClose={() => (showDownloadManager = false)} />
  {/if}

  <AppUpdaterPrompt />

  <Command.Dialog
    bind:open={showCommandPalette}
    title="Command Palette"
    description="Quick navigation and settings jump"
  >
    <Command.Input placeholder="Search commands..." />
    <Command.List>
      <Command.Empty>No commands found.</Command.Empty>
      <Command.Group heading="Navigation">
        <Command.Item onSelect={openSettingsShortcut}>Open Settings</Command.Item>
        <Command.Item onSelect={openModelsShortcut}>Open Models</Command.Item>
        <Command.Item onSelect={openApiShortcut}>Open API</Command.Item>
      </Command.Group>
      <Command.Group heading="Settings Jump">
        {#each settingsSearchStore.registry.slice(0, 14) as item (item.id)}
          <Command.Item onSelect={() => openSettingsSectionShortcut(item.section, item.id)}>
            {item.title}
          </Command.Item>
        {/each}
      </Command.Group>
    </Command.List>
  </Command.Dialog>

  {#if page.url.pathname === '/'}
    <div hidden>{@render children()}</div>
  {/if}

  <div class="toaster-wrapper">
    <!-- Dynamic import workaround for Toaster if needed, or just standard usage if imported -->
    {#await import('svelte-sonner') then { Toaster }}
      <Toaster position="bottom-right" richColors theme="dark" />
    {/await}
  </div>
</SidebarUI.Provider>

<style>
  :global(.app-shell) {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
    position: relative;
    background: var(--background);
  }

  /* ===== Header Wrapper (CSS for app-region drag) ===== */
  .app-header-wrapper {
    position: relative;
    height: 3.5rem;
    box-sizing: border-box;
    -webkit-app-region: drag;
    z-index: 100;
    background: var(--background);
    border-bottom: 1px solid var(--border);
  }

  .app-header-wrapper button {
    -webkit-app-region: no-drag;
  }

  /* Model Combobox Styles */
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
  :global(.model-combobox-trigger:focus-visible),
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
    padding: 0;
    border-radius: 0.5rem;
    border: 1px solid var(--border);
    background: transparent;
    color: inherit;
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

  .settings-breadcrumbs {
    display: flex;
    align-items: center;
    min-width: 0;
    margin-left: 12px;
    -webkit-app-region: no-drag;
  }

  /* Page Tabs */
  .page-tabs {
    display: flex;
    align-items: center;
    margin-left: 0.5rem;
    -webkit-app-region: no-drag;
  }

  :global(.page-tabs-root) {
    display: flex;
  }

  :global(.page-tabs-list) {
    display: flex;
    gap: 0.5rem;
    background: transparent;
  }

  :global(.page-tab) {
    padding: 0.4rem 0.9rem;
    background: transparent;
    border-radius: 0.5rem;
    font-size: 0.9rem;
    transition: background 0.2s ease;
  }

  :global(.page-tab:hover) {
    background: var(--accent);
  }

  :global(.page-tab[data-state='active']) {
    background: color-mix(in srgb, var(--primary) 16%, transparent);
    font-weight: 600;
  }

  /* Window Controls */

  :global(.win-btn) {
    width: 2.25rem;
    height: 2.25rem;
    border-radius: 0.375rem;
  }

  :global(.win-btn-close:hover) {
    background: var(--destructive) !important;
    color: var(--destructive-foreground) !important;
  }

  /* App Body */
  .app-body {
    position: relative;
    flex: 1;
    min-height: 0;
    min-width: 0;
    display: flex;
    overflow: hidden;
    background: var(--background);
  }

  .app-main {
    position: relative;
    flex: 1;
    min-width: 0;
    display: flex;
    overflow: hidden;
    padding: 0 0.5rem 0.5rem;
  }

  .app-main.settings-page {
    padding-left: 0;
    padding-right: 0;
  }

  .view-switch {
    position: relative;
    display: flex;
    flex: 1;
    min-height: 0;
    min-width: 0;
    width: 100%;
    height: 100%;
  }

  .page-container {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    min-width: 0;
    opacity: 0;
    visibility: hidden;
    pointer-events: none;
    transition:
      opacity 0.15s ease,
      visibility 0.15s ease;
    overflow-x: hidden;
    overflow-y: auto;
    background: var(--background);
  }

  .page-container.active {
    opacity: 1;
    visibility: visible;
    pointer-events: auto;
    z-index: 1;
  }

  /* About Modal */
  .about-modal {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    backdrop-filter: blur(4px);
  }

  .about-content {
    background: var(--card);
    color: var(--card-foreground);
    border: 1px solid var(--border);
    border-radius: 0.75rem;
    padding: 1.5rem;
    width: min(520px, calc(100vw - 2rem));
    box-shadow: 0 10px 40px -10px rgba(0, 0, 0, 0.35);
  }

  .about-content h2 {
    font-size: 1.25rem;
    font-weight: 600;
    margin-bottom: 1rem;
  }

  .about-info {
    margin-bottom: 1.5rem;
  }

  .about-info p {
    margin: 0.5rem 0;
    line-height: 1.5;
  }

  .about-actions {
    display: flex;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .github-btn {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 1rem;
    border-radius: 0.5rem;
    border: 1px solid var(--border);
    background: transparent;
    color: inherit;
    font-size: 0.875rem;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .github-btn:hover {
    background: var(--accent);
    border-color: var(--primary);
  }

  .close-btn {
    padding: 0.5rem 1rem;
    border-radius: 0.5rem;
    border: none;
    background: var(--primary);
    color: var(--primary-foreground);
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.2s ease;
  }

  .close-btn:hover {
    opacity: 0.9;
  }
</style>
