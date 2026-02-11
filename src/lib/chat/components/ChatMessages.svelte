<script lang="ts">
  import {
    Conversation,
    ConversationContent,
    ConversationScrollButton,
  } from '$lib/components/ai-elements/conversation';
  import MessageList from './MessageList.svelte';
  import type { ChatMessage } from '$lib/chat/types';

  interface Props {
    messages: ChatMessage[];
    isLoaded: boolean;
    onRegenerate: (index: number) => void | Promise<void>;
    onEdit: (index: number, content: string) => void | Promise<void>;
  }

  let { messages = $bindable([]), isLoaded, onRegenerate, onEdit }: Props = $props();

  let hasMessages = $derived((messages?.length ?? 0) > 0);
</script>

<div class="messages-area flex-1 min-h-0 relative overflow-hidden">
  <Conversation class="h-full w-full">
    {#if hasMessages}
      <ConversationContent class="messages-content pb-16">
        <MessageList
          bind:messages
          showModelNotice={false}
          onRegenerate={(index) => onRegenerate(index)}
          onEdit={(index, content) => onEdit(index, content)}
        />
      </ConversationContent>
      <ConversationScrollButton />
    {:else}
      <MessageList
        bind:messages
        showModelNotice={!isLoaded && messages.length === 0}
        onRegenerate={(index) => onRegenerate(index)}
        onEdit={(index, content) => onEdit(index, content)}
      />
    {/if}
  </Conversation>
</div>

<style>
  .messages-area::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 20px;
    height: 50px;
    background: linear-gradient(to top, transparent, var(--background));
    pointer-events: none;
    z-index: 1;
  }

  .messages-area::after {
    content: '';
    position: absolute;
    bottom: 0;
    left: 0;
    right: 20px;
    height: 100px;
    background: linear-gradient(to bottom, transparent, var(--background));
    pointer-events: none;
    z-index: 1;
  }
</style>
