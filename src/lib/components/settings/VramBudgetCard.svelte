<script lang="ts">
  import * as Card from '$lib/components/ui/card';
  import { Progress } from '$lib/components/ui/progress';
  import { Badge } from '$lib/components/ui/badge';
  import { t } from '$lib/i18n';
  import Lightning from 'phosphor-svelte/lib/Lightning';

  interface Props {
    usedGb: number;
    totalGb: number;
    predictedGb: number;
    recommendedGpuLayers: number | null;
  }

  let { usedGb, totalGb, predictedGb, recommendedGpuLayers }: Props = $props();

  const ratio = $derived(totalGb > 0 ? Math.min(100, Math.round((predictedGb / totalGb) * 100)) : 0);
  const status = $derived(ratio >= 95 ? 'risk' : ratio >= 80 ? 'warn' : 'safe');
</script>

<Card.Root>
  <Card.Header>
    <Card.Title class="flex items-center gap-2">
      <Lightning class="size-5" />
      {$t('settings.v2.performance.vram_budget.title')}
    </Card.Title>
    <Card.Description>{$t('settings.v2.performance.vram_budget.description')}</Card.Description>
  </Card.Header>
  <Card.Content class="space-y-3">
    <Progress value={ratio} class="h-2" />
    <div class="flex flex-wrap items-center gap-2 text-xs">
      <Badge variant={status === 'safe' ? 'secondary' : status === 'warn' ? 'outline' : 'destructive'}>
        {status === 'safe'
          ? $t('settings.v2.performance.vram_budget.safe')
          : status === 'warn'
            ? $t('settings.v2.performance.vram_budget.warn')
            : $t('settings.v2.performance.vram_budget.risk')}
      </Badge>
      <span class="text-muted-foreground">
        {$t('settings.v2.performance.vram_budget.predicted', {
          predicted: predictedGb.toFixed(2),
          total: totalGb.toFixed(2),
        })}
      </span>
    </div>
    <p class="text-xs text-muted-foreground">
      {$t('settings.v2.performance.vram_budget.current', { used: usedGb.toFixed(2) })}
    </p>
    {#if recommendedGpuLayers !== null}
      <p class="text-xs text-muted-foreground">
        {$t('settings.v2.performance.vram_budget.recommended', { count: recommendedGpuLayers })}
      </p>
    {/if}
  </Card.Content>
</Card.Root>
