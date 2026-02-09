<script lang="ts">
  /**
   * Models Page
   *
   * Manage local and remote GGUF models.
   */
  import { onMount, onDestroy } from 'svelte';
  import { t } from '$lib/i18n';
  import { folderPath, scanFolder } from '$lib/stores/local-models';
  import { setPageTabs, clearPageTabs, activePageTab } from '$lib/stores/page-tabs.svelte';
  import LocalModelsPanel from '$lib/components/model-manager/LocalModelsPanel.svelte';
  import RemoteModelsPanel from '$lib/components/model-manager/RemoteModelsPanel.svelte';

  type TabId = 'local' | 'remote';

  let activeTab = $state<TabId>('local');

  const tabs = $derived([
    { id: 'local' as TabId, label: $t('models.tabs.local') || 'Local Models' },
    { id: 'remote' as TabId, label: $t('models.tabs.remote') || 'Remote Models' },

  ]);

  onMount(() => {
    setPageTabs(tabs, 'local');
    if ($folderPath) {
      scanFolder($folderPath).catch((error) => console.error(error));
    }
  });

  onDestroy(() => {
    clearPageTabs();
  });

  $effect(() => {
    activeTab = ($activePageTab || 'local') as TabId;
  });

</script>

<div class="h-full overflow-hidden p-4 flex flex-col">
  <!-- Main Content -->
  <div class="flex-1 min-h-0">
    {#if activeTab === 'local'}
      <LocalModelsPanel />
    {:else if activeTab === 'remote'}
      <RemoteModelsPanel />

    {/if}
  </div>
</div>
