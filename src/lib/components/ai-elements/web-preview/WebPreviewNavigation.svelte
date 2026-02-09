<script lang="ts">
  import { cn } from '$lib/utils';
  import { getWebPreviewContext } from './web-preview-context.svelte';
  import { DownloadSimple, ArrowsOut } from 'phosphor-svelte';
  import WebPreviewNavigationButton from './WebPreviewNavigationButton.svelte';

  let {
    class: className,
    children,
    ...restProps
  }: {
    class?: string;
    children: import('svelte').Snippet;
    [key: string]: any;
  } = $props();

  let context = getWebPreviewContext();

  function handleDownload() {
    const content = context.srcdoc || context.url;
    if (!content) return;

    const blob = new Blob([content], { type: 'text/html' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'artifact.html';
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }

  function handleFullscreen() {
    const iframe = document.querySelector('.preview-iframe') as HTMLIFrameElement;
    if (iframe) {
      if (iframe.requestFullscreen) {
        iframe.requestFullscreen();
      }
    }
  }
</script>

<div class={cn('preview-nav', className)} {...restProps}>
  <div class="flex flex-1 items-center gap-1">
    {@render children()}
  </div>
  <div class="flex items-center gap-1">
    <WebPreviewNavigationButton tooltip="Download" onclick={handleDownload}>
      <DownloadSimple class="size-4" />
    </WebPreviewNavigationButton>
    <WebPreviewNavigationButton tooltip="Fullscreen" onclick={handleFullscreen}>
      <ArrowsOut class="size-4" />
    </WebPreviewNavigationButton>
  </div>
</div>

<style>
  .preview-nav {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.5rem;
    border-bottom: 1px solid var(--border);
    background: var(--background);
    position: relative;
    z-index: 10;
    border-radius: 1.5rem 1.5rem 0 0;
  }
</style>
