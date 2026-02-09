<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import * as Dialog from '$lib/components/ui/dialog/index.js';
  import * as Button from '$lib/components/ui/button/index.js';
  import { Input } from '$lib/components/ui/input/index.js';
  import { Spinner } from '$lib/components/ui/spinner';
  import { Globe, X } from 'phosphor-svelte';

  interface Props {
    open: boolean;
    onfiles?: (files: File[]) => void;
  }

  let { open = $bindable(false), onfiles }: Props = $props();

  let urlValue = $state('');
  let loading = $state(false);
  let errorMsg = $state('');

  async function handleSubmit() {
    errorMsg = '';
    const trimmed = urlValue.trim();
    if (!trimmed.startsWith('https://')) {
      errorMsg = 'Enter a valid HTTPS URL.';
      return;
    }

    loading = true;
    try {
      const result = await invoke<{
        content: number[];
        mime_type: string;
        filename: string;
      }>('fetch_url', { url: trimmed });

      const uint8Array = new Uint8Array(result.content);
      const blob = new Blob([uint8Array], { type: result.mime_type });
      const file = new File([blob], result.filename, { type: result.mime_type });

      onfiles?.([file]);
      open = false;
      urlValue = '';
    } catch (e) {
      errorMsg = typeof e === 'string' ? e : 'Failed to fetch URL';
    } finally {
      loading = false;
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-[480px] rounded-3xl">
    <Dialog.Header>
      <Dialog.Title>Add from URL</Dialog.Title>
      <Dialog.Description>Enter an HTTPS URL to fetch content. Max 10MB.</Dialog.Description>
    </Dialog.Header>

    <form
      class="space-y-4 pt-2"
      onsubmit={(e) => {
        e.preventDefault();
        handleSubmit();
      }}
    >
      <div class="space-y-2">
        <div class="relative">
          <Globe class="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            bind:value={urlValue}
            type="url"
            placeholder="https://example.com/file.txt"
            class="pl-9 rounded-xl"
            disabled={loading}
          />
        </div>
        {#if errorMsg}
          <p class="text-xs text-destructive">{errorMsg}</p>
        {/if}
      </div>

      <Dialog.Footer class="gap-2">
        <Button.Root
          variant="outline"
          class="rounded-full px-6"
          onclick={() => (open = false)}
          disabled={loading}
        >
          Cancel
        </Button.Root>
        <Button.Root type="submit" class="rounded-full px-6" disabled={loading || !urlValue.trim()}>
          {#if loading}
            <Spinner class="mr-2 size-4" />
            Fetching...
          {:else}
            Add
          {/if}
        </Button.Root>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
