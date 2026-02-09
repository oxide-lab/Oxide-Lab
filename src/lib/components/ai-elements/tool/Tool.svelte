<script lang="ts">
  import * as Collapsible from '$lib/components/ui/collapsible/index.js';
  import { cn } from '$lib/utils';
  import type { Snippet } from 'svelte';

  interface ToolProps {
    class?: string;
    children?: Snippet;
    [key: string]: any;
  }

  let { class: className = '', children, ...restProps }: ToolProps = $props();

  let id = $derived.by(() => crypto.randomUUID());
</script>

<Collapsible.Root
  {id}
  class={cn('not-prose mb-4 w-full rounded-md border', className)}
  {...restProps}
>
  {@render children?.()}
</Collapsible.Root>
