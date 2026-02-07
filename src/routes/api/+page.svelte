<script lang="ts">
  import { onMount } from 'svelte';
  import * as Card from '$lib/components/ui/card';
  import * as Tabs from '$lib/components/ui/tabs';
  import { Button } from '$lib/components/ui/button';
  import { Separator } from '$lib/components/ui/separator';
  import { Badge } from '$lib/components/ui/badge';
  import Copy from 'phosphor-svelte/lib/Copy';
  import Check from 'phosphor-svelte/lib/Check';
  import TerminalWindow from 'phosphor-svelte/lib/TerminalWindow';
  import Plug from 'phosphor-svelte/lib/Plug';
  import { getServerConfig } from '$lib/services/api-server';

  let serverRunning = $state(false);
  let port = $state(11434); // Default, will update
  let copied = $state(false);
  
  // Use a derived val or reactive statement if possible, or just update on mount
  let baseUrl = $derived(`http://localhost:${port}/v1`);

  let pythonExample = $derived(`from openai import OpenAI

client = OpenAI(
    base_url="${baseUrl}",
    api_key="not-needed"
)

completion = client.chat.completions.create(
    model="local-model",
    messages=[
        {"role": "user", "content": "Hello!"}
    ]
)

print(completion.choices[0].message.content)`);

  let curlExample = $derived(`curl ${baseUrl}/chat/completions \\
  -H "Content-Type: application/json" \\
  -d '{
    "model": "local-model",
    "messages": [
      { "role": "user", "content": "Hello!" }
    ]
  }'`);

  onMount(async () => {
    try {
        const config = await getServerConfig();
        port = config.port;
        baseUrl = `http://localhost:${port}/v1`;
        
        // Ping server to verify responsiveness
        try {
            const res = await fetch(`http://localhost:${port}/v1/models`);
            if (res.ok) {
                serverRunning = true;
            } else {
                serverRunning = false;
            }
        } catch (e) {
            console.warn("Failed to ping local API server", e);
            serverRunning = false;
        }
    } catch (e) {
        console.error("Failed to get server config", e);
    }
  });

  function copyToClipboard(text: string) {
    navigator.clipboard.writeText(text);
    copied = true;
    setTimeout(() => (copied = false), 2000);
  }
</script>

<div class="container mx-auto p-6 max-w-5xl space-y-8">
  <div class="flex items-center justify-between">
    <div>
      <h1 class="text-3xl font-bold tracking-tight">Local Server</h1>
      <p class="text-muted-foreground mt-1">
        Run a local OpenAI-compatible API server to use with your favorite tools.
      </p>
    </div>
    <div class="flex items-center gap-2">
       <span class="flex h-3 w-3 rounded-full {serverRunning ? 'bg-green-500' : 'bg-red-500'}"></span>
       <span class="text-sm font-medium">{serverRunning ? 'Server Running' : 'Server Stopped'}</span>
    </div>
  </div>

  <div class="grid gap-6 md:grid-cols-2">
    <!-- Server Status Card -->
    <Card.Root>
      <Card.Header>
        <Card.Title class="flex items-center gap-2">
          <Plug size={20} />
          Connection Details
        </Card.Title>
      </Card.Header>
      <Card.Content class="space-y-4">
        <div class="flex flex-col gap-2">
          <span class="text-sm font-medium text-muted-foreground">Base URL</span>
          <div class="flex items-center gap-2">
            <code class="relative rounded bg-muted px-[0.3rem] py-[0.2rem] font-mono text-sm font-semibold flex-1">
              {baseUrl}
            </code>
            <Button variant="ghost" size="icon" onclick={() => copyToClipboard(baseUrl)}>
              {#if copied}
                <Check size={16} class="text-green-500" />
              {:else}
                <Copy size={16} />
              {/if}
            </Button>
          </div>
        </div>
        
        <Separator />
        
        <div class="space-y-2">
            <h4 class="text-sm font-medium">Compatible Endpoints</h4>
            <div class="flex gap-2 flex-wrap">
                <Badge variant="secondary">POST /chat/completions</Badge>
                <Badge variant="secondary">POST /completions</Badge>
                <Badge variant="secondary">POST /embeddings</Badge>
                <Badge variant="secondary">POST /responses</Badge>
                <Badge variant="secondary">GET /models</Badge>
                <Badge variant="secondary">GET /models/{'{model}'}</Badge>
                <Badge variant="outline">POST /images/* (501)</Badge>
            </div>
        </div>
      </Card.Content>
    </Card.Root>

    <!-- Quick Tips / Client Config -->
    <Card.Root>
      <Card.Header>
        <Card.Title class="flex items-center gap-2">
            <TerminalWindow size={20} />
            Quick Setup
        </Card.Title>
      </Card.Header>
      <Card.Content class="space-y-4 text-sm text-muted-foreground">
        <p>
            To use with <strong>Cursor</strong>, <strong>VS Code</strong>, or other tools:
        </p>
        <ol class="list-decimal list-inside space-y-2 ml-1">
            <li>Set the <span class="text-foreground font-medium">Base URL</span> to <code>{baseUrl}</code></li>
            <li>Set <span class="text-foreground font-medium">API Key</span> to "lm-studio" (or any string)</li>
            <li>Select any model name (usually ignored by local server)</li>
        </ol>
      </Card.Content>
    </Card.Root>
  </div>

  <Tabs.Root value="python" class="w-full">
    <div class="flex items-center justify-between mb-4">
        <h2 class="text-lg font-semibold">Code Examples</h2>
        <Tabs.List>
        <Tabs.Trigger value="python">Python</Tabs.Trigger>
        <Tabs.Trigger value="curl">cURL</Tabs.Trigger>
        </Tabs.List>
    </div>
    
    <Tabs.Content value="python">
        <Card.Root>
            <Card.Content class="p-0 overflow-hidden rounded-lg bg-slate-950 dark:bg-slate-950">
                <div class="h-[300px] w-full p-4 overflow-auto custom-scrollbar">
                     <pre class="text-sm font-mono text-slate-50"><code>{pythonExample}</code></pre>
                </div>
                 <div class="flex justify-end p-2 border-t border-slate-800 bg-slate-900">
                    <Button variant="ghost" size="sm" class="text-slate-400 hover:text-white" onclick={() => copyToClipboard(pythonExample)}>
                        <Copy size={14} class="mr-2" /> Copy
                    </Button>
                </div>
            </Card.Content>
        </Card.Root>
    </Tabs.Content>
    
    <Tabs.Content value="curl">
        <Card.Root>
            <Card.Content class="p-0 overflow-hidden rounded-lg bg-slate-950 dark:bg-slate-950">
                 <div class="h-[300px] w-full p-4 overflow-auto custom-scrollbar">
                     <pre class="text-sm font-mono text-slate-50"><code>{curlExample}</code></pre>
                </div>
                <div class="flex justify-end p-2 border-t border-slate-800 bg-slate-900">
                    <Button variant="ghost" size="sm" class="text-slate-400 hover:text-white" onclick={() => copyToClipboard(curlExample)}>
                        <Copy size={14} class="mr-2" /> Copy
                    </Button>
                </div>
            </Card.Content>
        </Card.Root>
    </Tabs.Content>
  </Tabs.Root>
</div>
