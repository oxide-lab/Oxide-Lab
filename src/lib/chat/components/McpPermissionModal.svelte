<script lang="ts">
  import Tool from '$lib/components/ai-elements/tool/Tool.svelte';
  import ToolHeader from '$lib/components/ai-elements/tool/ToolHeader.svelte';
  import ToolInput from '$lib/components/ai-elements/tool/ToolInput.svelte';
  import * as Collapsible from '$lib/components/ui/collapsible/index.js';
  import * as Dialog from '$lib/components/ui/dialog/index.js';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import type { McpPermissionDecision, McpToolPermissionRequestEvent } from '$lib/chat/types';

  interface Props {
    request?: McpToolPermissionRequestEvent | null;
    onDecision?: (requestId: string, decision: McpPermissionDecision) => void | Promise<void>;
  }

  let { request = null, onDecision }: Props = $props();
  let resolving = $state(false);

  let open = $derived(Boolean(request));
  let toolType = $derived(
    request ? `${request.server_id}/${request.tool_name}` : 'MCP Tool',
  );
  let toolInput = $derived((request?.arguments ?? {}) as Record<string, unknown>);

  async function decide(decision: McpPermissionDecision) {
    if (!request || resolving) return;
    resolving = true;
    try {
      await onDecision?.(request.request_id, decision);
    } finally {
      resolving = false;
    }
  }
</script>

<Dialog.Root {open}>
  <Dialog.Content class="sm:max-w-[560px]">
    <Dialog.Header>
      <Dialog.Title>MCP tool permission</Dialog.Title>
      <Dialog.Description>
        The model requested a tool call. Review payload and choose permission scope.
      </Dialog.Description>
    </Dialog.Header>

    {#if request}
      <div class="space-y-3">
        <div class="flex flex-wrap items-center gap-2">
          <Badge variant="outline">Server: {request.server_id}</Badge>
          <Badge variant="secondary">Tool: {request.tool_name}</Badge>
        </div>
        <Tool class="mb-0" open>
          <ToolHeader type={toolType} state="input-streaming" />
          <Collapsible.Content>
            <ToolInput input={toolInput} />
          </Collapsible.Content>
        </Tool>
      </div>

      <Dialog.Footer class="mt-4 flex-wrap gap-2">
        <Button variant="destructive" onclick={() => decide('deny')} disabled={resolving}>Deny</Button>
        <Button variant="outline" onclick={() => decide('allow_once')} disabled={resolving}>
          Allow once
        </Button>
        <Button variant="outline" onclick={() => decide('allow_this_session')} disabled={resolving}>
          Allow this session
        </Button>
        <Button onclick={() => decide('allow_this_server')} disabled={resolving}>
          Allow this server
        </Button>
      </Dialog.Footer>
    {/if}
  </Dialog.Content>
</Dialog.Root>
