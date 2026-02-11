<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import Minus from 'phosphor-svelte/lib/Minus';
  import ArrowsIn from 'phosphor-svelte/lib/ArrowsIn';
  import ArrowsOut from 'phosphor-svelte/lib/ArrowsOut';
  import X from 'phosphor-svelte/lib/X';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount } from 'svelte';

  const appWindow = getCurrentWindow();
  let isMaximized = $state(false);

  async function toggleMaximize() {
    if (await appWindow.isMaximized()) {
      await appWindow.unmaximize();
      isMaximized = false;
    } else {
      await appWindow.maximize();
      isMaximized = true;
    }
  }

  onMount(() => {
    appWindow.isMaximized().then((max) => {
      isMaximized = max;
    });

    let unlisten: () => void;
    appWindow
      .onResized(() => {
        appWindow.isMaximized().then((maximized: boolean) => {
          isMaximized = maximized;
        });
      })
      .then((fn) => {
        unlisten = fn;
      });

    return () => {
      if (unlisten) unlisten();
    };
  });
</script>

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

<style>
  :global(.win-btn) {
    width: 2.25rem;
    height: 2.25rem;
    border-radius: 0.375rem;
  }

  :global(.win-btn-close:hover) {
    background: var(--destructive) !important;
    color: var(--destructive-foreground) !important;
  }
</style>
