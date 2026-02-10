<script lang="ts">
  /**
   * App Sidebar Component
   * Simple sidebar based on shadcn-svelte blocks.
   */
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import * as Sidebar from '$lib/components/ui/sidebar/index';
  import { useSidebar } from '$lib/components/ui/sidebar/context.svelte.js';

  // Phosphor Icons
  import Database from 'phosphor-svelte/lib/Database';
  import Gear from 'phosphor-svelte/lib/Gear';
  import Code from 'phosphor-svelte/lib/Code';
  import ChartLine from 'phosphor-svelte/lib/ChartLine';
  import ArrowCircleDown from 'phosphor-svelte/lib/ArrowCircleDown';
  import Plus from 'phosphor-svelte/lib/Plus';
  import SidebarSimple from 'phosphor-svelte/lib/SidebarSimple';
  import CaretDown from 'phosphor-svelte/lib/CaretDown';
  import DotsThreeOutline from 'phosphor-svelte/lib/DotsThreeOutline';
  import ShareNetwork from 'phosphor-svelte/lib/ShareNetwork';
  import PencilSimple from 'phosphor-svelte/lib/PencilSimple';
  import Trash from 'phosphor-svelte/lib/Trash';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';

  // Stores
  import {
    activeDownloads,
    downloadsLoaded,
    ensureDownloadManager,
  } from '$lib/stores/download-manager';
  import { experimentalFeatures } from '$lib/stores/experimental-features.svelte';
  import { t, locale } from '$lib/i18n';
  import { chatHistory, groupedSessions, currentSession } from '$lib/stores/chat-history';

  // Get sidebar state
  const sidebar = useSidebar();

  // Get current locale
  let currentLocale = $derived($locale);

  // Navigation items
  const baseNavigationItems = $derived([
    {
      id: 'models',
      title: currentLocale ? $t('sidebar.navigation.models') : 'Model Manager',
      icon: Database,
      path: '/models',
    },
    {
      id: 'settings',
      title: currentLocale ? $t('sidebar.navigation.settings') : 'Settings',
      icon: Gear,
      path: '/settings',
    },
    {
      id: 'api',
      title: currentLocale ? $t('sidebar.navigation.api') : 'API',
      icon: Code,
      path: '/api',
    },
  ] as const);

  const experimentalNavigationItems = $derived([
    {
      id: 'performance',
      title: currentLocale ? $t('sidebar.navigation.performance') : 'Performance',
      icon: ChartLine,
      path: '/performance',
    },
  ] as const);

  const navigationItems = $derived(
    experimentalFeatures.enabled
      ? [...experimentalNavigationItems, ...baseNavigationItems]
      : baseNavigationItems,
  );

  interface Props {
    onOpenDownloads?: () => void;
  }

  let { onOpenDownloads }: Props = $props();

  let currentPath = $derived(page.url.pathname);
  let isSettingsAbout = $derived(
    page.url.pathname === '/settings' && page.url.searchParams.get('section') === 'about',
  );
  let hideLeftHeaderButton = $derived(isSettingsAbout && sidebar.state !== 'collapsed');
  let hasActiveDownloads = $derived($downloadsLoaded && $activeDownloads.length > 0);

  // Translated labels
  let newChatLabel = $derived(currentLocale ? $t('sidebar.newChat') : 'New Chat');
  let todayLabel = $derived(currentLocale ? $t('sidebar.groups.today') : 'Today');
  let thisWeekLabel = $derived(currentLocale ? $t('sidebar.groups.thisWeek') : 'This Week');
  let olderLabel = $derived(currentLocale ? $t('sidebar.groups.older') : 'Older');
  let downloadsLabel = $derived(currentLocale ? $t('sidebar.footer.downloads') : 'Downloads');
  let chatsLabel = $derived(currentLocale ? $t('sidebar.chats') : 'Chats');
  let renameLabel = $derived(currentLocale ? $t('sidebar.rename') : 'Rename');
  let deleteLabel = $derived(currentLocale ? $t('sidebar.delete') : 'Delete');
  let deleteAllChatsLabel = $derived('Delete all chats');
  let shareChatsLabel = $derived('Share');
  let chatsExpanded = $state(true);
  let menuOpenSessionId = $state<string | null>(null);
  let hoveredSessionId = $state<string | null>(null);

  // Check if any groups have sessions
  let hasAnySessions = $derived(
    $groupedSessions.today.length > 0 ||
      $groupedSessions.thisWeek.length > 0 ||
      $groupedSessions.older.length > 0,
  );

  onMount(() => {
    void ensureDownloadManager();
  });

  async function confirmDestructive(message: string): Promise<boolean> {
    try {
      const { confirm } = await import('@tauri-apps/plugin-dialog');
      return await confirm(message, {
        title: 'Confirm',
        kind: 'warning',
        okLabel: 'Delete',
        cancelLabel: 'Cancel',
      });
    } catch {
      if (typeof window === 'undefined') return false;
      return window.confirm(message);
    }
  }

  function navigateTo(path: string) {
    if (currentPath === path) return;
    goto(path);
  }

  async function handleNewChat() {
    const _id = await chatHistory.createSession();
    if (currentPath !== '/') goto('/');
  }

  function handleLoadSession(id: string) {
    chatHistory.loadSession(id);
    if (currentPath !== '/') goto('/');
  }

  function handleDownloadsClick() {
    onOpenDownloads?.();
  }

  async function handleRenameSession(id: string, currentTitle: string) {
    const next = prompt(renameLabel, currentTitle || 'Untitled Chat');
    if (!next) return;
    const trimmed = next.trim();
    if (!trimmed) return;
    await chatHistory.renameSession(id, trimmed);
  }

  async function handleDeleteSession(id: string) {
    const ok = await confirmDestructive(deleteLabel + '?');
    if (!ok) return;
    await chatHistory.deleteSession(id);
  }

  async function handleDeleteAllChats() {
    const ok = await confirmDestructive(deleteAllChatsLabel + '?');
    if (!ok) return;
    await chatHistory.clearAll();
  }

  async function handleShareChats() {
    const sessions = [
      ...$groupedSessions.today,
      ...$groupedSessions.thisWeek,
      ...$groupedSessions.older,
    ];
    const payload = sessions
      .map((session, index) => `${index + 1}. ${session.title || 'Untitled Chat'}`)
      .join('\n');

    if (!payload) return;

    try {
      await navigator.clipboard.writeText(payload);
    } catch (err) {
      console.error('Failed to copy chats to clipboard:', err);
    }
  }

  function handleSessionMenuOpenChange(id: string, open: boolean) {
    if (open) {
      menuOpenSessionId = id;
      return;
    }

    if (menuOpenSessionId === id) {
      menuOpenSessionId = null;
    }
  }

  function handleSessionHover(id: string, hovering: boolean) {
    if (hovering) {
      hoveredSessionId = id;
      return;
    }

    if (hoveredSessionId === id) {
      hoveredSessionId = null;
    }
  }
</script>

<Sidebar.Root collapsible="icon">
  <!-- Header with brand -->
  <Sidebar.Header class="!flex-row h-14 items-center justify-between p-2">
    <!-- Brand button -->
    {#if hideLeftHeaderButton}
      <div class="size-10" aria-hidden="true"></div>
    {:else}
      <button
        type="button"
        class="brand-button flex items-center rounded-md py-2 pl-2.5 pr-2.5 size-10 hover:bg-sidebar-accent transition-colors"
        onclick={() => {
          if (sidebar.state === 'collapsed') {
            sidebar.toggle();
          } else {
            goto('/');
          }
        }}
      >
        <span class="brand-icon-wrapper relative w-5 h-5 min-w-5 min-h-5 shrink-0">
          {#if isSettingsAbout}
            <SidebarSimple
              size={20}
              weight="regular"
              class="w-5 h-5 absolute inset-0 text-sidebar-foreground"
            />
          {:else}
            <img
              src="/icon.svg"
              alt="Oxide Lab"
              class="brand-icon-default w-5 h-5 absolute inset-0"
            />
            <SidebarSimple
              size={20}
              weight="regular"
              class="brand-icon-hover w-5 h-5 absolute inset-0 text-sidebar-foreground"
            />
          {/if}
        </span>
      </button>
    {/if}

    <!-- Toggle button (only when expanded) -->
    {#if sidebar.state !== 'collapsed'}
      <button
        type="button"
        class="flex items-center justify-center rounded-md size-10 hover:bg-sidebar-accent transition-colors text-sidebar-foreground"
        onclick={() => sidebar.toggle()}
      >
        <SidebarSimple size={20} weight="regular" />
      </button>
    {/if}
  </Sidebar.Header>

  <Sidebar.Content class="overflow-hidden [scrollbar-gutter:auto]">
    <!-- New Chat -->
    <Sidebar.Group>
      <Sidebar.Menu>
        <Sidebar.MenuItem>
          <Sidebar.MenuButton tooltipContent={newChatLabel}>
            {#snippet child({ props })}
              <button {...props} onclick={handleNewChat}>
                <Plus size={16} weight="bold" />
                <span>{newChatLabel}</span>
              </button>
            {/snippet}
          </Sidebar.MenuButton>
        </Sidebar.MenuItem>
      </Sidebar.Menu>
    </Sidebar.Group>

    <!-- Navigation -->
    <Sidebar.Group>
      <Sidebar.Menu>
        {#each navigationItems as item}
          {@const Icon = item.icon}
          <Sidebar.MenuItem>
            <Sidebar.MenuButton tooltipContent={item.title} isActive={currentPath === item.path}>
              {#snippet child({ props })}
                <button {...props} onclick={() => navigateTo(item.path)}>
                  <Icon size={16} weight="regular" />
                  <span>{item.title}</span>
                </button>
              {/snippet}
            </Sidebar.MenuButton>
          </Sidebar.MenuItem>
        {/each}
      </Sidebar.Menu>
    </Sidebar.Group>

    <!-- Chats History (hidden when collapsed) -->
    {#if sidebar.state !== 'collapsed'}
      <Sidebar.Group
        class="group/chats-collapsible flex-1 min-h-0 flex flex-col !p-0 !pb-2 translate-x-2"
      >
        <Sidebar.GroupLabel>
          {#snippet child({ props })}
            <div class="relative w-full">
              <button
                {...props}
                type="button"
                class="flex w-full items-center rounded-md px-2 py-1.5"
                onclick={() => (chatsExpanded = !chatsExpanded)}
                aria-expanded={chatsExpanded}
              >
                <span class="inline-flex items-center gap-1.5">
                  <span>{chatsLabel}</span>
                  <CaretDown
                    size={14}
                    class="transition-transform duration-150"
                    style={`transform: rotate(${chatsExpanded ? '0deg' : '-90deg'})`}
                  />
                </span>
              </button>
              <DropdownMenu.Root>
                <DropdownMenu.Trigger>
                  {#snippet child({ props: dropdownTriggerProps })}
                    <button
                      {...dropdownTriggerProps}
                      type="button"
                      class="pointer-events-none absolute right-2 top-1/2 inline-flex h-8 w-8 -translate-y-1/2 items-center justify-center rounded-md p-0 text-sidebar-foreground/70 opacity-0 transition-opacity hover:bg-sidebar-accent/70 hover:text-sidebar-foreground group-hover/chats-collapsible:pointer-events-auto group-hover/chats-collapsible:opacity-100 data-[state=open]:pointer-events-auto data-[state=open]:opacity-100 data-[state=open]:bg-sidebar-accent/70 data-[state=open]:text-sidebar-foreground"
                      aria-label="Chat actions"
                      onclick={(event) => event.stopPropagation()}
                    >
                      <DotsThreeOutline size={16} weight="fill" />
                    </button>
                  {/snippet}
                </DropdownMenu.Trigger>
                <DropdownMenu.Content align="end" sideOffset={6}>
                  <DropdownMenu.Item onSelect={handleShareChats}>
                    <ShareNetwork class="size-4" weight="fill" />
                    <span>{shareChatsLabel}</span>
                  </DropdownMenu.Item>
                  <DropdownMenu.Item
                    class="text-destructive focus:text-destructive"
                    onSelect={handleDeleteAllChats}
                  >
                    <Trash class="size-4" />
                    <span>{deleteAllChatsLabel}</span>
                  </DropdownMenu.Item>
                </DropdownMenu.Content>
              </DropdownMenu.Root>
            </div>
          {/snippet}
        </Sidebar.GroupLabel>

        {#if chatsExpanded}
          <Sidebar.GroupContent
            class="chat-history-scrollbar flex-1 min-h-0 overflow-y-auto overflow-x-hidden -mr-2"
          >
            {#if !hasAnySessions}
              <div class="pl-2 pr-2 pt-2 text-sm text-sidebar-foreground/50">No chats yet</div>
            {/if}
            <!-- Today -->
            {#if $groupedSessions.today.length > 0}
              <div class="mb-1.5 pl-1.5 pr-2 pt-2 text-sm font-medium text-sidebar-foreground/50">
                {todayLabel}
              </div>
              <Sidebar.Menu class="pl-1 pr-2 -mr-1 gap-0.5">
                {#each $groupedSessions.today as session (session.id)}
                  <Sidebar.MenuItem
                    class="group/menu-item"
                    onmouseenter={() => handleSessionHover(session.id, true)}
                    onmouseleave={() => handleSessionHover(session.id, false)}
                  >
                    <Sidebar.MenuButton
                      isActive={$currentSession?.id === session.id}
                      size="sm"
                      class={`chat-session-button !h-auto rounded-lg px-[11px] py-[6px] pr-9 text-sm font-normal hover:bg-sidebar-accent/70 hover:text-sidebar-foreground data-[state=open]:hover:bg-sidebar-accent/70 data-[active=true]:font-normal ${
                        $currentSession?.id === session.id ||
                        hoveredSessionId === session.id ||
                        menuOpenSessionId === session.id
                          ? '!bg-sidebar-accent/70 text-sidebar-foreground'
                          : ''
                      }`}
                    >
                      {#snippet child({ props })}
                        <button {...props} onclick={() => handleLoadSession(session.id)}>
                          <span class="block h-5 min-w-0 truncate leading-5"
                            >{session.title || 'Untitled Chat'}</span
                          >
                        </button>
                      {/snippet}
                    </Sidebar.MenuButton>
                    <DropdownMenu.Root
                      onOpenChange={(open) => handleSessionMenuOpenChange(session.id, open)}
                    >
                      <DropdownMenu.Trigger>
                        {#snippet child({ props })}
                          <Sidebar.MenuAction
                            {...props}
                            showOnHover={false}
                            class="right-2.5 !top-1/2 h-5 w-5 !-translate-y-1/2 rounded-none p-0 text-sidebar-foreground/70 opacity-0 transition-opacity duration-75 hover:bg-transparent hover:text-sidebar-foreground/70 focus-visible:ring-0 group-hover/menu-item:opacity-100 data-[state=open]:opacity-100 peer-data-[active=true]/menu-button:!opacity-100"
                          >
                            <DotsThreeOutline size={14} weight="fill" />
                          </Sidebar.MenuAction>
                        {/snippet}
                      </DropdownMenu.Trigger>
                      <DropdownMenu.Content align="end" sideOffset={6}>
                        <DropdownMenu.Item
                          onSelect={() => handleRenameSession(session.id, session.title ?? '')}
                        >
                          <PencilSimple class="size-4" />
                          <span>{renameLabel}</span>
                        </DropdownMenu.Item>
                        <DropdownMenu.Item
                          class="text-destructive focus:text-destructive"
                          onSelect={() => handleDeleteSession(session.id)}
                        >
                          <Trash class="size-4" />
                          <span>{deleteLabel}</span>
                        </DropdownMenu.Item>
                      </DropdownMenu.Content>
                    </DropdownMenu.Root>
                  </Sidebar.MenuItem>
                {/each}
              </Sidebar.Menu>
            {/if}

            <!-- This Week -->
            {#if $groupedSessions.thisWeek.length > 0}
              <div class="mb-1.5 mt-4 pl-1.5 pr-2 text-sm font-medium text-sidebar-foreground/50">
                {thisWeekLabel}
              </div>
              <Sidebar.Menu class="pl-1 pr-2 -mr-1 gap-0.5">
                {#each $groupedSessions.thisWeek as session (session.id)}
                  <Sidebar.MenuItem
                    class="group/menu-item"
                    onmouseenter={() => handleSessionHover(session.id, true)}
                    onmouseleave={() => handleSessionHover(session.id, false)}
                  >
                    <Sidebar.MenuButton
                      isActive={$currentSession?.id === session.id}
                      size="sm"
                      class={`chat-session-button !h-auto rounded-lg px-[11px] py-[6px] pr-9 text-sm font-normal hover:bg-sidebar-accent/70 hover:text-sidebar-foreground data-[state=open]:hover:bg-sidebar-accent/70 data-[active=true]:font-normal ${
                        $currentSession?.id === session.id ||
                        hoveredSessionId === session.id ||
                        menuOpenSessionId === session.id
                          ? '!bg-sidebar-accent/70 text-sidebar-foreground'
                          : ''
                      }`}
                    >
                      {#snippet child({ props })}
                        <button {...props} onclick={() => handleLoadSession(session.id)}>
                          <span class="block h-5 min-w-0 truncate leading-5"
                            >{session.title || 'Untitled Chat'}</span
                          >
                        </button>
                      {/snippet}
                    </Sidebar.MenuButton>
                    <DropdownMenu.Root
                      onOpenChange={(open) => handleSessionMenuOpenChange(session.id, open)}
                    >
                      <DropdownMenu.Trigger>
                        {#snippet child({ props })}
                          <Sidebar.MenuAction
                            {...props}
                            showOnHover={false}
                            class="right-2.5 !top-1/2 h-5 w-5 !-translate-y-1/2 rounded-none p-0 text-sidebar-foreground/70 opacity-0 transition-opacity duration-75 hover:bg-transparent hover:text-sidebar-foreground/70 focus-visible:ring-0 group-hover/menu-item:opacity-100 data-[state=open]:opacity-100 peer-data-[active=true]/menu-button:!opacity-100"
                          >
                            <DotsThreeOutline size={14} weight="fill" />
                          </Sidebar.MenuAction>
                        {/snippet}
                      </DropdownMenu.Trigger>
                      <DropdownMenu.Content align="end" sideOffset={6}>
                        <DropdownMenu.Item
                          onSelect={() => handleRenameSession(session.id, session.title ?? '')}
                        >
                          <PencilSimple class="size-4" />
                          <span>{renameLabel}</span>
                        </DropdownMenu.Item>
                        <DropdownMenu.Item
                          class="text-destructive focus:text-destructive"
                          onSelect={() => handleDeleteSession(session.id)}
                        >
                          <Trash class="size-4" />
                          <span>{deleteLabel}</span>
                        </DropdownMenu.Item>
                      </DropdownMenu.Content>
                    </DropdownMenu.Root>
                  </Sidebar.MenuItem>
                {/each}
              </Sidebar.Menu>
            {/if}

            <!-- Older -->
            {#if $groupedSessions.older.length > 0}
              <div class="mb-1.5 mt-4 pl-1.5 pr-2 text-sm font-medium text-sidebar-foreground/50">
                {olderLabel}
              </div>
              <Sidebar.Menu class="pl-1 pr-2 -mr-1 gap-0.5">
                {#each $groupedSessions.older as session (session.id)}
                  <Sidebar.MenuItem
                    class="group/menu-item"
                    onmouseenter={() => handleSessionHover(session.id, true)}
                    onmouseleave={() => handleSessionHover(session.id, false)}
                  >
                    <Sidebar.MenuButton
                      isActive={$currentSession?.id === session.id}
                      size="sm"
                      class={`chat-session-button !h-auto rounded-lg px-[11px] py-[6px] pr-9 text-sm font-normal hover:bg-sidebar-accent/70 hover:text-sidebar-foreground data-[state=open]:hover:bg-sidebar-accent/70 data-[active=true]:font-normal ${
                        $currentSession?.id === session.id ||
                        hoveredSessionId === session.id ||
                        menuOpenSessionId === session.id
                          ? '!bg-sidebar-accent/70 text-sidebar-foreground'
                          : ''
                      }`}
                    >
                      {#snippet child({ props })}
                        <button {...props} onclick={() => handleLoadSession(session.id)}>
                          <span class="block h-5 min-w-0 truncate leading-5"
                            >{session.title || 'Untitled Chat'}</span
                          >
                        </button>
                      {/snippet}
                    </Sidebar.MenuButton>
                    <DropdownMenu.Root
                      onOpenChange={(open) => handleSessionMenuOpenChange(session.id, open)}
                    >
                      <DropdownMenu.Trigger>
                        {#snippet child({ props })}
                          <Sidebar.MenuAction
                            {...props}
                            showOnHover={false}
                            class="right-2.5 !top-1/2 h-5 w-5 !-translate-y-1/2 rounded-none p-0 text-sidebar-foreground/70 opacity-0 transition-opacity duration-75 hover:bg-transparent hover:text-sidebar-foreground/70 focus-visible:ring-0 group-hover/menu-item:opacity-100 data-[state=open]:opacity-100 peer-data-[active=true]/menu-button:!opacity-100"
                          >
                            <DotsThreeOutline size={14} weight="fill" />
                          </Sidebar.MenuAction>
                        {/snippet}
                      </DropdownMenu.Trigger>
                      <DropdownMenu.Content align="end" sideOffset={6}>
                        <DropdownMenu.Item
                          onSelect={() => handleRenameSession(session.id, session.title ?? '')}
                        >
                          <PencilSimple class="size-4" />
                          <span>{renameLabel}</span>
                        </DropdownMenu.Item>
                        <DropdownMenu.Item
                          class="text-destructive focus:text-destructive"
                          onSelect={() => handleDeleteSession(session.id)}
                        >
                          <Trash class="size-4" />
                          <span>{deleteLabel}</span>
                        </DropdownMenu.Item>
                      </DropdownMenu.Content>
                    </DropdownMenu.Root>
                  </Sidebar.MenuItem>
                {/each}
              </Sidebar.Menu>
            {/if}
          </Sidebar.GroupContent>
        {/if}
      </Sidebar.Group>
    {/if}
  </Sidebar.Content>

  <!-- Footer -->
  <Sidebar.Footer>
    <Sidebar.Menu>
      <Sidebar.MenuItem>
        <Sidebar.MenuButton tooltipContent={downloadsLabel} isActive={hasActiveDownloads}>
          {#snippet child({ props })}
            <button type="button" {...props} onclick={handleDownloadsClick}>
              <ArrowCircleDown size={16} weight="regular" />
              <span>{downloadsLabel}</span>
            </button>
          {/snippet}
        </Sidebar.MenuButton>
      </Sidebar.MenuItem>
    </Sidebar.Menu>
  </Sidebar.Footer>
</Sidebar.Root>

<style>
  :global(.brand-icon-hover) {
    opacity: 0;
    transition: opacity 0.15s ease;
  }

  :global(.brand-icon-default) {
    opacity: 1;
    transition: opacity 0.15s ease;
  }

  /* Hover effect only when collapsed */
  :global([data-state='collapsed']) .brand-button:hover :global(.brand-icon-hover) {
    opacity: 1;
  }

  :global([data-state='collapsed']) .brand-button:hover :global(.brand-icon-default) {
    opacity: 0;
  }

  .chat-history-scrollbar {
    scrollbar-width: thin;
    scrollbar-color: rgba(0, 0, 0, 0.1) transparent;
  }

  .chat-history-scrollbar::-webkit-scrollbar {
    width: 8px;
    height: 8px;
    background-color: transparent;
  }

  .chat-history-scrollbar::-webkit-scrollbar-thumb {
    background-color: rgba(0, 0, 0, 0.1);
    border-radius: 9999px;
  }

  .chat-history-scrollbar::-webkit-scrollbar-thumb:hover {
    background-color: rgba(0, 0, 0, 0.2);
  }

  :global(.dark) .chat-history-scrollbar {
    scrollbar-color: rgba(255, 255, 255, 0.1) transparent;
  }

  :global(.dark) .chat-history-scrollbar::-webkit-scrollbar-thumb {
    background-color: rgba(255, 255, 255, 0.1);
  }

  :global(.dark) .chat-history-scrollbar::-webkit-scrollbar-thumb:hover {
    background-color: rgba(255, 255, 255, 0.2);
  }
</style>
