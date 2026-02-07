<script lang="ts">
  import type { Snippet } from 'svelte';
  import { cn } from '$lib/utils';
  import { Label } from '$lib/components/ui/label';

  interface Props {
    id: string;
    title: string;
    description?: string;
    highlighted?: boolean;
    controlPosition?: 'end' | 'start';
    class?: string;
    children: Snippet;
  }

  let {
    id,
    title,
    description = '',
    highlighted = false,
    controlPosition = 'end',
    class: className = '',
    children,
  }: Props = $props();
</script>

<div
  id={id}
  class={cn(
    'rounded-lg border bg-card px-4 py-3 transition-colors focus-within:ring-2 focus-within:ring-ring',
    highlighted && 'border-primary bg-primary/5',
    className,
  )}
>
  {#if controlPosition === 'start'}
    <div class="flex items-start gap-3">
      <div id={`${id}-control`} class="pt-0.5">
        {@render children()}
      </div>
      <div class="min-w-0 space-y-1">
        <Label for={`${id}-control`} class="text-sm font-medium">{title}</Label>
        {#if description}
          <p class="text-xs text-muted-foreground break-words">{description}</p>
        {/if}
      </div>
    </div>
  {:else}
    <div class="flex flex-col gap-3 sm:grid sm:grid-cols-[minmax(0,1fr)_minmax(0,320px)] sm:items-start sm:gap-3">
      <div class="min-w-0 space-y-1">
        <Label for={`${id}-control`} class="text-sm font-medium">{title}</Label>
        {#if description}
          <p class="text-xs text-muted-foreground break-words">{description}</p>
        {/if}
      </div>
      <div id={`${id}-control`} class="min-w-0 w-full sm:justify-self-end sm:w-full sm:max-w-[320px]">
        {@render children()}
      </div>
    </div>
  {/if}
</div>
