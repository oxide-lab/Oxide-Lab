<script lang="ts">
  import * as Card from '$lib/components/ui/card';
  import { Badge } from '$lib/components/ui/badge';
  import { t } from '$lib/i18n';
  import type { OpenAiServerStatus } from '$lib/types/settings-v2';
  import ShieldCheck from 'phosphor-svelte/lib/ShieldCheck';
  import Warning from 'phosphor-svelte/lib/Warning';

  interface Props {
    status: OpenAiServerStatus | null;
  }

  let { status }: Props = $props();
</script>

<Card.Root>
  <Card.Header>
    <Card.Title class="flex items-center gap-2">
      <ShieldCheck class="size-5" />
      {$t('settings.v2.security_status.title')}
    </Card.Title>
    <Card.Description>{$t('settings.v2.security_status.description')}</Card.Description>
  </Card.Header>
  <Card.Content class="space-y-3">
    {#if status}
      <div class="flex flex-wrap gap-2">
        <Badge variant={status.running ? 'default' : 'outline'}>
          {$t('settings.v2.security_status.server')}: {status.running
            ? $t('settings.v2.security_status.on')
            : $t('settings.v2.security_status.off')}
        </Badge>
        <Badge variant={status.bind_host === '127.0.0.1' ? 'secondary' : 'destructive'}>
          {status.bind_host === '127.0.0.1'
            ? $t('settings.v2.security_status.localhost')
            : $t('settings.v2.security_status.lan')}
        </Badge>
        <Badge variant={status.auth_required ? 'default' : 'outline'}>
          {$t('settings.v2.security_status.auth')}: {status.auth_required
            ? $t('settings.v2.security_status.required')
            : $t('settings.v2.security_status.disabled')}
        </Badge>
        <Badge variant="outline">{$t('settings.v2.security_status.cors')}: {status.cors_mode}</Badge>
      </div>
      <p class="text-xs text-muted-foreground break-all">
        {$t('settings.v2.security_status.endpoint')}: {status.endpoint}
      </p>
      {#if status.warnings.length > 0}
        <div class="rounded-md border border-amber-500/40 bg-amber-500/10 p-2 text-xs text-amber-700">
          <div class="mb-1 flex items-center gap-1 font-medium">
            <Warning class="size-4" />
            {$t('settings.v2.security_status.warnings')}
          </div>
          {#each status.warnings as warning (warning)}
            <p>{warning}</p>
          {/each}
        </div>
      {/if}
    {:else}
      <p class="text-sm text-muted-foreground">{$t('settings.v2.security_status.unavailable')}</p>
    {/if}
  </Card.Content>
</Card.Root>
