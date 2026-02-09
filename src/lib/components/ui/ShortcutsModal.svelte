<script lang="ts">
  import * as Command from '$lib/components/ui/command/index.js';
  import * as Dialog from '$lib/components/ui/dialog/index.js';
  import { shortcuts } from '$lib/shortcuts';
  import { cn } from '$lib/utils';
  import { onMount } from 'svelte';

  interface Props {
    open: boolean;
  }

  let { open = $bindable(false) }: Props = $props();

  let isMac = $state(false);

  onMount(() => {
    isMac = navigator.platform.toUpperCase().includes('MAC');
  });

  const categories = ['Global', 'Chat'];

  function formatKey(key: string) {
    if (key === 'mod') return isMac ? '⌘' : 'Ctrl';
    if (key === 'shift') return '⇧';
    if (key === 'alt') return isMac ? '⌥' : 'Alt';
    if (key === 'escape') return 'Esc';
    return key.length === 1 ? key.toUpperCase() : key;
  }
</script>

<Command.Dialog bind:open class="rounded-3xl border shadow-md">
  <Command.Input placeholder="Search shortcuts..." />
  <Command.List class="max-h-[300px] overflow-y-auto">
    <Command.Empty>No shortcuts found.</Command.Empty>
    {#each categories as category}
      <Command.Group heading={category}>
        {#each Object.entries(shortcuts).filter(([_, s]) => s.category === category) as [id, shortcut]}
          <Command.Item value={shortcut.name} class="flex items-center justify-between px-4">
            <div class="flex flex-col">
              <span class="text-sm font-medium">{shortcut.name}</span>
              {#if shortcut.tooltip}
                <span class="text-xs text-muted-foreground">{shortcut.tooltip}</span>
              {/if}
            </div>
            <div class="flex items-center gap-1">
              {#each shortcut.keys as key}
                <kbd
                  class="pointer-events-none inline-flex h-5 select-none items-center gap-1 rounded border bg-muted px-1.5 font-mono text-[10px] font-medium text-muted-foreground opacity-100"
                >
                  {formatKey(key)}
                </kbd>
              {/each}
            </div>
          </Command.Item>
        {/each}
      </Command.Group>
      <Command.Separator />
    {/each}
  </Command.List>
</Command.Dialog>
