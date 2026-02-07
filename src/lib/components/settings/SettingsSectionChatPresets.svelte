<script lang="ts">
  import * as Card from '$lib/components/ui/card';
  import * as Select from '$lib/components/ui/select';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Textarea } from '$lib/components/ui/textarea';
  import { Badge } from '$lib/components/ui/badge';
  import SettingRow from '$lib/components/settings/SettingRow.svelte';
  import { t } from '$lib/i18n';
  import type { ChatPreset, ChatPresetSettings } from '$lib/types/settings-v2';
  import Plus from 'phosphor-svelte/lib/Plus';
  import Trash from 'phosphor-svelte/lib/Trash';
  import UploadSimple from 'phosphor-svelte/lib/UploadSimple';
  import DownloadSimple from 'phosphor-svelte/lib/DownloadSimple';

  interface Props {
    value: ChatPresetSettings;
    highlightedSettingId?: string | null;
    onChange: (next: ChatPresetSettings) => void;
    onApplyPresetToChat: (presetId: string) => void;
  }

  let { value, highlightedSettingId = null, onChange, onApplyPresetToChat }: Props = $props();

  let selectedPresetId = $state(value.default_preset_id);

  $effect(() => {
    if (!value.presets.some((preset) => preset.id === selectedPresetId)) {
      selectedPresetId = value.default_preset_id;
    }
  });

  const selectedPreset = $derived(
    value.presets.find((preset) => preset.id === selectedPresetId) ?? value.presets[0],
  );

  function updatePreset(nextPreset: ChatPreset) {
    onChange({
      ...value,
      presets: value.presets.map((preset) => (preset.id === nextPreset.id ? nextPreset : preset)),
    });
  }

  function createPreset() {
    const base = selectedPreset ?? value.presets[0];
    if (!base) return;
    const id = `preset_${Date.now()}`;
    const nextPreset: ChatPreset = {
      ...structuredClone(base),
      id,
      name: `${base.name} ${$t('settings.v2.chat_presets.copy_suffix')}`,
      builtin: false,
    };
    onChange({
      ...value,
      presets: [...value.presets, nextPreset],
      default_preset_id: value.default_preset_id,
    });
    selectedPresetId = id;
  }

  function removePreset(id: string) {
    const target = value.presets.find((preset) => preset.id === id);
    if (!target || target.builtin) return;
    const nextPresets = value.presets.filter((preset) => preset.id !== id);
    const nextDefault =
      value.default_preset_id === id ? (nextPresets[0]?.id ?? value.default_preset_id) : value.default_preset_id;
    onChange({
      ...value,
      presets: nextPresets,
      default_preset_id: nextDefault,
    });
    selectedPresetId = nextDefault;
  }

  async function importPresets() {
    const picker = document.createElement('input');
    picker.type = 'file';
    picker.accept = '.json,application/json';
    picker.onchange = async () => {
      const file = picker.files?.[0];
      if (!file) return;
      const text = await file.text();
      const payload = JSON.parse(text) as ChatPresetSettings;
      if (!payload.presets?.length) return;
      onChange({
        ...value,
        presets: payload.presets,
        default_preset_id: payload.default_preset_id || payload.presets[0].id,
      });
      selectedPresetId = payload.default_preset_id || payload.presets[0].id;
    };
    picker.click();
  }

  function exportPresets() {
    const blob = new Blob([JSON.stringify(value, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = 'oxide-chat-presets.json';
    anchor.click();
    URL.revokeObjectURL(url);
  }
</script>

<Card.Root>
  <Card.Header>
    <Card.Title>{$t('settings.v2.sections.chat_presets.title')}</Card.Title>
    <Card.Description>{$t('settings.v2.sections.chat_presets.description')}</Card.Description>
  </Card.Header>
  <Card.Content class="space-y-3">
    <SettingRow
      id="chat_presets.default_preset"
      title={$t('settings.v2.chat_presets.default_preset.title')}
      description={$t('settings.v2.chat_presets.default_preset.description')}
      highlighted={highlightedSettingId === 'chat_presets.default_preset'}
    >
      <Select.Root
        type="single"
        value={value.default_preset_id}
        onValueChange={(next) =>
          onChange({ ...value, default_preset_id: next ?? value.default_preset_id })}
      >
        <Select.Trigger class="w-full">{value.default_preset_id}</Select.Trigger>
        <Select.Content>
          {#each value.presets as preset (preset.id)}
            <Select.Item value={preset.id}>{preset.name}</Select.Item>
          {/each}
        </Select.Content>
      </Select.Root>
    </SettingRow>

    <div class="flex flex-wrap gap-2">
      <Button variant="outline" size="sm" onclick={createPreset}>
        <Plus class="mr-1 size-4" />
        {$t('settings.v2.chat_presets.actions.new')}
      </Button>
      <Button variant="outline" size="sm" onclick={importPresets}>
        <UploadSimple class="mr-1 size-4" />
        {$t('settings.v2.chat_presets.actions.import')}
      </Button>
      <Button variant="outline" size="sm" onclick={exportPresets}>
        <DownloadSimple class="mr-1 size-4" />
        {$t('settings.v2.chat_presets.actions.export')}
      </Button>
      {#if selectedPreset}
        <Button variant="secondary" size="sm" onclick={() => onApplyPresetToChat(selectedPreset.id)}>
          {$t('settings.v2.chat_presets.actions.apply')}
        </Button>
      {/if}
    </div>

    {#if selectedPreset}
      <div class="rounded-lg border p-3 space-y-3">
        <div class="flex flex-wrap items-center gap-2">
          <Select.Root
            type="single"
            value={selectedPresetId}
            onValueChange={(next) => (selectedPresetId = next ?? selectedPresetId)}
          >
            <Select.Trigger class="w-full max-w-full sm:w-[260px]">{selectedPreset.name}</Select.Trigger>
            <Select.Content>
              {#each value.presets as preset (preset.id)}
                <Select.Item value={preset.id}>{preset.name}</Select.Item>
              {/each}
            </Select.Content>
          </Select.Root>
          {#if selectedPreset.builtin}
            <Badge variant="secondary">{$t('settings.v2.chat_presets.builtin')}</Badge>
          {:else}
            <Button variant="destructive" size="sm" onclick={() => removePreset(selectedPreset.id)}>
              <Trash class="mr-1 size-4" />
              {$t('settings.v2.chat_presets.actions.delete')}
            </Button>
          {/if}
        </div>

        <SettingRow
          id="chat_presets.name"
          title={$t('settings.v2.chat_presets.name.title')}
          description={$t('settings.v2.chat_presets.name.description')}
        >
          <Input
            value={selectedPreset.name}
            oninput={(e) => updatePreset({ ...selectedPreset, name: (e.currentTarget as HTMLInputElement).value })}
          />
        </SettingRow>

        <SettingRow
          id="chat_presets.system_prompt"
          title={$t('settings.v2.chat_presets.system_prompt.title')}
          description={$t('settings.v2.chat_presets.system_prompt.description')}
        >
          <Textarea
            rows={4}
            value={selectedPreset.system_prompt}
            oninput={(e) =>
              updatePreset({ ...selectedPreset, system_prompt: (e.currentTarget as HTMLTextAreaElement).value })}
          />
        </SettingRow>

        <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
          <SettingRow
            id="chat_presets.temperature"
            title={$t('settings.v2.chat_presets.temperature.title')}
            description={$t('settings.v2.chat_presets.temperature.description')}
            highlighted={highlightedSettingId === 'chat_presets.temperature'}
          >
            <Input
              type="number"
              step="0.01"
              min="0"
              max="2"
              value={selectedPreset.sampling.temperature}
              oninput={(e) =>
                updatePreset({
                  ...selectedPreset,
                  sampling: { ...selectedPreset.sampling, temperature: Number((e.currentTarget as HTMLInputElement).value) || 0 },
                })}
            />
          </SettingRow>
          <SettingRow
            id="chat_presets.top_p"
            title={$t('settings.v2.chat_presets.top_p.title')}
            description={$t('settings.v2.chat_presets.top_p.description')}
          >
            <Input
              type="number"
              step="0.01"
              min="0"
              max="1"
              value={selectedPreset.sampling.top_p}
              oninput={(e) =>
                updatePreset({
                  ...selectedPreset,
                  sampling: { ...selectedPreset.sampling, top_p: Number((e.currentTarget as HTMLInputElement).value) || 0 },
                })}
            />
          </SettingRow>
          <SettingRow
            id="chat_presets.top_k"
            title={$t('settings.v2.chat_presets.top_k.title')}
            description={$t('settings.v2.chat_presets.top_k.description')}
          >
            <Input
              type="number"
              step="1"
              min="1"
              value={selectedPreset.sampling.top_k}
              oninput={(e) =>
                updatePreset({
                  ...selectedPreset,
                  sampling: { ...selectedPreset.sampling, top_k: Number((e.currentTarget as HTMLInputElement).value) || 1 },
                })}
            />
          </SettingRow>
          <SettingRow
            id="chat_presets.repeat_penalty"
            title={$t('settings.v2.chat_presets.repeat_penalty.title')}
            description={$t('settings.v2.chat_presets.repeat_penalty.description')}
          >
            <Input
              type="number"
              step="0.01"
              min="0.1"
              value={selectedPreset.sampling.repeat_penalty}
              oninput={(e) =>
                updatePreset({
                  ...selectedPreset,
                  sampling: { ...selectedPreset.sampling, repeat_penalty: Number((e.currentTarget as HTMLInputElement).value) || 1 },
                })}
            />
          </SettingRow>
          <SettingRow
            id="chat_presets.max_tokens"
            title={$t('settings.v2.chat_presets.max_tokens.title')}
            description={$t('settings.v2.chat_presets.max_tokens.description')}
          >
            <Input
              type="number"
              step="1"
              min="1"
              value={selectedPreset.sampling.max_tokens}
              oninput={(e) =>
                updatePreset({
                  ...selectedPreset,
                  sampling: { ...selectedPreset.sampling, max_tokens: Number((e.currentTarget as HTMLInputElement).value) || 512 },
                })}
            />
          </SettingRow>
          <SettingRow
            id="chat_presets.context"
            title={$t('settings.v2.chat_presets.context.title')}
            description={$t('settings.v2.chat_presets.context.description')}
          >
            <Input
              type="number"
              step="1"
              min="512"
              value={selectedPreset.context}
              oninput={(e) => updatePreset({ ...selectedPreset, context: Number((e.currentTarget as HTMLInputElement).value) || 4096 })}
            />
          </SettingRow>
        </div>
      </div>
    {/if}
  </Card.Content>
</Card.Root>
