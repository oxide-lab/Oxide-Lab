<script lang="ts">
  /**
   * Assistant Message Component
   * Displays an assistant message with reasoning, markdown content, and actions.
   */
  import { onMount } from 'svelte';
  import type { ChatMessage } from '$lib/chat/types';
  import { Reasoning, ReasoningContent } from '$lib/components/ai-elements/reasoning';
  import { Markdown } from '$lib/components/ai-elements/markdown';
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
    isLastMessage?: boolean;
    isFaded?: boolean;
    onCopy?: (content: string) => void;
    onRegenerate?: (index: number) => void;
  }

  let {
    message,
    index,
    isStreaming = false,
    isLastMessage = false,
    isFaded = false,
    onCopy,
    onRegenerate,
  }: Props = $props();

  // State for raw view
  let showRaw = $state(false);
  let copyTooltipOpen = $state(false);
  let regenerateTooltipOpen = $state(false);
  let rawTooltipOpen = $state(false);

  // Derived values
  let thinkingContent = $derived(message.thinking?.replace(/<think>/g, '').trim());
  let hasThinking = $derived(!!thinkingContent);
  let showActions = $derived(!isStreaming && message.content);

  function closeActionTooltips() {
    copyTooltipOpen = false;
    regenerateTooltipOpen = false;
    rawTooltipOpen = false;
  }

  function handleCopy() {
    closeActionTooltips();
    onCopy?.(message.content);
    navigator.clipboard.writeText(message.content);
  }

  function handleRegenerate() {
    closeActionTooltips();
    onRegenerate?.(index);
  }

  function toggleRaw() {
    closeActionTooltips();
    showRaw = !showRaw;
  }

  function getRawContent(): string {
    let raw = '';
    if (message.thinking) {
      raw += '<think>\n' + message.thinking + '\n</think>\n';
    }
    raw += message.content;
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

<div
  class="flex flex-col group w-full transition-opacity duration-300"
  class:opacity-50={isFaded}
>
  {#if showRaw}
    <!-- Raw view -->
    <pre
      class="raw-response text-sm bg-muted/50 p-4 rounded-lg overflow-x-auto whitespace-pre-wrap font-mono"
    >{getRawContent()}</pre>
  {:else}
    <!-- Reasoning block -->
    {#if hasThinking}
      <Reasoning isStreaming={message.isThinking} class="mb-3">
        <ReasoningContent>
          {thinkingContent}
        </ReasoningContent>
      </Reasoning>
    {/if}

    <!-- Main content -->
    <Markdown
      content={message.content}
      class="prose prose-sm dark:prose-invert max-w-none"
    />
  {/if}

  <!-- Actions (only show when not streaming and has content) -->
  {#if showActions}
    <div
      class={cn(
        'message-actions mt-2 flex gap-0.5 opacity-0 transition-opacity duration-150 group-hover:opacity-100',
        isLastMessage && 'opacity-100',
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
