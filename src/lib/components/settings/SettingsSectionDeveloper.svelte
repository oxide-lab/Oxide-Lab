<script lang="ts">
  import * as Card from '$lib/components/ui/card';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Select from '$lib/components/ui/select';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Input } from '$lib/components/ui/input';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import SettingRow from '$lib/components/settings/SettingRow.svelte';
  import SecurityStatusCard from '$lib/components/settings/SecurityStatusCard.svelte';
  import { t } from '$lib/i18n';
  import type { OpenAiServerConfig, OpenAiServerStatus } from '$lib/types/settings-v2';
  import Warning from 'phosphor-svelte/lib/Warning';

  interface Props {
    value: OpenAiServerConfig;
    status: OpenAiServerStatus | null;
    highlightedSettingId?: string | null;
    onChange: (next: OpenAiServerConfig) => void;
    onAddApiKey: (rawKey: string) => Promise<void>;
    onRestart: () => void;
  }

  let { value, status, highlightedSettingId = null, onChange, onAddApiKey, onRestart }: Props = $props();

  let apiKeyInput = $state('');
  let corsAllowlistText = $state(value.cors_allowlist.join(', '));
  let corsAnyConfirmOpen = $state(false);
  let lanAuthHintVisible = $state(false);

  $effect(() => {
    corsAllowlistText = value.cors_allowlist.join(', ');
  });

  async function submitApiKey() {
    const key = apiKeyInput.trim();
    if (!key) return;
    await onAddApiKey(key);
    apiKeyInput = '';
  }

  function commitCorsAllowlist() {
    const rows = corsAllowlistText
      .split(',')
      .map((v) => v.trim())
      .filter(Boolean);
    onChange({ ...value, cors_allowlist: rows });
  }

  function enableLan(nextChecked: boolean) {
    if (nextChecked && !value.auth_required) {
      lanAuthHintVisible = true;
      onChange({ ...value, bind_host: '0.0.0.0', auth_required: true });
      return;
    }
    onChange({ ...value, bind_host: nextChecked ? '0.0.0.0' : '127.0.0.1' });
  }

  function changeCorsMode(nextMode: OpenAiServerConfig['cors_mode']) {
    if (nextMode === 'any' && value.cors_mode !== 'any') {
      corsAnyConfirmOpen = true;
      return;
    }
    onChange({ ...value, cors_mode: nextMode });
  }

  function confirmCorsAny() {
    onChange({ ...value, cors_mode: 'any' });
    corsAnyConfirmOpen = false;
  }
</script>

<div class="space-y-3">
  <SecurityStatusCard status={status} />

  <Card.Root>
    <Card.Header>
      <Card.Title>{$t('settings.v2.sections.developer.title')}</Card.Title>
      <Card.Description>{$t('settings.v2.sections.developer.description')}</Card.Description>
    </Card.Header>
    <Card.Content class="space-y-3">
      <SettingRow
        id="developer.openai_server"
        title={$t('settings.v2.developer.openai_server.title')}
        description={$t('settings.v2.developer.openai_server.description')}
        highlighted={highlightedSettingId === 'developer.openai_server'}
        controlPosition="start"
      >
        <Checkbox
          checked={value.enabled}
          onCheckedChange={(checked) => onChange({ ...value, enabled: checked === true })}
        />
      </SettingRow>

      <SettingRow
        id="developer.bind_host"
        title={$t('settings.v2.developer.bind_host.title')}
        description={$t('settings.v2.developer.bind_host.description')}
        highlighted={highlightedSettingId === 'developer.bind_host'}
        controlPosition="start"
      >
        <Checkbox checked={value.bind_host === '0.0.0.0'} onCheckedChange={(checked) => enableLan(checked === true)} />
      </SettingRow>

      <SettingRow
        id="developer.port"
        title={$t('settings.v2.developer.port.title')}
        description={$t('settings.v2.developer.port.description')}
        highlighted={highlightedSettingId === 'developer.port'}
      >
        <Input
          type="number"
          min="1024"
          max="65535"
          value={value.port}
          onblur={(e) =>
            onChange({
              ...value,
              port: Math.min(65535, Math.max(1024, Number((e.currentTarget as HTMLInputElement).value) || 11434)),
            })}
        />
      </SettingRow>

      <SettingRow
        id="developer.auth_required"
        title={$t('settings.v2.developer.auth_required.title')}
        description={$t('settings.v2.developer.auth_required.description')}
        highlighted={highlightedSettingId === 'developer.auth_required'}
        controlPosition="start"
      >
        <Checkbox
          checked={value.auth_required}
          onCheckedChange={(checked) =>
            onChange({ ...value, auth_required: checked === true })}
        />
      </SettingRow>

      <SettingRow
        id="developer.api_key"
        title={$t('settings.v2.developer.api_keys.title')}
        description={$t('settings.v2.developer.api_keys.description')}
      >
        <div class="space-y-2">
          <div class="flex flex-col gap-2 sm:flex-row">
            <Input bind:value={apiKeyInput} placeholder={$t('settings.v2.developer.api_keys.placeholder')} />
            <Button
              variant="outline"
              size="sm"
              class="w-full sm:w-auto sm:shrink-0"
              onclick={submitApiKey}
            >
              {$t('settings.v2.developer.api_keys.add')}
            </Button>
          </div>
          <Badge variant="secondary">
            {$t('settings.v2.developer.api_keys.configured', { count: value.api_keys_hashed.length })}
          </Badge>
        </div>
      </SettingRow>

      <SettingRow
        id="developer.cors"
        title={$t('settings.v2.developer.cors.title')}
        description={$t('settings.v2.developer.cors.description')}
        highlighted={highlightedSettingId === 'developer.cors'}
      >
        <Select.Root
          type="single"
          value={value.cors_mode}
          onValueChange={(next) => changeCorsMode((next ?? 'same_origin') as OpenAiServerConfig['cors_mode'])}
        >
          <Select.Trigger class="w-full">{value.cors_mode}</Select.Trigger>
          <Select.Content>
            <Select.Item value="same_origin">{$t('settings.v2.developer.cors.same_origin')}</Select.Item>
            <Select.Item value="allowlist">{$t('settings.v2.developer.cors.allowlist')}</Select.Item>
            <Select.Item value="any">{$t('settings.v2.developer.cors.any')}</Select.Item>
          </Select.Content>
        </Select.Root>
      </SettingRow>

      {#if value.cors_mode === 'allowlist'}
        <SettingRow
          id="developer.cors_allowlist"
          title={$t('settings.v2.developer.cors_allowlist.title')}
          description={$t('settings.v2.developer.cors_allowlist.description')}
        >
          <Input
            bind:value={corsAllowlistText}
            onblur={commitCorsAllowlist}
            placeholder={$t('settings.v2.developer.cors_allowlist.placeholder')}
          />
        </SettingRow>
      {/if}

      <div class="rounded-md border border-amber-500/40 bg-amber-500/10 p-2 text-xs text-amber-700">
        <div class="mb-1 flex items-center gap-1 font-medium">
          <Warning class="size-4" />
          {$t('settings.v2.developer.risk.title')}
        </div>
        {$t('settings.v2.developer.risk.description')}
      </div>
      {#if lanAuthHintVisible}
        <div class="rounded-md border border-amber-500/40 bg-amber-500/10 p-2 text-xs text-amber-700">
          {$t('settings.v2.developer.alerts.lan_requires_auth')}
        </div>
      {/if}

      <Button variant="outline" size="sm" onclick={onRestart}>
        {$t('settings.v2.developer.actions.restart_server')}
      </Button>
    </Card.Content>
  </Card.Root>

  <Dialog.Root bind:open={corsAnyConfirmOpen}>
    <Dialog.Content class="sm:max-w-md">
      <Dialog.Header>
        <Dialog.Title>{$t('settings.v2.developer.cors.confirm_title')}</Dialog.Title>
        <Dialog.Description>{$t('settings.v2.developer.alerts.cors_any_confirm')}</Dialog.Description>
      </Dialog.Header>
      <Dialog.Footer>
        <Button variant="outline" onclick={() => (corsAnyConfirmOpen = false)}>
          {$t('settings.v2.common.cancel')}
        </Button>
        <Button variant="destructive" onclick={confirmCorsAny}>
          {$t('settings.v2.common.confirm')}
        </Button>
      </Dialog.Footer>
    </Dialog.Content>
  </Dialog.Root>
</div>
