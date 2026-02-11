<script lang="ts">
  import { get } from 'svelte/store';
  import { onDestroy } from 'svelte';
  import { Button } from '$lib/components/ui/button';
  import { Spinner } from '$lib/components/ui/spinner';
  import { t } from '$lib/i18n';
  import { appUpdaterStore } from '$lib/stores/app-updater';
  import ArrowSquareOut from 'phosphor-svelte/lib/ArrowSquareOut';
  import ArrowClockwise from 'phosphor-svelte/lib/ArrowClockwise';
  import Copy from 'phosphor-svelte/lib/Copy';
  import Check from 'phosphor-svelte/lib/Check';
  import GithubLogo from 'phosphor-svelte/lib/GithubLogo';
  import Globe from 'phosphor-svelte/lib/Globe';
  import ShieldCheck from 'phosphor-svelte/lib/ShieldCheck';
  import FileText from 'phosphor-svelte/lib/FileText';

  interface Props {
    appVersion: string;
  }

  let { appVersion }: Props = $props();
  let copied = $state(false);
  let checkingUpdates = $state(false);
  let copyTimer: ReturnType<typeof setTimeout> | null = null;

  const appName = 'Oxide Lab';
  const currentYear = new Date().getFullYear();
  const repoUrl = 'https://github.com/FerrisMind/Oxide-Lab';
  const websiteUrl = 'https://oxidelab.tech';
  const privacyUrl = `${repoUrl}/blob/main/README.md`;
  const thirdPartyLicensesUrl = `${repoUrl}/blob/main/THIRD_PARTY_LICENSES.md`;

  async function openExternal(url: string) {
    try {
      const { openUrl } = await import('@tauri-apps/plugin-opener');
      await openUrl(url);
    } catch (error) {
      console.warn('Failed to open url via system browser, fallback to window.open:', error);
      window.open(url, '_blank', 'noopener,noreferrer');
    }
  }

  async function copyVersion() {
    try {
      await navigator.clipboard.writeText(appVersion);
      copied = true;
      if (copyTimer) clearTimeout(copyTimer);
      copyTimer = setTimeout(() => {
        copied = false;
        copyTimer = null;
      }, 1200);
    } catch (error) {
      console.warn('Failed to copy version:', error);
    }
  }

  async function checkForUpdates() {
    if (checkingUpdates) return;

    const { toast } = await import('svelte-sonner');
    const updaterState = get(appUpdaterStore);
    if (updaterState.disabledByEnv) {
      toast.info($t('settings.v2.about.updater.disabled'));
      return;
    }

    checkingUpdates = true;
    try {
      const updateInfo = await appUpdaterStore.checkForUpdate({
        userInitiated: true,
        resetRemind: true,
      });
      if (updateInfo) {
        toast.success($t('settings.v2.about.updater.new_version', { version: updateInfo.version }));
        return;
      }
      toast.info($t('settings.v2.about.updater.no_update'));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error($t('settings.v2.about.updater.check_failed'), {
        description: message,
      });
    } finally {
      checkingUpdates = false;
    }
  }

  onDestroy(() => {
    if (copyTimer) {
      clearTimeout(copyTimer);
      copyTimer = null;
    }
  });
</script>

<section class="space-y-6 px-1 py-1 text-sm text-muted-foreground">
  <!-- Top: identity + version + update action -->
  <div class="space-y-4">
    <div class="flex items-center gap-4">
      <img src="/icon.svg" alt={appName} class="size-16 shrink-0 rounded-lg" />
      <div class="min-w-0">
        <h2 class="text-xl font-semibold text-foreground">{appName}</h2>
        <p class="mt-1 text-xs">{$t('settings.v2.sections.about.description')}</p>
      </div>
    </div>

    <div class="flex flex-wrap items-center gap-2">
      <span class="text-xs font-medium text-foreground/80">{$t('settings.v2.about.version_label')}</span>
      <code class="select-text rounded-md border bg-muted/50 px-2 py-1 font-mono text-xs text-foreground">
        {appVersion}
      </code>
      <Button variant="outline" size="sm" class="h-8" onclick={copyVersion}>
        {#if copied}
          <Check class="mr-1 size-4" />
          {$t('settings.v2.about.copied')}
        {:else}
          <Copy class="mr-1 size-4" />
          {$t('settings.v2.about.copy_version')}
        {/if}
      </Button>
      <Button
        variant="outline"
        size="sm"
        class="h-8"
        aria-label={$t('settings.v2.about.links.updates')}
        disabled={checkingUpdates}
        onclick={checkForUpdates}
      >
        {#if checkingUpdates}
          <Spinner class="mr-1 size-4" />
          {$t('settings.v2.about.updater.checking')}
        {:else}
          <ArrowClockwise class="mr-1 size-4" />
          {$t('settings.v2.about.check_updates')}
        {/if}
      </Button>
    </div>

    <p class="select-text text-xs">{$t('settings.v2.about.summary')}</p>
  </div>

  <div class="h-px bg-border"></div>

  <!-- Legal -->
  <div class="space-y-3">
    <h3 class="text-xs font-semibold uppercase tracking-wide text-foreground/80">
      {$t('settings.v2.about.legal_title')}
    </h3>
    <p class="select-text text-xs">{$t('settings.v2.about.copyright', { year: currentYear })}</p>
    <div class="flex flex-wrap gap-2">
      <Button
        variant="ghost"
        size="sm"
        class="h-8 px-2"
        aria-label={$t('settings.v2.about.links.privacy')}
        onclick={() => openExternal(privacyUrl)}
      >
        <ShieldCheck class="mr-1 size-4" />
        {$t('settings.v2.about.privacy_policy')}
        <ArrowSquareOut class="ml-1 size-3.5" />
      </Button>
      <Button
        variant="ghost"
        size="sm"
        class="h-8 px-2"
        aria-label={$t('settings.v2.about.links.licenses')}
        onclick={() => openExternal(thirdPartyLicensesUrl)}
      >
        <FileText class="mr-1 size-4" />
        {$t('settings.v2.about.third_party_licenses')}
        <ArrowSquareOut class="ml-1 size-3.5" />
      </Button>
    </div>
  </div>

  <div class="h-px bg-border"></div>

  <!-- Information -->
  <div class="space-y-3">
    <h3 class="text-xs font-semibold uppercase tracking-wide text-foreground/80">
      {$t('settings.v2.about.info_title')}
    </h3>
    <div class="flex flex-wrap gap-2">
      <Button
        variant="ghost"
        size="sm"
        class="h-8 px-2"
        aria-label={$t('settings.v2.about.github')}
        onclick={() => openExternal(repoUrl)}
      >
        <GithubLogo class="mr-1 size-4" />
        {$t('settings.v2.about.github')}
        <ArrowSquareOut class="ml-1 size-3.5" />
      </Button>
      <Button
        variant="ghost"
        size="sm"
        class="h-8 px-2"
        aria-label={$t('settings.v2.about.links.website')}
        onclick={() => openExternal(websiteUrl)}
      >
        <Globe class="mr-1 size-4" />
        {$t('settings.v2.about.website')}
        <ArrowSquareOut class="ml-1 size-3.5" />
      </Button>
    </div>
    <div class="space-y-1">
      <p class="text-xs font-medium text-foreground/80">{$t('settings.v2.about.credits_title')}</p>
      <p class="select-text text-xs leading-relaxed">
        {$t('settings.v2.about.credits_body')}
      </p>
    </div>
  </div>
</section>
