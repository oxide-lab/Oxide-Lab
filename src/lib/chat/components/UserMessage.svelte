<script lang="ts">
  /**
   * User Message Component
   * Displays a user message with attachments and edit functionality.
   */
  import { onMount } from 'svelte';
  import type { ChatMessage, Attachment } from '$lib/chat/types';
  import { isImageFile, isImageMimeType } from '$lib/chat/types';
  import { Button } from '$lib/components/ui/button';
  import * as Tooltip from '$lib/components/ui/tooltip';
  import { Markdown } from '$lib/components/ai-elements/markdown';
  import Copy from 'phosphor-svelte/lib/Copy';
  import PencilSimple from 'phosphor-svelte/lib/PencilSimple';
  import File from 'phosphor-svelte/lib/File';
  import { t } from '$lib/i18n';

  interface Props {
    message: ChatMessage;
    index: number;
    isFaded?: boolean;
    onCopy?: (content: string) => void;
    onEdit?: (index: number, newContent: string) => void;
  }

  let { message, index, isFaded = false, onCopy, onEdit }: Props = $props();

  // Edit state
  let isEditing = $state(false);
  let editContent = $state('');
  let copyTooltipOpen = $state(false);
  let editTooltipOpen = $state(false);

  // Derived: split attachments into images and files
  let imageAttachments = $derived(
    message.attachments?.filter((a) => isImageFile(a.filename) || isImageMimeType(a.mimeType)) ??
      [],
  );
  let fileAttachments = $derived(
    message.attachments?.filter((a) => !isImageFile(a.filename) && !isImageMimeType(a.mimeType)) ??
      [],
  );

  function closeActionTooltips() {
    copyTooltipOpen = false;
    editTooltipOpen = false;
  }

  function handleCopy() {
    closeActionTooltips();
    onCopy?.(message.content);
    navigator.clipboard.writeText(message.content);
  }

  function startEdit() {
    closeActionTooltips();
    editContent = message.content;
    isEditing = true;
  }

  function cancelEdit() {
    isEditing = false;
    editContent = '';
  }

  function submitEdit() {
    if (editContent.trim() && onEdit) {
      onEdit(index, editContent.trim());
    }
    isEditing = false;
    editContent = '';
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      submitEdit();
    }
    if (e.key === 'Escape') {
      cancelEdit();
    }
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

<div class="flex flex-col transition-opacity duration-300" class:opacity-50={isFaded}>
  <!-- Image attachments above the message -->
  {#if imageAttachments.length > 0}
    <div class="flex gap-2 mb-2 overflow-x-auto justify-end max-w-md self-end">
      {#each imageAttachments as attachment, i}
        <div class="flex-shrink-0">
          <img
            src={`data:${attachment.mimeType};base64,${attachment.data}`}
            alt={attachment.filename}
            class="w-16 h-16 object-cover rounded-md"
          />
        </div>
      {/each}
    </div>
  {/if}

  {#if isEditing}
    <!-- Edit mode -->
    <div class="w-full max-w-[85%] sm:max-w-[75%] self-end">
      <textarea
        bind:value={editContent}
        class="w-full min-h-[80px] p-3 rounded-lg bg-muted border border-border text-foreground resize-y"
        onkeydown={handleKeydown}
      ></textarea>
      <div class="flex justify-end gap-2 mt-2">
        <Button variant="ghost" size="sm" onclick={cancelEdit}>
          {$t('common.cancel') || 'Cancel'}
        </Button>
        <Button variant="default" size="sm" onclick={submitEdit}>
          {$t('common.save') || 'Save & Submit'}
        </Button>
      </div>
    </div>
  {:else}
    <!-- Normal view -->
    <div class="w-full group relative">
      <div class="flex justify-end">
        <div
          class="bg-muted text-foreground max-w-[90%] rounded-3xl rounded-tr-sm px-4 py-2 text-sm"
        >
          <!-- File attachments inside the bubble -->
          {#if fileAttachments.length > 0}
            <div class="flex gap-2 mb-2 overflow-x-auto flex-wrap">
              {#each fileAttachments as attachment}
                <div
                  class="flex items-center gap-2 py-1 px-2 rounded-lg bg-background/50 transition-colors flex-shrink-0"
                >
                  <File size={12} class="text-muted-foreground flex-shrink-0" />
                  <span class="text-xs text-muted-foreground max-w-[120px] truncate">
                    {attachment.filename}
                  </span>
                </div>
              {/each}
            </div>
          {/if}

          <!-- Message content -->
          <Markdown
            content={message.content}
            class="prose prose-sm dark:prose-invert max-w-none text-left"
          />
        </div>
      </div>

      <!-- Actions -->
      <div
        class="message-actions mt-1.5 flex justify-end gap-0.5 opacity-0 transition-opacity duration-150 group-hover:opacity-100"
      >
        <Tooltip.Provider>
          <Tooltip.Root delayDuration={60} bind:open={copyTooltipOpen}>
            <Tooltip.Trigger>
              <Button
                variant="ghost"
                size="icon"
                class="h-7 w-7 text-muted-foreground hover:text-foreground"
                onclick={handleCopy}
              >
                <Copy class="h-3.5 w-3.5" />
              </Button>
            </Tooltip.Trigger>
            <Tooltip.Content>{$t('chat.actions.copy') || 'Copy'}</Tooltip.Content>
          </Tooltip.Root>
        </Tooltip.Provider>

        {#if onEdit}
          <Tooltip.Provider>
            <Tooltip.Root delayDuration={60} bind:open={editTooltipOpen}>
              <Tooltip.Trigger>
                <Button
                  variant="ghost"
                  size="icon"
                  class="h-7 w-7 text-muted-foreground hover:text-foreground"
                  onclick={startEdit}
                >
                  <PencilSimple class="h-3.5 w-3.5" />
                </Button>
              </Tooltip.Trigger>
              <Tooltip.Content>{$t('chat.actions.edit') || 'Edit'}</Tooltip.Content>
            </Tooltip.Root>
          </Tooltip.Provider>
        {/if}
      </div>
    </div>
  {/if}
</div>
