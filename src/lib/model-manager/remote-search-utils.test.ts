import { describe, expect, it } from 'vitest';
import type { RemoteModelInfo } from '$lib/types/local-models';
import {
  applyFuzzyFallback,
  applyRemoteFilters,
  estimateVramGb,
  extractRepoFromHfUrl,
  getRelativeTimeLabel,
  updateSearchHistory,
  type RemoteSearchFilters,
} from '$lib/model-manager/remote-search-utils';

function createModel(overrides: Partial<RemoteModelInfo>): RemoteModelInfo {
  return {
    repo_id: 'TheBloke/Llama-2-7B-GGUF',
    name: 'Llama-2-7B-GGUF',
    author: 'TheBloke',
    description: 'A quantized model',
    license: 'mit',
    pipeline_tag: 'text-generation',
    library: 'transformers',
    languages: ['en'],
    downloads: 1000,
    likes: 100,
    tags: ['gguf', 'chat', 'instruct'],
    architectures: ['llama'],
    quantizations: ['Q4_K_M', 'Q5_K_M'],
    gguf_files: [
      {
        filename: 'model.Q4_K_M.gguf',
        size: 4 * 1024 * 1024 * 1024,
        quantization: 'Q4_K_M',
        download_url: 'https://huggingface.co/TheBloke/Llama-2-7B-GGUF/resolve/main/model.Q4_K_M.gguf',
      },
    ],
    last_modified: '2026-02-01T10:00:00Z',
    created_at: '2026-01-01T10:00:00Z',
    parameter_count: '7B',
    context_length: 8192,
    ...overrides,
  };
}

describe('remote-search utils', () => {
  it('deduplicates and limits search history', () => {
    const history = ['mistral', 'llama', 'q4_k_m', 'phi', 'orca'];
    const updated = updateSearchHistory(history, 'llama', 5);
    expect(updated).toEqual(['llama', 'mistral', 'q4_k_m', 'phi', 'orca']);

    const appended = updateSearchHistory(updated, 'new model', 5);
    expect(appended).toEqual(['new model', 'llama', 'mistral', 'q4_k_m', 'phi']);
  });

  it('parses huggingface url with repo and optional filename', () => {
    expect(extractRepoFromHfUrl('https://huggingface.co/TheBloke/Llama-2-7B-GGUF')).toEqual({
      repoId: 'TheBloke/Llama-2-7B-GGUF',
    });
    expect(
      extractRepoFromHfUrl(
        'https://huggingface.co/TheBloke/Llama-2-7B-GGUF/resolve/main/model.Q4_K_M.gguf',
      ),
    ).toEqual({
      repoId: 'TheBloke/Llama-2-7B-GGUF',
      filename: 'model.Q4_K_M.gguf',
    });
  });

  it('applies architecture, quantization and size filters', () => {
    const models = [
      createModel({ repo_id: 'a/llama-q4', architectures: ['llama'], quantizations: ['Q4_K_M'] }),
      createModel({
        repo_id: 'b/mistral-q8',
        architectures: ['mistral'],
        quantizations: ['Q8_0'],
        gguf_files: [
          {
            filename: 'model.Q8_0.gguf',
            size: 10 * 1024 * 1024 * 1024,
            quantization: 'Q8_0',
            download_url: 'https://huggingface.co/b/mistral-q8/resolve/main/model.Q8_0.gguf',
          },
        ],
      }),
    ];
    const filters: RemoteSearchFilters = {
      architectures: ['llama'],
      format: 'gguf',
      quantization: 'Q4_K_M',
      tags: [],
      license: 'any',
      pipelineTag: 'any',
      library: 'any',
      language: 'any',
      parameter: 'any',
      sizeBucket: 'lt4gb',
      minDownloads: 0,
      sortBy: 'downloads',
      sortOrder: 'desc',
      newThisWeek: false,
    };
    const filtered = applyRemoteFilters(models, filters);
    expect(filtered).toHaveLength(1);
    expect(filtered[0]?.repo_id).toBe('a/llama-q4');
  });

  it('returns fuzzy fallback hits from cached models', () => {
    const models = [
      createModel({ repo_id: 'TheBloke/Llama-2-7B-GGUF', name: 'Llama 2 GGUF' }),
      createModel({ repo_id: 'mistralai/Mistral-7B-Instruct-v0.3', name: 'Mistral Instruct' }),
    ];
    const hits = applyFuzzyFallback(models, 'mistral', 10);
    expect(hits).toHaveLength(1);
    expect(hits[0]?.repo_id).toContain('Mistral');
  });

  it('estimates vram in GB and formats relative time', () => {
    const vram = estimateVramGb(4 * 1024 * 1024 * 1024, 'Q4_K_M');
    expect(vram).toBeGreaterThan(0);
    const label = getRelativeTimeLabel('2026-02-05T00:00:00Z', new Date('2026-02-07T00:00:00Z'));
    expect(label).toContain('day');
  });

  it('matches license from tags and parameter bucket range', () => {
    const models = [
      createModel({
        repo_id: 'org/model-7b',
        license: undefined,
        tags: ['gguf', 'license:apache-2.0'],
        parameter_count: '7B',
      }),
      createModel({
        repo_id: 'org/model-13b',
        license: 'mit',
        tags: ['gguf'],
        parameter_count: '13B',
      }),
    ];

    const filters: RemoteSearchFilters = {
      architectures: [],
      format: 'gguf',
      quantization: 'any',
      tags: [],
      license: 'apache-2.0',
      pipelineTag: 'any',
      library: 'any',
      language: 'any',
      parameter: '3to9b',
      sizeBucket: 'any',
      minDownloads: 0,
      sortBy: 'downloads',
      sortOrder: 'desc',
      newThisWeek: false,
    };

    const filtered = applyRemoteFilters(models, filters);
    expect(filtered).toHaveLength(1);
    expect(filtered[0]?.repo_id).toBe('org/model-7b');
  });

  it('matches language from plain tag code', () => {
    const models = [
      createModel({
        repo_id: 'org/model-en',
        languages: [],
        tags: ['gguf', 'en'],
      }),
      createModel({
        repo_id: 'org/model-zh',
        languages: [],
        tags: ['gguf', 'zh'],
      }),
    ];

    const filters: RemoteSearchFilters = {
      architectures: [],
      format: 'gguf',
      quantization: 'any',
      tags: [],
      license: 'any',
      pipelineTag: 'any',
      library: 'any',
      language: 'en',
      parameter: 'any',
      sizeBucket: 'any',
      minDownloads: 0,
      sortBy: 'downloads',
      sortOrder: 'desc',
      newThisWeek: false,
    };

    const filtered = applyRemoteFilters(models, filters);
    expect(filtered).toHaveLength(1);
    expect(filtered[0]?.repo_id).toBe('org/model-en');
  });
});
