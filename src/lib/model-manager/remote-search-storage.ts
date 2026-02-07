import { applyFuzzyFallback } from '$lib/model-manager/remote-search-utils';
import type { RemoteModelInfo } from '$lib/types/local-models';

export interface RemoteSearchCacheEntry {
  query: string;
  updatedAt: number;
  items: RemoteModelInfo[];
}

const TRENDING_KEY = '__trending__';

function normalize(value: string): string {
  return value.trim().toLowerCase();
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
): RemoteSearchCacheEntry[] {
  try {
    const raw = storage.getItem(key);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];

    return parsed
      .filter(
        (entry): entry is RemoteSearchCacheEntry =>
          entry &&
          typeof entry.query === 'string' &&
          typeof entry.updatedAt === 'number' &&
          Array.isArray(entry.items),
      )
      .map((entry) => ({
        ...entry,
        items: dedupeByRepo(entry.items),
      }))
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
  if (!items.length) return entries;

  const cacheKey = normalize(query) || TRENDING_KEY;
  const nextEntry: RemoteSearchCacheEntry = {
    query: cacheKey,
    updatedAt: Date.now(),
    items: dedupeByRepo(items).slice(0, maxItemsPerQuery),
  };

  return [nextEntry, ...entries.filter((entry) => entry.query !== cacheKey)].slice(0, maxEntries);
}

export function getCachedFallback(
  entries: RemoteSearchCacheEntry[],
  query: string,
  fuzzyLimit = 30,
): RemoteModelInfo[] {
  if (!entries.length) return [];

  const normalized = normalize(query);
  if (!normalized) {
    const trending = entries.find((entry) => entry.query === TRENDING_KEY);
    return dedupeByRepo(trending?.items ?? entries[0]?.items ?? []);
  }

  const exact = entries.find((entry) => entry.query === normalized);
  if (exact?.items?.length) {
    return dedupeByRepo(exact.items);
  }

  return applyFuzzyFallback(dedupeByRepo(entries.flatMap((entry) => entry.items)), normalized, fuzzyLimit);
}
