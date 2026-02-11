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

  const rawIconModules = import.meta.glob('/node_modules/@lobehub/icons-static-svg/icons/*.svg', {
    query: '?raw',
    import: 'default',
  }) as Record<string, () => Promise<string>>;

  let iconName = $derived(getModelIconName(family));
  let hasError = $state(false);
  let isFallbackIcon = $state(false);
  let activeIconName = $derived(isFallbackIcon ? 'huggingface' : iconName);
  let inlineSvg = $state<string | null>(null);
  let iconMode = $state<'mono' | 'color'>('mono');
  let loadToken = 0;

  function resolveIconVariant(name: string): { key: string; mode: 'mono' | 'color' } | null {
    const colorKey = `/node_modules/@lobehub/icons-static-svg/icons/${name}-color.svg`;
    if (rawIconModules[colorKey]) {
      return { key: colorKey, mode: 'color' };
    }
    const monoKey = `/node_modules/@lobehub/icons-static-svg/icons/${name}.svg`;
    if (rawIconModules[monoKey]) {
      return { key: monoKey, mode: 'mono' };
    }
    return null;
  }

  $effect(() => {
    const nextName = activeIconName;
    const variant = resolveIconVariant(nextName);
    const token = ++loadToken;
    hasError = false;
    inlineSvg = null;

    if (!variant) {
      if (!isFallbackIcon) {
        isFallbackIcon = true;
        return;
      }
      inlineSvg = null;
      hasError = true;
      return;
    }

    rawIconModules[variant.key]()
      .then((svg) => {
        if (token !== loadToken) return;
        inlineSvg = svg;
        iconMode = variant.mode;
      })
      .catch(() => {
        if (token !== loadToken) return;
        if (!isFallbackIcon) {
          isFallbackIcon = true;
          return;
        }
        inlineSvg = null;
        hasError = true;
      });
  });

  // Reset hard error when original icon family changes
  $effect(() => {
    iconName;
    hasError = false;
    isFallbackIcon = false;
  });
</script>

{#if inlineSvg}
  <span
    class={`inline-flex items-center justify-center shrink-0 ${
      iconMode === 'mono'
        ? activeIconName === 'yandex'
          ? 'text-red-600 dark:text-red-500'
          : 'text-foreground'
        : ''
    } ${className}`}
    style={`width:${size}px;height:${size}px;`}
    aria-label={family || 'Model'}
  >
    <span class={`inline-block size-full ${iconMode === 'mono' ? 'model-icon-svg-mono' : ''}`}>
      {@html inlineSvg}
    </span>
  </span>
{:else}
  <div
    class={`inline-flex items-center justify-center rounded-sm bg-muted text-[10px] text-muted-foreground shrink-0 ${className}`}
    style={`width:${size}px;height:${size}px;`}
  >
    HF
  </div>
{/if}

<style>
  :global(.model-icon-svg-mono svg) {
    width: 100%;
    height: 100%;
    fill: currentColor !important;
    color: currentColor !important;
  }
</style>
