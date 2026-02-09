<script lang="ts">
  /**
   * Model Icon Component
   *
   * Displays an icon for a model based on its family or name.
   * Uses Lobe Icons SVG library from npm package.
   */
  import { getModelIconName } from '$lib/components/ai-elements/markdown/utils/model-icons';
  interface Props {
    family?: string | null;
    size?: number;
    class?: string;
  }

  let { family = null, size = 24, class: className = '' }: Props = $props();

  // Pre-load all icons using import.meta.glob
  const iconModules = import.meta.glob('/node_modules/@lobehub/icons-static-svg/icons/*.svg', {
    query: '?url',
    import: 'default',
    eager: true,
  }) as Record<string, string>;

  const rawIconModules = import.meta.glob('/node_modules/@lobehub/icons-static-svg/icons/*.svg', {
    query: '?raw',
    import: 'default',
    eager: true,
  }) as Record<string, string>;

  let iconName = $derived(getModelIconName(family));
  let hasError = $state(false);
  let isFallbackIcon = $state(false);
  let activeIconName = $derived(isFallbackIcon ? 'huggingface' : iconName);

  // Check if color version exists
  let colorIconSrc = $derived.by(() => {
    const key = `/node_modules/@lobehub/icons-static-svg/icons/${activeIconName}-color.svg`;
    return iconModules[key] ?? null;
  });

  // Get raw SVG for non-color version to support styling (e.g. white color)
  let inlineSvg = $derived.by(() => {
    if (colorIconSrc) return null; // Use img tag for color icons
    const key = `/node_modules/@lobehub/icons-static-svg/icons/${activeIconName}.svg`;
    return rawIconModules[key] ?? null;
  });

  // Fallback if no inline SVG and no color icon
  let fallbackSrc = $derived.by(() => {
    if (colorIconSrc || inlineSvg) return null;
    const key = `/node_modules/@lobehub/icons-static-svg/icons/${activeIconName}.svg`;
    return iconModules[key] ?? null;
  });

  function handleError() {
    if (!isFallbackIcon) {
      isFallbackIcon = true;
      return;
    }
    hasError = true;
  }

  // Reset error when icon changes
  $effect(() => {
    iconName;
    hasError = false;
    isFallbackIcon = false;
  });
</script>

{#if inlineSvg}
  <!-- Non-color icons are rendered inline to support currentColor (white in dark mode) -->
  <span
    class={`inline-flex items-center justify-center shrink-0 ${
      activeIconName === 'yandex' ? 'text-red-600 dark:text-red-500' : 'text-foreground'
    } ${className}`}
    style={`width:${size}px;height:${size}px;`}
    aria-label={family || 'Model'}
  >
    <span
      class="inline-block size-full [&&>svg]:size-full [&&>svg]:fill-current [&&>svg]:text-current"
    >
      {@html inlineSvg}
    </span>
  </span>
{:else if colorIconSrc}
  <img
    src={colorIconSrc}
    alt={family || 'Model'}
    width={size}
    height={size}
    class={`shrink-0 ${className}`}
    onerror={handleError}
  />
{:else if fallbackSrc}
  <img
    src={fallbackSrc}
    alt={family || 'Model'}
    width={size}
    height={size}
    class={`shrink-0 ${className}`}
    onerror={handleError}
  />
{:else}
  <div
    class={`inline-flex items-center justify-center rounded-sm bg-muted text-[10px] text-muted-foreground shrink-0 ${className}`}
    style={`width:${size}px;height:${size}px;`}
  >
    HF
  </div>
{/if}

<style>
  img {
    object-fit: contain;
  }

  /* Force currentColor for inline SVGs to respect text color (white in dark mode) */
  :global(.fill-current svg) {
    fill: currentColor !important;
  }

  :global(.text-current svg) {
    color: currentColor !important;
  }
</style>
