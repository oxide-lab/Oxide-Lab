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
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { APP_VERSION } from '$lib/version';

  // UI Components
  import AppSidebar from '$lib/components/app-sidebar.svelte';
  import * as SidebarUI from '$lib/components/ui/sidebar/index';
  import DownloadManagerModal from '$lib/components/DownloadManagerModal.svelte';
  import AppUpdaterPrompt from '$lib/components/updater/AppUpdaterPrompt.svelte';
  import AppHeader from '$lib/components/layout/AppHeader.svelte';
  import AboutModal from '$lib/components/layout/AboutModal.svelte';

  // Core
  import { t, initI18n } from '$lib/i18n';
  import { experimentalFeatures } from '$lib/stores/experimental-features.svelte';
  import { initLoadedModels } from '$lib/stores/local-models';
  import { chatState } from '$lib/stores/chat';
  import { appUpdaterStore, getUpdateCheckIntervalMs } from '$lib/stores/app-updater';

  // Pages
  import Chat from '$lib/chat/Chat.svelte';

  const { children } = $props();

  let isSidebarOpen = $state(false);
  let showDownloadManager = $state(false);
  let showAbout = $state(false);
  let appVersion = $state(APP_VERSION);
  let viewportWidth = $state(1024);

  const appWindow = getCurrentWindow();

  // Redirect experimental pages when features disabled
  $effect(() => {
    if (experimentalFeatures.initialized && !experimentalFeatures.enabled) {
      const experimentalPaths: string[] = ['/api', '/performance']; // Example
      if (experimentalPaths.includes(page.url.pathname)) {
        goto('/');
      }
    }
  });

  function handleViewportResize() {
    viewportWidth = window.innerWidth;
  }

  async function loadAppVersion() {
    try {
      const appInfo = (await invoke('get_app_info')) as { version: string };
      appVersion = appInfo.version;
    } catch (error) {
      appVersion = APP_VERSION;
    }
  }

  let unlistenFn: (() => void) | null = null;
  let unlistenLoadedModels: (() => void) | null = null;
  let updaterCheckInterval: ReturnType<typeof setInterval> | null = null;
  let viewportResizeRaf: number | null = null;
  let layoutActive = true;

  function delay(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  async function runStartupTask(
    taskName: string,
    task: () => Promise<unknown>,
    options?: {
      retries?: number;
      retryDelayMs?: number;
      notifyUser?: boolean;
      userMessage?: string;
      toast?: { error: (message: string) => void };
    },
  ): Promise<boolean> {
    const retries = options?.retries ?? 0;
    const retryDelayMs = options?.retryDelayMs ?? 600;
    const notifyUser = options?.notifyUser ?? true;

    for (let attempt = 0; attempt <= retries; attempt += 1) {
      try {
        await task();
        return true;
      } catch (error) {
        const isLastAttempt = attempt === retries;
        console.error(`[layout] ${taskName} failed (attempt ${attempt + 1}/${retries + 1})`, error);
        if (!isLastAttempt) {
          await delay(retryDelayMs * (attempt + 1));
          continue;
        }

        if (notifyUser && options?.toast) {
          options.toast.error(options.userMessage ?? `Failed to run ${taskName}`);
        }
      }
    }

    return false;
  }

  onMount(async () => {
    initI18n(page.url.pathname);
    const { toast } = await import('svelte-sonner');

    void runStartupTask('experimentalFeatures.loadState', () => experimentalFeatures.loadState(), {
      retries: 1,
      retryDelayMs: 800,
      userMessage: $t('common.error') || 'Failed to load experimental feature flags',
      toast,
    });

    unlistenLoadedModels = await initLoadedModels();

    const { initializeBackend } = await import('$lib/services/backend');
    void runStartupTask('initializeBackend', () => initializeBackend(), {
      retries: 1,
      retryDelayMs: 1000,
      userMessage: $t('common.error') || 'Failed to initialize backend services',
      toast,
    });

    if (!import.meta.env.DEV) {
      void runStartupTask(
        'appUpdaterStore.checkForUpdate',
        () => appUpdaterStore.checkForUpdate({ userInitiated: false, suppressErrors: false }),
        {
          retries: 1,
          retryDelayMs: 1200,
          notifyUser: false,
        },
      );

      updaterCheckInterval = setInterval(() => {
        if (!layoutActive) return;
        void runStartupTask(
          'appUpdaterStore.checkForUpdate(interval)',
          () => appUpdaterStore.checkForUpdate({ userInitiated: false, suppressErrors: false }),
          {
            retries: 1,
            retryDelayMs: 1200,
            notifyUser: false,
          },
        );
      }, getUpdateCheckIntervalMs());
    }

    handleViewportResize();
    const resizeUnlisten = await appWindow.onResized(() => {
      if (viewportResizeRaf !== null) cancelAnimationFrame(viewportResizeRaf);
      viewportResizeRaf = requestAnimationFrame(() => {
        viewportResizeRaf = null;
        handleViewportResize();
      });
    });

    const unlistenUnload = await listen<string>('model_unloaded', () => {
      chatState.update((s) => ({ ...s, isLoaded: false, isLoadingModel: false, loadingStage: '' }));
      toast.info($t('common.model.unloaded') || 'Model unloaded');
    });

    const unlistenQueueWait = await listen<{ waited_ms: number }>(
      'scheduler_queue_wait',
      (event) => {
        toast.info($t('common.loader.loading') || 'Request queued');
      },
    );

    unlistenFn = () => {
      resizeUnlisten();
      unlistenUnload();
      unlistenQueueWait();
      if (viewportResizeRaf !== null) {
        cancelAnimationFrame(viewportResizeRaf);
        viewportResizeRaf = null;
      }
    };

    void runStartupTask('loadAppVersion', () => loadAppVersion(), {
      retries: 1,
      retryDelayMs: 500,
      notifyUser: false,
    });
  });

  onDestroy(() => {
    layoutActive = false;
    unlistenFn?.();
    unlistenLoadedModels?.();
    if (updaterCheckInterval) clearInterval(updaterCheckInterval);
    import('$lib/services/backend')
      .then(({ cleanupBackend }) => cleanupBackend())
      .catch((error) => {
        console.error('[layout] cleanupBackend failed', error);
      });
  });
</script>

<SidebarUI.Provider bind:open={isSidebarOpen}>
  <AppSidebar onOpenDownloads={() => (showDownloadManager = true)} />

  <SidebarUI.Inset class="app-shell">
    <AppHeader {viewportWidth} pathname={page.url.pathname} />

    <div class="app-body">
      <main class="app-main" class:settings-page={page.url.pathname === '/settings'}>
        <div class="view-switch">
          <div class="page-container" class:active={page.url.pathname === '/'}>
            <Chat />
          </div>

          {#if page.url.pathname !== '/'}
            <div class="page-container active custom-scrollbar">
              {@render children()}
            </div>
          {/if}
        </div>
      </main>
    </div>
  </SidebarUI.Inset>

  <AboutModal open={showAbout} {appVersion} onClose={() => (showAbout = false)} />

  {#if showDownloadManager}
    <DownloadManagerModal onClose={() => (showDownloadManager = false)} />
  {/if}

  <AppUpdaterPrompt />

  {#await import('svelte-sonner') then { Toaster }}
    <Toaster position="bottom-right" richColors theme="dark" />
  {/await}
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
</style>
