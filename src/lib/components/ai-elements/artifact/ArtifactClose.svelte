<script lang="ts">
  import * as Button from '$lib/components/ui/button/index.js';
  import { cn } from '$lib/utils';
  import { X } from 'phosphor-svelte';

  interface Props {
    class?: string;
    size?: 'default' | 'sm' | 'lg' | 'icon' | undefined;
    variant?: 'default' | 'destructive' | 'outline' | 'secondary' | 'ghost' | 'link' | undefined;
    onclick?: (e: MouseEvent) => void;
    children?: import('svelte').Snippet;
  }

  let {
    class: className,
    children,
    size = 'sm',
    variant = 'ghost',
    ...restProps
  }: Props = $props();
</script>

<Button.Root
  class={cn('text-muted-foreground hover:text-foreground size-8 p-0', className)}
  {size}
  type="button"
  {variant}
  {...restProps}
>
  {#if children}
    {@render children()}
  {:else}
    <X class="size-4" />
  {/if}
  <span class="sr-only">Close</span>
</Button.Root>
