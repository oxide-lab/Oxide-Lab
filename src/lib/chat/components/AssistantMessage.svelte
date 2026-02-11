<script lang="ts">
  /**
   * Assistant Message Component
   * Displays an assistant message with reasoning, markdown content, and actions.
   */
  import { onMount } from 'svelte';
  import type { ChatMessage, McpToolCallView } from '$lib/chat/types';
  import { Reasoning, ReasoningContent } from '$lib/components/ai-elements/reasoning';
  import { Markdown } from '$lib/components/ai-elements/markdown';
  import Tool from '$lib/components/ai-elements/tool/Tool.svelte';
  import ToolHeader from '$lib/components/ai-elements/tool/ToolHeader.svelte';
  import ToolInput from '$lib/components/ai-elements/tool/ToolInput.svelte';
  import ToolOutput from '$lib/components/ai-elements/tool/ToolOutput.svelte';
  import * as Collapsible from '$lib/components/ui/collapsible';
  import { Button } from '$lib/components/ui/button';
  import * as Tooltip from '$lib/components/ui/tooltip';
  import Copy from 'phosphor-svelte/lib/Copy';
  import ArrowsClockwise from 'phosphor-svelte/lib/ArrowsClockwise';
  import Code from 'phosphor-svelte/lib/Code';
  import { t } from '$lib/i18n';
  import { cn } from '../../utils';

  interface Props {
    message: ChatMessage;
    index: number;
    isStreaming?: boolean;
    isFaded?: boolean;
    onCopy?: (content: string) => void;
    onRegenerate?: (index: number) => void;
  }

  let {
    message,
    index,
    isStreaming = false,
    isFaded = false,
    onCopy,
    onRegenerate,
  }: Props = $props();

  // State for raw view
  let showRaw = $state(false);
  let copyTooltipOpen = $state(false);
  let regenerateTooltipOpen = $state(false);
  let rawTooltipOpen = $state(false);

  function cleanThinking(text: string): string {
    return text.replace(/<\/?think>/gi, '').trim();
  }

  function splitInlineThinking(content: string): { thinking: string; content: string } {
    const source = content ?? '';
    const closeTag = '</think>';
    const openTag = '<think>';
    const lower = source.toLowerCase();
    const openIdx = lower.indexOf(openTag);
    const closeIdx = lower.indexOf(closeTag);

    if (openIdx >= 0 && closeIdx > openIdx) {
      const thinking = source.slice(openIdx + openTag.length, closeIdx).trim();
      const contentWithoutThinking = `${source.slice(0, openIdx)}${source.slice(closeIdx + closeTag.length)}`.trimStart();
      return { thinking, content: contentWithoutThinking };
    }

    if (openIdx < 0 && closeIdx >= 0) {
      const thinking = source.slice(0, closeIdx).trim();
      const contentWithoutThinking = source.slice(closeIdx + closeTag.length).trimStart();
      return { thinking, content: contentWithoutThinking };
    }

    return { thinking: '', content: source };
  }

  // Derived values
  let fallbackThinking = $derived(
    !message.thinking ? splitInlineThinking(message.content ?? '') : { thinking: '', content: message.content ?? '' },
  );
  let thinkingContent = $derived(
    message.thinking ? cleanThinking(message.thinking) : cleanThinking(fallbackThinking.thinking),
  );
  let renderedContent = $derived(
    message.thinking ? (message.content ?? '') : fallbackThinking.content,
  );
  let hasThinking = $derived(!!thinkingContent);
  let showActions = $derived(!isStreaming && !!(renderedContent || hasThinking));
  let retrievalSources = $derived(message.sources ?? []);
  let retrievalWarnings = $derived(message.retrievalWarnings ?? []);
  let inlineToolCalls = $derived((message.mcpToolCalls ?? []) as McpToolCallView[]);

  function closeActionTooltips() {
    copyTooltipOpen = false;
    regenerateTooltipOpen = false;
    rawTooltipOpen = false;
  }

  function handleCopy() {
    closeActionTooltips();
    onCopy?.(renderedContent || '');
    navigator.clipboard.writeText(renderedContent || '');
  }

  function handleRegenerate() {
    closeActionTooltips();
    onRegenerate?.(index);
  }

  function toggleRaw() {
    closeActionTooltips();
    showRaw = !showRaw;
  }

  async function openWebSource(url: string) {
    const { openUrl } = await import('@tauri-apps/plugin-opener');
    await openUrl(url);
  }

  function getParentFolder(path: string): string {
    const normalized = path.replaceAll('/', '\\');
    const idx = normalized.lastIndexOf('\\');
    if (idx <= 0) return path;
    return normalized.slice(0, idx);
  }

  async function openLocalSourceFolder(path: string) {
    const { openPath } = await import('@tauri-apps/plugin-opener');
    await openPath(getParentFolder(path));
  }

  function getRawContent(): string {
    let raw = '';
    if (message.thinking) {
      raw += '<think>\n' + message.thinking + '\n</think>\n';
    }
    raw += message.content ?? '';
    return raw;
  }

  onMount(() => {
    const onWheelOrTouch = () => closeActionTooltips();
    const onScroll = () => closeActionTooltips();

    window.addEventListener('wheel', onWheelOrTouch, { passive: true });
    window.addEventListener('touchmove', onWheelOrTouch, { passive: true });
    window.addEventListener('scroll', onScroll, true);

    return () => {
      window.removeEventListener('wheel', onWheelOrTouch);
      window.removeEventListener('touchmove', onWheelOrTouch);
      window.removeEventListener('scroll', onScroll, true);
    };
  });
</script>

<div class="flex flex-col group w-full transition-opacity duration-300" class:opacity-50={isFaded}>
  {#if showRaw}
    <!-- Raw view -->
    <pre
      class="raw-response text-sm bg-muted/50 p-4 rounded-lg overflow-x-auto whitespace-pre-wrap font-mono">{getRawContent()}</pre>
  {:else}
    <!-- Reasoning block -->
    {#if hasThinking}
      <Reasoning isStreaming={message.isThinking} class="mb-2">
        <ReasoningContent>
          {thinkingContent}
        </ReasoningContent>
      </Reasoning>
    {/if}

    <!-- Main content -->
    <Markdown content={renderedContent} class="prose prose-sm dark:prose-invert max-w-none" />
  {/if}

  {#if inlineToolCalls.length > 0}
    <div class="mt-3 space-y-2">
      {#each inlineToolCalls as item (item.call_id)}
        <Tool class="mb-0 shadow-sm" open={item.state !== 'output-available'}>
          <ToolHeader type={`${item.server_id}/${item.tool_name}`} state={item.state} />
          <Collapsible.Content>
            <ToolInput input={item.input ?? { call_id: item.call_id }} />
            <ToolOutput output={item.output} errorText={item.errorText} />
          </Collapsible.Content>
        </Tool>
      {/each}
    </div>
  {/if}

  {#if retrievalWarnings.length > 0}
    <div class="mt-3 space-y-2">
      {#each retrievalWarnings as warning, warningIdx (`${warningIdx}-${warning}`)}
        <div
          class="rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-amber-700"
        >
          {warning}
        </div>
      {/each}
    </div>
  {/if}

  {#if retrievalSources.length > 0}
    <div class="mt-3 rounded-lg border border-border/70 bg-muted/35 p-3">
      <div class="mb-2 text-xs font-medium text-muted-foreground">
        Sources ({retrievalSources.length})
      </div>
      <div class="space-y-2">
        {#each retrievalSources as source, sourceIdx (`${sourceIdx}-${source.title}-${source.url ?? source.path ?? ''}`)}
          <div class="rounded-md border border-border/60 bg-background/70 px-3 py-2">
            <div class="flex items-start justify-between gap-2">
              <div class="min-w-0">
                <p class="truncate text-sm font-medium">{sourceIdx + 1}. {source.title}</p>
                {#if source.url}
                  <p class="truncate text-xs text-muted-foreground">{source.url}</p>
                {:else if source.path}
                  <p class="truncate text-xs text-muted-foreground">{source.path}</p>
                {/if}
              </div>
              {#if source.url}
                <Button
                  variant="outline"
                  size="sm"
                  onclick={() => source.url && openWebSource(source.url)}
                >
                  Open
                </Button>
              {:else if source.path}
                <Button
                  variant="outline"
                  size="sm"
                  onclick={() => source.path && openLocalSourceFolder(source.path)}
                >
                  Open folder
                </Button>
              {/if}
            </div>
            {#if source.snippet}
              <p class="mt-2 line-clamp-3 text-xs text-muted-foreground">{source.snippet}</p>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {/if}

  <!-- Actions (only show when not streaming and has content) -->
  {#if showActions}
    <div
      class={cn(
        'message-actions mt-1.5 flex gap-0.5 opacity-0 transition-opacity duration-150 group-hover:opacity-100',
      )}
    >
      <Tooltip.Provider>
        <Tooltip.Root delayDuration={60} bind:open={copyTooltipOpen}>
          <Tooltip.Trigger>
            <Button
              variant="ghost"
              size="icon"
              class="h-8 w-8 text-muted-foreground hover:text-foreground"
              onclick={handleCopy}
            >
              <Copy class="h-4 w-4" />
            </Button>
          </Tooltip.Trigger>
          <Tooltip.Content>{$t('chat.actions.copy') || 'Copy'}</Tooltip.Content>
        </Tooltip.Root>
      </Tooltip.Provider>

      {#if onRegenerate}
        <Tooltip.Provider>
          <Tooltip.Root delayDuration={60} bind:open={regenerateTooltipOpen}>
            <Tooltip.Trigger>
              <Button
                variant="ghost"
                size="icon"
                class="h-8 w-8 text-muted-foreground hover:text-foreground"
                onclick={handleRegenerate}
              >
                <ArrowsClockwise class="h-4 w-4" />
              </Button>
            </Tooltip.Trigger>
            <Tooltip.Content>{$t('chat.actions.regenerate') || 'Regenerate'}</Tooltip.Content>
          </Tooltip.Root>
        </Tooltip.Provider>
      {/if}

      <Tooltip.Provider>
        <Tooltip.Root delayDuration={60} bind:open={rawTooltipOpen}>
          <Tooltip.Trigger>
            <Button
              variant={showRaw ? 'secondary' : 'ghost'}
              size="icon"
              class="h-8 w-8 text-muted-foreground hover:text-foreground"
              onclick={toggleRaw}
            >
              <Code class="h-4 w-4" />
            </Button>
          </Tooltip.Trigger>
          <Tooltip.Content>{showRaw ? 'Rendered' : 'Raw'}</Tooltip.Content>
        </Tooltip.Root>
      </Tooltip.Provider>
    </div>
  {/if}
</div>
