<script lang="ts">
  /**
   * Composer Component
   *
   * Chat input area with send/stop controls and attachments.
   * Uses ai-elements PromptInput components.
   */
  import ArrowUp from 'phosphor-svelte/lib/ArrowUp';
  import Stop from 'phosphor-svelte/lib/Stop';
  import Paperclip from 'phosphor-svelte/lib/Paperclip';
  import Broom from 'phosphor-svelte/lib/Broom';
  import SlidersHorizontal from 'phosphor-svelte/lib/SlidersHorizontal';
  import { Button } from '$lib/components/ui/button';
  import {
    PromptInput,
    PromptInputTextarea,
    PromptInputTools,
    PromptInputButton,
    PromptInputAttachments,
    PromptInputAttachment,
    type PromptInputMessage,
  } from '$lib/components/ai-elements/prompt-input';
  import { cn } from '../../utils';
  import { t } from '$lib/i18n';
  import { chatState } from '$lib/stores/chat';

  type AttachDetail = {
    filename: string;
    content: string;
  };

  const TEXT_EXTENSIONS = [
    'txt',
    'md',
    'markdown',
    'json',
    'log',
    'csv',
    'yaml',
    'yml',
    'xml',
    'html',
  ];
  const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'gif'];
  const MAX_FILE_SIZE = 20 * 1024 * 1024;

  interface Props {
    prompt?: string;
    busy?: boolean;
    isLoaded?: boolean;
    canStop?: boolean;
    supports_text?: boolean;
    supports_image?: boolean;
    supports_audio?: boolean;
    supports_video?: boolean;
    isLoaderPanelVisible?: boolean;
    isChatHistoryVisible?: boolean;
    hasMessages?: boolean;
    onSend?: () => void;
    onStop?: () => void;
    onClear?: () => void;
    onAttach?: (detail: AttachDetail) => void;
    onToggleLoaderPanel?: () => void;
    onToggleChatHistory?: () => void;
  }

  let {
    prompt = $bindable(''),
    busy = false,
    isLoaded = false,
    canStop = false,
    supports_text = true,
    supports_image = false,
    supports_audio: _supports_audio = false,
    supports_video: _supports_video = false,
    isLoaderPanelVisible = false,
    isChatHistoryVisible: _isChatHistoryVisible = false,
    hasMessages = false,
    onSend,
    onStop,
    onClear,
    onAttach,
    onToggleLoaderPanel,
    onToggleChatHistory: _onToggleChatHistory,
  }: Props = $props();

  let attachError: string | null = $state(null);
  let errorTimer: ReturnType<typeof setTimeout> | null = null;

  // Build accept string for file input
  const accept = $derived(buildAccept());
  const sendDisabled = $derived(!isLoaded || (!busy && !prompt.trim()));

  function buildAccept() {
    const extensions: string[] = [];
    if (supports_text) extensions.push(...TEXT_EXTENSIONS.map((ext) => `.${ext}`));
    if (supports_image) extensions.push(...IMAGE_EXTENSIONS.map((ext) => `.${ext}`));
    return extensions.join(',') || TEXT_EXTENSIONS.map((ext) => `.${ext}`).join(',');
  }

  function clearErrorTimer() {
    if (errorTimer) {
      clearTimeout(errorTimer);
      errorTimer = null;
    }
  }

  function setError(message: string) {
    attachError = message;
    clearErrorTimer();
    errorTimer = setTimeout(() => {
      attachError = null;
      errorTimer = null;
    }, 4000);
  }

  function triggerSend() {
    // Use $chatState for proper reactivity (props may not update correctly)
    if ($chatState.busy || !$chatState.isLoaded || !prompt.trim()) return;
    onSend?.();
  }

  function triggerStop() {
    console.log('[Composer] triggerStop called, canStop:', canStop);
    if (!canStop) {
      console.log('[Composer] triggerStop blocked by canStop=false');
      return;
    }
    onStop?.();
  }

  function triggerClear() {
    if (!prompt && !attachError) return;
    prompt = '';
    attachError = null;
    clearErrorTimer();
    onClear?.();
  }

  function triggerSettings() {
    onToggleLoaderPanel?.();
  }

  function handleSubmit(message: PromptInputMessage) {
    // Handle attached files
    for (const file of message.files ?? []) {
      if (file.filename && file.url) {
        onAttach?.({ filename: file.filename, content: file.url });
      }
    }
    triggerSend();
  }

  function handleError(err: { code: string; message: string }) {
    setError(err.message);
  }
</script>

<div class="w-full max-w-2xl lg:max-w-3xl xl:max-w-4xl mx-auto">
  <PromptInput
    class="composer"
    {accept}
    multiple={false}
    maxFileSize={MAX_FILE_SIZE}
    onSubmit={handleSubmit}
    onError={handleError}
  >
    <div class="flex flex-col">
      <!-- Attachments Preview -->
      <PromptInputAttachments class="px-0">
        {#snippet children(file)}
          <PromptInputAttachment data={file} />
        {/snippet}
      </PromptInputAttachments>

      <!-- Main Input Area -->
      <PromptInputTextarea
        bind:value={prompt}
        placeholder={isLoaded
          ? $t('chat.composer.placeholder') || 'Send a message...'
          : $t('chat.composer.placeholderNotLoaded') || 'Load a model to start chatting'}
        class="composer-input"
      />

      <!-- Toolbar -->
      <div class="flex justify-between items-center gap-2 p-2">
        <PromptInputTools class="flex gap-0">
          <!-- Attach button -->
          <PromptInputButton
            onclick={() => {
              const input = document.querySelector(
                'input[name="prompt-input-files"]',
              ) as HTMLInputElement;
              input?.click();
            }}
            disabled={busy || !isLoaded}
            aria-label={$t('chat.composer.attach') || 'Attach file'}
          >
            <Paperclip size={16} weight="bold" />
          </PromptInputButton>

          <!-- Settings button -->
          <PromptInputButton
            class={cn(isLoaderPanelVisible && 'text-primary')}
            onclick={triggerSettings}
            disabled={!isLoaded || busy}
            aria-label={$t('chat.composer.loaderSettings') || 'Model settings'}
          >
            <SlidersHorizontal size={16} weight="bold" />
          </PromptInputButton>

          <!-- Clear button -->
          {#if prompt || attachError}
            <PromptInputButton
              onclick={triggerClear}
              aria-label={$t('chat.composer.clear') || 'Clear'}
            >
              <Broom size={16} weight="bold" />
            </PromptInputButton>
          {/if}
        </PromptInputTools>

        <PromptInputTools class="flex items-center gap-2 [&_button:first-child]:rounded-bl-full">
          <!-- Send/Stop button -->
          <Button
            variant="default"
            size="icon"
            class="rounded-full size-9"
            onclick={busy ? triggerStop : triggerSend}
            disabled={sendDisabled}
            aria-label={busy
              ? $t('chat.composer.stop') || 'Stop'
              : $t('chat.composer.send') || 'Send'}
            type="button"
          >
            {#if busy}
              <Stop size={16} weight="bold" />
            {:else}
              <ArrowUp size={16} weight="bold" />
            {/if}
          </Button>
        </PromptInputTools>
      </div>

      <!-- Error Display -->
      {#if attachError}
        <div class="composer-error">
          <p class="text-sm text-destructive">{attachError}</p>
        </div>
      {/if}
    </div>
  </PromptInput>
</div>

<style>
  /* ===== Composer Container (CSS for border-radius and shadow) ===== */
  :global(.composer) {
    border-radius: 1.5rem;
    border: 1px solid var(--border);
    background: var(--popover);
    box-shadow: 0 1px 2px 0 rgb(0 0 0 / 0.05);
    padding: 0;
    padding-top: 0.25rem;
  }

  /* ===== Textarea Input Styles (CSS for complex resets) ===== */
  :global(.composer-input) {
    min-height: 44px;
    max-height: 120px;
    width: 100%;
    resize: none;
    background: transparent !important;
    border: none !important;
    outline: none !important;
    box-shadow: none !important;
    padding: 0.75rem 1rem;
    font-size: 1rem;
    line-height: 1.3;
    scrollbar-width: thin;
    scrollbar-color: var(--muted-foreground) transparent;
  }
  
  :global(.composer-input::-webkit-scrollbar) {
    width: 6px;
    height: 6px;
  }

  :global(.composer-input::-webkit-scrollbar-track) {
    background: transparent;
  }

  :global(.composer-input::-webkit-scrollbar-thumb) {
    background-color: var(--border);
    border-radius: 9999px;
  }

  :global(.composer-input::-webkit-scrollbar-thumb:hover) {
    background-color: var(--muted-foreground);
  }

  :global(.composer-input:focus),
  :global(.composer-input:focus-visible) {
    border: none !important;
    outline: none !important;
    box-shadow: none !important;
  }

  /* ===== Error Display (CSS for color-mix) ===== */
  .composer-error {
    padding: 0.5rem 1rem;
    margin-bottom: 0.5rem;
    border-radius: 0.5rem;
    background: color-mix(in srgb, var(--destructive) 10%, transparent);
    border: 1px solid color-mix(in srgb, var(--destructive) 30%, transparent);
  }
</style>
