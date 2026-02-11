<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import { Progress } from '$lib/components/ui/progress';
  import { t } from '$lib/i18n';
  import { appUpdaterStore } from '$lib/stores/app-updater';
  import DownloadSimple from 'phosphor-svelte/lib/DownloadSimple';
  import CaretDown from 'phosphor-svelte/lib/CaretDown';

  const updater = appUpdaterStore;
  let showReleaseNotes = $state(false);
  const shouldShowPrompt = $derived($updater.isUpdateAvailable && !$updater.remindMeLater);
  const progressPercent = $derived(Math.round(($updater.downloadProgress || 0) * 100));

  function formatBytes(value?: number): string {
    if (!value || value <= 0) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    let size = value;
    let index = 0;
    while (size >= 1024 && index < units.length - 1) {
      size /= 1024;
      index += 1;
    }
    return index === 0 ? `${Math.round(size)} ${units[index]}` : `${size.toFixed(1)} ${units[index]}`;
  }

  async function handleUpdateNow() {
    try {
      await updater.downloadAndInstallUpdate();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      const { toast } = await import('svelte-sonner');
      toast.error($t('settings.v2.about.updater.install_failed'), {
        description: message,
      });
    }
  }
</script>

{#if shouldShowPrompt}
  <aside class="app-updater-prompt" role="status" aria-live="polite">
    <div class="app-updater-header">
      <div class="app-updater-title-wrap">
        <DownloadSimple class="size-5 text-primary" />
        <div class="space-y-0.5">
          <h3 class="app-updater-title">
            {$t('settings.v2.about.updater.new_version', { version: $updater.updateInfo?.version ?? '—' })}
          </h3>
          <p class="text-xs text-muted-foreground">{$t('settings.v2.about.updater.available')}</p>
        </div>
      </div>
    </div>

    {#if $updater.isDownloading}
      <div class="space-y-2 rounded-md border bg-muted/20 p-2.5">
        <div class="flex items-center justify-between text-xs font-medium">
          <span>{$t('settings.v2.about.updater.downloading')}</span>
          <span>{progressPercent}%</span>
        </div>
        <Progress value={progressPercent} class="h-2" />
        <p class="text-xs text-muted-foreground">
          {$t('settings.v2.about.updater.progress', {
            downloaded: formatBytes($updater.downloadedBytes),
            total: formatBytes($updater.totalBytes),
          })}
        </p>
      </div>
    {/if}

    {#if $updater.updateInfo?.body}
      <Button
        variant="ghost"
        size="sm"
        class="h-8 w-full justify-between px-2 text-xs"
        onclick={() => (showReleaseNotes = !showReleaseNotes)}
      >
        <span>
          {showReleaseNotes
            ? $t('settings.v2.about.updater.hide_release_notes')
            : $t('settings.v2.about.updater.show_release_notes')}
        </span>
        <CaretDown
          size={14}
          class="transition-transform duration-150"
          style={`transform: rotate(${showReleaseNotes ? '180deg' : '0deg'})`}
        />
      </Button>
      {#if showReleaseNotes}
        <pre class="app-updater-notes custom-scrollbar">{$updater.updateInfo.body}</pre>
      {/if}
    {/if}

    <div class="flex items-center justify-end gap-2 pt-1">
      <Button
        variant="ghost"
        size="sm"
        class="h-8"
        disabled={$updater.isDownloading}
        onclick={() => updater.setRemindMeLater(true)}
      >
        {$t('settings.v2.about.updater.remind_later')}
      </Button>
      <Button size="sm" class="h-8" disabled={$updater.isDownloading} onclick={handleUpdateNow}>
        {$updater.isDownloading
          ? $t('settings.v2.about.updater.downloading')
          : $t('settings.v2.about.updater.update_now')}
      </Button>
    </div>
  </aside>
{/if}

<style>
  .app-updater-prompt {
    position: fixed;
    right: 1rem;
    bottom: 1rem;
    z-index: 1250;
    width: min(26rem, calc(100vw - 2rem));
    border: 1px solid var(--border);
    border-radius: 0.75rem;
    background: color-mix(in oklab, var(--background) 94%, black 6%);
    box-shadow: 0 18px 40px hsl(0 0% 0% / 0.35);
    padding: 0.85rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .app-updater-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .app-updater-title-wrap {
    display: flex;
    align-items: flex-start;
    gap: 0.6rem;
    min-width: 0;
  }

  .app-updater-title {
    font-size: 0.95rem;
    line-height: 1.25rem;
    font-weight: 600;
    color: var(--foreground);
  }

  .app-updater-notes {
    margin: 0;
    white-space: pre-wrap;
    font-family: inherit;
    font-size: 0.75rem;
    line-height: 1.25;
    border: 1px solid var(--border);
    background: var(--muted);
    border-radius: 0.5rem;
    padding: 0.6rem;
    max-height: 10rem;
    overflow: auto;
    color: var(--muted-foreground);
  }
</style>
