import { applyFuzzyFallback } from '$lib/model-manager/remote-search-utils';
import type { RemoteModelInfo } from '$lib/types/local-models';

export interface RemoteSearchCachePage {
  offset: number;
  limit: number;
  updatedAt: number;
  items: RemoteModelInfo[];
}

export interface RemoteSearchCacheEntry {
  query: string;
  updatedAt: number;
  pages: RemoteSearchCachePage[];
}

const TRENDING_KEY = '__trending__';

function normalize(value: string): string {
  return value.trim().toLowerCase();
}

function normalizeQueryKey(query: string): string {
  return normalize(query) || TRENDING_KEY;
}

function sanitizeModel(input: unknown): RemoteModelInfo | null {
  if (!input || typeof input !== 'object') return null;
  const model = input as Partial<RemoteModelInfo>;
  if (typeof model.repo_id !== 'string' || model.repo_id.trim().length === 0) return null;

  return {
    repo_id: model.repo_id,
    name: typeof model.name === 'string' ? model.name : model.repo_id.split('/').at(-1) ?? model.repo_id,
    author: typeof model.author === 'string' ? model.author : model.repo_id.split('/')[0],
    description: typeof model.description === 'string' ? model.description : undefined,
    license: typeof model.license === 'string' ? model.license : undefined,
    pipeline_tag: typeof model.pipeline_tag === 'string' ? model.pipeline_tag : undefined,
    library: typeof model.library === 'string' ? model.library : undefined,
    languages: Array.isArray(model.languages)
      ? model.languages.filter((item): item is string => typeof item === 'string')
      : [],
    downloads: typeof model.downloads === 'number' ? model.downloads : 0,
    likes: typeof model.likes === 'number' ? model.likes : 0,
    tags: Array.isArray(model.tags) ? model.tags.filter((tag): tag is string => typeof tag === 'string') : [],
    architectures: Array.isArray(model.architectures)
      ? model.architectures.filter((item): item is string => typeof item === 'string')
      : [],
    quantizations: Array.isArray(model.quantizations)
      ? model.quantizations.filter((item): item is string => typeof item === 'string')
      : [],
    gguf_files: Array.isArray(model.gguf_files)
      ? model.gguf_files.filter(
          (file): file is RemoteModelInfo['gguf_files'][number] =>
            !!file && typeof file.filename === 'string' && typeof file.download_url === 'string',
        )
      : [],
    last_modified: typeof model.last_modified === 'string' ? model.last_modified : undefined,
    created_at: typeof model.created_at === 'string' ? model.created_at : undefined,
    parameter_count: typeof model.parameter_count === 'string' ? model.parameter_count : undefined,
    context_length: typeof model.context_length === 'number' ? model.context_length : undefined,
  };
}

function dedupeByRepo(items: unknown[]): RemoteModelInfo[] {
  const seen = new Set<string>();
  const result: RemoteModelInfo[] = [];

  for (const rawItem of items) {
    const item = sanitizeModel(rawItem);
    if (!item) continue;
    if (seen.has(item.repo_id)) continue;
    seen.add(item.repo_id);
    result.push(item);
  }

  return result;
}

function sanitizePage(input: unknown, maxItemsPerPage: number): RemoteSearchCachePage | null {
  if (!input || typeof input !== 'object') return null;
  const rawPage = input as Partial<RemoteSearchCachePage>;
  if (!Array.isArray(rawPage.items)) return null;

  const items = dedupeByRepo(rawPage.items).slice(0, maxItemsPerPage);
  if (!items.length) return null;

  const offset = typeof rawPage.offset === 'number' && Number.isFinite(rawPage.offset) && rawPage.offset >= 0
    ? Math.floor(rawPage.offset)
    : 0;
  const limit = typeof rawPage.limit === 'number' && Number.isFinite(rawPage.limit) && rawPage.limit > 0
    ? Math.floor(rawPage.limit)
    : Math.max(1, items.length);
  const updatedAt = typeof rawPage.updatedAt === 'number' && Number.isFinite(rawPage.updatedAt)
    ? rawPage.updatedAt
    : Date.now();

  return {
    offset,
    limit,
    updatedAt,
    items,
  };
}

function normalizeEntry(
  input: unknown,
  maxPagesPerQuery: number,
  maxItemsPerPage: number,
): RemoteSearchCacheEntry | null {
  if (!input || typeof input !== 'object') return null;
  const rawEntry = input as {
    query?: unknown;
    updatedAt?: unknown;
    pages?: unknown;
    items?: unknown;
  };

  if (typeof rawEntry.query !== 'string') return null;
  const query = normalizeQueryKey(rawEntry.query);

  let pages: RemoteSearchCachePage[] = [];
  if (Array.isArray(rawEntry.pages)) {
    pages = rawEntry.pages
      .map((page) => sanitizePage(page, maxItemsPerPage))
      .filter((page): page is RemoteSearchCachePage => page !== null);
  }

  // Backward-compat for legacy { items: RemoteModelInfo[] } cache shape.
  if (pages.length === 0 && Array.isArray(rawEntry.items)) {
    const legacy = sanitizePage(
      {
        offset: 0,
        limit: maxItemsPerPage,
        updatedAt: rawEntry.updatedAt,
        items: rawEntry.items,
      },
      maxItemsPerPage,
    );
    if (legacy) {
      pages = [legacy];
    }
  }

  if (!pages.length) return null;

  pages = pages
    .sort((a, b) => a.offset - b.offset)
    .slice(0, maxPagesPerQuery);

  const updatedAt = typeof rawEntry.updatedAt === 'number' && Number.isFinite(rawEntry.updatedAt)
    ? rawEntry.updatedAt
    : Math.max(...pages.map((page) => page.updatedAt));

  return {
    query,
    updatedAt,
    pages,
  };
}

function flattenEntryItems(entry: RemoteSearchCacheEntry, maxItems: number): RemoteModelInfo[] {
  const out: RemoteModelInfo[] = [];
  const seen = new Set<string>();

  for (const page of [...entry.pages].sort((a, b) => a.offset - b.offset)) {
    for (const item of page.items) {
      if (seen.has(item.repo_id)) continue;
      seen.add(item.repo_id);
      out.push(item);
      if (out.length >= maxItems) return out;
    }
  }

  return out;
}

export function loadSearchHistory(storage: Storage, key: string, maxItems = 10): string[] {
  try {
    const raw = storage.getItem(key);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((item): item is string => typeof item === 'string' && item.trim().length > 0)
      .slice(0, maxItems);
  } catch {
    return [];
  }
}

export function saveSearchHistory(
  storage: Storage,
  key: string,
  history: string[],
  maxItems = 10,
): void {
  storage.setItem(key, JSON.stringify(history.slice(0, maxItems)));
}

export function loadSearchCache(
  storage: Storage,
  key: string,
  maxEntries = 20,
  maxPagesPerQuery = 8,
  maxItemsPerPage = 60,
): RemoteSearchCacheEntry[] {
  try {
    const raw = storage.getItem(key);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];

    return parsed
      .map((entry) => normalizeEntry(entry, maxPagesPerQuery, maxItemsPerPage))
      .filter((entry): entry is RemoteSearchCacheEntry => entry !== null)
      .slice(0, maxEntries);
  } catch {
    return [];
  }
}

export function saveSearchCache(
  storage: Storage,
  key: string,
  entries: RemoteSearchCacheEntry[],
  maxEntries = 20,
): void {
  storage.setItem(key, JSON.stringify(entries.slice(0, maxEntries)));
}

export function upsertSearchCache(
  entries: RemoteSearchCacheEntry[],
  query: string,
  items: RemoteModelInfo[],
  maxEntries = 20,
  maxItemsPerQuery = 50,
): RemoteSearchCacheEntry[] {
  return upsertSearchCachePage(
    entries,
    query,
    0,
    maxItemsPerQuery,
    items,
    maxEntries,
    1,
    maxItemsPerQuery,
  );
}

export function upsertSearchCachePage(
  entries: RemoteSearchCacheEntry[],
  query: string,
  offset: number,
  limit: number,
  items: RemoteModelInfo[],
  maxEntries = 20,
  maxPagesPerQuery = 8,
  maxItemsPerPage = 60,
): RemoteSearchCacheEntry[] {
  if (!items.length) return entries;

  const cacheKey = normalizeQueryKey(query);
  const now = Date.now();
  const nextPage: RemoteSearchCachePage = {
    offset: Math.max(0, Math.floor(offset)),
    limit: Math.max(1, Math.floor(limit)),
    updatedAt: now,
    items: dedupeByRepo(items).slice(0, maxItemsPerPage),
  };
  if (!nextPage.items.length) return entries;

  const previous = entries.find((entry) => entry.query === cacheKey);
  const mergedPages = [
    nextPage,
    ...(previous?.pages ?? []).filter((page) => page.offset !== nextPage.offset),
  ]
    .sort((a, b) => a.offset - b.offset)
    .slice(0, maxPagesPerQuery);

  const nextEntry: RemoteSearchCacheEntry = {
    query: cacheKey,
    updatedAt: now,
    pages: mergedPages,
  };

  return [nextEntry, ...entries.filter((entry) => entry.query !== cacheKey)]
    .slice(0, maxEntries);
}

export function getCachedSearchPage(
  entries: RemoteSearchCacheEntry[],
  query: string,
  offset: number,
): RemoteModelInfo[] {
  const cacheKey = normalizeQueryKey(query);
  const entry = entries.find((item) => item.query === cacheKey);
  if (!entry) return [];

  const page = entry.pages.find((item) => item.offset === Math.max(0, Math.floor(offset)));
  if (!page) return [];
  return dedupeByRepo(page.items);
}

export function getCachedQueryResults(
  entries: RemoteSearchCacheEntry[],
  query: string,
  maxItems = 120,
): RemoteModelInfo[] {
  const cacheKey = normalizeQueryKey(query);
  const entry = entries.find((item) => item.query === cacheKey);
  if (!entry) return [];
  return flattenEntryItems(entry, maxItems);
}

export function getCachedFallback(
  entries: RemoteSearchCacheEntry[],
  query: string,
  fuzzyLimit = 30,
): RemoteModelInfo[] {
  if (!entries.length) return [];

  const normalized = normalize(query);
  if (!normalized) {
    const trending = getCachedQueryResults(entries, TRENDING_KEY, fuzzyLimit);
    if (trending.length) return trending;
    return entries.length ? flattenEntryItems(entries[0], fuzzyLimit) : [];
  }

  const exact = getCachedQueryResults(entries, normalized, fuzzyLimit);
  if (exact.length) {
    return exact;
  }

  return applyFuzzyFallback(
    dedupeByRepo(entries.flatMap((entry) => flattenEntryItems(entry, fuzzyLimit))),
    normalized,
    fuzzyLimit,
  );
}
