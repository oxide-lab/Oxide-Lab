<script lang="ts">
  /**
   * Message List Component
   *
   * Displays chat messages using modular UserMessage and AssistantMessage components.
   * Includes support for reasoning/chain-of-thought and attachments.
   */
  import type { ChatMessage } from '$lib/chat/types';
  import {
    Conversation,
    ConversationContent,
    ConversationEmptyState,
  } from '$lib/components/ai-elements/conversation';
  import Sparkle from 'phosphor-svelte/lib/Sparkle';
  import { t, locale } from '$lib/i18n';
  import { chatState } from '$lib/stores/chat';
  import UserMessage from './UserMessage.svelte';
  import AssistantMessage from './AssistantMessage.svelte';

  // Props using Svelte 5 runes
  let {
    messages = $bindable([]),
    showModelNotice = false,
    onRegenerate,
    onEdit,
  }: {
    messages?: ChatMessage[];
    showModelNotice?: boolean;
    onRegenerate?: (index: number) => void;
    onEdit?: (index: number, newContent: string) => void;
  } = $props();

  // Derived value for placeholder only state
  let placeholderOnly = $derived(showModelNotice && messages.length === 0);

  // Current locale guard to avoid calling $t before translations are loaded
  let currentLocale = $derived($locale);

  // Action handlers
  function handleCopy(content: string) {
    navigator.clipboard.writeText(content);
  }

  function handleRegenerate(index: number) {
    onRegenerate?.(index);
  }

  function handleEdit(index: number, newContent: string) {
    onEdit?.(index, newContent);
  }
</script>

{#if placeholderOnly}
  <ConversationEmptyState class="min-h-0">
    <Sparkle size={36} weight="duotone" class="text-muted-foreground/50" />
    <p class="text-sm text-muted-foreground/80">
      {currentLocale ? $t('chat.notice.selectModel') : 'Select a model to start chatting'}
    </p>
  </ConversationEmptyState>
{:else if messages.length === 0}
  <ConversationEmptyState class="min-h-0">
    <Sparkle size={36} weight="duotone" class="text-muted-foreground/50" />
    <p class="text-sm text-muted-foreground/80">
      {currentLocale ? $t('chat.notice.startConversation') : 'Start a conversation'}
    </p>
  </ConversationEmptyState>
{:else}
  <div class="flex flex-col gap-4 sm:gap-6 lg:gap-8 py-6 sm:py-8">
    {#each messages as m, i (i)}
      {@const isAssistant = m.role === 'assistant'}
      {@const isLastMessage = i === messages.length - 1}
      {@const isStreaming = isLastMessage && isAssistant && $chatState.busy}

      <div
        class="w-full mx-auto px-3 sm:px-4 md:px-6 max-w-2xl lg:max-w-3xl xl:max-w-4xl"
      >
        {#if isAssistant}
          <AssistantMessage
            message={m}
            index={i}
            {isStreaming}
            {isLastMessage}
            onCopy={handleCopy}
            onRegenerate={handleRegenerate}
          />
        {:else}
          <UserMessage
            message={m}
            index={i}
            onCopy={handleCopy}
            onEdit={handleEdit}
          />
        {/if}
      </div>
    {/each}
  </div>
{/if}
