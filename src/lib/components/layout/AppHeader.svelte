<script lang="ts">
  import { page } from '$app/state';
  import { t } from '$lib/i18n';
  import { Button } from '$lib/components/ui/button';
  import { CaretDown } from 'phosphor-svelte';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
  import * as Breadcrumb from '$lib/components/ui/breadcrumb';
  import ModelPicker from './ModelPicker.svelte';
  import WindowControls from './WindowControls.svelte';
  import PageTabs from './PageTabs.svelte';
  import SlidersHorizontal from 'phosphor-svelte/lib/SlidersHorizontal';
  import Cube from 'phosphor-svelte/lib/Cube';
  import Gauge from 'phosphor-svelte/lib/Gauge';
  import Cpu from 'phosphor-svelte/lib/Cpu';
  import ChatsCircle from 'phosphor-svelte/lib/ChatsCircle';
  import Globe from 'phosphor-svelte/lib/Globe';
  import Shield from 'phosphor-svelte/lib/Shield';
  import Code from 'phosphor-svelte/lib/Code';
  import Info from 'phosphor-svelte/lib/Info';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import type { SettingsSectionId } from '$lib/types/settings-v2';

  interface Props {
    viewportWidth: number;
    pathname: string;
  }

  let { viewportWidth, pathname }: Props = $props();

  const appWindow = getCurrentWindow();

  const settingsSectionLabelMap: Record<SettingsSectionId, string> = {
    general: 'settings.v2.sections.general.title',
    models_storage: 'settings.v2.sections.models_storage.title',
    performance: 'settings.v2.sections.performance.title',
    hardware: 'settings.v2.sections.hardware.title',
    chat_presets: 'settings.v2.sections.chat_presets.title',
    web_rag: 'settings.v2.sections.web_rag.title',
    privacy_data: 'settings.v2.sections.privacy_data.title',
    developer: 'settings.v2.sections.developer.title',
    about: 'settings.v2.sections.about.title',
  };

  const settingsSectionOrder: SettingsSectionId[] = [
    'general',
    'models_storage',
    'chat_presets',
    'web_rag',
    'privacy_data',
    'developer',
    'about',
    'performance',
    'hardware',
  ];

  const settingsSectionIconMap: Record<SettingsSectionId, any> = {
    general: SlidersHorizontal,
    models_storage: Cube,
    performance: Gauge,
    hardware: Cpu,
    chat_presets: ChatsCircle,
    web_rag: Globe,
    privacy_data: Shield,
    developer: Code,
    about: Info,
  };

  function getSettingsSectionFromQuery(): SettingsSectionId {
    const section = page.url.searchParams.get('section') as SettingsSectionId | null;
    if (!section || !(section in settingsSectionLabelMap)) return 'general';
    return section;
  }

  async function startDragging(event: MouseEvent) {
    const target = event.target as HTMLElement;
    if (target.closest('button, input, textarea, select, a, [data-no-drag]')) {
      event.stopPropagation();
      return;
    }
    await appWindow.startDragging();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="app-header-wrapper" onmousedown={startDragging}>
  <header class="flex items-center justify-between px-2 h-14 bg-background">
    <div class="flex-1 flex items-center justify-start gap-2 sm:gap-4">
      {#if pathname === '/settings'}
        <div class="settings-breadcrumbs" data-no-drag>
          <Breadcrumb.Root>
            <Breadcrumb.List>
              <Breadcrumb.Item>
                <Breadcrumb.Link
                  href="/settings?section=general"
                  class="text-sm font-semibold text-foreground"
                >
                  {$t('settings.v2.page.title')}
                </Breadcrumb.Link>
              </Breadcrumb.Item>

              {#if viewportWidth < 1024}
                {@const section = getSettingsSectionFromQuery()}
                {@const Icon = settingsSectionIconMap[section]}
                <Breadcrumb.Separator />
                <Breadcrumb.Item>
                  <DropdownMenu.Root>
                    <DropdownMenu.Trigger>
                      {#snippet child({ props })}
                        <Button {...props} variant="ghost" size="sm" class="h-7 gap-1 px-2">
                          <Icon class="size-3.5" />
                          {$t(settingsSectionLabelMap[section])}
                          <CaretDown size={12} />
                        </Button>
                      {/snippet}
                    </DropdownMenu.Trigger>
                    <DropdownMenu.Content align="start" sideOffset={6} class="w-56 z-[1400]">
                      {#each settingsSectionOrder as sectionId}
                        {@const SectionIcon = settingsSectionIconMap[sectionId]}
                        <DropdownMenu.Item onSelect={() => {}}>
                          <SectionIcon class="size-4" />
                          {$t(settingsSectionLabelMap[sectionId])}
                        </DropdownMenu.Item>
                      {/each}
                    </DropdownMenu.Content>
                  </DropdownMenu.Root>
                </Breadcrumb.Item>
              {/if}
            </Breadcrumb.List>
          </Breadcrumb.Root>
        </div>
      {/if}

      {#if pathname === '/'}
        <ModelPicker />
      {/if}

      <PageTabs {viewportWidth} {pathname} />
    </div>

    <div class="flex items-center gap-2">
      <WindowControls />
    </div>
  </header>
</div>

<style>
  .app-header-wrapper {
    position: relative;
    height: 3.5rem;
    box-sizing: border-box;
    -webkit-app-region: drag;
    z-index: 100;
    background: var(--background);
    border-bottom: 1px solid var(--border);
  }

  .app-header-wrapper :global(button) {
    -webkit-app-region: no-drag;
  }

  .settings-breadcrumbs {
    display: flex;
    align-items: center;
    min-width: 0;
    margin-left: 12px;
    -webkit-app-region: no-drag;
  }
</style>
