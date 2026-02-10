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
  import { Input } from '$lib/components/ui/input';

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
  const AUDIO_EXTENSIONS = ['mp3', 'wav', 'm4a', 'ogg'];
  const VIDEO_EXTENSIONS = ['mp4', 'webm', 'mov', 'mkv'];
  const MAX_ATTACHMENTS_PER_MESSAGE = 4;
  const MAX_FILE_SIZE = 20 * 1024 * 1024;

  interface Props {
    prompt?: string;
    busy?: boolean;
    isLoaded?: boolean;
    canStop?: boolean;
    retrievalUrlEnabled?: boolean;
    retrievalUrls?: string[];
    retrievalLocalEnabled?: boolean;
    mcpEnabled?: boolean;
    supports_text?: boolean;
    supports_image?: boolean;
    supports_audio?: boolean;
    supports_video?: boolean;
    isLoaderPanelVisible?: boolean;
    isChatHistoryVisible?: boolean;
    hasMessages?: boolean;
    onSend?: (message?: PromptInputMessage) => void | Promise<void>;
    onStop?: () => void;
    onRetrievalUrlToggle?: (enabled: boolean) => void;
    onRetrievalUrlsChange?: (urls: string[]) => void;
    onRetrievalLocalToggle?: (enabled: boolean) => void;
    onMcpToggle?: (enabled: boolean) => void;
    onClear?: () => void;
    onToggleLoaderPanel?: () => void;
    onToggleChatHistory?: () => void;
  }

  let {
    prompt = $bindable(''),
    busy = false,
    isLoaded = false,
    canStop = false,
    retrievalUrlEnabled = false,
    retrievalUrls = [],
    retrievalLocalEnabled = false,
    mcpEnabled = false,
    supports_text = true,
    supports_image = false,
    supports_audio: _supports_audio = false,
    supports_video: _supports_video = false,
    isLoaderPanelVisible = false,
    isChatHistoryVisible: _isChatHistoryVisible = false,
    hasMessages = false,
    onSend,
    onStop,
    onRetrievalUrlToggle,
    onRetrievalUrlsChange,
    onRetrievalLocalToggle,
    onMcpToggle,
    onClear,
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
  const hasPendingAttachments = $derived((attachmentsContext?.files.length ?? 0) > 0);
  const sendDisabled = $derived(!isLoaded || (!busy && !prompt.trim() && !hasPendingAttachments));

  function buildAccept() {
    const extensions: string[] = [];
    if (supports_text) extensions.push(...TEXT_EXTENSIONS.map((ext) => `.${ext}`));
    if (supports_image) extensions.push(...IMAGE_EXTENSIONS.map((ext) => `.${ext}`));
    if (_supports_audio) extensions.push(...AUDIO_EXTENSIONS.map((ext) => `.${ext}`));
    if (_supports_video) extensions.push(...VIDEO_EXTENSIONS.map((ext) => `.${ext}`));
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

  async function handleSubmit(message: PromptInputMessage) {
    await onSend?.(message);
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

  function parseUrls(raw: string): string[] {
    const parts = raw
      .split(/[\n,]/g)
      .map((v) => v.trim())
      .filter(Boolean);
    const dedup = new Set<string>();
    for (const url of parts) {
      dedup.add(url);
    }
    return Array.from(dedup);
  }

  function toggleUrlRetrieval() {
    if (busy) return;
    onRetrievalUrlToggle?.(!retrievalUrlEnabled);
  }

  function updateUrlList(value: string) {
    if (busy) return;
    onRetrievalUrlsChange?.(parseUrls(value));
  }

  function toggleLocalRetrieval() {
    if (busy) return;
    onRetrievalLocalToggle?.(!retrievalLocalEnabled);
  }

  function toggleMcp() {
    if (busy) return;
    onMcpToggle?.(!mcpEnabled);
  }
</script>

<UrlFetchModal bind:open={showUrlFetchModal} onfiles={handleUrlFiles} />
<ShortcutsModal bind:open={showShortcutsModal} />

<div class="w-full max-w-2xl lg:max-w-3xl xl:max-w-4xl mx-auto">
  <PromptInput
    class="composer"
    {accept}
    bind:attachmentsContext
    multiple={true}
    maxFiles={MAX_ATTACHMENTS_PER_MESSAGE}
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
      {#if retrievalUrlEnabled}
        <div class="px-3 pb-2">
          <Input
            value={(retrievalUrls ?? []).join('\n')}
            placeholder="https://example.com/article"
            oninput={(event) => updateUrlList((event.currentTarget as HTMLInputElement).value)}
          />
          <p class="pt-1 text-[11px] text-muted-foreground">
            One URL per line (or comma-separated)
          </p>
        </div>
      {/if}

      <!-- Toolbar -->
      <div class="flex justify-between items-center gap-2 p-2">
        <PromptInputTools class="flex gap-0">
          <div
            class="flex items-center gap-1 rounded-full border border-border/80 bg-muted/35 px-1 py-1 mr-1 overflow-x-auto"
          >
            <button
              type="button"
              class={cn(
                'h-7 rounded-full px-2 text-xs transition-colors',
                retrievalUrlEnabled
                  ? 'bg-background text-foreground'
                  : 'text-muted-foreground hover:text-foreground',
              )}
              disabled={busy || !isLoaded}
              onclick={toggleUrlRetrieval}
            >
              URLs
            </button>
            <button
              type="button"
              class={cn(
                'h-7 rounded-full px-2 text-xs transition-colors',
                retrievalLocalEnabled
                  ? 'bg-background text-foreground'
                  : 'text-muted-foreground hover:text-foreground',
              )}
              disabled={busy || !isLoaded}
              onclick={toggleLocalRetrieval}
            >
              Docs
            </button>
            <button
              type="button"
              class={cn(
                'h-7 rounded-full px-2 text-xs transition-colors',
                mcpEnabled
                  ? 'bg-background text-foreground'
                  : 'text-muted-foreground hover:text-foreground',
              )}
              disabled={busy || !isLoaded}
              onclick={toggleMcp}
            >
              Tools
            </button>
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
            onclick={busy ? triggerStop : undefined}
            disabled={sendDisabled}
            aria-label={busy
              ? $t('chat.composer.stop') || 'Stop'
              : $t('chat.composer.send') || 'Send'}
            type={busy ? 'button' : 'submit'}
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
    font-size: 0.875rem;
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
