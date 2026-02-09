<script lang="ts">
  import * as Button from '$lib/components/ui/button/index.js';
  import * as Tooltip from '$lib/components/ui/tooltip/index.js';
  import { cn } from '$lib/utils';

  interface Props {
    class?: string;
    tooltip?: string;
    label?: string;
    icon?: any;
    size?: 'default' | 'sm' | 'lg' | 'icon' | undefined;
    variant?: 'default' | 'destructive' | 'outline' | 'secondary' | 'ghost' | 'link' | undefined;
    onclick?: (e: MouseEvent) => void;
    children?: import('svelte').Snippet;
  }

  let {
    tooltip,
    label,
    icon: Icon,
    children,
    class: className,
    size = 'sm',
    variant = 'ghost',
    ...restProps
  }: Props = $props();
</script>

{#snippet buttonContent()}
  <Button.Root
    class={cn('text-muted-foreground hover:text-foreground size-8 p-0', className)}
    {size}
    type="button"
    {variant}
    {...restProps}
  >
    {#if Icon}
      <Icon class="size-4" />
    {:else if children}
      {@render children()}
    {/if}
    <span class="sr-only">{label || tooltip}</span>
  </Button.Root>
{/snippet}

{#if tooltip}
  <Tooltip.Provider>
    <Tooltip.Root>
      <Tooltip.Trigger>
        {@render buttonContent()}
      </Tooltip.Trigger>
      <Tooltip.Content>
        <p>{tooltip}</p>
      </Tooltip.Content>
    </Tooltip.Root>
  </Tooltip.Provider>
{:else}
  {@render buttonContent()}
{/if}
