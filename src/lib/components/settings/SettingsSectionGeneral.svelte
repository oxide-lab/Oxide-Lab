<script lang="ts">
  import * as Card from '$lib/components/ui/card';
  import * as Select from '$lib/components/ui/select';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import SettingRow from '$lib/components/settings/SettingRow.svelte';
  import { t } from '$lib/i18n';
  import type { GeneralSettings } from '$lib/types/settings-v2';

  interface Props {
    value: GeneralSettings;
    highlightedSettingId?: string | null;
    onChange: (next: GeneralSettings) => void;
  }

  let { value, highlightedSettingId = null, onChange }: Props = $props();
</script>

<Card.Root>
  <Card.Header>
    <Card.Title>{$t('settings.v2.sections.general.title')}</Card.Title>
    <Card.Description>{$t('settings.v2.sections.general.description')}</Card.Description>
  </Card.Header>
  <Card.Content class="space-y-3">
    <SettingRow
      id="general.locale"
      title={$t('settings.v2.general.locale.title')}
      description={$t('settings.v2.general.locale.description')}
      highlighted={highlightedSettingId === 'general.locale'}
    >
      <Select.Root
        type="single"
        value={value.locale}
        onValueChange={(next) => onChange({ ...value, locale: (next ?? 'en') as GeneralSettings['locale'] })}
      >
        <Select.Trigger class="w-full">{value.locale}</Select.Trigger>
        <Select.Content>
          <Select.Item value="en">{$t('settings.v2.general.locale.en')}</Select.Item>
          <Select.Item value="ru">{$t('settings.v2.general.locale.ru')}</Select.Item>
          <Select.Item value="pt-BR">{$t('settings.v2.general.locale.pt_br')}</Select.Item>
        </Select.Content>
      </Select.Root>
    </SettingRow>

    <SettingRow
      id="general.theme"
      title={$t('settings.v2.general.theme.title')}
      description={$t('settings.v2.general.theme.description')}
      highlighted={highlightedSettingId === 'general.theme'}
    >
      <Select.Root
        type="single"
        value={value.theme}
        onValueChange={(next) => onChange({ ...value, theme: (next ?? 'system') as GeneralSettings['theme'] })}
      >
        <Select.Trigger class="w-full">{value.theme}</Select.Trigger>
        <Select.Content>
          <Select.Item value="system">{$t('settings.v2.general.theme.system')}</Select.Item>
          <Select.Item value="light">{$t('settings.v2.general.theme.light')}</Select.Item>
          <Select.Item value="dark">{$t('settings.v2.general.theme.dark')}</Select.Item>
        </Select.Content>
      </Select.Root>
    </SettingRow>

    <SettingRow
      id="general.auto_update"
      title={$t('settings.v2.general.auto_update.title')}
      description={$t('settings.v2.general.auto_update.description')}
      highlighted={highlightedSettingId === 'general.auto_update'}
      controlPosition="start"
    >
      <Checkbox
        checked={value.auto_update}
        onCheckedChange={(checked) => onChange({ ...value, auto_update: checked === true })}
      />
    </SettingRow>

    <SettingRow
      id="general.launch_on_startup"
      title={$t('settings.v2.general.launch_on_startup.title')}
      description={$t('settings.v2.general.launch_on_startup.description')}
      highlighted={highlightedSettingId === 'general.launch_on_startup'}
      controlPosition="start"
    >
      <Checkbox
        checked={value.launch_on_startup}
        onCheckedChange={(checked) =>
          onChange({ ...value, launch_on_startup: checked === true })}
      />
    </SettingRow>

    <SettingRow
      id="general.search_history_enabled"
      title={$t('settings.v2.general.search_history.title')}
      description={$t('settings.v2.general.search_history.description')}
      highlighted={highlightedSettingId === 'general.search_history_enabled'}
      controlPosition="start"
    >
      <Checkbox
        checked={value.search_history_enabled}
        onCheckedChange={(checked) =>
          onChange({ ...value, search_history_enabled: checked === true })}
      />
    </SettingRow>
  </Card.Content>
</Card.Root>
