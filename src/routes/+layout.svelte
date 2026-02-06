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
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { derived } from 'svelte/store';

  // Phosphor Icons
  import Minus from 'phosphor-svelte/lib/Minus';
  import ArrowsIn from 'phosphor-svelte/lib/ArrowsIn';
  import ArrowsOut from 'phosphor-svelte/lib/ArrowsOut';
  import X from 'phosphor-svelte/lib/X';
  import CaretDown from 'phosphor-svelte/lib/CaretDown';
  import Check from 'phosphor-svelte/lib/CheckCircle';
  import GithubLogo from 'phosphor-svelte/lib/GithubLogo';
  import ArrowClockwise from 'phosphor-svelte/lib/ArrowClockwise';

  // UI Components
  import AppSidebar from '$lib/components/app-sidebar.svelte';
  import * as SidebarUI from '$lib/components/ui/sidebar/index';
  import { Button } from '$lib/components/ui/button';
  import { Spinner } from '$lib/components/ui/spinner';
  import * as Popover from '$lib/components/ui/popover';
  import * as Command from '$lib/components/ui/command';
  import * as Tabs from '$lib/components/ui/tabs';
  import DownloadManagerModal from '$lib/components/DownloadManagerModal.svelte';

  // Core
  import { cn } from '../lib/utils';
  import { t, locale, initI18n } from '$lib/i18n';
  import { experimentalFeatures } from '$lib/stores/experimental-features.svelte';
  import { pageTabsList, activePageTab } from '$lib/stores/page-tabs.svelte';
  import type { TabId } from '$lib/stores/page-tabs.svelte';
  import { chatState } from '$lib/stores/chat';
  import { folderPath, models, scanFolder } from '$lib/stores/local-models';
  import type { ModelInfo } from '$lib/types/local-models';


  // Pages for mount-all pattern
  import Chat from '$lib/chat/Chat.svelte';

  const { children } = $props();

  let isMaximized = $state(false);
  let isSidebarOpen = $state(false);
  let isModelPickerOpen = $state(false);
  let comboboxTrigger = $state<HTMLButtonElement | null>(null);
  let showDownloadManager = $state(false);
  let showAbout = $state(false);
  let appVersion = $state('0.13.1');
  let modalElement = $state<HTMLDivElement | null>(null);

  const appWindow = getCurrentWindow();

  // Derived stores for model picker
  const quickModels = derived(models, ($models) =>
    $models.filter(
      (model: ModelInfo) =>
        Boolean(model.source_repo_name?.trim() || model.name?.trim()),
    ),
  );
  const currentModelPath = derived(chatState, ($chatState) => $chatState.modelPath);
  const pendingModelPath = derived(chatState, ($chatState) => $chatState.pendingModelPath);
  const isModelLoading = derived(chatState, ($chatState) => $chatState.isLoadingModel);
  const modelLoadingStage = derived(chatState, ($chatState) => $chatState.loadingStage);
  const currentModel = derived(
    [quickModels, currentModelPath],
    ([$quickModels, $currentModelPath]) =>
      $quickModels.find((model: ModelInfo) => model.path === $currentModelPath),
  );
  const currentDisplayName = derived(currentModel, ($currentModel) =>
    formatModelLabel($currentModel),
  );
  const isReloadAvailable = derived([pendingModelPath, currentModelPath], ([$pending, $current]) =>
    Boolean($pending && $pending !== $current),
  );

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

  function formatModelLabel(model: ModelInfo | null | undefined) {
    if (!model) return $t('common.model.selectModel') || 'Select model';
    const publisher = model.metadata?.author ?? model.source_repo_id?.split('/')[0] ?? 'local';
    const title = model.name ?? model.source_repo_name ?? 'Unnamed';
    return `${publisher}/${title}`;
  }

  function handleReloadModel() {
    // TODO: Integrate with backend - this exposes the reload function
    const ox = (window as any).__oxide;
    if (ox?.reloadSelectedModel) {
      ox.reloadSelectedModel();
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
      appVersion = '0.13.1';
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

  onMount(async () => {
    // Initialize i18n
    initI18n(page.url.pathname);

    // Load experimental features
    void experimentalFeatures.loadState();

    // Initialize backend connections (download manager, model cards, performance listeners)
    const { initializeBackend } = await import('$lib/services/backend');
    void initializeBackend().catch((err) => console.warn('Backend initialization failed:', err));

    // Scan local models if folder is set
    if ($folderPath) {
      void scanFolder($folderPath).catch((err) => console.warn('Failed to scan local models', err));
    }

    // Setup window state
    isMaximized = await appWindow.isMaximized();
    unlistenFn = await appWindow.onResized(() => {
      appWindow.isMaximized().then((maximized: boolean) => {
        isMaximized = maximized;
      });
    });

    const { listen } = await import('@tauri-apps/api/event');
    const { toast, Toaster } = await import('svelte-sonner');

    const unlistenUnload = await listen<string>('model_unloaded', (event) => {
        console.log('Model unloaded automatically:', event.payload);
        
        chatState.update(s => ({
            ...s,
            isLoaded: false,
            modelPath: '', // Force selector reset
            pendingModelPath: '',
            errorText: '',
            // Clear loading state too just in case
            isLoadingModel: false,
            loadingStage: ''
        }));

        toast.info($t('common.model.unloaded') || 'Model unloaded due to inactivity', {
            description: event.payload
        });
    });

    // Merge unlisten functions
    const originalUnlisten = unlistenFn;
    unlistenFn = () => {
        originalUnlisten();
        unlistenUnload();
    };
  });

  import { onDestroy } from 'svelte';
  onDestroy(() => {
    if (unlistenFn) unlistenFn();

    // Cleanup backend connections
    import('$lib/services/backend').then(({ cleanupBackend }) => {
      cleanupBackend();
    });
  });
</script>

<SidebarUI.Provider bind:open={isSidebarOpen}>
  <AppSidebar onOpenDownloads={() => (showDownloadManager = true)} onOpenAbout={toggleAbout} />

  <SidebarUI.Inset class="app-shell">
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="app-header-wrapper" onmousedown={startDragging}>
      <header class="flex items-center justify-between px-2 h-14 bg-background">
        <div class="flex-1 flex items-center justify-start gap-2 sm:gap-4">
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
                              model.path !== $currentModelPath && 'model-combobox-check--hidden',
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
                          {#if model.path === $currentModelPath}
                            <span class="model-combobox-item-badge">
                              {$t('common.model.current') || 'Current'}
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
              >
                <ArrowClockwise size={16} weight="bold" />
                {$t('common.model.reloadModel') || 'Reload'}
              </button>
            {/if}
          {/if}

          <!-- Page Tabs (for /models) -->
          {#if page.url.pathname === '/models' && $pageTabsList.length > 0}
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
        </div>

        <div class="flex items-center gap-1">
          <Button variant="ghost" size="icon" class="win-btn" onclick={() => appWindow.minimize()}>
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
      </header>
    </div>

    <div class="app-body">
      <main class="app-main" class:models-compact={page.url.pathname === '/models'}>
        <div class="view-switch">
          <!-- Chat page always mounted -->
          <div class="page-container" class:active={page.url.pathname === '/'}>
            <Chat />
          </div>

          <!-- Other pages rendered via slot -->
          {#if page.url.pathname !== '/'}
            <div class="page-container active">
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
            <strong>{$t('about.technologies') || 'Technologies'}:</strong> Tauri, Svelte, Candle
          </p>
          <p><strong>{$t('about.version') || 'Version'}:</strong> {appVersion}</p>
        </div>
        <div class="about-actions">
          <button
            class="github-btn"
            onclick={() => openUrl('https://github.com/FerrisMind/Oxide-Lab')}
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
    gap: 0.5rem;
    padding: 0.25rem 0.5rem;
    border-radius: 0.5rem;
    border: 1px solid var(--border);
    background: transparent;
    color: inherit;
    font-size: 0.85rem;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .model-reload-btn:hover {
    background: var(--accent);
    border-color: var(--primary);
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
    flex: 1;
    min-height: 0;
    display: flex;
    overflow: hidden;
    background: var(--background);
  }

  .app-main {
    flex: 1;
    display: flex;
    overflow: hidden;
    padding: 0 0.5rem 0.5rem;
  }

  .view-switch {
    position: relative;
    display: flex;
    flex: 1;
    min-height: 0;
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
    opacity: 0;
    visibility: hidden;
    pointer-events: none;
    transition:
      opacity 0.15s ease,
      visibility 0.15s ease;
    overflow: auto;
    background: var(--background);
    scrollbar-width: thin;
    scrollbar-color: var(--muted-foreground) transparent;
  }

  .page-container::-webkit-scrollbar {
    width: 6px;
    height: 6px;
  }

  .page-container::-webkit-scrollbar-track {
    background: transparent;
  }

  .page-container::-webkit-scrollbar-thumb {
    background-color: var(--border);
    border-radius: 9999px;
  }

  .page-container::-webkit-scrollbar-thumb:hover {
    background-color: var(--muted-foreground);
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
