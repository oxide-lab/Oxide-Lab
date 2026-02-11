<script lang="ts">
  import Composer from './Composer.svelte';

  interface Props {
    prompt: string;
    busy: boolean;
    isLoaded: boolean;
    canStop: boolean;
    retrievalUrlEnabled: boolean;
    retrievalUrls: string[];
    retrievalLocalEnabled: boolean;
    mcpEnabled: boolean;
    supports_text: boolean;
    supports_image: boolean;
    supports_audio: boolean;
    supports_video: boolean;
    isLoaderPanelVisible: boolean;
    isChatHistoryVisible: boolean;
    hasMessages: boolean;
    onRetrievalUrlToggle: (enabled: boolean) => void;
    onRetrievalUrlsChange: (urls: string[]) => void;
    onRetrievalLocalToggle: (enabled: boolean) => void;
    onMcpToggle: (enabled: boolean) => void;
    onSend: () => void | Promise<void>;
    onStop: () => void | Promise<void>;
    onToggleLoaderPanel: () => void;
    onToggleChatHistory: () => void;
  }

  let {
    prompt = $bindable(),
    busy,
    isLoaded,
    canStop,
    retrievalUrlEnabled,
    retrievalUrls,
    retrievalLocalEnabled,
    mcpEnabled,
    supports_text,
    supports_image,
    supports_audio,
    supports_video,
    isLoaderPanelVisible,
    isChatHistoryVisible,
    hasMessages,
    onRetrievalUrlToggle,
    onRetrievalUrlsChange,
    onRetrievalLocalToggle,
    onMcpToggle,
    onSend,
    onStop,
    onToggleLoaderPanel,
    onToggleChatHistory,
  }: Props = $props();
</script>

<div
  class="composer-area shrink-0 relative z-10 px-3 sm:px-4 lg:px-6 pb-3 sm:pb-4 bg-background"
  class:composer-centered={!hasMessages}
>
  <Composer
    bind:prompt
    {busy}
    {isLoaded}
    {canStop}
    {retrievalUrlEnabled}
    {retrievalUrls}
    {retrievalLocalEnabled}
    {mcpEnabled}
    {supports_text}
    {supports_image}
    {supports_audio}
    {supports_video}
    {isLoaderPanelVisible}
    {isChatHistoryVisible}
    {hasMessages}
    {onRetrievalUrlToggle}
    {onRetrievalUrlsChange}
    {onRetrievalLocalToggle}
    {onMcpToggle}
    {onSend}
    {onStop}
    {onToggleLoaderPanel}
    {onToggleChatHistory}
  />
</div>

<style>
  .composer-centered {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 100%;
    max-width: 48rem;
    padding: 1rem;
    background: transparent;
  }
</style>
