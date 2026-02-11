<script lang="ts">
  import { cn } from '$lib/utils';
  import { Button } from '$lib/components/ui/button';
  import * as Tooltip from '$lib/components/ui/tooltip';
  import {
    WebPreview,
    WebPreviewBody,
    WebPreviewNavigation,
  } from '$lib/components/ai-elements/web-preview';
  import { htmlPreviewStore, previewHtml } from '$lib/stores/html-preview';
  import X from 'phosphor-svelte/lib/X';
  import ArrowsOut from 'phosphor-svelte/lib/ArrowsOut';
  import ShieldCheck from 'phosphor-svelte/lib/ShieldCheck';
  import ShieldWarning from 'phosphor-svelte/lib/ShieldWarning';

  let { class: className }: { class?: string } = $props();

  // Sandbox mode: false = secure (no same-origin), true = trusted (with same-origin)
  let trustedMode = $state(false);

  const secureSandbox = 'allow-scripts allow-forms allow-popups allow-presentation';
  const trustedSandbox = 'allow-scripts allow-same-origin allow-forms allow-popups allow-presentation';

  let currentSandbox = $derived(trustedMode ? trustedSandbox : secureSandbox);

  // Inject viewport, responsive styles, and scrollbar styles into HTML content
  const injectedHead = `
<meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no">
<style>
  /* Full viewport fit - no margins, no scrollbars */
  html, body {
    margin: 0 !important;
    padding: 0 !important;
    width: 100% !important;
    height: 100% !important;
    overflow: hidden !important;
    box-sizing: border-box !important;
  }
  *, *::before, *::after { 
    box-sizing: border-box !important;
    margin: 0;
  }
  /* Scale content to fit */
  body > * {
    max-width: 100% !important;
    max-height: 100vh !important;
  }
  img, video, canvas, svg { 
    max-width: 100% !important; 
    max-height: 100vh !important;
    height: auto !important;
    object-fit: contain !important;
  }
  iframe, embed, object { max-width: 100% !important; }
</style>`;



  let wrappedHtml = $derived.by(() => {
    const html = $previewHtml ?? '';
    if (!html) return '';
    // Ignore CSP injected inside generated HTML so preview scripts follow app-level policy.
    const sanitizedHtml = html.replace(
      /<meta[^>]+http-equiv=["']content-security-policy["'][^>]*>/gi,
      '',
    );
    // Inject styles at the beginning of the HTML
    if (sanitizedHtml.includes('<head>')) {
      return sanitizedHtml.replace('<head>', `<head>${injectedHead}`);
    } else if (sanitizedHtml.includes('<html>')) {
      return sanitizedHtml.replace('<html>', `<html><head>${injectedHead}</head>`);
    } else {
      return `<!DOCTYPE html><html><head>${injectedHead}</head><body>${sanitizedHtml}</body></html>`;
    }
  });

  function handleClose() {
    htmlPreviewStore.closePreview();
    trustedMode = false; // Reset on close
  }

  function handleOpenExternal() {
    if ($previewHtml) {
      const blob = new Blob([$previewHtml], { type: 'text/html' });
      const url = URL.createObjectURL(blob);
      window.open(url, '_blank');
    }
  }

  function toggleTrustedMode() {
    trustedMode = !trustedMode;
  }
</script>

<div class={cn('flex flex-col h-full bg-background', className)}>
  <WebPreview class="h-full">
    <WebPreviewNavigation class="justify-between">
      <div class="flex items-center gap-2">
        <span class="text-sm font-medium text-muted-foreground px-2">HTML Preview</span>
        {#if trustedMode}
          <span class="text-xs px-1.5 py-0.5 rounded bg-yellow-500/20 text-yellow-600 dark:text-yellow-400">
            Trusted
          </span>
        {/if}
      </div>
      <div class="flex items-center gap-1">
        <Tooltip.Provider>
          <Tooltip.Root delayDuration={60}>
            <Tooltip.Trigger>
              <Button
                variant={trustedMode ? 'secondary' : 'ghost'}
                size="icon"
                class={cn(
                  'h-7 w-7',
                  trustedMode
                    ? 'text-yellow-600 dark:text-yellow-400'
                    : 'text-muted-foreground hover:text-foreground'
                )}
                onclick={toggleTrustedMode}
                aria-label={trustedMode ? 'Switch to secure mode' : 'Switch to trusted mode'}
              >
                {#if trustedMode}
                  <ShieldWarning size={16} weight="fill" />
                {:else}
                  <ShieldCheck size={16} />
                {/if}
              </Button>
            </Tooltip.Trigger>
            <Tooltip.Content side="bottom">
              {trustedMode
                ? 'Trusted mode: localStorage enabled. Click to switch to secure.'
                : 'Secure mode: localStorage blocked. Click to enable if needed.'}
            </Tooltip.Content>
          </Tooltip.Root>
        </Tooltip.Provider>
        <Button
          variant="ghost"
          size="icon"
          class="h-7 w-7 text-muted-foreground hover:text-foreground"
          onclick={handleOpenExternal}
          aria-label="Open in new tab"
        >
          <ArrowsOut size={16} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          class="h-7 w-7 text-muted-foreground hover:text-foreground"
          onclick={handleClose}
          aria-label="Close preview"
        >
          <X size={16} />
        </Button>
      </div>
    </WebPreviewNavigation>
    <WebPreviewBody srcdoc={wrappedHtml} sandbox={currentSandbox} />
  </WebPreview>
</div>
