<script lang="ts">
  import { pageTabsList, activePageTab, type TabId } from '$lib/stores/page-tabs.svelte';
  import { t } from '$lib/i18n';
  import * as Breadcrumb from '$lib/components/ui/breadcrumb';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
  import { Button } from '$lib/components/ui/button';
  import * as Tabs from '$lib/components/ui/tabs';
  import { Cube, Globe, ChatsCircle, CaretDown } from 'phosphor-svelte';

  interface Props {
    viewportWidth: number;
    pathname: string;
  }

  let { viewportWidth, pathname }: Props = $props();

  const modelsTabIconMap: Record<string, any> = {
    local: Cube,
    remote: Globe,
    recommendations: ChatsCircle,
  };

  function currentHeaderTabValue(): TabId | '' {
    return $activePageTab || $pageTabsList[0]?.id || '';
  }

  function handleHeaderTabsChange(nextValue: string) {
    if (!nextValue || nextValue === $activePageTab) return;
    activePageTab.set(nextValue as TabId);
  }

  function getCurrentModelsTab() {
    const currentId = currentHeaderTabValue();
    return $pageTabsList.find((tab) => tab.id === currentId) ?? $pageTabsList[0] ?? null;
  }
</script>

{#if pathname === '/models' && $pageTabsList.length > 0}
  {#if viewportWidth < 1024}
    {@const currentModelsTab = getCurrentModelsTab()}
    <div class="settings-breadcrumbs" data-no-drag>
      <Breadcrumb.Root>
        <Breadcrumb.List>
          <Breadcrumb.Item>
            <Breadcrumb.Link href="/models" class="text-sm font-semibold text-foreground">
              {$t('models.title') || 'Models'}
            </Breadcrumb.Link>
          </Breadcrumb.Item>

          {#if currentModelsTab}
            {@const CurrentModelsTabIcon = modelsTabIconMap[currentModelsTab.id] ?? Cube}
            <Breadcrumb.Separator />
            <Breadcrumb.Item>
              <DropdownMenu.Root>
                <DropdownMenu.Trigger>
                  {#snippet child({ props })}
                    <Button {...props} variant="ghost" size="sm" class="h-7 gap-1 px-2">
                      <CurrentModelsTabIcon class="size-3.5" />
                      {currentModelsTab.label}
                      <CaretDown size={12} />
                    </Button>
                  {/snippet}
                </DropdownMenu.Trigger>
                <DropdownMenu.Content align="start" sideOffset={6} class="w-56 z-[1400]">
                  {#each $pageTabsList as tab (tab.id)}
                    {@const ModelsTabIcon = modelsTabIconMap[tab.id] ?? Cube}
                    <DropdownMenu.Item onSelect={() => handleHeaderTabsChange(tab.id)}>
                      <ModelsTabIcon class="size-4" />
                      {tab.label}
                    </DropdownMenu.Item>
                  {/each}
                </DropdownMenu.Content>
              </DropdownMenu.Root>
            </Breadcrumb.Item>
          {/if}
        </Breadcrumb.List>
      </Breadcrumb.Root>
    </div>
  {:else}
    <div class="page-tabs" data-no-drag>
      <Tabs.Root
        value={currentHeaderTabValue()}
        class="page-tabs-root"
        onValueChange={handleHeaderTabsChange}
      >
        <Tabs.List class="page-tabs-list" aria-label="Page tabs">
          {#each $pageTabsList as tab}
            <Tabs.Trigger class="page-tab" value={tab.id}>
              {tab.label}
            </Tabs.Trigger>
          {/each}
        </Tabs.List>
      </Tabs.Root>
    </div>
  {/if}
{/if}

<style>
  .settings-breadcrumbs {
    display: flex;
    align-items: center;
    min-width: 0;
    margin-left: 12px;
    -webkit-app-region: no-drag;
  }

  .page-tabs {
    display: flex;
    align-items: center;
    margin-left: 0.5rem;
    -webkit-app-region: no-drag;
  }

  :global(.page-tabs-root) {
    display: flex;
  }

  :global(.page-tabs-list) {
    display: flex;
    gap: 0.5rem;
    background: transparent;
  }

  :global(.page-tab) {
    padding: 0.4rem 0.9rem;
    background: transparent;
    border-radius: 0.5rem;
    font-size: 0.9rem;
    transition: background 0.2s ease;
  }

  :global(.page-tab:hover) {
    background: var(--accent);
  }

  :global(.page-tab[data-state='active']) {
    background: color-mix(in srgb, var(--primary) 16%, transparent);
    font-weight: 600;
  }
</style>
