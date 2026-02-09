<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import { openPath } from '@tauri-apps/plugin-opener';
  import * as Card from '$lib/components/ui/card';
  import { Button } from '$lib/components/ui/button';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Input } from '$lib/components/ui/input';
  import * as Select from '$lib/components/ui/select';
  import SettingRow from '$lib/components/settings/SettingRow.svelte';
  import type { WebRagSettings } from '$lib/types/settings-v2';

  type LocalRagSourceRecord = {
    id: string;
    path: string;
    kind: string;
    created_at: number;
    updated_at: number;
  };

  type LocalRagStats = {
    sqlite_vec_available: boolean;
    sources_count: number;
    documents_count: number;
    chunks_count: number;
  };

  interface Props {
    value: WebRagSettings;
    highlightedSettingId?: string | null;
    onChange: (next: WebRagSettings) => void;
  }

  let { value, highlightedSettingId = null, onChange }: Props = $props();

  let sourcePath = $state('');
  let sources = $state<LocalRagSourceRecord[]>([]);
  let stats = $state<LocalRagStats | null>(null);
  let sourceBusy = $state(false);
  let statusText = $state('');

  function updateWebSearch(patch: Partial<WebRagSettings['web_search']>) {
    onChange({
      ...value,
      web_search: {
        ...value.web_search,
        ...patch,
      },
    });
  }

  function updateLocalRag(patch: Partial<WebRagSettings['local_rag']>) {
    const nextLocal = {
      ...value.local_rag,
      ...patch,
    };
    if (nextLocal.chunk_overlap_chars >= nextLocal.chunk_size_chars) {
      nextLocal.chunk_overlap_chars = Math.max(0, nextLocal.chunk_size_chars - 1);
    }
    onChange({
      ...value,
      local_rag: nextLocal,
    });
  }

  function updateEmbeddings(patch: Partial<WebRagSettings['embeddings_provider']>) {
    onChange({
      ...value,
      embeddings_provider: {
        ...value.embeddings_provider,
        ...patch,
      },
    });
  }

  async function refreshSources() {
    try {
      const [sourceRows, ragStats] = await Promise.all([
        invoke<LocalRagSourceRecord[]>('rag_list_sources'),
        invoke<LocalRagStats>('rag_get_stats'),
      ]);
      sources = sourceRows;
      stats = ragStats;
    } catch (err) {
      statusText = `Failed to load Local RAG state: ${String(err)}`;
    }
  }

  async function pickFolder() {
    const selected = await openDialog({ directory: true, multiple: false, title: 'Select source folder' });
    if (typeof selected === 'string') {
      sourcePath = selected;
    }
  }

  async function pickFile() {
    const selected = await openDialog({
      directory: false,
      multiple: false,
      title: 'Select source file',
      filters: [{ name: 'Documents', extensions: ['txt', 'md', 'pdf'] }],
    });
    if (typeof selected === 'string') {
      sourcePath = selected;
    }
  }

  async function addSource() {
    const path = sourcePath.trim();
    if (!path || sourceBusy) return;
    sourceBusy = true;
    statusText = 'Indexing source...';
    try {
      await invoke('rag_add_source', { path });
      sourcePath = '';
      statusText = 'Source indexed';
      await refreshSources();
    } catch (err) {
      statusText = `Failed to index source: ${String(err)}`;
    } finally {
      sourceBusy = false;
    }
  }

  async function reindexSource(sourceId: string) {
    if (sourceBusy) return;
    sourceBusy = true;
    statusText = 'Reindexing source...';
    try {
      await invoke('rag_reindex_source', { sourceId });
      statusText = 'Source reindexed';
      await refreshSources();
    } catch (err) {
      statusText = `Failed to reindex source: ${String(err)}`;
    } finally {
      sourceBusy = false;
    }
  }

  async function removeSource(sourceId: string) {
    if (sourceBusy) return;
    sourceBusy = true;
    statusText = 'Removing source...';
    try {
      await invoke('rag_remove_source', { sourceId });
      statusText = 'Source removed';
      await refreshSources();
    } catch (err) {
      statusText = `Failed to remove source: ${String(err)}`;
    } finally {
      sourceBusy = false;
    }
  }

  async function clearIndex() {
    if (sourceBusy) return;
    sourceBusy = true;
    statusText = 'Clearing index...';
    try {
      await invoke('rag_clear_index');
      statusText = 'Index cleared';
      await refreshSources();
    } catch (err) {
      statusText = `Failed to clear index: ${String(err)}`;
    } finally {
      sourceBusy = false;
    }
  }

  async function testEmbeddingsProvider() {
    if (sourceBusy) return;
    sourceBusy = true;
    statusText = 'Testing embeddings provider...';
    try {
      await invoke('rag_test_embeddings_provider');
      statusText = 'Embeddings provider is available';
    } catch (err) {
      statusText = `Embeddings provider check failed: ${String(err)}`;
    } finally {
      sourceBusy = false;
    }
  }

  async function openSourceFolder(sourceId: string) {
    try {
      const folderPath = await invoke<string>('rag_open_source_folder', { sourceId });
      await openPath(folderPath);
    } catch (err) {
      statusText = `Failed to open source folder: ${String(err)}`;
    }
  }

  onMount(() => {
    void refreshSources();
  });
</script>

<Card.Root>
  <Card.Header>
    <Card.Title>Web Search + RAG</Card.Title>
    <Card.Description>
      Configure Lite/Pro web retrieval and Local RAG sources.
    </Card.Description>
  </Card.Header>
  <Card.Content class="space-y-3">
    <SettingRow
      id="web_rag.web_search.default_mode"
      title="Default Web Search Mode"
      description="Composer default retrieval mode."
      highlighted={highlightedSettingId === 'web_rag.web_search.default_mode'}
    >
      <Select.Root
        type="single"
        value={value.web_search.default_mode}
        onValueChange={(next) =>
          updateWebSearch({
            default_mode: (next as WebRagSettings['web_search']['default_mode']) ?? 'lite',
          })}
      >
        <Select.Trigger class="w-full">
          {value.web_search.default_mode.toUpperCase()}
        </Select.Trigger>
        <Select.Content>
          <Select.Item value="off" label="OFF" />
          <Select.Item value="lite" label="LITE" />
          <Select.Item value="pro" label="PRO" />
        </Select.Content>
      </Select.Root>
    </SettingRow>

    <SettingRow
      id="web_rag.web_search.pro_beta_enabled"
      title="Enable Search Pro (beta)"
      description="Pro mode requires embeddings provider."
      highlighted={highlightedSettingId === 'web_rag.web_search.pro_beta_enabled'}
      controlPosition="start"
    >
      <Checkbox
        checked={value.web_search.pro_beta_enabled}
        onCheckedChange={(checked) => updateWebSearch({ pro_beta_enabled: checked === true })}
      />
    </SettingRow>

    <SettingRow
      id="web_rag.local_rag.beta_enabled"
      title="Enable Local RAG (beta)"
      description="Use indexed local files as retrieval context."
      highlighted={highlightedSettingId === 'web_rag.local_rag.beta_enabled'}
      controlPosition="start"
    >
      <Checkbox
        checked={value.local_rag.beta_enabled}
        onCheckedChange={(checked) => updateLocalRag({ beta_enabled: checked === true })}
      />
    </SettingRow>

    <SettingRow
      id="web_rag.local_rag.top_k"
      title="Local RAG top-k"
      description="Maximum retrieved chunks per query."
      highlighted={highlightedSettingId === 'web_rag.local_rag.top_k'}
    >
      <Input
        type="number"
        min={1}
        max={30}
        value={String(value.local_rag.top_k)}
        onblur={(event) =>
          updateLocalRag({
            top_k: Math.max(1, Math.min(30, Number((event.currentTarget as HTMLInputElement).value) || 5)),
          })}
      />
    </SettingRow>

    <SettingRow
      id="web_rag.local_rag.chunk_size_chars"
      title="Chunk size (chars)"
      description="Chunk size for local indexing."
      highlighted={highlightedSettingId === 'web_rag.local_rag.chunk_size_chars'}
    >
      <Input
        type="number"
        min={300}
        max={8000}
        value={String(value.local_rag.chunk_size_chars)}
        onblur={(event) =>
          updateLocalRag({
            chunk_size_chars: Math.max(
              300,
              Math.min(8000, Number((event.currentTarget as HTMLInputElement).value) || 1200),
            ),
          })}
      />
    </SettingRow>

    <SettingRow
      id="web_rag.local_rag.chunk_overlap_chars"
      title="Chunk overlap (chars)"
      description="Overlap between adjacent chunks."
      highlighted={highlightedSettingId === 'web_rag.local_rag.chunk_overlap_chars'}
    >
      <Input
        type="number"
        min={0}
        max={2000}
        value={String(value.local_rag.chunk_overlap_chars)}
        onblur={(event) =>
          updateLocalRag({
            chunk_overlap_chars: Math.max(
              0,
              Math.min(2000, Number((event.currentTarget as HTMLInputElement).value) || 180),
            ),
          })}
      />
    </SettingRow>

    <SettingRow
      id="web_rag.local_rag.max_file_size_mb"
      title="Max file size (MB)"
      description="Files above this limit are skipped during indexing."
      highlighted={highlightedSettingId === 'web_rag.local_rag.max_file_size_mb'}
    >
      <Input
        type="number"
        min={1}
        max={1024}
        value={String(value.local_rag.max_file_size_mb)}
        onblur={(event) =>
          updateLocalRag({
            max_file_size_mb: Math.max(
              1,
              Math.min(1024, Number((event.currentTarget as HTMLInputElement).value) || 20),
            ),
          })}
      />
    </SettingRow>

    <SettingRow
      id="web_rag.web_search.max_snippets"
      title="Max snippets (Lite)"
      description="Maximum snippets fetched from DDG Lite."
      highlighted={highlightedSettingId === 'web_rag.web_search.max_snippets'}
    >
      <Input
        type="number"
        min={1}
        max={25}
        value={String(value.web_search.max_snippets)}
        onblur={(event) =>
          updateWebSearch({
            max_snippets: Math.max(1, Math.min(25, Number((event.currentTarget as HTMLInputElement).value) || 8)),
          })}
      />
    </SettingRow>

    <SettingRow
      id="web_rag.web_search.max_pages"
      title="Max pages (Pro)"
      description="How many pages to fetch in Pro mode."
      highlighted={highlightedSettingId === 'web_rag.web_search.max_pages'}
    >
      <Input
        type="number"
        min={1}
        max={10}
        value={String(value.web_search.max_pages)}
        onblur={(event) =>
          updateWebSearch({
            max_pages: Math.max(1, Math.min(10, Number((event.currentTarget as HTMLInputElement).value) || 5)),
          })}
      />
    </SettingRow>

    <SettingRow
      id="web_rag.web_search.max_snippet_chars"
      title="Max snippet chars"
      description="Per-snippet character limit for Lite mode."
      highlighted={highlightedSettingId === 'web_rag.web_search.max_snippet_chars'}
    >
      <Input
        type="number"
        min={120}
        max={2000}
        value={String(value.web_search.max_snippet_chars)}
        onblur={(event) =>
          updateWebSearch({
            max_snippet_chars: Math.max(
              120,
              Math.min(2000, Number((event.currentTarget as HTMLInputElement).value) || 500),
            ),
          })}
      />
    </SettingRow>

    <SettingRow
      id="web_rag.web_search.max_retrieval_tokens"
      title="Max retrieval tokens"
      description="Upper retrieval context budget before prompt assembly."
      highlighted={highlightedSettingId === 'web_rag.web_search.max_retrieval_tokens'}
    >
      <Input
        type="number"
        min={64}
        value={String(value.web_search.max_retrieval_tokens)}
        onblur={(event) =>
          updateWebSearch({
            max_retrieval_tokens: Math.max(64, Number((event.currentTarget as HTMLInputElement).value) || 1200),
          })}
      />
    </SettingRow>

    <SettingRow
      id="web_rag.embeddings_provider.base_url"
      title="Embeddings API URL"
      description="OpenAI-compatible endpoint (for example http://localhost:11434/v1)."
      highlighted={highlightedSettingId === 'web_rag.embeddings_provider.base_url'}
    >
      <Input
        value={value.embeddings_provider.base_url}
        placeholder="http://localhost:11434/v1"
        onblur={(event) =>
          updateEmbeddings({ base_url: (event.currentTarget as HTMLInputElement).value.trim() })}
      />
    </SettingRow>

    <SettingRow
      id="web_rag.embeddings_provider.model"
      title="Embeddings model"
      description="Provider model name for /embeddings."
      highlighted={highlightedSettingId === 'web_rag.embeddings_provider.model'}
    >
      <Input
        value={value.embeddings_provider.model}
        placeholder="nomic-embed-text"
        onblur={(event) => updateEmbeddings({ model: (event.currentTarget as HTMLInputElement).value.trim() })}
      />
    </SettingRow>

    <SettingRow
      id="web_rag.embeddings_provider.api_key"
      title="Embeddings API key"
      description="Optional bearer token."
      highlighted={highlightedSettingId === 'web_rag.embeddings_provider.api_key'}
    >
      <Input
        type="password"
        value={value.embeddings_provider.api_key}
        placeholder="sk-..."
        onblur={(event) => updateEmbeddings({ api_key: (event.currentTarget as HTMLInputElement).value })}
      />
    </SettingRow>

    <SettingRow
      id="web_rag.embeddings_provider.timeout_ms"
      title="Embeddings timeout (ms)"
      description="HTTP timeout for provider requests."
      highlighted={highlightedSettingId === 'web_rag.embeddings_provider.timeout_ms'}
    >
      <Input
        type="number"
        min={1000}
        max={180000}
        value={String(value.embeddings_provider.timeout_ms)}
        onblur={(event) =>
          updateEmbeddings({
            timeout_ms: Math.max(
              1000,
              Math.min(180000, Number((event.currentTarget as HTMLInputElement).value) || 20000),
            ),
          })}
      />
    </SettingRow>

    <div class="flex flex-wrap items-center gap-2">
      <Button variant="outline" size="sm" onclick={testEmbeddingsProvider} disabled={sourceBusy}>
        Test embeddings provider
      </Button>
      <Button variant="outline" size="sm" onclick={refreshSources} disabled={sourceBusy}>
        Refresh sources
      </Button>
      <Button variant="destructive" size="sm" onclick={clearIndex} disabled={sourceBusy}>
        Clear index
      </Button>
    </div>

    <Card.Root class="border-dashed">
      <Card.Header class="space-y-2">
        <Card.Title class="text-base">Local RAG sources</Card.Title>
        <Card.Description>
          sqlite-vec: {stats?.sqlite_vec_available ? 'available' : 'unavailable'} | sources: {stats?.sources_count ?? 0} |
          docs: {stats?.documents_count ?? 0} | chunks: {stats?.chunks_count ?? 0}
        </Card.Description>
      </Card.Header>
      <Card.Content class="space-y-3">
        <div class="flex flex-col gap-2 sm:flex-row">
          <Input bind:value={sourcePath} placeholder="Path to folder or file (.txt/.md/.pdf)" />
          <Button variant="outline" size="sm" onclick={pickFolder} disabled={sourceBusy}>Pick folder</Button>
          <Button variant="outline" size="sm" onclick={pickFile} disabled={sourceBusy}>Pick file</Button>
          <Button size="sm" onclick={addSource} disabled={sourceBusy || !sourcePath.trim()}>Add / Index</Button>
        </div>

        {#if statusText}
          <div class="rounded-md border border-border/80 bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
            {statusText}
          </div>
        {/if}

        {#if sources.length === 0}
          <p class="text-xs text-muted-foreground">No indexed sources yet.</p>
        {:else}
          <div class="space-y-2">
            {#each sources as source (source.id)}
              <div class="rounded-md border border-border/70 bg-background/70 p-3">
                <p class="truncate text-sm font-medium">{source.path}</p>
                <p class="text-xs text-muted-foreground">
                  {source.kind} | updated {new Date(source.updated_at * 1000).toLocaleString()}
                </p>
                <div class="mt-2 flex flex-wrap gap-2">
                  <Button size="sm" variant="outline" onclick={() => reindexSource(source.id)} disabled={sourceBusy}>
                    Reindex
                  </Button>
                  <Button size="sm" variant="outline" onclick={() => openSourceFolder(source.id)} disabled={sourceBusy}>
                    Open folder
                  </Button>
                  <Button size="sm" variant="destructive" onclick={() => removeSource(source.id)} disabled={sourceBusy}>
                    Remove
                  </Button>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </Card.Content>
    </Card.Root>
  </Card.Content>
</Card.Root>
