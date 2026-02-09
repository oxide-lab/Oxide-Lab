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
  import Microphone from 'phosphor-svelte/lib/Microphone';
  import Globe from 'phosphor-svelte/lib/Globe';
  import Command from 'phosphor-svelte/lib/Command';
  import { Button } from '$lib/components/ui/button';
  import {
    PromptInput,
    PromptInputTextarea,
    PromptInputTools,
    PromptInputButton,
    PromptInputAttachments,
    PromptInputAttachment,
    type PromptInputMessage,
    type AttachmentsContext,
  } from '$lib/components/ai-elements/prompt-input';
  import UrlFetchModal from '$lib/components/ui/UrlFetchModal.svelte';
  import ShortcutsModal from '$lib/components/ui/ShortcutsModal.svelte';
  import { cn } from '../../utils';
  import { t } from '$lib/i18n';
  import { chatState } from '$lib/stores/chat';
  import type { RetrievalWebMode } from '$lib/chat/types';

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
    retrievalMode?: RetrievalWebMode;
    retrievalLocalEnabled?: boolean;
    retrievalProEnabled?: boolean;
    retrievalLocalBetaEnabled?: boolean;
    retrievalEmbeddingsConfigured?: boolean;
    supports_text?: boolean;
    supports_image?: boolean;
    supports_audio?: boolean;
    supports_video?: boolean;
    isLoaderPanelVisible?: boolean;
    isChatHistoryVisible?: boolean;
    hasMessages?: boolean;
    onSend?: () => void;
    onStop?: () => void;
    onRetrievalModeChange?: (mode: RetrievalWebMode) => void;
    onRetrievalLocalToggle?: (enabled: boolean) => void;
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
    retrievalMode = 'lite',
    retrievalLocalEnabled = false,
    retrievalProEnabled = false,
    retrievalLocalBetaEnabled = false,
    retrievalEmbeddingsConfigured = false,
    supports_text = true,
    supports_image = false,
    supports_audio: _supports_audio = false,
    supports_video: _supports_video = false,
    isLoaderPanelVisible = false,
    isChatHistoryVisible: _isChatHistoryVisible = false,
    hasMessages = false,
    onSend,
    onStop,
    onRetrievalModeChange,
    onRetrievalLocalToggle,
    onClear,
    onAttach,
    onToggleLoaderPanel,
    onToggleChatHistory: _onToggleChatHistory,
  }: Props = $props();

  let attachmentsContext = $state<AttachmentsContext>();
  let attachError: string | null = $state(null);
  let errorTimer: ReturnType<typeof setTimeout> | null = null;
  let showUrlFetchModal = $state(false);
  let showShortcutsModal = $state(false);

  // Build accept string for file input
  const accept = $derived(buildAccept());
  const sendDisabled = $derived(!isLoaded || (!busy && !prompt.trim()));
  const proDisabled = $derived(!retrievalProEnabled || !retrievalEmbeddingsConfigured);
  const proDisabledReason = $derived(
    !retrievalProEnabled
      ? 'Search Pro is disabled in settings'
      : !retrievalEmbeddingsConfigured
        ? 'Configure embeddings provider in settings first'
        : '',
  );

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

  function handleUrlFiles(files: File[]) {
    if (attachmentsContext) {
      attachmentsContext.add(files);
    } else {
      console.warn('AttachmentsContext not ready');
    }
  }

  function setRetrievalMode(mode: RetrievalWebMode) {
    if (busy) return;
    if (mode === 'pro' && proDisabled) return;
    onRetrievalModeChange?.(mode);
  }

  function toggleLocalRetrieval() {
    if (busy) return;
    onRetrievalLocalToggle?.(!retrievalLocalEnabled);
  }
</script>

<UrlFetchModal bind:open={showUrlFetchModal} onfiles={handleUrlFiles} />
<ShortcutsModal bind:open={showShortcutsModal} />

<div class="w-full max-w-2xl lg:max-w-3xl xl:max-w-4xl mx-auto">
  <PromptInput
    class="composer"
    {accept}
    bind:attachmentsContext
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
        class="composer-input custom-scrollbar"
      />

      <!-- Toolbar -->
      <div class="flex justify-between items-center gap-2 p-2">
        <PromptInputTools class="flex gap-0">
          <div class="flex items-center gap-1 rounded-full border border-border/80 bg-muted/35 px-1 py-1 mr-1 overflow-x-auto">
            <button
              type="button"
              class={cn(
                'h-7 rounded-full px-2 text-xs transition-colors',
                retrievalMode === 'off'
                  ? 'bg-background text-foreground'
                  : 'text-muted-foreground hover:text-foreground',
              )}
              disabled={busy || !isLoaded}
              onclick={() => setRetrievalMode('off')}
            >
              Off
            </button>
            <button
              type="button"
              class={cn(
                'h-7 rounded-full px-2 text-xs transition-colors',
                retrievalMode === 'lite'
                  ? 'bg-background text-foreground'
                  : 'text-muted-foreground hover:text-foreground',
              )}
              disabled={busy || !isLoaded}
              onclick={() => setRetrievalMode('lite')}
            >
              Lite
            </button>
            <button
              type="button"
              class={cn(
                'h-7 rounded-full px-2 text-xs transition-colors',
                retrievalMode === 'pro'
                  ? 'bg-background text-foreground'
                  : 'text-muted-foreground hover:text-foreground',
                proDisabled && 'cursor-not-allowed opacity-50',
              )}
              title={proDisabledReason}
              disabled={busy || !isLoaded || proDisabled}
              onclick={() => setRetrievalMode('pro')}
            >
              Pro
            </button>
            {#if retrievalLocalBetaEnabled}
              <button
                type="button"
                class={cn(
                  'h-7 rounded-full px-2 text-xs transition-colors',
                  retrievalLocalEnabled
                    ? 'bg-background text-foreground'
                    : 'text-muted-foreground hover:text-foreground',
                )}
                disabled={busy || !isLoaded || !retrievalEmbeddingsConfigured}
                title={!retrievalEmbeddingsConfigured ? 'Configure embeddings provider in settings first' : ''}
                onclick={toggleLocalRetrieval}
              >
                Local
              </button>
            {/if}
          </div>

          <!-- Attach button -->
          <PromptInputButton
            onclick={() => attachmentsContext?.openFileDialog()}
            disabled={busy || !isLoaded}
            aria-label={$t('chat.composer.attach') || 'Attach file'}
          >
            <Paperclip size={16} weight="bold" />
          </PromptInputButton>

          <!-- Settings button -->
          <PromptInputButton
            class={cn(isLoaderPanelVisible && 'text-primary')}
            onclick={triggerSettings}
            disabled={busy}
            aria-label={$t('chat.composer.loaderSettings') || 'Model settings'}
          >
            <SlidersHorizontal size={16} weight="bold" />
          </PromptInputButton>

          <!-- URL Fetch button -->
          <PromptInputButton
            onclick={() => (showUrlFetchModal = true)}
            disabled={busy || !isLoaded}
            aria-label={$t('chat.composer.urlFetch') || 'Add from URL'}
          >
            <Globe size={16} weight="bold" />
          </PromptInputButton>

          <!-- Shortcuts button -->
          <PromptInputButton
            onclick={() => (showShortcutsModal = true)}
            aria-label={$t('chat.composer.shortcuts') || 'Shortcuts'}
          >
            <Command size={16} weight="bold" />
          </PromptInputButton>

          <!-- Voice input button -->
          <PromptInputButton
            onclick={() => console.log('Voice input triggered')}
            disabled={busy || !isLoaded}
            aria-label={$t('chat.composer.voice') || 'Voice input'}
          >
            <Microphone size={16} weight="bold" />
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
