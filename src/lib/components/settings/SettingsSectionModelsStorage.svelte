<script lang="ts">
  import * as Card from '$lib/components/ui/card';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Input } from '$lib/components/ui/input';
  import { Button } from '$lib/components/ui/button';
  import { goto } from '$app/navigation';
  import SettingRow from '$lib/components/settings/SettingRow.svelte';
  import { t } from '$lib/i18n';
  import type { ModelsStorageSettings } from '$lib/types/settings-v2';

  interface Props {
    value: ModelsStorageSettings;
    highlightedSettingId?: string | null;
    onChange: (next: ModelsStorageSettings) => void;
  }

  let { value, highlightedSettingId = null, onChange }: Props = $props();

  let modelsDir = $state(value.models_dir ?? '');
  let cacheDir = $state(value.cache_dir ?? '');

  $effect(() => {
    modelsDir = value.models_dir ?? '';
    cacheDir = value.cache_dir ?? '';
  });

  function commit() {
    onChange({
      ...value,
      models_dir: modelsDir.trim() ? modelsDir.trim() : null,
      cache_dir: cacheDir.trim() ? cacheDir.trim() : null,
    });
  }
</script>

<Card.Root>
  <Card.Header>
    <Card.Title>{$t('settings.v2.sections.models_storage.title')}</Card.Title>
    <Card.Description>{$t('settings.v2.sections.models_storage.description')}</Card.Description>
  </Card.Header>
  <Card.Content class="space-y-3">
    <SettingRow
      id="models_storage.models_dir"
      title={$t('settings.v2.models_storage.models_dir.title')}
      description={$t('settings.v2.models_storage.models_dir.description')}
      highlighted={highlightedSettingId === 'models_storage.models_dir'}
    >
      <Input bind:value={modelsDir} placeholder={$t('settings.v2.models_storage.models_dir.placeholder')} onblur={commit} />
    </SettingRow>

    <SettingRow
      id="models_storage.cache_dir"
      title={$t('settings.v2.models_storage.cache_dir.title')}
      description={$t('settings.v2.models_storage.cache_dir.description')}
      highlighted={highlightedSettingId === 'models_storage.cache_dir'}
    >
      <Input bind:value={cacheDir} placeholder={$t('settings.v2.models_storage.cache_dir.placeholder')} onblur={commit} />
    </SettingRow>

    <SettingRow
      id="models_storage.model_selector_search"
      title={$t('settings.v2.models_storage.model_selector_search.title')}
      description={$t('settings.v2.models_storage.model_selector_search.description')}
      highlighted={highlightedSettingId === 'models_storage.model_selector_search'}
      controlPosition="start"
    >
      <Checkbox
        checked={value.model_selector_search}
        onCheckedChange={(checked) =>
          onChange({ ...value, model_selector_search: checked === true })}
      />
    </SettingRow>

    <div class="flex gap-2">
      <Button variant="outline" size="sm" onclick={() => goto('/models')}>{$t('settings.v2.models_storage.links.models')}</Button>
      <Button variant="outline" size="sm" onclick={() => goto('/api')}>{$t('settings.v2.models_storage.links.api')}</Button>
    </div>
  </Card.Content>
</Card.Root>
