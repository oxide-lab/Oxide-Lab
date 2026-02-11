<script lang="ts">
  import * as Sidebar from '$lib/components/ui/sidebar/index';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
  import DotsThreeOutline from 'phosphor-svelte/lib/DotsThreeOutline';
  import PencilSimple from 'phosphor-svelte/lib/PencilSimple';
  import Trash from 'phosphor-svelte/lib/Trash';
  import type { ChatSession } from '$lib/stores/chat-history';

  interface Props {
    title: string;
    sessions: ChatSession[];
    currentSessionId: string | null;
    hoveredSessionId: string | null;
    menuOpenSessionId: string | null;
    renameLabel: string;
    deleteLabel: string;
    withTopMargin?: boolean;
    onHover: (id: string, hovering: boolean) => void;
    onMenuOpenChange: (id: string, open: boolean) => void;
    onLoadSession: (id: string) => void;
    onRenameSession: (id: string, currentTitle: string) => void | Promise<void>;
    onDeleteSession: (id: string) => void | Promise<void>;
  }

  let {
    title,
    sessions,
    currentSessionId,
    hoveredSessionId,
    menuOpenSessionId,
    renameLabel,
    deleteLabel,
    withTopMargin = false,
    onHover,
    onMenuOpenChange,
    onLoadSession,
    onRenameSession,
    onDeleteSession,
  }: Props = $props();
</script>

{#if sessions.length > 0}
  <div class={`mb-1.5 pl-1.5 pr-2 text-sm font-medium text-sidebar-foreground/50 ${withTopMargin ? 'mt-4' : 'pt-2'}`}>
    {title}
  </div>
  <Sidebar.Menu class="pl-1 pr-2 -mr-1 gap-0.5">
    {#each sessions as session (session.id)}
      <Sidebar.MenuItem
        class="group/menu-item"
        onmouseenter={() => onHover(session.id, true)}
        onmouseleave={() => onHover(session.id, false)}
      >
        <Sidebar.MenuButton
          isActive={currentSessionId === session.id}
          size="sm"
          class={`chat-session-button !h-auto rounded-lg px-[11px] py-[6px] pr-9 text-sm font-normal hover:bg-sidebar-accent/70 hover:text-sidebar-foreground data-[state=open]:hover:bg-sidebar-accent/70 data-[active=true]:font-normal ${
            currentSessionId === session.id ||
            hoveredSessionId === session.id ||
            menuOpenSessionId === session.id
              ? '!bg-sidebar-accent/70 text-sidebar-foreground'
              : ''
          }`}
        >
          {#snippet child({ props })}
            <button {...props} onclick={() => onLoadSession(session.id)}>
              <span class="block h-5 min-w-0 truncate leading-5">{session.title || 'Untitled Chat'}</span>
            </button>
          {/snippet}
        </Sidebar.MenuButton>
        <DropdownMenu.Root onOpenChange={(open) => onMenuOpenChange(session.id, open)}>
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
            <DropdownMenu.Item onSelect={() => onRenameSession(session.id, session.title ?? '')}>
              <PencilSimple class="size-4" />
              <span>{renameLabel}</span>
            </DropdownMenu.Item>
            <DropdownMenu.Item
              class="text-destructive focus:text-destructive"
              onSelect={() => onDeleteSession(session.id)}
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
