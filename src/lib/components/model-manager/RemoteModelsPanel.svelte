<script lang="ts">
  import { get } from 'svelte/store';
  import { onDestroy, onMount } from 'svelte';
  import * as Select from '$lib/components/ui/select';
  import * as Progress from '$lib/components/ui/progress';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Pagination from '$lib/components/ui/pagination';
  import * as Separator from '$lib/components/ui/separator';
  import * as Slider from '$lib/components/ui/slider';
  import { Input } from '$lib/components/ui/input';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Spinner } from '$lib/components/ui/spinner';
  import { Skeleton } from '$lib/components/ui/skeleton';
  import { Markdown } from '$lib/components/ai-elements/markdown';
  import { t } from '$lib/i18n';
  import { LocalModelsService } from '$lib/services/local-models';
  import { folderPath, models as localModels, scanFolder } from '$lib/stores/local-models';
  import {
    activeDownloads,
    cancelDownload,
    downloadHistory,
    ensureDownloadManager,
    pauseDownload,
    resumeDownload,
  } from '$lib/stores/download-manager';
  import { SimSearch } from '$lib/utils/simsearch';
  import type { DownloadHistoryEntry, DownloadJob, ModelInfo, RemoteGGUFFile, RemoteModelInfo } from '$lib/types/local-models';
  import {
    applyRemoteFilters,
    estimateVramGb,
    extractRepoFromHfUrl,
    formatBytes,
    getRelativeTimeLabel,
    PARAMETER_BUCKETS,
    updateSearchHistory,
    type RemoteSearchFilters,
  } from '$lib/model-manager/remote-search-utils';
  import {
    getCachedFallback,
    loadSearchCache,
    loadSearchHistory,
    saveSearchCache,
    saveSearchHistory,
    upsertSearchCache,
    type RemoteSearchCacheEntry,
  } from '$lib/model-manager/remote-search-storage';
  import {
    extractParameterLabel,
    formatEtaLabel,
    formatSpeedLabel,
    getDownloadProgress,
    getFileJob,
    isVerifiedAuthor,
  } from '$lib/model-manager/remote-model-helpers';
  import MagnifyingGlass from 'phosphor-svelte/lib/MagnifyingGlass';
  import Cube from 'phosphor-svelte/lib/Cube';
  import LinkSimple from 'phosphor-svelte/lib/LinkSimple';
  import SlidersHorizontal from 'phosphor-svelte/lib/SlidersHorizontal';
  import DownloadSimple from 'phosphor-svelte/lib/DownloadSimple';
  import Heart from 'phosphor-svelte/lib/Heart';
  import CheckCircle from 'phosphor-svelte/lib/CheckCircle';
  import WarningCircle from 'phosphor-svelte/lib/WarningCircle';
  import Play from 'phosphor-svelte/lib/Play';
  import Pause from 'phosphor-svelte/lib/Pause';
  import XCircle from 'phosphor-svelte/lib/XCircle';

  const SEARCH_DEBOUNCE_MS = 350;
  const SEARCH_HISTORY_KEY = 'models.remote.search.history.v1';
  const SEARCH_CACHE_KEY = 'models.remote.search.cache.v1';
  const REMOTE_BATCH_LIMIT = 1000;
  const RESULTS_COLUMNS = 2;
  const RESULTS_ROWS_PER_COLUMN = 15;
  const RESULTS_PER_PAGE = RESULTS_COLUMNS * RESULTS_ROWS_PER_COLUMN;
  const PARAMETER_FILTER_STEPS = [
    { value: 'any', label: 'Any' },
    ...PARAMETER_BUCKETS.map((bucket) => ({ value: bucket.value, label: bucket.label })),
  ];

  type SearchKind = 'none' | 'rate_limit' | 'offline' | 'api';
  type SearchSortMode = 'trending' | 'downloads' | 'likes' | 'updated';

  let searchInputRef = $state<HTMLInputElement | null>(null);
  let searchQuery = $state('');
  let searchHistory = $state<string[]>([]);

  let cachedEntries = $state<RemoteSearchCacheEntry[]>([]);
  let remoteResults = $state<RemoteModelInfo[]>([]);
  let selectedRepoId = $state<string | null>(null);
  let selectedFileByRepo = $state<Record<string, string>>({});
  let pendingUrlFilename = $state<string | null>(null);
  let modelDialogOpen = $state(false);
  let resultsPage = $state(1);

  let isInitialLoading = $state(true);
  let isSearching = $state(false);
  let hasSearched = $state(false);
  let searchError = $state<string | null>(null);
  let searchHint = $state<string | null>(null);
  let searchKind = $state<SearchKind>('none');
  let usingFallback = $state(false);
  let rateLimitedUntil = $state<number | null>(null);
  let nowTick = $state(Date.now());
  let remoteOffset = $state(0);
  let remoteHasMore = $state(false);
  let isLoadingMore = $state(false);

  let readmeLoading = $state(false);
  let readmeContent = $state('');
  let readmeError = $state<string | null>(null);
  let readmeRepoId = $state('');
  let readmeRequestToken = 0;

  let loadAfterDownload = $state(true);
  let autoLoadTargets = $state<string[]>([]);
  let pendingDownloads = $state<Record<string, boolean>>({});
  let downloadErrors = $state<Record<string, string>>({});
  let handledHistoryIds = $state<string[]>([]);
  let metadataFetchInFlight = $state<Record<string, boolean>>({});
  let metadataFetchAttempted = $state<Record<string, boolean>>({});

  let debounceHandle: ReturnType<typeof setTimeout> | null = null;
  let tickerHandle: ReturnType<typeof setInterval> | null = null;
  let searchRequestToken = 0;
  let parameterSliderValue = $state(0);

  let filters = $state<RemoteSearchFilters>({
    architectures: [],
    format: 'gguf',
    quantization: 'any',
    tags: [],
    license: 'any',
    pipelineTag: 'any',
    library: 'any',
    language: 'any',
    parameter: 'any',
    sizeBucket: 'any',
    minDownloads: 0,
    sortBy: 'downloads',
    sortOrder: 'desc',
    newThisWeek: true,
  });

  const filteredResults = $derived(applyRemoteFilters(remoteResults, filters));
  const totalResultsPages = $derived(Math.max(1, Math.ceil(filteredResults.length / RESULTS_PER_PAGE)));
  const pagedResults = $derived.by(() => {
    const start = (resultsPage - 1) * RESULTS_PER_PAGE;
    return filteredResults.slice(start, start + RESULTS_PER_PAGE);
  });
  const currentSortMode = $derived.by<SearchSortMode>(() => {
    if (filters.sortBy === 'likes' && filters.sortOrder === 'desc') return 'likes';
    if (filters.sortBy === 'updated' && filters.sortOrder === 'desc') return 'updated';
    if (filters.sortBy === 'downloads' && filters.sortOrder === 'desc') {
      return filters.newThisWeek ? 'trending' : 'downloads';
    }
    return 'trending';
  });
  const selectedModel = $derived(filteredResults.find((item) => item.repo_id === selectedRepoId) ?? null);
  const selectedFile = $derived.by(() => {
    if (!selectedModel) return null;
    const selectedName = selectedFileByRepo[selectedModel.repo_id];
    return selectedModel.gguf_files.find((file) => file.filename === selectedName) ?? selectedModel.gguf_files[0] ?? null;
  });
  const readmePreview = $derived(readmeContent.length > 2500 ? `${readmeContent.slice(0, 2500)}\n\n...` : readmeContent);
  const rateLimitSeconds = $derived(rateLimitedUntil && rateLimitedUntil > nowTick ? Math.ceil((rateLimitedUntil - nowTick) / 1000) : 0);
  const knownPipelineTags = $derived.by(() =>
    Array.from(
      new Set(
        remoteResults
          .map((model) => (model.pipeline_tag ?? '').trim())
          .filter(Boolean),
      ),
    ).sort((a, b) => a.localeCompare(b)),
  );
  const knownLicenses = $derived.by(() =>
    Array.from(
      new Set([
        ...remoteResults
          .map((model) => (model.license ?? '').trim().toLowerCase())
          .filter(Boolean),
        ...remoteResults.flatMap((model) =>
          (Array.isArray(model?.tags) ? model.tags : [])
            .map((tag) => tag.trim().toLowerCase())
            .filter((tag) => tag.startsWith('license:'))
            .map((tag) => tag.slice('license:'.length).trim())
            .filter(Boolean),
        ),
      ]),
    ).sort((a, b) => a.localeCompare(b)),
  );
  const knownLanguages = $derived.by(() =>
    Array.from(
      new Set([
        ...remoteResults.flatMap((model) =>
          (Array.isArray(model?.languages) ? model.languages : [])
            .map((language) => language.trim().toLowerCase())
            .filter((language) => /^[a-z]{2}(?:-[a-z]{2})?$/.test(language))
            .filter(Boolean),
        ),
        ...remoteResults.flatMap((model) =>
          (Array.isArray(model?.tags) ? model.tags : [])
            .map((tag) => tag.trim().toLowerCase())
            .map((tag) => (tag.startsWith('language:') ? tag.slice('language:'.length).trim() : tag))
            .filter((tag) => /^[a-z]{2}(?:-[a-z]{2})?$/.test(tag))
            .filter(Boolean),
        ),
      ]),
    ).sort((a, b) => a.localeCompare(b)),
  );
  const hasActiveFilters = $derived.by(() => {
    return (
      filters.license !== 'any' ||
      filters.pipelineTag !== 'any' ||
      filters.language !== 'any' ||
      filters.parameter !== 'any'
    );
  });
  const localFallbackMatches = $derived.by(() => {
    if (!searchError || !searchQuery.trim() || !$localModels.length) return [] as ModelInfo[];
    const index = new SimSearch(
      $localModels.map((model) => ({
        id: model.path,
        text: [model.name, model.model_name ?? '', model.architecture ?? '', model.quantization ?? '', model.source_repo_name ?? ''].join(' '),
      })),
    );
    const ids = new Set(index.search(searchQuery.trim().toLowerCase(), 8).map((entry) => entry.id));
    return $localModels.filter((model) => ids.has(model.path)).slice(0, 5);
  });

  function findFileHistory(repoId: string, filename: string): DownloadHistoryEntry | null {
    return get(downloadHistory).find((entry) => entry.repo_id === repoId && entry.filename.toLowerCase() === filename.toLowerCase()) ?? null;
  }

  function fileKey(repoId: string, filename: string): string {
    return `${repoId}::${filename}`;
  }

  function dedupeByRepoId(items: RemoteModelInfo[]): RemoteModelInfo[] {
    const seen = new Set<string>();
    const result: RemoteModelInfo[] = [];
    for (const item of items) {
      if (seen.has(item.repo_id)) continue;
      seen.add(item.repo_id);
      result.push(item);
    }
    return result;
  }

  function isFilePending(repoId: string, filename: string): boolean {
    return pendingDownloads[fileKey(repoId, filename)] === true;
  }

  function hasResolvedParameterCount(model: RemoteModelInfo): boolean {
    return Boolean(model.parameter_count?.trim());
  }

  function updateRemoteModelMetadata(
    repoId: string,
    parameterCount?: string,
    contextLength?: number,
  ) {
    let changed = false;
    const next = remoteResults.map((model) => {
      if (model.repo_id !== repoId) return model;

      const nextParameter = parameterCount?.trim() || model.parameter_count;
      const nextContext = contextLength && contextLength > 0 ? contextLength : model.context_length;

      if (nextParameter === model.parameter_count && nextContext === model.context_length) {
        return model;
      }

      changed = true;
      return {
        ...model,
        parameter_count: nextParameter,
        context_length: nextContext,
      };
    });

    if (!changed) return;
    remoteResults = next;

    const raw = searchQuery.trim();
    const parsed = extractRepoFromHfUrl(raw);
    const query = parsed?.repoId ?? raw;
    cachedEntries = upsertSearchCache(cachedEntries, query, next, 20, 1000);
    persistCache();
  }

  async function fetchMissingModelMetadata(repoId: string) {
    if (!repoId || metadataFetchAttempted[repoId] || metadataFetchInFlight[repoId]) return;
    const model = remoteResults.find((item) => item.repo_id === repoId);
    if (!model || hasResolvedParameterCount(model)) return;

    metadataFetchAttempted = { ...metadataFetchAttempted, [repoId]: true };
    metadataFetchInFlight = { ...metadataFetchInFlight, [repoId]: true };

    try {
      const metadata = await LocalModelsService.getRemoteModelMetadata(repoId);
      updateRemoteModelMetadata(
        repoId,
        metadata.parameter_count,
        metadata.context_length,
      );
    } catch {
      // Keep list responsive; missing metadata is non-fatal.
    } finally {
      const next = { ...metadataFetchInFlight };
      delete next[repoId];
      metadataFetchInFlight = next;
    }
  }

  function loadStoredState() {
    if (typeof window === 'undefined') return;
    searchHistory = loadSearchHistory(window.localStorage, SEARCH_HISTORY_KEY, 10);
    cachedEntries = loadSearchCache(window.localStorage, SEARCH_CACHE_KEY, 20);
  }

  function persistHistory() {
    if (typeof window === 'undefined') return;
    saveSearchHistory(window.localStorage, SEARCH_HISTORY_KEY, searchHistory, 10);
  }

  function persistCache() {
    if (typeof window === 'undefined') return;
    saveSearchCache(window.localStorage, SEARCH_CACHE_KEY, cachedEntries, 20);
  }

  async function performSearch(rawQuery: string) {
    const token = ++searchRequestToken;
    const raw = rawQuery.trim();
    const parsed = extractRepoFromHfUrl(raw);
    const query = parsed?.repoId ?? raw;
    pendingUrlFilename = parsed?.filename ?? null;
    resultsPage = 1;
    isSearching = true;
    hasSearched = query.length > 0;
    searchError = null;
    searchHint = null;
    searchKind = 'none';
    usingFallback = false;
    remoteOffset = 0;
    remoteHasMore = false;
    isLoadingMore = false;

    try {
      const live = await LocalModelsService.searchRemote(query || 'gguf', { limit: REMOTE_BATCH_LIMIT, offset: 0 });
      if (token !== searchRequestToken) return;

      remoteResults = dedupeByRepoId(live);
      remoteOffset = REMOTE_BATCH_LIMIT;
      remoteHasMore = live.length >= REMOTE_BATCH_LIMIT;
      cachedEntries = upsertSearchCache(cachedEntries, query, remoteResults, 20, 1000);
      persistCache();

      if (query) {
        searchHistory = updateSearchHistory(searchHistory, query, 10);
        persistHistory();
      }
    } catch (error) {
      if (token !== searchRequestToken) return;
      const message = error instanceof Error ? error.message : String(error);
      searchError = message;
      const lower = message.toLowerCase();
      if (lower.includes('429') || lower.includes('too many')) {
        searchKind = 'rate_limit';
        rateLimitedUntil = Date.now() + 30_000;
      } else if (
        lower.includes('offline') ||
        lower.includes('network') ||
        lower.includes('timeout') ||
        lower.includes('connection') ||
        lower.includes('fetch')
      ) {
        searchKind = 'offline';
      } else {
        searchKind = 'api';
      }

      const fallback = getCachedFallback(cachedEntries, query, 30);
      if (fallback.length) {
        remoteResults = fallback;
        remoteHasMore = false;
        usingFallback = true;
        searchHint = searchKind === 'offline' ? 'Offline mode: cached results shown.' : 'Cached results shown due to API error.';
      } else {
        remoteResults = [];
        remoteHasMore = false;
      }
    } finally {
      if (token !== searchRequestToken) return;
      isSearching = false;
      isInitialLoading = false;
    }
  }

  async function loadMoreResults() {
    if (isLoadingMore || isSearching || !remoteHasMore) return;

    const token = searchRequestToken;
    const raw = searchQuery.trim();
    const parsed = extractRepoFromHfUrl(raw);
    const query = parsed?.repoId ?? raw;
    isLoadingMore = true;

    try {
      const batch = await LocalModelsService.searchRemote(query || 'gguf', {
        limit: REMOTE_BATCH_LIMIT,
        offset: remoteOffset,
      });
      if (token !== searchRequestToken) return;

      if (batch.length === 0) {
        remoteHasMore = false;
        return;
      }

      remoteResults = dedupeByRepoId([...remoteResults, ...batch]);
      remoteOffset += REMOTE_BATCH_LIMIT;
      remoteHasMore = batch.length >= REMOTE_BATCH_LIMIT;
      cachedEntries = upsertSearchCache(cachedEntries, query, remoteResults, 20, 1000);
      persistCache();
    } catch (error) {
      if (token !== searchRequestToken) return;
      remoteHasMore = false;
      searchHint = error instanceof Error ? error.message : String(error);
    } finally {
      if (token === searchRequestToken) {
        isLoadingMore = false;
      }
    }
  }

  function queueSearch() {
    if (debounceHandle) clearTimeout(debounceHandle);
    debounceHandle = setTimeout(() => {
      debounceHandle = null;
      void performSearch(searchQuery);
    }, SEARCH_DEBOUNCE_MS);
  }

  function runSearchNow(query = searchQuery) {
    if (debounceHandle) {
      clearTimeout(debounceHandle);
      debounceHandle = null;
    }
    void performSearch(query);
  }

  function handleSortModeChange(value: string | undefined) {
    const mode = (value ?? 'trending') as SearchSortMode;
    if (mode === 'likes') {
      filters = { ...filters, sortBy: 'likes', sortOrder: 'desc', newThisWeek: false };
      return;
    }
    if (mode === 'updated') {
      filters = { ...filters, sortBy: 'updated', sortOrder: 'desc', newThisWeek: false };
      return;
    }
    if (mode === 'downloads') {
      filters = { ...filters, sortBy: 'downloads', sortOrder: 'desc', newThisWeek: false };
      return;
    }
    filters = { ...filters, sortBy: 'downloads', sortOrder: 'desc', newThisWeek: true };
  }

  function openModelDialog(repoId: string) {
    selectedRepoId = repoId;
    modelDialogOpen = true;
  }

  function resetFilters() {
    resultsPage = 1;
    filters = {
      architectures: [],
      format: 'gguf',
      quantization: 'any',
      tags: [],
      license: 'any',
      pipelineTag: 'any',
      library: 'any',
      language: 'any',
      parameter: 'any',
      sizeBucket: 'any',
      minDownloads: 0,
      sortBy: 'downloads',
      sortOrder: 'desc',
      newThisWeek: true,
    };
    parameterSliderValue = 0;
  }

  async function openExternal(url: string) {
    try {
      const { openUrl } = await import('@tauri-apps/plugin-opener');
      await openUrl(url);
    } catch {
      window.open(url, '_blank', 'noopener,noreferrer');
    }
  }

  async function handleDownload(model: RemoteModelInfo, file: RemoteGGUFFile) {
    const targetFolder = get(folderPath);
    if (!targetFolder) {
      alert($t('models.remote.selectFolderAlert'));
      return;
    }

    const key = fileKey(model.repo_id, file.filename);
    if (pendingDownloads[key] || getFileJob(get(activeDownloads), model.repo_id, file.filename)) return;

    pendingDownloads = { ...pendingDownloads, [key]: true };
    delete downloadErrors[key];
    downloadErrors = { ...downloadErrors };
    if (loadAfterDownload && !autoLoadTargets.includes(key)) {
      autoLoadTargets = [...autoLoadTargets, key];
    }

    try {
      await LocalModelsService.downloadRemoteModel(
        model.repo_id,
        file.filename,
        targetFolder,
        file.download_url,
        file.size > 0 ? file.size : undefined,
        file.sha256,
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      downloadErrors = { ...downloadErrors, [key]: message };
      const next = { ...pendingDownloads };
      delete next[key];
      pendingDownloads = next;
      autoLoadTargets = autoLoadTargets.filter((item) => item !== key);
    }
  }

  async function handlePauseResume(job: DownloadJob) {
    try {
      if (job.status === 'paused') {
        await resumeDownload(job);
        return;
      }
      await pauseDownload(job);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      downloadErrors = {
        ...downloadErrors,
        [fileKey(job.repo_id, job.filename)]: message,
      };
    }
  }

  async function handleCancel(job: DownloadJob) {
    try {
      await cancelDownload(job);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      downloadErrors = {
        ...downloadErrors,
        [fileKey(job.repo_id, job.filename)]: message,
      };
    }
  }

  async function fetchReadme(repoId: string) {
    if (!repoId) return;
    if (readmeRepoId === repoId && readmeContent) return;

    const token = ++readmeRequestToken;
    readmeLoading = true;
    readmeError = null;

    try {
      const content = await LocalModelsService.getModelReadme(repoId);
      if (token !== readmeRequestToken) return;
      readmeContent = content;
      readmeRepoId = repoId;
    } catch (error) {
      if (token !== readmeRequestToken) return;
      readmeContent = '';
      readmeRepoId = repoId;
      readmeError = error instanceof Error ? error.message : String(error);
    } finally {
      if (token === readmeRequestToken) {
        readmeLoading = false;
      }
    }
  }

  function loadIntoChat(path: string) {
    const bridge = (
      window as unknown as {
        __oxide?: { loadModelFromManager?: (args: { path: string; format: 'gguf' }) => void };
      }
    ).__oxide;
    bridge?.loadModelFromManager?.({ path, format: 'gguf' });
  }

  function handleGlobalKeydown(event: KeyboardEvent) {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
      event.preventDefault();
      searchInputRef?.focus();
      searchInputRef?.select();
      return;
    }

    if (event.key === '/' && !event.ctrlKey && !event.metaKey && !event.altKey) {
      const target = event.target as HTMLElement | null;
      const editable = target?.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target?.tagName ?? '');
      if (!editable) {
        event.preventDefault();
        searchInputRef?.focus();
        searchInputRef?.select();
      }
    }
  }

  $effect(() => {
    if (resultsPage > totalResultsPages) {
      resultsPage = totalResultsPages;
    }
    if (resultsPage < 1) {
      resultsPage = 1;
    }
  });

  $effect(() => {
    filters;
    resultsPage = 1;
  });

  $effect(() => {
    if (!filteredResults.length) {
      selectedRepoId = null;
      modelDialogOpen = false;
      return;
    }
    if (!selectedRepoId || !filteredResults.some((item) => item.repo_id === selectedRepoId)) {
      selectedRepoId = filteredResults[0]?.repo_id ?? null;
    }
  });

  $effect(() => {
    if (!selectedModel) return;
    if (!selectedFileByRepo[selectedModel.repo_id] && selectedModel.gguf_files[0]?.filename) {
      selectedFileByRepo = { ...selectedFileByRepo, [selectedModel.repo_id]: selectedModel.gguf_files[0].filename };
    }
  });

  $effect(() => {
    if (!selectedModel || !pendingUrlFilename) return;
    const pendingFilename = pendingUrlFilename;
    const found = selectedModel.gguf_files.find(
      (file) => file.filename.toLowerCase() === pendingFilename.toLowerCase(),
    );
    if (!found) return;
    selectedFileByRepo = { ...selectedFileByRepo, [selectedModel.repo_id]: found.filename };
    pendingUrlFilename = null;
  });

  $effect(() => {
    if (!selectedModel) {
      readmeRepoId = '';
      readmeContent = '';
      readmeError = null;
      return;
    }
    void fetchReadme(selectedModel.repo_id);
    void fetchMissingModelMetadata(selectedModel.repo_id);
  });

  $effect(() => {
    for (const model of pagedResults) {
      if (hasResolvedParameterCount(model)) continue;
      void fetchMissingModelMetadata(model.repo_id);
    }
  });

  $effect(() => {
    if (filters.parameter === 'any') return;
    const missing = remoteResults
      .filter((model) => !hasResolvedParameterCount(model))
      .slice(0, 20);
    for (const model of missing) {
      void fetchMissingModelMetadata(model.repo_id);
    }
  });

  $effect(() => {
    const activeKeys = new Set($activeDownloads.map((job) => fileKey(job.repo_id, job.filename)));
    const historyKeys = new Set($downloadHistory.map((entry) => fileKey(entry.repo_id, entry.filename)));
    const next = { ...pendingDownloads };
    let changed = false;

    for (const key of Object.keys(next)) {
      if (activeKeys.has(key) || historyKeys.has(key)) {
        delete next[key];
        changed = true;
      }
    }

    if (changed) pendingDownloads = next;
  });

  $effect(() => {
    if (!$downloadHistory.length) return;
    const handled = new Set(handledHistoryIds);
    const auto = new Set(autoLoadTargets);
    let handledChanged = false;
    let autoChanged = false;
    let shouldRescan = false;

    for (const entry of $downloadHistory) {
      if (handled.has(entry.id)) continue;
      handled.add(entry.id);
      handledChanged = true;
      const key = fileKey(entry.repo_id, entry.filename);

      if (entry.status === 'completed') {
        if (auto.delete(key)) {
          autoChanged = true;
          loadIntoChat(entry.destination_path);
        }
        if ($folderPath && entry.destination_path.startsWith($folderPath)) {
          shouldRescan = true;
        }
      } else if ((entry.status === 'error' || entry.status === 'cancelled') && auto.delete(key)) {
        autoChanged = true;
      }
    }

    if (handledChanged) handledHistoryIds = Array.from(handled).slice(-400);
    if (autoChanged) autoLoadTargets = Array.from(auto);
    if (shouldRescan && $folderPath) {
      void scanFolder($folderPath, true);
    }
  });

  onMount(() => {
    loadStoredState();
    void ensureDownloadManager();
    void performSearch('');
    window.addEventListener('keydown', handleGlobalKeydown);
    tickerHandle = setInterval(() => {
      nowTick = Date.now();
    }, 1000);
    return () => {
      window.removeEventListener('keydown', handleGlobalKeydown);
      if (debounceHandle) clearTimeout(debounceHandle);
      if (tickerHandle) clearInterval(tickerHandle);
    };
  });

  onDestroy(() => {
    if (debounceHandle) clearTimeout(debounceHandle);
    if (tickerHandle) clearInterval(tickerHandle);
  });
</script>

<div class="h-full min-h-0 flex flex-col gap-3">
  <div class="grid flex-1 min-h-0 grid-cols-[minmax(0,30%)_8px_minmax(0,70%)]">
    <aside class="h-full min-w-0 min-h-0 rounded-lg border bg-card p-3 overflow-y-auto overflow-x-hidden custom-scrollbar space-y-3">
      <div class="flex flex-wrap items-center gap-2">
        <span class="inline-flex items-center text-xs font-medium text-muted-foreground"><SlidersHorizontal class="mr-1 size-3.5" />Filters</span>
        <Button size="sm" variant="ghost" class="h-7 px-2 text-[11px]" onclick={resetFilters} disabled={!hasActiveFilters}>Clear</Button>
      </div>

      <div class="space-y-2">
        <p class="text-[11px] text-muted-foreground">Pipeline Tag</p>
        <Select.Root type="single" value={filters.pipelineTag} onValueChange={(value) => (filters = { ...filters, pipelineTag: value ?? 'any' })}>
          <Select.Trigger class="h-8 w-full text-xs">{filters.pipelineTag === 'any' ? 'Any pipeline tag' : filters.pipelineTag}</Select.Trigger>
          <Select.Content>
            <Select.Item value="any">Any pipeline tag</Select.Item>
            {#each knownPipelineTags as pipelineTag}<Select.Item value={pipelineTag}>{pipelineTag}</Select.Item>{/each}
          </Select.Content>
        </Select.Root>
      </div>

      <div class="space-y-2">
        <p class="text-[11px] text-muted-foreground">License</p>
        <Select.Root type="single" value={filters.license} onValueChange={(value) => (filters = { ...filters, license: value ?? 'any' })}>
          <Select.Trigger class="h-8 w-full text-xs">{filters.license === 'any' ? 'Any license' : filters.license}</Select.Trigger>
          <Select.Content>
            <Select.Item value="any">Any license</Select.Item>
            {#each knownLicenses as license}<Select.Item value={license}>{license}</Select.Item>{/each}
          </Select.Content>
        </Select.Root>
      </div>

      <div class="space-y-2">
        <p class="text-[11px] text-muted-foreground">Language</p>
        <Select.Root type="single" value={filters.language} onValueChange={(value) => (filters = { ...filters, language: value ?? 'any' })}>
          <Select.Trigger class="h-8 w-full text-xs">{filters.language === 'any' ? 'Any language' : filters.language}</Select.Trigger>
          <Select.Content>
            <Select.Item value="any">Any language</Select.Item>
            {#each knownLanguages as language}<Select.Item value={language}>{language}</Select.Item>{/each}
          </Select.Content>
        </Select.Root>
      </div>

      <div class="space-y-2">
        <p class="text-[11px] text-muted-foreground">Parameters</p>
        <Slider.Root
          type="single"
          min={0}
          max={PARAMETER_FILTER_STEPS.length - 1}
          step={1}
          value={parameterSliderValue}
          onValueChange={(value) => {
            const idx = Math.max(0, Math.min(PARAMETER_FILTER_STEPS.length - 1, Math.round(Number(value) || 0)));
            parameterSliderValue = idx;
            filters = { ...filters, parameter: PARAMETER_FILTER_STEPS[idx]?.value ?? 'any' };
          }}
        />
        <div class="grid grid-cols-3 gap-x-2 gap-y-1 text-[10px] text-muted-foreground">
          {#each PARAMETER_FILTER_STEPS as step}
            <span>{step.label}</span>
          {/each}
        </div>
        <p class="text-[11px] text-muted-foreground">
          Selected: {PARAMETER_FILTER_STEPS[parameterSliderValue]?.label ?? 'Any'}
        </p>
      </div>
    </aside>

    <div class="min-h-0 flex items-stretch justify-center">
      <Separator.Root orientation="vertical" class="h-full" />
    </div>

    <section class="h-full min-w-0 min-h-0 rounded-lg border bg-card flex flex-col overflow-hidden">
      <div class="border-b p-3">
        <div class="grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2">
          <span class="text-xs text-muted-foreground whitespace-nowrap">{filteredResults.length} models</span>
          <div class="relative min-w-0">
            <MagnifyingGlass class="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
            <Input
              bind:ref={searchInputRef}
              type="search"
              value={searchQuery}
              placeholder="Model name, author/repo, or https://huggingface.co/..."
              class="h-10 pl-10 pr-20"
              oninput={(event) => {
                searchQuery = (event.target as HTMLInputElement).value;
                queueSearch();
              }}
              onkeydown={(event) => {
                if (event.key === 'Enter') {
                  event.preventDefault();
                  runSearchNow();
                }
              }}
            />
            <div class="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 flex items-center text-[11px] text-muted-foreground">
              {#if isSearching}<Spinner class="size-3.5" />{/if}
            </div>
          </div>
          <div class="w-[190px]">
            <Select.Root type="single" value={currentSortMode} onValueChange={handleSortModeChange}>
              <Select.Trigger class="h-10 w-full text-xs">Sort: {
                currentSortMode === 'likes'
                  ? 'Most likes'
                  : currentSortMode === 'updated'
                    ? 'Recently updated'
                    : currentSortMode === 'downloads'
                      ? 'Most downloads'
                      : 'Trending'
              }</Select.Trigger>
              <Select.Content>
                <Select.Item value="trending">Trending</Select.Item>
                <Select.Item value="downloads">Most downloads</Select.Item>
                <Select.Item value="likes">Most likes</Select.Item>
                <Select.Item value="updated">Recently updated</Select.Item>
              </Select.Content>
            </Select.Root>
          </div>
        </div>
      </div>
      <div class="flex-1 min-h-0 overflow-y-auto overflow-x-hidden custom-scrollbar p-2">
        {#if isInitialLoading}
          <div class="space-y-2">
            {#each Array.from({ length: 6 }) as _, index (`skeleton-${index}`)}
              <div class="rounded-md border p-3 space-y-2">
                <Skeleton class="h-4 w-2/3" />
                <Skeleton class="h-3 w-full" />
              </div>
            {/each}
          </div>
        {:else if filteredResults.length === 0}
          <div class="h-full min-h-[220px] flex flex-col items-center justify-center gap-3 text-center px-6">
            <Cube class="size-12 text-muted-foreground/60" weight="light" />
            <p class="text-sm font-medium">No models found</p>
            <div class="flex gap-2">
              <Button variant="outline" size="sm" onclick={resetFilters} disabled={!hasActiveFilters}>Clear</Button>
              <Button variant="outline" size="sm" onclick={() => runSearchNow()}>Refresh</Button>
            </div>
            {#if localFallbackMatches.length > 0}
              <p class="text-xs text-muted-foreground">Local matches: {localFallbackMatches.length}</p>
            {/if}
          </div>
        {:else}
          <div class="grid grid-cols-2 gap-2">
            {#each pagedResults as model (model.repo_id)}
              {@const selected = selectedRepoId === model.repo_id}
              {@const pipelineTag = (model.pipeline_tag ?? '').trim()}
              <div
                role="button"
                tabindex="0"
                class="rounded-md border p-2 transition-colors focus:outline-none focus:ring-2 focus:ring-primary/40 {selected ? 'border-primary/40 bg-primary/10' : 'bg-muted/20 hover:bg-muted/40'}"
                onclick={() => openModelDialog(model.repo_id)}
                onkeydown={(event) => {
                  if (event.key === 'Enter' || event.key === ' ') {
                    event.preventDefault();
                    openModelDialog(model.repo_id);
                  }
                }}
              >
                <div class="grid grid-cols-[minmax(0,1fr)_auto] gap-x-3 gap-y-1">
                  <div class="flex min-w-0 items-center gap-1.5">
                    <p class="truncate text-sm font-medium">{model.repo_id}</p>
                    {#if isVerifiedAuthor(model)}<CheckCircle class="size-3.5 shrink-0 text-emerald-600" weight="fill" />{/if}
                  </div>
                  <div class="row-span-2 flex flex-col items-end justify-center gap-1 text-[11px] text-muted-foreground">
                    <span class="inline-flex items-center gap-1 leading-tight">
                      <Heart class="size-3" />
                      {model.likes.toLocaleString()}
                    </span>
                    <span class="inline-flex items-center gap-1 leading-tight">
                      <DownloadSimple class="size-3" />
                      {model.downloads.toLocaleString()}
                    </span>
                  </div>
                  <p class="truncate text-[11px] text-muted-foreground">
                    {pipelineTag ? `${pipelineTag} · ` : ''}{extractParameterLabel(model)} · Updated {getRelativeTimeLabel(model.last_modified)}
                  </p>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>
      {#if filteredResults.length > RESULTS_PER_PAGE || remoteHasMore}
        <footer class="border-t px-3 py-2 flex items-center justify-between gap-2 text-xs text-muted-foreground">
          <div>
            {#if remoteHasMore}
              <Button variant="outline" size="sm" class="h-8 text-[11px]" onclick={() => void loadMoreResults()} disabled={isLoadingMore || isSearching}>
                {#if isLoadingMore}
                  <Spinner class="mr-1 size-3.5" />
                  Loading more
                {:else}
                  Load more from Hugging Face
                {/if}
              </Button>
            {/if}
          </div>
          {#if filteredResults.length > RESULTS_PER_PAGE}
            <Pagination.Root count={filteredResults.length} perPage={RESULTS_PER_PAGE} bind:page={resultsPage} siblingCount={1}>
              {#snippet children({ pages, currentPage })}
                <Pagination.Content>
                  <Pagination.Item>
                    <Pagination.Previous />
                  </Pagination.Item>
                  {#each pages as page (page.key)}
                    {#if page.type === 'ellipsis'}
                      <Pagination.Item>
                        <Pagination.Ellipsis />
                      </Pagination.Item>
                    {:else}
                      <Pagination.Item>
                        <Pagination.Link {page} isActive={currentPage === page.value} size="sm" class="h-8 min-w-8 text-[11px]">
                          {page.value}
                        </Pagination.Link>
                      </Pagination.Item>
                    {/if}
                  {/each}
                  <Pagination.Item>
                    <Pagination.Next />
                  </Pagination.Item>
                </Pagination.Content>
              {/snippet}
            </Pagination.Root>
          {/if}
        </footer>
      {/if}
    </section>
  </div>

  <Dialog.Root bind:open={modelDialogOpen}>
    <Dialog.Content class="max-w-[min(96vw,1080px)] h-[min(90vh,860px)] flex flex-col">
      {#if selectedModel}
        <Dialog.Header>
          <Dialog.Title class="truncate pr-6">{selectedModel.repo_id}</Dialog.Title>
          <Dialog.Description>
            Updated {getRelativeTimeLabel(selectedModel.last_modified)}
          </Dialog.Description>
        </Dialog.Header>

        <div class="border-b px-1 pb-3 space-y-2 shrink-0">
          <div class="flex items-start justify-between gap-3">
            <div class="min-w-0">
              <p class="text-xs text-muted-foreground">
                Detailed model card with file-level download actions
              </p>
            </div>
            <Button variant="outline" size="sm" class="shrink-0" onclick={() => openExternal(`https://huggingface.co/${selectedModel.repo_id}`)}>
              <LinkSimple class="mr-1 size-4" />
              Open on HF
            </Button>
          </div>
          <div class="flex flex-wrap gap-1.5">
            <Badge variant="outline">{extractParameterLabel(selectedModel)}</Badge>
            <Badge variant="outline"><DownloadSimple class="mr-1 size-3.5" />{selectedModel.downloads.toLocaleString()}</Badge>
            <Badge variant="outline"><Heart class="mr-1 size-3.5" />{selectedModel.likes.toLocaleString()}</Badge>
          </div>
          {#if selectedModel.description}<p class="text-sm text-muted-foreground line-clamp-3">{selectedModel.description}</p>{/if}
          <label class="flex items-center gap-2 text-xs text-muted-foreground">
            <Checkbox checked={loadAfterDownload} onCheckedChange={(value) => (loadAfterDownload = value === true)} />
            Load after download
          </label>
        </div>

        <div class="flex-1 min-h-0 overflow-y-auto overflow-x-hidden custom-scrollbar px-1 pt-3 space-y-4">
          {#if selectedFile}
            {@const selectedKey = fileKey(selectedModel.repo_id, selectedFile.filename)}
            {@const selectedJob = getFileJob($activeDownloads, selectedModel.repo_id, selectedFile.filename)}
            {@const selectedHistory = findFileHistory(selectedModel.repo_id, selectedFile.filename)}
            <div class="rounded-md border bg-muted/25 p-3 space-y-2">
              <div class="flex justify-between gap-3">
                <div class="min-w-0">
                  <p class="truncate text-sm font-medium">{selectedFile.filename}</p>
                  <p class="text-xs text-muted-foreground">{formatBytes(selectedFile.size)} · {selectedFile.quantization ?? 'unknown quant'} · ~{estimateVramGb(selectedFile.size, selectedFile.quantization)} GB VRAM</p>
                </div>
                <div class="flex items-center gap-1 shrink-0">
                  {#if selectedJob}
                    <Button variant="ghost" size="icon" class="size-8" onclick={() => void handlePauseResume(selectedJob)}>{#if selectedJob.status === 'paused'}<Play class="size-4" />{:else}<Pause class="size-4" />{/if}</Button>
                    <Button variant="ghost" size="icon" class="size-8" onclick={() => void handleCancel(selectedJob)}><XCircle class="size-4" /></Button>
                  {:else}
                    <Button size="sm" onclick={() => void handleDownload(selectedModel, selectedFile)} disabled={isFilePending(selectedModel.repo_id, selectedFile.filename)}>
                      {#if isFilePending(selectedModel.repo_id, selectedFile.filename)}<Spinner class="mr-1 size-4" />Queued{:else}<DownloadSimple class="mr-1 size-4" />Download{/if}
                    </Button>
                  {/if}
                </div>
              </div>
              {#if selectedJob}
                <Progress.Root value={getDownloadProgress(selectedJob)} max={100} class="h-1.5" />
                <p class="text-xs text-muted-foreground">{Math.round(getDownloadProgress(selectedJob))}% · {formatSpeedLabel(selectedJob.speed_bytes_per_sec)} · ETA {formatEtaLabel(selectedJob.eta_seconds)}</p>
              {:else if selectedHistory?.status === 'completed'}
                <p class="text-xs text-emerald-600">Downloaded</p>
              {/if}
              {#if downloadErrors[selectedKey]}<p class="text-xs text-destructive">{downloadErrors[selectedKey]}</p>{/if}
            </div>
          {/if}

          <div class="space-y-2">
            <h4 class="text-sm font-medium">Files</h4>
            <div class="rounded-md border overflow-hidden">
              <div class="max-h-[250px] overflow-auto custom-scrollbar">
                <table class="w-full text-xs table-fixed">
                  <thead class="sticky top-0 bg-muted/80">
                    <tr>
                      <th class="px-2 py-2 text-left w-[46%]">File</th>
                      <th class="px-2 py-2 text-left w-[16%]">Quant</th>
                      <th class="px-2 py-2 text-left w-[14%]">Size</th>
                      <th class="px-2 py-2 text-left w-[14%]">VRAM</th>
                      <th class="px-2 py-2 text-right w-[10%]">Action</th>
                    </tr>
                  </thead>
                  <tbody>
                    {#each selectedModel.gguf_files as file (file.filename)}
                      {@const rowJob = getFileJob($activeDownloads, selectedModel.repo_id, file.filename)}
                      <tr class="border-t cursor-pointer hover:bg-muted/40 {selectedFile?.filename === file.filename ? 'bg-primary/10' : ''}" onclick={() => (selectedFileByRepo = { ...selectedFileByRepo, [selectedModel.repo_id]: file.filename })}>
                        <td class="px-2 py-2"><p class="truncate font-mono text-[11px]">{file.filename}</p></td>
                        <td class="px-2 py-2 truncate">{file.quantization ?? '—'}</td>
                        <td class="px-2 py-2">{formatBytes(file.size)}</td>
                        <td class="px-2 py-2">~{estimateVramGb(file.size, file.quantization)} GB</td>
                        <td class="px-2 py-2 text-right">
                          {#if rowJob}
                            <div class="inline-flex items-center gap-1">
                              <Button variant="ghost" size="icon" class="size-7" onclick={(event) => { event.stopPropagation(); void handlePauseResume(rowJob); }}>{#if rowJob.status === 'paused'}<Play class="size-3.5" />{:else}<Pause class="size-3.5" />{/if}</Button>
                              <Button variant="ghost" size="icon" class="size-7" onclick={(event) => { event.stopPropagation(); void handleCancel(rowJob); }}><XCircle class="size-3.5" /></Button>
                            </div>
                          {:else}
                            <Button variant="ghost" size="icon" class="size-7" onclick={(event) => { event.stopPropagation(); void handleDownload(selectedModel, file); }} disabled={isFilePending(selectedModel.repo_id, file.filename)}>
                              {#if isFilePending(selectedModel.repo_id, file.filename)}<Spinner class="size-3.5" />{:else}<DownloadSimple class="size-3.5" />{/if}
                            </Button>
                          {/if}
                        </td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </div>
            </div>
          </div>

          <div class="space-y-2">
            <div class="flex items-center justify-between gap-2">
              <h4 class="text-sm font-medium">README preview</h4>
              <Button variant="ghost" size="sm" class="h-7 px-2 text-xs" onclick={() => openExternal(`https://huggingface.co/${selectedModel.repo_id}`)}>Open full page</Button>
            </div>
            <div class="rounded-md border bg-muted/20 p-3">
              {#if readmeLoading}
                <div class="space-y-2">
                  <Skeleton class="h-4 w-3/4" />
                  <Skeleton class="h-4 w-full" />
                  <Skeleton class="h-20 w-full" />
                </div>
              {:else if readmeError}
                <p class="text-xs text-muted-foreground">README unavailable: {readmeError}</p>
              {:else if readmePreview}
                <Markdown content={readmePreview} class="text-sm leading-relaxed" />
              {:else}
                <p class="text-xs text-muted-foreground">No README content available.</p>
              {/if}
            </div>
          </div>
        </div>
      {:else}
        <div class="h-[300px] flex items-center justify-center p-6 text-center">
          <div class="space-y-2 text-sm text-muted-foreground">
            <p>Select a model to inspect files and start download.</p>
            <p class="text-xs">Use filters above to narrow by pipeline tag, license, language, and parameters.</p>
          </div>
        </div>
      {/if}
    </Dialog.Content>
  </Dialog.Root>
</div>
