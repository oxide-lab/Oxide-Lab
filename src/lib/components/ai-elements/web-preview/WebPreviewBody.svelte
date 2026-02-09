<script lang="ts">
  import { cn } from '$lib/utils';
  import { getWebPreviewContext } from './web-preview-context.svelte.js';

  let {
    loading,
    src,
    srcdoc,
    sandbox = 'allow-scripts allow-forms allow-popups allow-presentation',
    class: className,
    ...restProps
  }: {
    loading?: import('svelte').Snippet;
    src?: string;
    srcdoc?: string;
    sandbox?: string;
    class?: string;
    [key: string]: any;
  } = $props();

  let context = getWebPreviewContext();

  $effect(() => {
    if (srcdoc !== undefined) {
      context.setSrcdoc(srcdoc);
    }
  });

  let finalSrc = $derived.by(() => (srcdoc ? undefined : (src ?? context.url) || undefined));
</script>

<div class="preview-body-container">
  <iframe
    class={cn('preview-iframe', className)}
    {sandbox}
    src={finalSrc}
    {srcdoc}
    title="Preview"
    {...restProps}
  ></iframe>
  {#if loading}
    {@render loading()}
  {/if}
</div>

<style>
  .preview-body-container {
    flex: 1;
    overflow: hidden;
    border-radius: 0 0 1.5rem 1.5rem;
    position: relative;
  }

  .preview-iframe {
    width: 100%;
    height: 100%;
    border: 0;
    border-radius: 0 0 1.5rem 1.5rem;
    background: var(--background);
    display: block;
  }
</style>
