<script lang="ts">
  import type { Snippet } from 'svelte';
  import { cn } from '$lib/utils';

  interface Props {
    header: Snippet;
    sidebar: Snippet;
    content: Snippet;
    showSidebar?: boolean;
    class?: string;
  }

  let { header, sidebar, content, showSidebar = true, class: className = '' }: Props = $props();
</script>

<div class={cn('flex h-full min-h-0 flex-col gap-3', className)}>
  <div class="shrink-0 bg-background">
    {@render header()}
  </div>
  <div class="flex min-h-0 flex-1 flex-row overflow-hidden" style="gap: clamp(14px, 1.6vw, 16px);">
    {#if showSidebar}
      <div class="min-w-0 basis-[30%]">
        {@render sidebar()}
      </div>
    {/if}
    <div
      class={cn(
        'settings-content-scroll-edge custom-scrollbar min-h-0 min-w-0 overflow-y-auto overflow-x-hidden',
        showSidebar ? 'basis-[70%]' : 'flex-1'
      )}
    >
      {@render content()}
    </div>
  </div>
</div>

<style>
  .settings-content-scroll-edge {
    margin-right: 0;
  }
</style>
