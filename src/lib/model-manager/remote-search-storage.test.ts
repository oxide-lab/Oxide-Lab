import { describe, expect, it } from 'vitest';
import type { RemoteModelInfo } from '$lib/types/local-models';
import {
  getCachedFallback,
  getCachedQueryResults,
  getCachedSearchPage,
  upsertSearchCache,
  upsertSearchCachePage,
  type RemoteSearchCacheEntry,
} from '$lib/model-manager/remote-search-storage';

function makeModel(repoId: string): RemoteModelInfo {
  return {
    repo_id: repoId,
    name: repoId.split('/')[1] ?? repoId,
    author: repoId.split('/')[0],
    description: undefined,
    license: 'mit',
    pipeline_tag: 'text-generation',
    library: 'transformers',
    languages: ['en'],
    downloads: 100,
    likes: 10,
    tags: ['gguf'],
    architectures: [],
    quantizations: [],
    gguf_files: [],
    last_modified: undefined,
    created_at: undefined,
    parameter_count: undefined,
    context_length: undefined,
  };
}

describe('remote-search-storage', () => {
  it('upserts cache entry and keeps latest first', () => {
    const first = makeModel('a/one');
    const second = makeModel('b/two');

    let cache: RemoteSearchCacheEntry[] = [];
    cache = upsertSearchCache(cache, 'llama', [first]);
    cache = upsertSearchCache(cache, 'mistral', [second]);

    expect(cache).toHaveLength(2);
    expect(cache[0]?.query).toBe('mistral');
    expect(cache[1]?.query).toBe('llama');
  });

  it('returns exact cached fallback first', () => {
    const first = makeModel('a/one');
    const second = makeModel('b/two');

    const cache = upsertSearchCache(
      upsertSearchCache([], 'llama', [first]),
      'mistral',
      [second],
    );

    const fallback = getCachedFallback(cache, 'llama');
    expect(fallback).toHaveLength(1);
    expect(fallback[0]?.repo_id).toBe('a/one');
  });

  it('stores and reads cache pages by query + offset', () => {
    const firstPage = [makeModel('a/one'), makeModel('b/two')];
    const secondPage = [makeModel('c/three')];

    let cache: RemoteSearchCacheEntry[] = [];
    cache = upsertSearchCachePage(cache, 'qwen', 0, 2, firstPage);
    cache = upsertSearchCachePage(cache, 'qwen', 2, 2, secondPage);

    const cachedPage0 = getCachedSearchPage(cache, 'qwen', 0);
    const cachedPage2 = getCachedSearchPage(cache, 'qwen', 2);
    const flattened = getCachedQueryResults(cache, 'qwen');

    expect(cachedPage0.map((item) => item.repo_id)).toEqual(['a/one', 'b/two']);
    expect(cachedPage2.map((item) => item.repo_id)).toEqual(['c/three']);
    expect(flattened.map((item) => item.repo_id)).toEqual(['a/one', 'b/two', 'c/three']);
  });

  it('keeps legacy upsert API backward-compatible', () => {
    const legacy = upsertSearchCache([], 'phi', [makeModel('x/one')]);
    const snapshot = getCachedQueryResults(legacy, 'phi');
    expect(snapshot).toHaveLength(1);
    expect(snapshot[0]?.repo_id).toBe('x/one');
  });
});
