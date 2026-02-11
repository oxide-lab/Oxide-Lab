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
  import ModelIcon from '$lib/components/ModelIcon.svelte';
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
  import type {
    DownloadHistoryEntry,
    DownloadJob,
    ModelInfo,
    RemoteGGUFFile,
    RemoteModelInfo,
  } from '$lib/types/local-models';
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
    getCachedQueryResults,
    getCachedSearchPage,
    loadSearchCache,
    loadSearchHistory,
    saveSearchCache,
    saveSearchHistory,
    upsertSearchCachePage,
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
  import staticFirstPageModels from '$lib/model-manager/static-first-page-models.json';
  import MagnifyingGlass from 'phosphor-svelte/lib/MagnifyingGlass';
  import Cube from 'phosphor-svelte/lib/Cube';
  import GlobeSimple from 'phosphor-svelte/lib/GlobeSimple';
  import Scales from 'phosphor-svelte/lib/Scales';
  import SlidersHorizontal from 'phosphor-svelte/lib/SlidersHorizontal';
  import LinkSimple from 'phosphor-svelte/lib/LinkSimple';
  import DownloadSimple from 'phosphor-svelte/lib/DownloadSimple';
  import Heart from 'phosphor-svelte/lib/Heart';
  import Play from 'phosphor-svelte/lib/Play';
  import Pause from 'phosphor-svelte/lib/Pause';
  import XCircle from 'phosphor-svelte/lib/XCircle';

  const SEARCH_DEBOUNCE_MS = 350;
  const SEARCH_HISTORY_KEY = 'models.remote.search.history.v1';
  const SEARCH_CACHE_KEY = 'models.remote.search.cache.v1';
  const SEARCH_TOTALS_KEY = 'models.remote.search.total.v1';
  const STATIC_FIRST_PAGE_CACHE_QUERY = '__static_first_page_repo_ids__';
  const REMOTE_RETRY_COOLDOWN_MS = 30_000;
  const REMOTE_CACHE_MAX_ENTRIES = 20;
  const REMOTE_CACHE_MAX_PAGES_PER_QUERY = 8;
  const REMOTE_CACHE_MAX_ITEMS_PER_PAGE = 30;
  const REMOTE_FILTER_CATALOG_BOOTSTRAP_ITEMS = 300;
  const STATIC_CATALOG_ONLY_MODE = true;
  const RESULTS_COLUMNS = 2;
  const RESULTS_ROWS_PER_COLUMN = 15;
  const RESULTS_PER_PAGE = RESULTS_COLUMNS * RESULTS_ROWS_PER_COLUMN;
  const STATIC_FIRST_PAGE_REPO_IDS = Array.from(
    new Set(
      (staticFirstPageModels as StaticFirstPageModel[])
        .map((model) => model.repo_id?.trim())
        .filter((value): value is string => Boolean(value)),
    ),
  );
  const STATIC_FIRST_PAGE_MODELS = STATIC_FIRST_PAGE_REPO_IDS.map((repoId) => ({
    repo_id: repoId,
    name: repoId.split('/').at(-1) || repoId,
    author: repoId.split('/')[0] || '',
    description: '',
    license: '',
    pipeline_tag: '',
    library: 'gguf',
    languages: [],
    downloads: 0,
    likes: 0,
    tags: ['static:first-page'],
    architectures: [],
    quantizations: [],
    gguf_files: [],
    last_modified: '',
    created_at: '',
    parameter_count: '',
    context_length: undefined,
  }));
  const STATIC_MODEL_ICON_BY_REPO = new Map(
    (staticFirstPageModels as StaticFirstPageModel[])
      .filter(
        (item) =>
          typeof item.repo_id === 'string' &&
          typeof item.icon === 'string' &&
          item.icon.trim().length > 0,
      )
      .map((item) => [item.repo_id.trim(), item.icon!.trim()] as const),
  );
  const STATIC_FIRST_PAGE_COUNT = STATIC_FIRST_PAGE_MODELS.length;
  const PARAMETER_RANGE_STEPS = PARAMETER_BUCKETS;

  type SearchKind = 'none' | 'rate_limit' | 'offline' | 'api';
  type SearchSortMode = 'trending' | 'downloads' | 'likes' | 'updated';
  type FilterTab = 'main' | 'pipeline' | 'license' | 'language';
  type StaticFirstPageModel = {
    repo_id: string;
    icon?: string;
  };
  type PipelineCategoryId =
    | 'multimodal'
    | 'computer-vision'
    | 'natural-language-processing'
    | 'audio'
    | 'tabular'
    | 'reinforcement-learning'
    | 'other';
  type PipelineCategoryDefinition = {
    id: PipelineCategoryId;
    label: string;
    headerClass: string;
    chipActiveClass: string;
    chipIdleClass: string;
  };

  const FILTER_MAIN_LIMIT = 8;
  const PIPELINE_CATEGORY_DEFINITIONS: PipelineCategoryDefinition[] = [
    {
      id: 'multimodal',
      label: 'Multimodal',
      headerClass: 'text-orange-500 dark:text-orange-300',
      chipActiveClass: 'border-orange-500/60 bg-orange-500/15 text-orange-700 dark:text-orange-200',
      chipIdleClass:
        'border-orange-500/30 bg-orange-500/5 text-orange-700/90 hover:bg-orange-500/12 dark:text-orange-300',
    },
    {
      id: 'computer-vision',
      label: 'Computer Vision',
      headerClass: 'text-sky-500 dark:text-sky-300',
      chipActiveClass: 'border-sky-500/60 bg-sky-500/15 text-sky-700 dark:text-sky-200',
      chipIdleClass:
        'border-sky-500/30 bg-sky-500/5 text-sky-700/90 hover:bg-sky-500/12 dark:text-sky-300',
    },
    {
      id: 'natural-language-processing',
      label: 'Natural Language Processing',
      headerClass: 'text-red-500 dark:text-red-300',
      chipActiveClass: 'border-red-500/60 bg-red-500/15 text-red-700 dark:text-red-200',
      chipIdleClass:
        'border-red-500/30 bg-red-500/5 text-red-700/90 hover:bg-red-500/12 dark:text-red-300',
    },
    {
      id: 'audio',
      label: 'Audio',
      headerClass: 'text-emerald-500 dark:text-emerald-300',
      chipActiveClass:
        'border-emerald-500/60 bg-emerald-500/15 text-emerald-700 dark:text-emerald-200',
      chipIdleClass:
        'border-emerald-500/30 bg-emerald-500/5 text-emerald-700/90 hover:bg-emerald-500/12 dark:text-emerald-300',
    },
    {
      id: 'tabular',
      label: 'Tabular',
      headerClass: 'text-fuchsia-500 dark:text-fuchsia-300',
      chipActiveClass:
        'border-fuchsia-500/60 bg-fuchsia-500/15 text-fuchsia-700 dark:text-fuchsia-200',
      chipIdleClass:
        'border-fuchsia-500/30 bg-fuchsia-500/5 text-fuchsia-700/90 hover:bg-fuchsia-500/12 dark:text-fuchsia-300',
    },
    {
      id: 'reinforcement-learning',
      label: 'Reinforcement Learning',
      headerClass: 'text-yellow-500 dark:text-yellow-300',
      chipActiveClass: 'border-yellow-500/60 bg-yellow-500/15 text-yellow-700 dark:text-yellow-200',
      chipIdleClass:
        'border-yellow-500/30 bg-yellow-500/5 text-yellow-700/90 hover:bg-yellow-500/12 dark:text-yellow-300',
    },
    {
      id: 'other',
      label: 'Other',
      headerClass: 'text-violet-500 dark:text-violet-300',
      chipActiveClass: 'border-violet-500/60 bg-violet-500/15 text-violet-700 dark:text-violet-200',
      chipIdleClass:
        'border-violet-500/30 bg-violet-500/5 text-violet-700/90 hover:bg-violet-500/12 dark:text-violet-300',
    },
  ];

  const PIPELINE_CATEGORY_MEMBERS: Record<PipelineCategoryId, string[]> = {
    multimodal: [
      'audio-text-to-text',
      'image-text-to-text',
      'image-text-to-image',
      'image-text-to-video',
      'visual-question-answering',
      'document-question-answering',
      'video-text-to-text',
      'visual-document-retrieval',
      'any-to-any',
    ],
    'computer-vision': [
      'depth-estimation',
      'image-classification',
      'object-detection',
      'image-segmentation',
      'text-to-image',
      'image-to-text',
      'image-to-image',
      'image-to-video',
      'unconditional-image-generation',
      'video-classification',
      'text-to-video',
      'zero-shot-image-classification',
      'mask-generation',
      'zero-shot-object-detection',
      'text-to-3d',
      'image-to-3d',
      'image-feature-extraction',
      'keypoint-detection',
      'video-to-video',
    ],
    'natural-language-processing': [
      'text-classification',
      'token-classification',
      'table-question-answering',
      'question-answering',
      'zero-shot-classification',
      'translation',
      'summarization',
      'feature-extraction',
      'text-generation',
      'fill-mask',
      'sentence-similarity',
      'text-ranking',
    ],
    audio: [
      'text-to-speech',
      'text-to-audio',
      'automatic-speech-recognition',
      'audio-to-audio',
      'audio-classification',
      'voice-activity-detection',
    ],
    tabular: ['tabular-classification', 'tabular-regression', 'time-series-forecasting'],
    'reinforcement-learning': ['reinforcement-learning', 'robotics'],
    other: ['graph-machine-learning'],
  };

  const PIPELINE_TAG_CATEGORY_MAP = new Map<string, PipelineCategoryId>();
  for (const category of PIPELINE_CATEGORY_DEFINITIONS) {
    for (const tag of PIPELINE_CATEGORY_MEMBERS[category.id]) {
      PIPELINE_TAG_CATEGORY_MAP.set(tag, category.id);
    }
  }

  let searchInputRef = $state<HTMLInputElement | null>(null);
  let searchQuery = $state('');
  let searchHistory = $state<string[]>([]);
  let activeFilterTab = $state<FilterTab>('main');
  let pipelineFilterQuery = $state('');
  let licenseFilterQuery = $state('');
  let languageFilterQuery = $state('');

  let cachedEntries = $state<RemoteSearchCacheEntry[]>([]);
  let remoteResults = $state<RemoteModelInfo[]>([]);
  let filterCatalogResults = $state<RemoteModelInfo[]>(STATIC_FIRST_PAGE_MODELS);
  let selectedRepoId = $state<string | null>(null);
  let selectedFileByRepo = $state<Record<string, string>>({});
  let pendingUrlFilename = $state<string | null>(null);
  let modelDialogOpen = $state(false);
  let resultsPage = $state(1);

  let isInitialLoading = $state(true);
  let isSearching = $state(false);
  let isPageLoading = $state(false);
  let hasSearched = $state(false);
  let searchError = $state<string | null>(null);
  let searchHint = $state<string | null>(null);
  let searchKind = $state<SearchKind>('none');
  let usingFallback = $state(false);
  let rateLimitedUntil = $state<number | null>(null);
  let remoteBlockedUntil = $state<number | null>(null);
  let nowTick = $state(Date.now());
  let totalMatches = $state<number | null>(null);
  let totalByQuery = $state<Record<string, number>>({});
  let currentPageResults = $state<RemoteModelInfo[]>([]);

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
  let ownerAvatarByPublisher = $state<Record<string, string>>({});
  let ownerAvatarFetchInFlight = $state<Record<string, boolean>>({});

  let debounceHandle: ReturnType<typeof setTimeout> | null = null;
  let tickerHandle: ReturnType<typeof setInterval> | null = null;
  let searchRequestToken = $state(0);
  let pageRequestToken = $state(0);
  let lastLoadedPageKey = $state('');
  let lastFiltersKey = $state('');
  let pageCursorByQuery = $state<Record<string, Record<number, string | null>>>({});
  let parameterSliderValue = $state<[number, number]>([
    0,
    Math.max(0, PARAMETER_RANGE_STEPS.length - 1),
  ]);

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
    parameterMinB: null,
    parameterMaxB: null,
    sizeBucket: 'any',
    minDownloads: 0,
    sortBy: 'downloads',
    sortOrder: 'desc',
    newThisWeek: true,
  });

  const filteredResults = $derived(applyRemoteFilters(currentPageResults, filters));
  const knownRemoteTotal = $derived.by(
    () =>
      totalMatches ??
      totalByQuery[queryCacheKey(parseSearchQuery(searchQuery).query || 'gguf')] ??
      Math.max(0, remoteResults.length - STATIC_FIRST_PAGE_COUNT),
  );
  const effectiveTotalMatches = $derived(
    STATIC_CATALOG_ONLY_MODE
      ? filteredResults.length
      : STATIC_FIRST_PAGE_COUNT + Math.max(RESULTS_PER_PAGE, knownRemoteTotal),
  );
  const totalResultsPages = $derived(
    STATIC_CATALOG_ONLY_MODE
      ? 1
      : Math.max(1, Math.ceil(Math.max(0, effectiveTotalMatches) / RESULTS_PER_PAGE)),
  );
  const pagedResults = $derived(filteredResults);
  const currentSortMode = $derived.by<SearchSortMode>(() => {
    if (filters.sortBy === 'likes' && filters.sortOrder === 'desc') return 'likes';
    if (filters.sortBy === 'updated' && filters.sortOrder === 'desc') return 'updated';
    if (filters.sortBy === 'downloads' && filters.sortOrder === 'desc') {
      return filters.newThisWeek ? 'trending' : 'downloads';
    }
    return 'trending';
  });
  const selectedModel = $derived(
    remoteResults.find((item) => item.repo_id === selectedRepoId) ?? null,
  );
  const selectedFile = $derived.by(() => {
    if (!selectedModel) return null;
    const selectedName = selectedFileByRepo[selectedModel.repo_id];
    return (
      selectedModel.gguf_files.find((file) => file.filename === selectedName) ??
      selectedModel.gguf_files[0] ??
      null
    );
  });
  const readmePreview = $derived(
    readmeContent.length > 2500 ? `${readmeContent.slice(0, 2500)}\n\n...` : readmeContent,
  );
  const rateLimitSeconds = $derived(
    rateLimitedUntil && rateLimitedUntil > nowTick
      ? Math.ceil((rateLimitedUntil - nowTick) / 1000)
      : 0,
  );
  const remoteBlockedSeconds = $derived(
    remoteBlockedUntil && remoteBlockedUntil > nowTick
      ? Math.ceil((remoteBlockedUntil - nowTick) / 1000)
      : 0,
  );
  const knownPipelineTags = $derived.by(() =>
    Array.from(
      new Set(
        filterCatalogResults.map((model) => (model.pipeline_tag ?? '').trim()).filter(Boolean),
      ),
    ).sort((a, b) => a.localeCompare(b)),
  );
  const knownLicenses = $derived.by(() =>
    Array.from(
      new Set([
        ...filterCatalogResults
          .map((model) => (model.license ?? '').trim().toLowerCase())
          .filter(Boolean),
        ...filterCatalogResults.flatMap((model) =>
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
        ...filterCatalogResults.flatMap((model) =>
          (Array.isArray(model?.languages) ? model.languages : [])
            .map((language) => language.trim().toLowerCase())
            .filter((language) => /^[a-z]{2}(?:-[a-z]{2})?$/.test(language))
            .filter(Boolean),
        ),
        ...filterCatalogResults.flatMap((model) =>
          (Array.isArray(model?.tags) ? model.tags : [])
            .map((tag) => tag.trim().toLowerCase())
            .map((tag) =>
              tag.startsWith('language:') ? tag.slice('language:'.length).trim() : tag,
            )
            .filter((tag) => /^[a-z]{2}(?:-[a-z]{2})?$/.test(tag))
            .filter(Boolean),
        ),
      ]),
    ).sort((a, b) => a.localeCompare(b)),
  );
  const pipelineFilterOptions = $derived.by(() => {
    const values = ['any', ...knownPipelineTags];
    if (filters.pipelineTag !== 'any' && !values.includes(filters.pipelineTag)) {
      values.push(filters.pipelineTag);
    }
    return values;
  });
  const licenseFilterOptions = $derived.by(() => {
    const values = ['any', ...knownLicenses];
    if (filters.license !== 'any' && !values.includes(filters.license)) {
      values.push(filters.license);
    }
    return values;
  });
  const languageFilterOptions = $derived.by(() => {
    const values = ['any', ...knownLanguages];
    if (filters.language !== 'any' && !values.includes(filters.language)) {
      values.push(filters.language);
    }
    return values;
  });
  const pipelineOptionCounts = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const model of filterCatalogResults) {
      const tag = normalizePipelineTag(model.pipeline_tag ?? '');
      if (!tag) continue;
      counts.set(tag, (counts.get(tag) ?? 0) + 1);
    }
    return counts;
  });
  const licenseOptionCounts = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const model of filterCatalogResults) {
      const direct = (model.license ?? '').trim().toLowerCase();
      if (direct) counts.set(direct, (counts.get(direct) ?? 0) + 1);
      const tags = Array.isArray(model.tags) ? model.tags : [];
      for (const tag of tags) {
        const normalized = tag.trim().toLowerCase();
        if (!normalized.startsWith('license:')) continue;
        const value = normalized.slice('license:'.length).trim();
        if (!value) continue;
        counts.set(value, (counts.get(value) ?? 0) + 1);
      }
    }
    return counts;
  });
  const languageOptionCounts = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const model of filterCatalogResults) {
      const languages = Array.isArray(model.languages) ? model.languages : [];
      for (const language of languages) {
        const normalized = language.trim().toLowerCase();
        if (!/^[a-z]{2}(?:-[a-z]{2})?$/.test(normalized)) continue;
        counts.set(normalized, (counts.get(normalized) ?? 0) + 1);
      }
      const tags = Array.isArray(model.tags) ? model.tags : [];
      for (const tag of tags) {
        const normalized = tag.trim().toLowerCase();
        const value = normalized.startsWith('language:')
          ? normalized.slice('language:'.length).trim()
          : normalized;
        if (!/^[a-z]{2}(?:-[a-z]{2})?$/.test(value)) continue;
        counts.set(value, (counts.get(value) ?? 0) + 1);
      }
    }
    return counts;
  });
  const mainPipelineOptions = $derived.by(() =>
    getMainFilterOptions(
      pipelineFilterOptions,
      pipelineOptionCounts,
      FILTER_MAIN_LIMIT,
      normalizePipelineTag,
    ),
  );
  const mainLicenseOptions = $derived.by(() =>
    getMainFilterOptions(licenseFilterOptions, licenseOptionCounts, FILTER_MAIN_LIMIT),
  );
  const mainLanguageOptions = $derived.by(() =>
    getMainFilterOptions(languageFilterOptions, languageOptionCounts, FILTER_MAIN_LIMIT),
  );
  const hiddenPipelineOptions = $derived(
    Math.max(0, Math.max(0, pipelineFilterOptions.length - 1) - mainPipelineOptions.length),
  );
  const hiddenLicenseOptions = $derived(
    Math.max(0, Math.max(0, licenseFilterOptions.length - 1) - mainLicenseOptions.length),
  );
  const hiddenLanguageOptions = $derived(
    Math.max(0, Math.max(0, languageFilterOptions.length - 1) - mainLanguageOptions.length),
  );
  const visiblePipelineOptions = $derived.by(() =>
    getSearchedFilterOptions(pipelineFilterOptions, pipelineFilterQuery),
  );
  const visiblePipelineGroups = $derived.by(() => {
    const grouped = new Map<PipelineCategoryId, string[]>();
    for (const pipelineTag of visiblePipelineOptions) {
      const category = getPipelineCategory(pipelineTag);
      const list = grouped.get(category) ?? [];
      list.push(pipelineTag);
      grouped.set(category, list);
    }

    return PIPELINE_CATEGORY_DEFINITIONS.map((category) => ({
      ...category,
      options: (grouped.get(category.id) ?? []).sort((left, right) => left.localeCompare(right)),
    })).filter((group) => group.options.length > 0);
  });
  const visibleLicenseOptions = $derived.by(() =>
    getSearchedFilterOptions(licenseFilterOptions, licenseFilterQuery),
  );
  const visibleLanguageOptions = $derived.by(() =>
    getSearchedFilterOptions(languageFilterOptions, languageFilterQuery),
  );
  const hasActiveFilters = $derived.by(() => {
    return (
      filters.license !== 'any' ||
      filters.pipelineTag !== 'any' ||
      filters.language !== 'any' ||
      filters.parameterMinB !== null ||
      filters.parameterMaxB !== null
    );
  });
  const localFallbackMatches = $derived.by(() => {
    if (!searchError || !searchQuery.trim() || !$localModels.length) return [] as ModelInfo[];
    const index = new SimSearch(
      $localModels.map((model) => ({
        id: model.path,
        text: [
          model.name,
          model.model_name ?? '',
          model.architecture ?? '',
          model.quantization ?? '',
          model.source_repo_name ?? '',
        ].join(' '),
      })),
    );
    const ids = new Set(index.search(searchQuery.trim().toLowerCase(), 8).map((entry) => entry.id));
    return $localModels.filter((model) => ids.has(model.path)).slice(0, 5);
  });

  function findFileHistory(repoId: string, filename: string): DownloadHistoryEntry | null {
    return (
      get(downloadHistory).find(
        (entry) =>
          entry.repo_id === repoId && entry.filename.toLowerCase() === filename.toLowerCase(),
      ) ?? null
    );
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

  function withStaticTag(model: RemoteModelInfo): RemoteModelInfo {
    const tags = Array.isArray(model.tags) ? model.tags : [];
    if (tags.includes('static:first-page')) return model;
    return { ...model, tags: [...tags, 'static:first-page'] };
  }

  async function fetchRemoteModelByRepoId(repoId: string): Promise<RemoteModelInfo | null> {
    const exact = await LocalModelsService.getRemoteModelInfo(repoId);
    return exact ? withStaticTag(exact) : null;
  }

  async function hydrateStaticFirstPageModels(token: number): Promise<void> {
    const cached = getCachedSearchPage(cachedEntries, STATIC_FIRST_PAGE_CACHE_QUERY, 0).map(
      withStaticTag,
    );
    const byRepo = new Map(cached.map((item) => [item.repo_id, item]));

    const initial = STATIC_FIRST_PAGE_REPO_IDS.map((repoId) => byRepo.get(repoId)).filter(
      (item): item is RemoteModelInfo => Boolean(item),
    );
    if (initial.length) {
      currentPageResults = initial;
      mergeRemoteResults(initial);
      mergeFilterCatalog(initial);
    }

    const missing = STATIC_FIRST_PAGE_REPO_IDS.filter((repoId) => !byRepo.has(repoId));
    if (!missing.length || shouldSkipRemoteFetch()) return;

    const fetched: RemoteModelInfo[] = [];
    for (let i = 0; i < missing.length; i += 4) {
      if (token !== searchRequestToken) return;
      const chunk = missing.slice(i, i + 4);
      const chunkResult = await Promise.all(
        chunk.map((repoId) => fetchRemoteModelByRepoId(repoId)),
      );
      for (const model of chunkResult) {
        if (!model) continue;
        byRepo.set(model.repo_id, model);
        fetched.push(model);
      }
      const hydrated = STATIC_FIRST_PAGE_REPO_IDS.map((repoId) => byRepo.get(repoId)).filter(
        (item): item is RemoteModelInfo => Boolean(item),
      );
      currentPageResults = hydrated;
      mergeRemoteResults(hydrated);
      mergeFilterCatalog(hydrated);
    }

    if (fetched.length) {
      const hydrated = STATIC_FIRST_PAGE_REPO_IDS.map((repoId) => byRepo.get(repoId)).filter(
        (item): item is RemoteModelInfo => Boolean(item),
      );
      cachedEntries = upsertSearchCachePage(
        cachedEntries,
        STATIC_FIRST_PAGE_CACHE_QUERY,
        0,
        RESULTS_PER_PAGE,
        hydrated,
        REMOTE_CACHE_MAX_ENTRIES,
        REMOTE_CACHE_MAX_PAGES_PER_QUERY,
        REMOTE_CACHE_MAX_ITEMS_PER_PAGE,
      );
      persistCache();
    }
  }

  function parseSearchQuery(rawQuery: string): { query: string; filename: string | null } {
    const raw = rawQuery.trim();
    const parsed = extractRepoFromHfUrl(raw);
    return {
      query: parsed?.repoId ?? raw,
      filename: parsed?.filename ?? null,
    };
  }

  function queryCacheKey(query: string): string {
    const normalized = query.trim().toLowerCase();
    return normalized.length > 0 ? normalized : '__trending__';
  }

  function buildFiltersKey(value: RemoteSearchFilters): string {
    const tags = [...value.tags].sort().join(',');
    const architectures = [...value.architectures].sort().join(',');
    return [
      architectures,
      value.format,
      value.quantization,
      tags,
      value.license,
      value.pipelineTag,
      value.library,
      value.language,
      value.parameter,
      value.parameterMinB ?? '',
      value.parameterMaxB ?? '',
      value.sizeBucket,
      String(value.minDownloads),
      value.sortBy,
      value.sortOrder,
      value.newThisWeek ? '1' : '0',
    ].join('|');
  }

  function normalizePipelineTag(value: string): string {
    return value
      .trim()
      .toLowerCase()
      .replace(/[_\s]+/g, '-');
  }

  function getPipelineCategory(value: string): PipelineCategoryId {
    return PIPELINE_TAG_CATEGORY_MAP.get(normalizePipelineTag(value)) ?? 'other';
  }

  function getPipelineCategoryDefinition(value: string): PipelineCategoryDefinition {
    const category = getPipelineCategory(value);
    return (
      PIPELINE_CATEGORY_DEFINITIONS.find((definition) => definition.id === category) ??
      PIPELINE_CATEGORY_DEFINITIONS[PIPELINE_CATEGORY_DEFINITIONS.length - 1]
    );
  }

  function pipelineChipClass(value: string, active: boolean): string {
    const category = getPipelineCategoryDefinition(value);
    const base =
      'inline-flex h-7 items-center gap-1.5 rounded-md border px-2.5 text-[11px] font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40';
    return `${base} ${active ? category.chipActiveClass : category.chipIdleClass}`;
  }

  function pipelineOverflowChipClass(): string {
    const base =
      'inline-flex h-7 items-center gap-1.5 rounded-md border px-2.5 text-[11px] font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40';
    return `${base} border-primary/25 bg-primary/5 text-primary/90 hover:bg-primary/10`;
  }

  function filterChipClass(kind: 'license' | 'language', active: boolean): string {
    const base =
      'inline-flex h-7 items-center gap-1.5 rounded-md border px-2.5 text-[11px] font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40';

    if (kind === 'license') {
      return `${base} ${
        active
          ? 'border-amber-500/55 bg-amber-500/15 text-amber-700 dark:text-amber-200'
          : 'border-amber-500/30 bg-amber-500/5 text-amber-700/85 hover:bg-amber-500/12 dark:text-amber-300'
      }`;
    }

    return `${base} ${
      active
        ? 'border-sky-500/55 bg-sky-500/15 text-sky-700 dark:text-sky-200'
        : 'border-sky-500/30 bg-sky-500/5 text-sky-700/85 hover:bg-sky-500/12 dark:text-sky-300'
    }`;
  }

  function getMainFilterOptions(
    options: string[],
    counts: Map<string, number>,
    limit: number,
    normalizeKey: (value: string) => string = (value) => value,
  ): string[] {
    return options
      .filter((value) => value !== 'any')
      .sort((left, right) => {
        const countDelta =
          (counts.get(normalizeKey(right)) ?? 0) - (counts.get(normalizeKey(left)) ?? 0);
        if (countDelta !== 0) return countDelta;
        return left.localeCompare(right);
      })
      .slice(0, limit);
  }

  function getSearchedFilterOptions(options: string[], rawQuery: string): string[] {
    const query = rawQuery.trim().toLowerCase();
    if (!query) return options.filter((value) => value !== 'any');
    return options
      .filter((value) => value !== 'any')
      .filter((value) => value.toLowerCase().includes(query));
  }

  function filterTabClass(tab: FilterTab): string {
    const active = activeFilterTab === tab;
    const base = 'h-7 rounded-md px-3 text-[11px] font-medium transition-colors';
    return active
      ? `${base} bg-primary/20 text-primary`
      : `${base} text-muted-foreground hover:bg-muted/40 hover:text-foreground`;
  }

  function getKnownPageCursor(query: string, page: number): string | null | undefined {
    const key = queryCacheKey(query);
    return pageCursorByQuery[key]?.[page];
  }

  function setKnownPageCursor(query: string, page: number, cursor: string | null): void {
    const key = queryCacheKey(query);
    const existing = pageCursorByQuery[key] ?? {};
    if (existing[page] === cursor) return;
    pageCursorByQuery = {
      ...pageCursorByQuery,
      [key]: {
        ...existing,
        [page]: cursor,
      },
    };
  }

  async function ensureCursorForPage(
    query: string,
    page: number,
    token: number,
  ): Promise<string | null | undefined> {
    const pageIndex = Math.max(1, Math.floor(page));
    const known = getKnownPageCursor(query, pageIndex);
    if (known !== undefined) return known;
    if (pageIndex === 1) {
      setKnownPageCursor(query, 1, null);
      return null;
    }

    const prevCursor = await ensureCursorForPage(query, pageIndex - 1, token);
    if (token !== searchRequestToken || prevCursor === undefined) return undefined;

    // Try to discover cursor for this page by fetching previous page.
    const prevOffset = (pageIndex - 2) * RESULTS_PER_PAGE;
    const prevCached = getCachedSearchPage(cachedEntries, query, prevOffset);
    if (prevCached.length) {
      const nextKnown = getKnownPageCursor(query, pageIndex);
      if (nextKnown !== undefined) return nextKnown;
    }

    const prevPageResult = await LocalModelsService.searchRemote(query || 'gguf', {
      limit: RESULTS_PER_PAGE,
      cursor: prevCursor ?? undefined,
      offset: prevOffset,
    });
    if (token !== searchRequestToken) return undefined;

    const prevItems = dedupeByRepoId(prevPageResult.items ?? []);
    cachedEntries = upsertSearchCachePage(
      cachedEntries,
      query,
      prevOffset,
      RESULTS_PER_PAGE,
      prevItems,
      REMOTE_CACHE_MAX_ENTRIES,
      REMOTE_CACHE_MAX_PAGES_PER_QUERY,
      REMOTE_CACHE_MAX_ITEMS_PER_PAGE,
    );
    persistCache();

    const nextCursor = prevPageResult.next_cursor?.trim() || null;
    if (!nextCursor) return undefined;
    setKnownPageCursor(query, pageIndex, nextCursor);
    return nextCursor;
  }

  function isFilePending(repoId: string, filename: string): boolean {
    return pendingDownloads[fileKey(repoId, filename)] === true;
  }

  function hasResolvedParameterCount(model: RemoteModelInfo): boolean {
    return Boolean(model.parameter_count?.trim());
  }

  function isStaticFirstPageModel(model: RemoteModelInfo): boolean {
    return (Array.isArray(model.tags) ? model.tags : []).some(
      (tag) => tag.toLowerCase() === 'static:first-page',
    );
  }

  function getModelIconFamily(model: RemoteModelInfo): string {
    const normalizedRepo = model.repo_id.trim().toLowerCase();
    const publisher = model.author?.trim().toLowerCase() || normalizedRepo.split('/')[0] || '';

    // Publisher to lobe-hub icon mapping - ONLY publishers with actual icons in lobe-hub
    // All unknown publishers automatically fallback to 'huggingface' icon
    const PUBLISHER_ICON_MAP: Record<string, string> = {
      // Major model providers
      qwen: 'qwen',
      alibaba: 'alibaba',
      'alibaba-nlp': 'alibaba',
      google: 'google',
      microsoft: 'microsoft',
      mistralai: 'mistral',
      'mistral-community': 'mistral',
      cohereforai: 'cohere',
      cohere: 'cohere',
      yandex: 'yandex',
      'deepseek-ai': 'deepseek',
      deepseek: 'deepseek',
      'meta-llama': 'meta',
      meta: 'meta',
      tiiuae: 'tii',
      anthropic: 'claude',
      openai: 'openai',
      stabilityai: 'stability',
      nvidia: 'nvidia',
      ibm: 'ibm',
      apple: 'apple',
      ai21: 'ai21',
      allenai: 'ai2',
      '360ai': 'ai360',
      'arcee-ai': 'arcee',
      'black-forest-labs': 'bfl',
      bfl: 'bfl',
      snowflake: 'snowflake',
      upstage: 'upstage',
      xai: 'xai',
      intel: 'intel',

      // Chinese tech companies
      'stepfun-ai': 'stepfun',
      stepfun: 'stepfun',
      'baichuan-inc': 'baichuan',
      thudm: 'chatglm',
      zhipuai: 'zhipu',
      'zai-org': 'zai',
      zai: 'zai',
      baidu: 'baidu',
      bytedance: 'bytedance',
      '01-ai': 'yi',
      tencent: 'tencent',
      huawei: 'huawei',
      minimax: 'minimax',
      internlm: 'internlm',
      'internlm-community': 'internlm',
      infinigence: 'infinigence',
      moonshotai: 'moonshot',
      sensetime: 'sensenova',

      // Infrastructure / Providers
      groq: 'groq',
      together: 'together',
      fireworksai: 'fireworks',
      'fireworks-ai': 'fireworks',
      replicate: 'replicate',
      anyscale: 'anyscale',
      perplexity: 'perplexity',
      cloudflare: 'cloudflare',
      leptonai: 'leptonai',
      deepinfra: 'deepinfra',
      novitaai: 'novita',
      ollama: 'ollama',
      openrouter: 'openrouter',
      'lm-studio': 'lmstudio',
      'lmstudio-community': 'lmstudio',
      'lmstudio-ai': 'lmstudio',
      'liquid-ai': 'liquid',
      liquidai: 'liquid',
      lightricks: 'lightricks',
      gradientai: 'gradient',

      // Community & Research
      nousresearch: 'nousresearch',
      rwkv: 'rwkv',
      cerebras: 'cerebras',
      sambanova: 'sambanova',
      jinaai: 'jina',
      'vllm-project': 'vllm',
      elevenlabs: 'elevenlabs',
      openchat: 'openchat',
    };

    // Automatic fallback to huggingface for all unknown publishers
    return PUBLISHER_ICON_MAP[publisher] || 'huggingface';
  }

  function hasLobeHubPublisherIcon(model: RemoteModelInfo): boolean {
    return getModelIconFamily(model) !== 'huggingface';
  }

  function getCustomModelIconSvg(model: RemoteModelInfo): string | null {
    return STATIC_MODEL_ICON_BY_REPO.get(model.repo_id) ?? null;
  }

  function getPublisher(model: RemoteModelInfo): string {
    return (
      model.author?.trim().toLowerCase() || model.repo_id.split('/')[0]?.trim().toLowerCase() || ''
    );
  }

  function isUnslothPublisher(model: RemoteModelInfo): boolean {
    return getPublisher(model) === 'unsloth';
  }

  function getOwnerAvatarUrl(model: RemoteModelInfo): string | null {
    const publisher = getPublisher(model);
    if (!publisher) return null;
    if (hasLobeHubPublisherIcon(model)) return null;
    const cached = ownerAvatarByPublisher[publisher];
    if (cached === '') return null;
    return cached || null;
  }

  function handleOwnerAvatarError(model: RemoteModelInfo, _event: Event): void {
    const publisher = getPublisher(model);
    if (!publisher) return;
    ownerAvatarByPublisher = { ...ownerAvatarByPublisher, [publisher]: '' };
  }

  async function resolveOwnerAvatar(publisher: string): Promise<void> {
    const normalized = publisher.trim().toLowerCase();
    if (!normalized) return;
    if (ownerAvatarByPublisher[normalized] !== undefined) return;
    if (ownerAvatarFetchInFlight[normalized]) return;

    ownerAvatarFetchInFlight = { ...ownerAvatarFetchInFlight, [normalized]: true };

    const extractAvatarUrl = (payload: unknown): string | null => {
      const data = payload as Record<string, unknown> | null;
      if (!data) return null;
      const candidate =
        (typeof data.avatarUrl === 'string' && data.avatarUrl) ||
        (typeof data.avatar_url === 'string' && data.avatar_url) ||
        (typeof data.avatar === 'string' && data.avatar) ||
        '';
      if (!candidate) return null;
      if (candidate.toLowerCase().includes('gravatar.com')) return null;
      return candidate;
    };

    try {
      const orgResponse = await fetch(
        `https://huggingface.co/api/organizations/${normalized}/overview`,
        {
          credentials: 'omit',
        },
      );
      if (orgResponse.ok) {
        const orgData = await orgResponse.json();
        const avatar = extractAvatarUrl(orgData);
        ownerAvatarByPublisher = { ...ownerAvatarByPublisher, [normalized]: avatar ?? '' };
        return;
      }

      const userResponse = await fetch(`https://huggingface.co/api/users/${normalized}/overview`, {
        credentials: 'omit',
      });
      if (userResponse.ok) {
        const userData = await userResponse.json();
        const avatar = extractAvatarUrl(userData);
        ownerAvatarByPublisher = { ...ownerAvatarByPublisher, [normalized]: avatar ?? '' };
        return;
      }

      ownerAvatarByPublisher = { ...ownerAvatarByPublisher, [normalized]: '' };
    } catch {
      ownerAvatarByPublisher = { ...ownerAvatarByPublisher, [normalized]: '' };
    } finally {
      const next = { ...ownerAvatarFetchInFlight };
      delete next[normalized];
      ownerAvatarFetchInFlight = next;
    }
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
  }

  async function fetchMissingModelMetadata(repoId: string) {
    if (!repoId || metadataFetchAttempted[repoId] || metadataFetchInFlight[repoId]) return;
    const model = remoteResults.find((item) => item.repo_id === repoId);
    if (!model || hasResolvedParameterCount(model)) return;

    metadataFetchAttempted = { ...metadataFetchAttempted, [repoId]: true };
    metadataFetchInFlight = { ...metadataFetchInFlight, [repoId]: true };

    try {
      const metadata = await LocalModelsService.getRemoteModelMetadata(repoId);
      updateRemoteModelMetadata(repoId, metadata.parameter_count, metadata.context_length);
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
    cachedEntries = loadSearchCache(
      window.localStorage,
      SEARCH_CACHE_KEY,
      REMOTE_CACHE_MAX_ENTRIES,
      REMOTE_CACHE_MAX_PAGES_PER_QUERY,
      REMOTE_CACHE_MAX_ITEMS_PER_PAGE,
    );
    try {
      const raw = window.localStorage.getItem(SEARCH_TOTALS_KEY);
      const parsed = raw ? JSON.parse(raw) : {};
      if (parsed && typeof parsed === 'object') {
        const safeEntries = Object.fromEntries(
          Object.entries(parsed).filter(
            ([key, value]) =>
              typeof key === 'string' &&
              typeof value === 'number' &&
              Number.isFinite(value) &&
              value >= 0,
          ),
        );
        totalByQuery = safeEntries as Record<string, number>;
      }
    } catch {
      totalByQuery = {};
    }
  }

  function persistHistory() {
    if (typeof window === 'undefined') return;
    saveSearchHistory(window.localStorage, SEARCH_HISTORY_KEY, searchHistory, 10);
  }

  function persistCache() {
    if (typeof window === 'undefined') return;
    saveSearchCache(window.localStorage, SEARCH_CACHE_KEY, cachedEntries, REMOTE_CACHE_MAX_ENTRIES);
  }

  function persistTotals() {
    if (typeof window === 'undefined') return;
    window.localStorage.setItem(SEARCH_TOTALS_KEY, JSON.stringify(totalByQuery));
  }

  function cacheTotalForQuery(query: string, total: number) {
    if (!Number.isFinite(total) || total < 0) return;
    const key = queryCacheKey(query);
    if (totalByQuery[key] === total) return;
    totalByQuery = { ...totalByQuery, [key]: total };
    persistTotals();
  }

  async function fetchTotalMatches(query: string, token: number) {
    if (!query) {
      totalMatches = null;
      return;
    }
    try {
      const nextTotal = await LocalModelsService.getRemoteSearchTotal(query);
      if (token !== searchRequestToken) return;
      if (typeof nextTotal === 'number' && Number.isFinite(nextTotal) && nextTotal >= 0) {
        totalMatches = nextTotal;
        cacheTotalForQuery(query, nextTotal);
      }
    } catch {
      // Non-fatal: list still works without explicit total count.
    }
  }

  function isNetworkFailureMessage(message: string): boolean {
    const lower = message.toLowerCase();
    return (
      lower.includes('offline') ||
      lower.includes('network') ||
      lower.includes('timeout') ||
      lower.includes('connection') ||
      lower.includes('fetch') ||
      lower.includes('error sending request')
    );
  }

  function shouldSkipRemoteFetch(): boolean {
    if (typeof navigator !== 'undefined' && navigator.onLine === false) {
      return true;
    }
    return Boolean(remoteBlockedUntil && remoteBlockedUntil > Date.now());
  }

  function mergeRemoteResults(items: RemoteModelInfo[]) {
    if (!items.length) return;
    remoteResults = dedupeByRepoId([...remoteResults, ...items]);
  }

  function mergeFilterCatalog(items: RemoteModelInfo[]) {
    if (!items.length) return;
    filterCatalogResults = dedupeByRepoId([...filterCatalogResults, ...items]);
  }

  async function loadSearchPage(query: string, page: number, token: number, pageToken: number) {
    const pageIndex = Math.max(1, Math.floor(page));
    const offset = (pageIndex - 1) * RESULTS_PER_PAGE;
    const pageKey = `${queryCacheKey(query)}::${pageIndex}`;
    if (
      pageKey === lastLoadedPageKey &&
      token === searchRequestToken &&
      currentPageResults.length > 0
    ) {
      return;
    }
    const cursor = await ensureCursorForPage(query, pageIndex, token);
    if (token !== searchRequestToken) return;
    if (cursor === undefined) {
      if (pageToken !== pageRequestToken || pageIndex !== resultsPage) return;
      currentPageResults = [];
      searchHint = 'No more pages available for this query.';
      return;
    }

    const cachedPage = getCachedSearchPage(cachedEntries, query, offset);
    if (cachedPage.length) {
      if (pageToken !== pageRequestToken || pageIndex !== resultsPage) return;
      currentPageResults = dedupeByRepoId(cachedPage);
      mergeRemoteResults(cachedPage);
      mergeFilterCatalog(cachedPage);
      searchHint = isSearching ? 'Showing cached page while refreshing from Hugging Face...' : null;
    }

    const livePage = await LocalModelsService.searchRemote(query || 'gguf', {
      limit: RESULTS_PER_PAGE,
      cursor: cursor ?? undefined,
      offset,
    });
    if (token !== searchRequestToken) return;

    const live = dedupeByRepoId(livePage.items ?? []);
    if (pageToken !== pageRequestToken || pageIndex !== resultsPage) return;
    currentPageResults = dedupeByRepoId(live);
    mergeRemoteResults(live);
    mergeFilterCatalog(live);
    const nextCursor = livePage.next_cursor?.trim();
    if (nextCursor) {
      setKnownPageCursor(query, pageIndex + 1, nextCursor);
    }
    cachedEntries = upsertSearchCachePage(
      cachedEntries,
      query,
      offset,
      RESULTS_PER_PAGE,
      live,
      REMOTE_CACHE_MAX_ENTRIES,
      REMOTE_CACHE_MAX_PAGES_PER_QUERY,
      REMOTE_CACHE_MAX_ITEMS_PER_PAGE,
    );
    lastLoadedPageKey = pageKey;
    persistCache();
  }

  async function performSearch(rawQuery: string) {
    const token = ++searchRequestToken;
    const parsed = parseSearchQuery(rawQuery);
    const query = parsed.query;
    pendingUrlFilename = parsed.filename;
    resultsPage = 1;
    isSearching = false;
    hasSearched = true;
    searchError = null;
    searchHint = null;
    searchKind = 'none';
    usingFallback = false;
    currentPageResults = [];
    remoteResults = [];
    filterCatalogResults = [];
    lastLoadedPageKey = '';
    setKnownPageCursor(query, 1, null);
    totalMatches =
      totalByQuery[queryCacheKey(query)] ?? totalByQuery[queryCacheKey(query || 'gguf')] ?? null;
    const cachedSnapshot = getCachedQueryResults(
      cachedEntries,
      query,
      REMOTE_FILTER_CATALOG_BOOTSTRAP_ITEMS,
    );
    if (cachedSnapshot.length) {
      mergeRemoteResults(cachedSnapshot);
      mergeFilterCatalog(cachedSnapshot);
    }

    if (!STATIC_CATALOG_ONLY_MODE || !query) {
      if (!query) {
        isSearching = true;
        isPageLoading = true;
        await hydrateStaticFirstPageModels(token);
        if (token !== searchRequestToken) return;
        isSearching = false;
        isPageLoading = false;
      }
      if (query) {
        searchHistory = updateSearchHistory(searchHistory, query, 10);
        persistHistory();
      }
      if (token !== searchRequestToken) return;
      isInitialLoading = false;
      return;
    }

    isSearching = true;
    isPageLoading = true;
    try {
      const cachedPage = getCachedSearchPage(cachedEntries, query, 0);
      if (cachedPage.length) {
        currentPageResults = dedupeByRepoId(cachedPage);
        mergeRemoteResults(cachedPage);
        mergeFilterCatalog(cachedPage);
      } else {
        currentPageResults = [];
      }

      const livePage = await LocalModelsService.searchRemote(query, {
        limit: RESULTS_PER_PAGE,
        offset: 0,
      });
      if (token !== searchRequestToken) return;
      const live = dedupeByRepoId(livePage.items ?? []);
      currentPageResults = live;
      mergeRemoteResults(live);
      mergeFilterCatalog(live);
      cachedEntries = upsertSearchCachePage(
        cachedEntries,
        query,
        0,
        RESULTS_PER_PAGE,
        live,
        REMOTE_CACHE_MAX_ENTRIES,
        REMOTE_CACHE_MAX_PAGES_PER_QUERY,
        REMOTE_CACHE_MAX_ITEMS_PER_PAGE,
      );
      persistCache();
      searchHistory = updateSearchHistory(searchHistory, query, 10);
      persistHistory();
      usingFallback = false;
      searchHint = null;
    } catch (error) {
      if (token !== searchRequestToken) return;
      const message = error instanceof Error ? error.message : String(error);
      searchError = message;
      if (isNetworkFailureMessage(message)) {
        searchKind = 'offline';
        remoteBlockedUntil = Date.now() + REMOTE_RETRY_COOLDOWN_MS;
      } else {
        searchKind = 'api';
      }
      const fallback = getCachedFallback(cachedEntries, query, RESULTS_PER_PAGE);
      currentPageResults = dedupeByRepoId(fallback);
      mergeRemoteResults(fallback);
      mergeFilterCatalog(fallback);
      usingFallback = fallback.length > 0;
      searchHint =
        fallback.length > 0 ? 'Cached search results shown.' : 'Search unavailable right now.';
    } finally {
      if (token !== searchRequestToken) return;
      isSearching = false;
      isPageLoading = false;
      isInitialLoading = false;
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

  function handlePageChange(newPage: number) {
    resultsPage = newPage;
    if (newPage === 1) {
      pageRequestToken += 1;
      const hydratedStatic = remoteResults.filter((item) => isStaticFirstPageModel(item));
      currentPageResults = hydratedStatic.length ? hydratedStatic : STATIC_FIRST_PAGE_MODELS;
      remoteResults = dedupeByRepoId([
        ...STATIC_FIRST_PAGE_MODELS,
        ...remoteResults.filter((item) => !isStaticFirstPageModel(item)),
      ]);
      searchHint = null;
      searchError = null;
      isPageLoading = false;
      isSearching = false;
      return;
    }
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
    activeFilterTab = 'main';
    pipelineFilterQuery = '';
    licenseFilterQuery = '';
    languageFilterQuery = '';
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
      parameterMinB: null,
      parameterMaxB: null,
      sizeBucket: 'any',
      minDownloads: 0,
      sortBy: 'downloads',
      sortOrder: 'desc',
      newThisWeek: true,
    };
    parameterSliderValue = [0, Math.max(0, PARAMETER_RANGE_STEPS.length - 1)];
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
    if (pendingDownloads[key] || getFileJob(get(activeDownloads), model.repo_id, file.filename))
      return;

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
      const editable =
        target?.isContentEditable ||
        ['INPUT', 'TEXTAREA', 'SELECT'].includes(target?.tagName ?? '');
      if (!editable) {
        event.preventDefault();
        searchInputRef?.focus();
        searchInputRef?.select();
      }
    }
  }

  function handleWindowOnline() {
    remoteBlockedUntil = null;
    if (searchKind === 'offline') {
      searchKind = 'none';
      searchHint = null;
      searchError = null;
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
    const nextFiltersKey = buildFiltersKey(filters);
    if (!lastFiltersKey) {
      lastFiltersKey = nextFiltersKey;
      return;
    }
    if (nextFiltersKey !== lastFiltersKey) {
      lastFiltersKey = nextFiltersKey;
      resultsPage = 1;
    }
  });

  $effect(() => {
    if (STATIC_CATALOG_ONLY_MODE) return;
    if (resultsPage === 1) {
      const hydratedStatic = remoteResults.filter((item) => isStaticFirstPageModel(item));
      currentPageResults = hydratedStatic.length ? hydratedStatic : STATIC_FIRST_PAGE_MODELS;
      return;
    }
    if (searchRequestToken === 0 || isSearching) return;
    const token = searchRequestToken;
    const pageToken = ++pageRequestToken;
    const parsed = parseSearchQuery(searchQuery);
    const query = parsed.query;
    const totalQuery = query || 'gguf';

    if (shouldSkipRemoteFetch()) {
      const offset = (Math.max(1, Math.floor(resultsPage)) - 1) * RESULTS_PER_PAGE;
      const cachedPage = getCachedSearchPage(cachedEntries, query, offset);
      currentPageResults = dedupeByRepoId(cachedPage);
      if (cachedPage.length) {
        mergeRemoteResults(cachedPage);
        mergeFilterCatalog(cachedPage);
      }
      searchKind = 'offline';
      searchHint =
        remoteBlockedSeconds > 0
          ? `Hugging Face temporarily unavailable. Retrying in ~${remoteBlockedSeconds}s.`
          : 'Offline mode: showing cached results only.';
      isSearching = false;
      isPageLoading = false;
      return;
    }

    isSearching = true;
    isPageLoading = true;
    searchHint = null;
    currentPageResults = [];

    void Promise.all([
      loadSearchPage(query, resultsPage, token, pageToken),
      fetchTotalMatches(totalQuery, token),
    ])
      .catch((error) => {
        if (token !== searchRequestToken) return;
        const message = error instanceof Error ? error.message : String(error);
        searchHint = message;
        searchError = message;
        if (isNetworkFailureMessage(message)) {
          searchKind = 'offline';
          remoteBlockedUntil = Date.now() + REMOTE_RETRY_COOLDOWN_MS;
          searchHint = `Hugging Face unavailable. Using cache, retry in ~${Math.ceil(REMOTE_RETRY_COOLDOWN_MS / 1000)}s.`;
        }
      })
      .finally(() => {
        if (token !== searchRequestToken || pageToken !== pageRequestToken) return;
        isSearching = false;
        isPageLoading = false;
      });
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
      selectedFileByRepo = {
        ...selectedFileByRepo,
        [selectedModel.repo_id]: selectedModel.gguf_files[0].filename,
      };
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
    const seen = new Set<string>();
    for (const model of currentPageResults) {
      if (hasLobeHubPublisherIcon(model)) continue;
      const publisher = getPublisher(model);
      if (!publisher || seen.has(publisher)) continue;
      seen.add(publisher);
      if (ownerAvatarByPublisher[publisher] !== undefined) continue;
      if (ownerAvatarFetchInFlight[publisher]) continue;
      void resolveOwnerAvatar(publisher);
    }
  });

  $effect(() => {
    if (!selectedModel) {
      readmeRepoId = '';
      readmeContent = '';
      readmeError = null;
      return;
    }
    if (shouldSkipRemoteFetch()) {
      readmeRepoId = '';
      readmeContent = '';
      readmeError = 'Hugging Face unavailable.';
      return;
    }
    if (!isStaticFirstPageModel(selectedModel)) {
      void fetchReadme(selectedModel.repo_id);
    }
    void fetchMissingModelMetadata(selectedModel.repo_id);
  });

  $effect(() => {
    for (const model of pagedResults) {
      if (hasResolvedParameterCount(model)) continue;
      void fetchMissingModelMetadata(model.repo_id);
    }
  });

  $effect(() => {
    if (filters.parameterMinB === null && filters.parameterMaxB === null) return;
    const missing = remoteResults.filter((model) => !hasResolvedParameterCount(model)).slice(0, 20);
    for (const model of missing) {
      void fetchMissingModelMetadata(model.repo_id);
    }
  });

  $effect(() => {
    const activeKeys = new Set($activeDownloads.map((job) => fileKey(job.repo_id, job.filename)));
    const historyKeys = new Set(
      $downloadHistory.map((entry) => fileKey(entry.repo_id, entry.filename)),
    );
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
    window.addEventListener('online', handleWindowOnline);
    tickerHandle = setInterval(() => {
      nowTick = Date.now();
    }, 1000);
    return () => {
      window.removeEventListener('keydown', handleGlobalKeydown);
      window.removeEventListener('online', handleWindowOnline);
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
    <aside
      class="h-full min-w-0 min-h-0 rounded-lg bg-card p-3 overflow-y-auto overflow-x-hidden custom-scrollbar space-y-3"
    >
      <div class="sticky top-0 bg-card z-10 pb-2 pt-0">
        <div class="flex flex-wrap items-center gap-2">
          <span class="inline-flex items-center text-xs font-medium text-muted-foreground"
            ><SlidersHorizontal class="mr-1 size-3.5" />Filters</span
          >
          <Button
            size="sm"
            variant="ghost"
            class="h-7 px-2 text-[11px]"
            onclick={resetFilters}
            disabled={!hasActiveFilters}>Clear</Button
          >
        </div>

        <div class="space-y-2">
          <div class="flex flex-wrap items-center gap-1">
            <button
              type="button"
              class={filterTabClass('main')}
              aria-pressed={activeFilterTab === 'main'}
              onclick={() => (activeFilterTab = 'main')}>Main</button
            >
            <button
              type="button"
              class={filterTabClass('pipeline')}
              aria-pressed={activeFilterTab === 'pipeline'}
              onclick={() => (activeFilterTab = 'pipeline')}>Pipeline Tag</button
            >
            <button
              type="button"
              class={filterTabClass('license')}
              aria-pressed={activeFilterTab === 'license'}
              onclick={() => (activeFilterTab = 'license')}>License</button
            >
            <button
              type="button"
              class={filterTabClass('language')}
              aria-pressed={activeFilterTab === 'language'}
              onclick={() => (activeFilterTab = 'language')}>Language</button
            >
          </div>
        </div>
      </div>

      {#if activeFilterTab === 'main'}
        <div class="space-y-3">
          <div class="space-y-2">
            <p class="text-[11px] text-muted-foreground">Pipeline Tag</p>
            <div class="flex flex-wrap gap-2">
              {#each mainPipelineOptions as pipelineTag}
                {@const active = filters.pipelineTag === pipelineTag}
                <button
                  type="button"
                  class={pipelineChipClass(pipelineTag, active)}
                  aria-pressed={active}
                  onclick={() => {
                    filters = { ...filters, pipelineTag: active ? 'any' : pipelineTag };
                  }}
                >
                  <span>{pipelineTag}</span>
                </button>
              {/each}
              {#if hiddenPipelineOptions > 0}
                <button
                  type="button"
                  class={pipelineOverflowChipClass()}
                  onclick={() => (activeFilterTab = 'pipeline')}
                >
                  <span>+{hiddenPipelineOptions}</span>
                </button>
              {/if}
            </div>
          </div>

          <div class="space-y-2">
            <p class="text-[11px] text-muted-foreground">License</p>
            <div class="flex flex-wrap gap-2">
              {#each mainLicenseOptions as license}
                {@const active = filters.license === license}
                <button
                  type="button"
                  class={filterChipClass('license', active)}
                  aria-pressed={active}
                  onclick={() => {
                    filters = { ...filters, license: active ? 'any' : license };
                  }}
                >
                  <span>{license}</span>
                </button>
              {/each}
              {#if hiddenLicenseOptions > 0}
                <button
                  type="button"
                  class={filterChipClass('license', false)}
                  onclick={() => (activeFilterTab = 'license')}
                >
                  <span>+{hiddenLicenseOptions}</span>
                </button>
              {/if}
            </div>
          </div>

          <div class="space-y-2">
            <p class="text-[11px] text-muted-foreground">Language</p>
            <div class="flex flex-wrap gap-2">
              {#each mainLanguageOptions as language}
                {@const active = filters.language === language}
                <button
                  type="button"
                  class={filterChipClass('language', active)}
                  aria-pressed={active}
                  onclick={() => {
                    filters = { ...filters, language: active ? 'any' : language };
                  }}
                >
                  <span>{language}</span>
                </button>
              {/each}
              {#if hiddenLanguageOptions > 0}
                <button
                  type="button"
                  class={filterChipClass('language', false)}
                  onclick={() => (activeFilterTab = 'language')}
                >
                  <span>+{hiddenLanguageOptions}</span>
                </button>
              {/if}
            </div>
          </div>

          <div class="space-y-2">
            <p class="text-[11px] text-muted-foreground">Parameters</p>
            <Slider.Root
              type="multiple"
              min={0}
              max={Math.max(0, PARAMETER_RANGE_STEPS.length - 1)}
              step={1}
              bind:value={parameterSliderValue}
              onValueChange={(value) => {
                const input = Array.isArray(value)
                  ? value
                  : [Number(value) || 0, Number(value) || 0];
                const left = Math.max(
                  0,
                  Math.min(PARAMETER_RANGE_STEPS.length - 1, Math.round(Number(input[0]) || 0)),
                );
                const right = Math.max(
                  0,
                  Math.min(PARAMETER_RANGE_STEPS.length - 1, Math.round(Number(input[1]) || 0)),
                );
                const minIdx = Math.min(left, right);
                const maxIdx = Math.max(left, right);
                const minBucket = PARAMETER_RANGE_STEPS[minIdx];
                const maxBucket = PARAMETER_RANGE_STEPS[maxIdx];
                parameterSliderValue = [minIdx, maxIdx];
                filters = {
                  ...filters,
                  parameter: 'any',
                  parameterMinB: minBucket?.minB ?? 0,
                  parameterMaxB: maxBucket?.maxB ?? null,
                };
              }}
            />
            <div class="grid grid-cols-3 gap-x-2 gap-y-1 text-[10px] text-muted-foreground">
              {#each PARAMETER_RANGE_STEPS as step}
                <span>{step.label}</span>
              {/each}
            </div>
            <p class="text-[11px] text-muted-foreground">
              Selected: {PARAMETER_RANGE_STEPS[parameterSliderValue[0]]?.label ?? 'Any'} - {PARAMETER_RANGE_STEPS[
                parameterSliderValue[1]
              ]?.label ?? 'Any'}
            </p>
          </div>
        </div>
      {:else if activeFilterTab === 'pipeline'}
        <div class="space-y-2">
          <div class="relative">
            <MagnifyingGlass
              class="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground"
            />
            <Input
              type="search"
              value={pipelineFilterQuery}
              placeholder="Filter pipeline tags by name"
              class="h-8 pl-8 text-xs"
              oninput={(event) => {
                pipelineFilterQuery = (event.target as HTMLInputElement).value;
              }}
            />
          </div>
          {#if visiblePipelineGroups.length === 0}
            <p class="text-[11px] text-muted-foreground">No pipeline tags found.</p>
          {:else}
            {#each visiblePipelineGroups as group (group.id)}
              <div class="space-y-2">
                <p class={`text-[11px] font-medium ${group.headerClass}`}>{group.label}</p>
                <div class="flex flex-wrap gap-2">
                  {#each group.options as pipelineTag}
                    {@const active = filters.pipelineTag === pipelineTag}
                    <button
                      type="button"
                      class={pipelineChipClass(pipelineTag, active)}
                      aria-pressed={active}
                      onclick={() => {
                        filters = { ...filters, pipelineTag: active ? 'any' : pipelineTag };
                      }}
                    >
                      <span>{pipelineTag}</span>
                    </button>
                  {/each}
                </div>
              </div>
            {/each}
          {/if}
        </div>
      {:else if activeFilterTab === 'license'}
        <div class="space-y-2">
          <div class="relative">
            <MagnifyingGlass
              class="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground"
            />
            <Input
              type="search"
              value={licenseFilterQuery}
              placeholder="Filter licenses by name"
              class="h-8 pl-8 text-xs"
              oninput={(event) => {
                licenseFilterQuery = (event.target as HTMLInputElement).value;
              }}
            />
          </div>
          <div class="flex flex-wrap gap-2">
            {#if visibleLicenseOptions.length === 0}
              <p class="text-[11px] text-muted-foreground">No licenses found.</p>
            {:else}
              {#each visibleLicenseOptions as license}
                {@const active = filters.license === license}
                <button
                  type="button"
                  class={filterChipClass('license', active)}
                  aria-pressed={active}
                  onclick={() => {
                    filters = { ...filters, license: active ? 'any' : license };
                  }}
                >
                  <span>{license}</span>
                </button>
              {/each}
            {/if}
          </div>
        </div>
      {:else}
        <div class="space-y-2">
          <div class="relative">
            <MagnifyingGlass
              class="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground"
            />
            <Input
              type="search"
              value={languageFilterQuery}
              placeholder="Filter languages by name"
              class="h-8 pl-8 text-xs"
              oninput={(event) => {
                languageFilterQuery = (event.target as HTMLInputElement).value;
              }}
            />
          </div>
          <div class="flex flex-wrap gap-2">
            {#if visibleLanguageOptions.length === 0}
              <p class="text-[11px] text-muted-foreground">No languages found.</p>
            {:else}
              {#each visibleLanguageOptions as language}
                {@const active = filters.language === language}
                <button
                  type="button"
                  class={filterChipClass('language', active)}
                  aria-pressed={active}
                  onclick={() => {
                    filters = { ...filters, language: active ? 'any' : language };
                  }}
                >
                  <span>{language}</span>
                </button>
              {/each}
            {/if}
          </div>
        </div>
      {/if}
    </aside>

    <div class="min-h-0 flex items-stretch justify-center">
      <Separator.Root orientation="vertical" class="h-full" />
    </div>

    <section class="h-full min-w-0 min-h-0 rounded-lg bg-card flex flex-col overflow-hidden">
      <div class="p-3">
        <div class="grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2">
          <span class="text-xs text-muted-foreground whitespace-nowrap">
            {effectiveTotalMatches > 0
              ? effectiveTotalMatches.toLocaleString()
              : filteredResults.length.toLocaleString()} models
          </span>
          <div class="relative min-w-0">
            <MagnifyingGlass
              class="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground"
            />
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
            <div
              class="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 flex items-center text-[11px] text-muted-foreground"
            >
              {#if isSearching}<Spinner class="size-3.5" />{/if}
            </div>
          </div>
          <div class="w-[190px]">
            <Select.Root type="single" value={currentSortMode} onValueChange={handleSortModeChange}>
              <Select.Trigger class="h-10 w-full text-xs"
                >Sort: {currentSortMode === 'likes'
                  ? 'Most likes'
                  : currentSortMode === 'updated'
                    ? 'Recently updated'
                    : currentSortMode === 'downloads'
                      ? 'Most downloads'
                      : 'Trending'}</Select.Trigger
              >
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
        {#if isInitialLoading || isPageLoading}
          <div class="space-y-2">
            {#each Array.from({ length: 6 }) as _, index (`skeleton-${index}`)}
              <div class="rounded-md border p-3 space-y-2">
                <Skeleton class="h-4 w-2/3" />
                <Skeleton class="h-3 w-full" />
              </div>
            {/each}
          </div>
        {:else if filteredResults.length === 0}
          <div
            class="h-full min-h-[220px] flex flex-col items-center justify-center gap-3 text-center px-6"
          >
            <Cube class="size-12 text-muted-foreground/60" weight="light" />
            <p class="text-sm font-medium">No models found</p>
            <div class="flex gap-2">
              <Button
                variant="outline"
                size="sm"
                onclick={resetFilters}
                disabled={!hasActiveFilters}>Clear</Button
              >
              <Button variant="outline" size="sm" onclick={() => runSearchNow()}>Refresh</Button>
            </div>
            {#if localFallbackMatches.length > 0}
              <p class="text-xs text-muted-foreground">
                Local matches: {localFallbackMatches.length}
              </p>
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
                class="rounded-md border p-2 transition-colors focus:outline-none focus:ring-2 focus:ring-primary/40 {selected
                  ? 'border-primary/40 bg-primary/10'
                  : 'bg-muted/20 hover:bg-muted/40'}"
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
                    {#if getOwnerAvatarUrl(model)}
                      <img
                        src={getOwnerAvatarUrl(model) ?? undefined}
                        alt={`${getPublisher(model)} avatar`}
                        class="size-4 flex-shrink-0 rounded-sm object-cover"
                        loading="lazy"
                        decoding="async"
                        onerror={(event) => handleOwnerAvatarError(model, event)}
                      />
                    {:else if getCustomModelIconSvg(model)}
                      <span
                        class="inline-flex size-4 flex-shrink-0 items-center justify-center rounded-sm [&>svg]:size-4"
                      >
                        {@html getCustomModelIconSvg(model) ?? ''}
                      </span>
                    {:else}
                      <ModelIcon
                        family={getModelIconFamily(model)}
                        size={16}
                        class="size-4 flex-shrink-0 rounded-sm"
                      />
                    {/if}
                    <p class="truncate text-sm font-medium">{model.repo_id}</p>
                    {#if isVerifiedAuthor(model)}<LinkSimple
                        class="size-3.5 shrink-0 text-emerald-600"
                        weight="fill"
                      />{/if}
                  </div>
                  <div
                    class="row-span-2 flex flex-col items-end justify-center gap-1 text-[11px] text-muted-foreground"
                  >
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
                    {pipelineTag ? `${pipelineTag} · ` : ''}{extractParameterLabel(model)} · Updated
                    {getRelativeTimeLabel(model.last_modified)}
                  </p>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>
      {#if totalResultsPages > 1}
        <footer
          class="px-3 py-2 flex items-center justify-between gap-2 text-xs text-muted-foreground"
        >
          <div>Page {resultsPage} of {totalResultsPages}</div>
          <Pagination.Root
            count={Math.max(1, effectiveTotalMatches)}
            perPage={RESULTS_PER_PAGE}
            bind:page={resultsPage}
            siblingCount={1}
            onPageChange={handlePageChange}
          >
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
                      <Pagination.Link
                        {page}
                        isActive={currentPage === page.value}
                        size="sm"
                        class="h-8 min-w-8 text-[11px]"
                      >
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
        </footer>
      {/if}
    </section>
  </div>

  <Dialog.Root bind:open={modelDialogOpen}>
    <Dialog.Content
      class="!w-[min(97vw,1320px)] !max-w-[min(97vw,1320px)] sm:!max-w-[min(97vw,1320px)] h-[min(90vh,860px)] flex flex-col"
    >
      {#if selectedModel}
        <Dialog.Header>
          <Dialog.Title class="truncate pr-6">{selectedModel.repo_id}</Dialog.Title>
          <Dialog.Description>
            Updated {getRelativeTimeLabel(selectedModel.last_modified)}
          </Dialog.Description>
        </Dialog.Header>

        <div class="border-b px-1 pb-3 shrink-0">
          <div class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_420px]">
            <div class="space-y-2 min-w-0">
              <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                  <p class="text-xs text-muted-foreground">
                    Detailed model card with file-level download actions
                  </p>
                </div>
              </div>
              <div class="flex flex-wrap gap-1.5">
                <Badge variant="outline">{extractParameterLabel(selectedModel)}</Badge>
                <Badge variant="outline"
                  ><DownloadSimple
                    class="mr-1 size-3.5"
                  />{selectedModel.downloads.toLocaleString()}</Badge
                >
                <Badge variant="outline"
                  ><Heart class="mr-1 size-3.5" />{selectedModel.likes.toLocaleString()}</Badge
                >
              </div>
              {#if selectedModel.description}<p class="text-sm text-muted-foreground line-clamp-3">
                  {selectedModel.description}
                </p>{/if}
              <label class="flex items-center gap-2 text-xs text-muted-foreground">
                <Checkbox
                  checked={loadAfterDownload}
                  onCheckedChange={(value) => (loadAfterDownload = value === true)}
                />
                Load after download
              </label>
            </div>

            <div class="space-y-0 self-start -mt-4">
              <div class="rounded-md border bg-muted/20 p-3 pt-2 space-y-2">
                <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
                  <div class="min-w-0 flex justify-center">
                    {#if selectedFile}
                      <p class="text-center text-xs text-muted-foreground break-words">
                        Selected: {selectedFile.quantization ?? '—'} · {formatBytes(
                          selectedFile.size,
                        )}
                        · ~{estimateVramGb(selectedFile.size, selectedFile.quantization)} GB VRAM
                      </p>
                    {:else}
                      <p class="text-center text-xs text-muted-foreground break-words">
                        Selected: —
                      </p>
                    {/if}
                  </div>
                  <Button
                    variant="outline"
                    size="sm"
                    class="shrink-0"
                    onclick={() => openExternal(`https://huggingface.co/${selectedModel.repo_id}`)}
                  >
                    <LinkSimple class="mr-1 size-4" />
                    Open on HF
                  </Button>
                </div>

                <div class="grid grid-cols-[minmax(0,1fr)_auto] items-start gap-2">
                  <Select.Root
                    type="single"
                    value={selectedFile?.filename ?? ''}
                    onValueChange={(value) => {
                      if (!value) return;
                      selectedFileByRepo = {
                        ...selectedFileByRepo,
                        [selectedModel.repo_id]: value,
                      };
                    }}
                  >
                    <Select.Trigger class="w-full justify-between text-left">
                      {#if selectedFile}
                        <span class="truncate font-mono text-[11px]">{selectedFile.filename}</span>
                      {:else}
                        <span class="text-xs text-muted-foreground">Select GGUF</span>
                      {/if}
                    </Select.Trigger>
                    <Select.Content class="z-[1310] max-h-[260px]">
                      {#each selectedModel.gguf_files as file (file.filename)}
                        <Select.Item value={file.filename}>
                          <div class="min-w-0 space-y-0.5">
                            <p class="truncate font-mono text-[11px]">{file.filename}</p>
                            <p class="text-[10px] text-muted-foreground">
                              {file.quantization ?? '—'} · {formatBytes(file.size)} · ~{estimateVramGb(
                                file.size,
                                file.quantization,
                              )} GB VRAM
                            </p>
                          </div>
                        </Select.Item>
                      {/each}
                    </Select.Content>
                  </Select.Root>
                  <div class="flex items-center gap-1 shrink-0">
                    {#if selectedFile}
                      {#if getFileJob($activeDownloads, selectedModel.repo_id, selectedFile.filename)}
                        {@const selectedJob = getFileJob(
                          $activeDownloads,
                          selectedModel.repo_id,
                          selectedFile.filename,
                        )}
                        <Button
                          variant="ghost"
                          size="icon"
                          class="size-8"
                          onclick={() => void handlePauseResume(selectedJob!)}
                          >{#if selectedJob!.status === 'paused'}<Play
                              class="size-4"
                            />{:else}<Pause class="size-4" />{/if}</Button
                        >
                        <Button
                          variant="ghost"
                          size="icon"
                          class="size-8"
                          onclick={() => void handleCancel(selectedJob!)}
                          ><XCircle class="size-4" /></Button
                        >
                      {:else}
                        <Button
                          size="sm"
                          onclick={() => void handleDownload(selectedModel, selectedFile)}
                          disabled={isFilePending(selectedModel.repo_id, selectedFile.filename)}
                        >
                          {#if isFilePending(selectedModel.repo_id, selectedFile.filename)}<Spinner
                              class="mr-1 size-4"
                            />Queued{:else}<DownloadSimple class="mr-1 size-4" />Download{/if}
                        </Button>
                      {/if}
                    {/if}
                  </div>
                </div>
                {#if selectedFile}
                  {#if getFileJob($activeDownloads, selectedModel.repo_id, selectedFile.filename)}
                    {@const selectedJob = getFileJob(
                      $activeDownloads,
                      selectedModel.repo_id,
                      selectedFile.filename,
                    )}
                    <Progress.Root
                      value={getDownloadProgress(selectedJob)}
                      max={100}
                      class="h-1.5"
                    />
                    <p class="text-xs text-muted-foreground">
                      {Math.round(getDownloadProgress(selectedJob!))}% · {formatSpeedLabel(
                        selectedJob!.speed_bytes_per_sec,
                      )} · ETA {formatEtaLabel(selectedJob!.eta_seconds)}
                    </p>
                  {:else if findFileHistory(selectedModel.repo_id, selectedFile.filename)?.status === 'completed'}
                    <p class="text-xs text-emerald-600">Downloaded</p>
                  {/if}
                  {#if downloadErrors[fileKey(selectedModel.repo_id, selectedFile.filename)]}
                    <p class="text-xs text-destructive">
                      {downloadErrors[fileKey(selectedModel.repo_id, selectedFile.filename)]}
                    </p>
                  {/if}
                {/if}
              </div>
            </div>
          </div>
        </div>

        <div
          class="flex-1 min-h-0 overflow-y-auto overflow-x-hidden custom-scrollbar px-1 pt-1 space-y-2"
        >
          <div class="space-y-2">
            <div class="flex items-center justify-between gap-2">
              <h4 class="text-sm font-medium">README preview</h4>
              <Button
                variant="ghost"
                size="sm"
                class="h-7 px-2 text-xs"
                onclick={() => openExternal(`https://huggingface.co/${selectedModel.repo_id}`)}
                >Open full page</Button
              >
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
            <p class="text-xs">
              Use filters above to narrow by pipeline tag, license, language, and parameters.
            </p>
          </div>
        </div>
      {/if}
    </Dialog.Content>
  </Dialog.Root>
</div>
