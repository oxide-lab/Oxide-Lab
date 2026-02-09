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
  import type { McpServerConfig, WebRagSettings } from '$lib/types/settings-v2';

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

  type McpToolDescriptor = {
    server_id: string;
    name: string;
    description?: string | null;
    input_schema: unknown;
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

  let mcpServerId = $state('');
  let mcpServerTransport = $state<'stdio' | 'streamable_http'>('stdio');
  let mcpServerCommand = $state('');
  let mcpServerArgs = $state('');
  let mcpServerUrl = $state('');
  let mcpServerHeadersJson = $state('{}');
  let mcpServerEnvJson = $state('{}');
  let mcpTools = $state<McpToolDescriptor[]>([]);
  let selectedMcpTool = $state('');
  let mcpArgsJson = $state('{}');
  let mcpRunBusy = $state(false);
  let mcpRunResult = $state('');

  function updateUrlFetch(patch: Partial<WebRagSettings['url_fetch']>) {
    onChange({
      ...value,
      url_fetch: {
        ...value.url_fetch,
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

  function updateMcp(patch: Partial<WebRagSettings['mcp']>) {
    onChange({
      ...value,
      mcp: {
        ...value.mcp,
        ...patch,
      },
    });
  }

  function addMcpServer() {
    const id = mcpServerId.trim();
    if (!id) return;
    const args = mcpServerArgs
      .split(' ')
      .map((item) => item.trim())
      .filter(Boolean);
    const headers = parseStringMapJson(mcpServerHeadersJson, 'MCP headers');
    if (!headers) return;
    const env = parseStringMapJson(mcpServerEnvJson, 'MCP env');
    if (!env) return;
    if (
      mcpServerTransport === 'streamable_http' &&
      mcpServerUrl.trim().includes('mcp.context7.com') &&
      !hasContext7Auth(headers)
    ) {
      statusText =
        'Context7 requires headers with CONTEXT7_API_KEY or Authorization for streamable_http transport.';
      return;
    }
    const server: McpServerConfig = {
      id,
      enabled: true,
      transport: mcpServerTransport,
      command: mcpServerTransport === 'stdio' ? mcpServerCommand.trim() : null,
      args,
      url: mcpServerTransport === 'streamable_http' ? mcpServerUrl.trim() : null,
      headers,
      env,
    };
    updateMcp({
      servers: [...value.mcp.servers.filter((entry) => entry.id !== id), server],
    });
    mcpServerId = '';
    mcpServerCommand = '';
    mcpServerArgs = '';
    mcpServerUrl = '';
    mcpServerHeadersJson = '{}';
    mcpServerEnvJson = '{}';
  }

  function hasContext7Auth(headers: Record<string, string>) {
    return Object.keys(headers).some((key) => {
      const normalized = key.trim().toLowerCase();
      return (
        normalized === 'context7_api_key' ||
        normalized === 'authorization' ||
        normalized === 'context7-api-key' ||
        normalized === 'x-context7-api-key' ||
        normalized === 'x-api-key'
      );
    });
  }

  function parseStringMapJson(raw: string, label: string): Record<string, string> | null {
    const trimmed = raw.trim();
    if (!trimmed) return {};
    try {
      const parsed = JSON.parse(trimmed);
      if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
        statusText = `${label} must be a JSON object`;
        return null;
      }
      const out: Record<string, string> = {};
      for (const [key, value] of Object.entries(parsed)) {
        if (typeof value !== 'string') {
          statusText = `${label} values must be strings`;
          return null;
        }
        out[String(key)] = value;
      }
      return out;
    } catch (err) {
      statusText = `Invalid ${label} JSON: ${String(err)}`;
      return null;
    }
  }

  function removeMcpServer(id: string) {
    updateMcp({
      servers: value.mcp.servers.filter((entry) => entry.id !== id),
    });
  }

  function toggleMcpServer(id: string, enabled: boolean) {
    updateMcp({
      servers: value.mcp.servers.map((entry) =>
        entry.id === id ? { ...entry, enabled } : entry,
      ),
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
    if (typeof selected === 'string') sourcePath = selected;
  }

  async function pickFile() {
    const selected = await openDialog({
      directory: false,
      multiple: false,
      title: 'Select source file',
      filters: [{ name: 'Documents', extensions: ['txt', 'md', 'pdf'] }],
    });
    if (typeof selected === 'string') sourcePath = selected;
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

  async function restartMcpServers() {
    try {
      await invoke('mcp_restart_servers');
      await refreshMcpTools();
      statusText = 'MCP servers restarted';
    } catch (err) {
      statusText = `Failed to restart MCP servers: ${String(err)}`;
    }
  }

  async function refreshMcpTools() {
    try {
      const tools = await invoke<McpToolDescriptor[]>('mcp_list_tools');
      mcpTools = tools;
      if (tools.length > 0 && !selectedMcpTool) {
        selectedMcpTool = `${tools[0].server_id}::${tools[0].name}`;
      }
    } catch (err) {
      mcpTools = [];
      statusText = `Failed to list MCP tools: ${String(err)}`;
    }
  }

  function selectedMcpRoute() {
    const [serverId, toolName] = selectedMcpTool.split('::');
    if (!serverId || !toolName) return null;
    return { serverId, toolName };
  }

  async function runMcpTool() {
    const route = selectedMcpRoute();
    if (!route || mcpRunBusy) return;
    let parsedArgs: Record<string, unknown> = {};
    try {
      const parsed = JSON.parse(mcpArgsJson || '{}');
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
        parsedArgs = parsed as Record<string, unknown>;
      } else {
        statusText = 'Tool arguments must be a JSON object';
        return;
      }
    } catch (err) {
      statusText = `Invalid JSON args: ${String(err)}`;
      return;
    }

    mcpRunBusy = true;
    statusText = `Running MCP tool ${route.serverId}/${route.toolName}...`;
    let stopPermissionListener: (() => void) | null = null;
    try {
      const { listen } = await import('@tauri-apps/api/event');
      stopPermissionListener = await listen<{ request_id?: string }>(
        'mcp_tool_permission_request',
        async (event) => {
          const requestId = event.payload?.request_id;
          if (!requestId) return;
          try {
            await invoke('mcp_resolve_tool_permission', {
              requestId,
              decision: 'allow_once',
            });
          } catch {
            /* ignore manual-run permission race */
          }
        },
      );
      const result = await invoke<unknown>('mcp_call_tool', {
        serverId: route.serverId,
        toolName: route.toolName,
        arguments: parsedArgs,
      });
      mcpRunResult = JSON.stringify(result, null, 2);
      statusText = 'MCP tool call completed';
    } catch (err) {
      mcpRunResult = '';
      statusText = `MCP tool call failed: ${String(err)}`;
    } finally {
      if (stopPermissionListener) {
        try {
          stopPermissionListener();
        } catch {
          /* ignore */
        }
      }
      mcpRunBusy = false;
    }
  }

  async function copyMcpResult() {
    if (!mcpRunResult) return;
    await navigator.clipboard.writeText(mcpRunResult);
    statusText = 'Tool result copied to clipboard';
  }

  function insertMcpResultIntoPrompt() {
    if (!mcpRunResult) return;
    window.dispatchEvent(
      new CustomEvent('oxide:insert-prompt', {
        detail: {
          text: `MCP tool result:\n${mcpRunResult}`,
        },
      }),
    );
    statusText = 'Tool result inserted into composer prompt';
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
    void refreshMcpTools();
  });
</script>

<Card.Root>
  <Card.Header>
    <Card.Title>URL Fetch + Document RAG + MCP</Card.Title>
    <Card.Description>
      Configure URL retrieval, local document index, embeddings provider, and MCP tool servers.
    </Card.Description>
  </Card.Header>
  <Card.Content class="space-y-3">
    <SettingRow
      id="web_rag.url_fetch.enabled_by_default"
      title="Enable URL fetch by default"
      description="Composer starts with URL retrieval toggle enabled."
      highlighted={highlightedSettingId === 'web_rag.url_fetch.enabled_by_default'}
      controlPosition="start"
    >
      <Checkbox
        checked={value.url_fetch.enabled_by_default}
        onCheckedChange={(checked) => updateUrlFetch({ enabled_by_default: checked === true })}
      />
    </SettingRow>

    <SettingRow
      id="web_rag.url_fetch.max_urls"
      title="Max URLs per request"
      description="Upper bound for fetched URLs."
      highlighted={highlightedSettingId === 'web_rag.url_fetch.max_urls'}
    >
      <Input
        type="number"
        min={1}
        max={50}
        value={String(value.url_fetch.max_urls)}
        onblur={(event) =>
          updateUrlFetch({
            max_urls: Math.max(1, Math.min(50, Number((event.currentTarget as HTMLInputElement).value) || 6)),
          })}
      />
    </SettingRow>

    <SettingRow
      id="web_rag.url_fetch.max_chars_per_page"
      title="Max chars per page"
      description="Maximum extracted text length from one URL."
      highlighted={highlightedSettingId === 'web_rag.url_fetch.max_chars_per_page'}
    >
      <Input
        type="number"
        min={200}
        value={String(value.url_fetch.max_chars_per_page)}
        onblur={(event) =>
          updateUrlFetch({
            max_chars_per_page: Math.max(200, Number((event.currentTarget as HTMLInputElement).value) || 5000),
          })}
      />
    </SettingRow>

    <SettingRow
      id="web_rag.url_fetch.max_total_tokens"
      title="Max retrieval tokens"
      description="Upper retrieval context budget before prompt assembly."
      highlighted={highlightedSettingId === 'web_rag.url_fetch.max_total_tokens'}
    >
      <Input
        type="number"
        min={64}
        value={String(value.url_fetch.max_total_tokens)}
        onblur={(event) =>
          updateUrlFetch({
            max_total_tokens: Math.max(64, Number((event.currentTarget as HTMLInputElement).value) || 1200),
          })}
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
      id="web_rag.mcp.enabled"
      title="Enable MCP tools"
      description="Allow tool calls through configured MCP servers."
      highlighted={highlightedSettingId === 'web_rag.mcp.enabled'}
      controlPosition="start"
    >
      <Checkbox
        checked={value.mcp.enabled}
        onCheckedChange={(checked) => updateMcp({ enabled: checked === true })}
      />
    </SettingRow>

    <SettingRow
      id="web_rag.mcp.default_permission_mode"
      title="MCP permission mode"
      description="Default permission strategy for tool calls."
      highlighted={highlightedSettingId === 'web_rag.mcp.default_permission_mode'}
    >
      <Select.Root
        type="single"
        value={value.mcp.default_permission_mode}
        onValueChange={(next) =>
          updateMcp({
            default_permission_mode:
              (next as WebRagSettings['mcp']['default_permission_mode']) ?? 'per_call',
          })}
      >
        <Select.Trigger class="w-full">{value.mcp.default_permission_mode}</Select.Trigger>
        <Select.Content>
          <Select.Item value="per_call" label="per_call" />
          <Select.Item value="allow_this_session" label="allow_this_session" />
          <Select.Item value="allow_this_server" label="allow_this_server" />
        </Select.Content>
      </Select.Root>
    </SettingRow>

    <SettingRow
      id="web_rag.mcp.max_tool_rounds"
      title="MCP max tool rounds"
      description="Hard stop for agent tool-call loop."
      highlighted={highlightedSettingId === 'web_rag.mcp.max_tool_rounds'}
    >
      <Input
        type="number"
        min={1}
        max={16}
        value={String(value.mcp.max_tool_rounds)}
        onblur={(event) =>
          updateMcp({
            max_tool_rounds: Math.max(1, Math.min(16, Number((event.currentTarget as HTMLInputElement).value) || 4)),
          })}
      />
    </SettingRow>

    <SettingRow
      id="web_rag.mcp.tool_call_timeout_ms"
      title="MCP tool timeout (ms)"
      description="Per-call timeout for MCP tool execution."
      highlighted={highlightedSettingId === 'web_rag.mcp.tool_call_timeout_ms'}
    >
      <Input
        type="number"
        min={1000}
        max={300000}
        value={String(value.mcp.tool_call_timeout_ms)}
        onblur={(event) =>
          updateMcp({
            tool_call_timeout_ms: Math.max(
              1000,
              Math.min(300000, Number((event.currentTarget as HTMLInputElement).value) || 20000),
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
      <Button variant="outline" size="sm" onclick={restartMcpServers} disabled={sourceBusy}>
        Restart MCP servers
      </Button>
      <Button variant="destructive" size="sm" onclick={clearIndex} disabled={sourceBusy}>
        Clear index
      </Button>
    </div>

    <Card.Root class="border-dashed">
      <Card.Header class="space-y-2">
        <Card.Title class="text-base">MCP servers</Card.Title>
        <Card.Description>Add minimal server configs for stdio or streamable_http.</Card.Description>
      </Card.Header>
      <Card.Content class="space-y-3">
        <div class="grid gap-2 sm:grid-cols-2">
          <Input bind:value={mcpServerId} placeholder="Server id" />
          <Select.Root type="single" value={mcpServerTransport} onValueChange={(v) => (mcpServerTransport = (v as 'stdio' | 'streamable_http') ?? 'stdio')}>
            <Select.Trigger>{mcpServerTransport}</Select.Trigger>
            <Select.Content>
              <Select.Item value="stdio" label="stdio" />
              <Select.Item value="streamable_http" label="streamable_http" />
            </Select.Content>
          </Select.Root>
          {#if mcpServerTransport === 'stdio'}
            <Input bind:value={mcpServerCommand} placeholder="Command (npx / uvx / python)" />
            <Input bind:value={mcpServerArgs} placeholder="Args (space-separated)" />
          {:else}
            <Input class="sm:col-span-2" bind:value={mcpServerUrl} placeholder="https://example.com/mcp" />
          {/if}
          <Input
            class="sm:col-span-2"
            bind:value={mcpServerHeadersJson}
            placeholder={"Headers JSON, for example: {\"Context7-API-Key\":\"...\"}"}
          />
          <Input
            class="sm:col-span-2"
            bind:value={mcpServerEnvJson}
            placeholder={"Env JSON, for example: {\"PATH\":\"...\"}"}
          />
        </div>
        <Button size="sm" onclick={addMcpServer} disabled={!mcpServerId.trim()}>
          Add MCP server
        </Button>
        {#if value.mcp.servers.length === 0}
          <p class="text-xs text-muted-foreground">No MCP servers configured yet.</p>
        {:else}
          <div class="space-y-2">
            {#each value.mcp.servers as server (server.id)}
              <div class="rounded-md border border-border/70 bg-background/70 p-3">
                <div class="flex items-center justify-between gap-2">
                  <p class="truncate text-sm font-medium">{server.id}</p>
                  <Checkbox
                    checked={server.enabled}
                    onCheckedChange={(checked) => toggleMcpServer(server.id, checked === true)}
                  />
                </div>
                <p class="text-xs text-muted-foreground">
                  {server.transport === 'stdio'
                    ? `${server.command ?? ''} ${(server.args ?? []).join(' ')}`
                    : server.url}
                </p>
                {#if Object.keys(server.headers ?? {}).length > 0 || Object.keys(server.env ?? {}).length > 0}
                  <p class="text-[11px] text-muted-foreground">
                    headers: {Object.keys(server.headers ?? {}).length} | env: {Object.keys(server.env ?? {}).length}
                  </p>
                {/if}
                <div class="mt-2 flex flex-wrap gap-2">
                  <Button size="sm" variant="destructive" onclick={() => removeMcpServer(server.id)}>
                    Remove
                  </Button>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </Card.Content>
    </Card.Root>

    <Card.Root class="border-dashed">
      <Card.Header class="space-y-2">
        <Card.Title class="text-base">MCP manual tool runner</Card.Title>
        <Card.Description>Invoke discovered tools with JSON args for quick validation.</Card.Description>
      </Card.Header>
      <Card.Content class="space-y-3">
        {#if mcpTools.length === 0}
          <p class="text-xs text-muted-foreground">
            No tools available. Start MCP servers and click “Restart MCP servers”.
          </p>
        {:else}
          <Select.Root
            type="single"
            value={selectedMcpTool}
            onValueChange={(v) => (selectedMcpTool = String(v ?? ''))}
          >
            <Select.Trigger class="w-full">{selectedMcpTool || 'Select tool'}</Select.Trigger>
            <Select.Content>
              {#each mcpTools as tool (tool.server_id + '::' + tool.name)}
                <Select.Item
                  value={tool.server_id + '::' + tool.name}
                  label={tool.server_id + ' / ' + tool.name}
                />
              {/each}
            </Select.Content>
          </Select.Root>
          <Input bind:value={mcpArgsJson} placeholder="JSON object, for example: query=hello" />
          <div class="flex flex-wrap gap-2">
            <Button size="sm" onclick={runMcpTool} disabled={mcpRunBusy || !selectedMcpTool}>
              Run tool
            </Button>
            <Button size="sm" variant="outline" onclick={copyMcpResult} disabled={!mcpRunResult}>
              Copy result
            </Button>
            <Button
              size="sm"
              variant="outline"
              onclick={insertMcpResultIntoPrompt}
              disabled={!mcpRunResult}
            >
              Insert into prompt
            </Button>
          </div>
          {#if mcpRunResult}
            <pre class="max-h-56 overflow-auto rounded-md border border-border/70 bg-background/70 p-3 text-xs">{mcpRunResult}</pre>
          {/if}
        {/if}
      </Card.Content>
    </Card.Root>

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
