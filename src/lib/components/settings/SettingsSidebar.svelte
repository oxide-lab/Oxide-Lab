<script lang="ts">
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import { t } from '$lib/i18n';
  import { cn } from '$lib/utils';
  import type { SettingsSectionId } from '$lib/types/settings-v2';

  interface Section {
    id: SettingsSectionId;
    label: string;
    icon: any;
  }

  interface Props {
    sections: Section[];
    activeSection: SettingsSectionId;
    dirtyCounts: Record<SettingsSectionId, number>;
    onSelect: (id: SettingsSectionId) => void;
    class?: string;
  }

  let { sections, activeSection, dirtyCounts, onSelect, class: className = '' }: Props = $props();
</script>

<aside class={cn('w-full rounded-xl border bg-card p-2', className)}>
  <nav aria-label="Settings sections" class="space-y-1">
    {#each sections as section (section.id)}
      <Button
        variant="ghost"
        class={cn(
          'h-10 w-full justify-start gap-2 px-3',
          activeSection === section.id && 'bg-muted text-foreground',
        )}
        aria-current={activeSection === section.id ? 'page' : undefined}
        onclick={() => onSelect(section.id)}
      >
        <section.icon class="size-4" />
        <span class="flex-1 text-left text-sm">{section.label}</span>
        {#if dirtyCounts[section.id] > 0}
          <Badge variant="secondary" class="text-[10px]">
            {$t('settings.v2.sidebar.changed', { count: dirtyCounts[section.id] })}
          </Badge>
        {/if}
      </Button>
    {/each}
  </nav>
</aside>
