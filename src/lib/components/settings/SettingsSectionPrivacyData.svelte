<script lang="ts">
  import * as Card from '$lib/components/ui/card';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Button } from '$lib/components/ui/button';
  import SettingRow from '$lib/components/settings/SettingRow.svelte';
  import { t } from '$lib/i18n';
  import type { DataLocations, PrivacyDataSettings, ClearDataScope } from '$lib/types/settings-v2';

  interface Props {
    value: PrivacyDataSettings;
    locations: DataLocations | null;
    highlightedSettingId?: string | null;
    onChange: (next: PrivacyDataSettings) => void;
    onExportData: () => void;
    onClearData: (scope: ClearDataScope) => void;
  }

  let {
    value,
    locations,
    highlightedSettingId = null,
    onChange,
    onExportData,
    onClearData,
  }: Props = $props();
</script>

<Card.Root>
  <Card.Header>
    <Card.Title>{$t('settings.v2.sections.privacy_data.title')}</Card.Title>
    <Card.Description>{$t('settings.v2.sections.privacy_data.description')}</Card.Description>
  </Card.Header>
  <Card.Content class="space-y-3">
    <SettingRow
      id="privacy_data.telemetry_enabled"
      title={$t('settings.v2.privacy_data.telemetry.title')}
      description={$t('settings.v2.privacy_data.telemetry.description')}
      highlighted={highlightedSettingId === 'privacy_data.telemetry_enabled'}
      controlPosition="start"
    >
      <Checkbox
        checked={value.telemetry_enabled}
        onCheckedChange={(checked) => onChange({ ...value, telemetry_enabled: checked === true })}
      />
    </SettingRow>

    <SettingRow
      id="privacy_data.crash_reports_enabled"
      title={$t('settings.v2.privacy_data.crash_reports.title')}
      description={$t('settings.v2.privacy_data.crash_reports.description')}
      highlighted={highlightedSettingId === 'privacy_data.crash_reports_enabled'}
      controlPosition="start"
    >
      <Checkbox
        checked={value.crash_reports_enabled}
        onCheckedChange={(checked) =>
          onChange({ ...value, crash_reports_enabled: checked === true })}
      />
    </SettingRow>

    {#if locations}
      <div class="rounded-md border p-3 text-xs text-muted-foreground space-y-1">
        <p class="break-all">{$t('settings.v2.privacy_data.locations.settings')}: {locations.settings_file}</p>
        <p class="break-all">{$t('settings.v2.privacy_data.locations.chats_db')}: {locations.chat_db}</p>
        <p class="break-all">RAG DB: {locations.rag_db}</p>
        <p class="break-all">{$t('settings.v2.privacy_data.locations.profile')}: {locations.profile_dir}</p>
      </div>
    {/if}

    <div class="flex flex-wrap gap-2">
      <Button variant="outline" size="sm" onclick={onExportData}>{$t('settings.v2.privacy_data.actions.export')}</Button>
      <Button variant="outline" size="sm" onclick={() => onClearData('chats')}>{$t('settings.v2.privacy_data.actions.clear_chats')}</Button>
      <Button variant="outline" size="sm" onclick={() => onClearData('downloads')}>
        {$t('settings.v2.privacy_data.actions.clear_downloads')}
      </Button>
      <Button variant="destructive" size="sm" onclick={() => onClearData('all')}>
        {$t('settings.v2.privacy_data.actions.factory_reset')}
      </Button>
    </div>
  </Card.Content>
</Card.Root>
