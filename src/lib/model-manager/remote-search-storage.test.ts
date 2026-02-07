import { describe, expect, it } from 'vitest';
import type { RemoteModelInfo } from '$lib/types/local-models';
import {
  getCachedFallback,
  upsertSearchCache,
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
});
