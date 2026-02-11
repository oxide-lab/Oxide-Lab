import { describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import {
  ensureDownloadManager,
  refreshDownloadSnapshot,
  activeDownloads,
  downloadManagerError,
} from '$lib/stores/download-manager';

const serviceMocks = vi.hoisted(() => ({
  getDownloadSnapshot: vi.fn(async () => ({
    active: [
      {
        id: '1',
        repo_id: 'a/b',
        filename: 'model.gguf',
        download_url: 'https://example.com/a',
        destination_dir: '/tmp',
        downloaded_bytes: 1,
        status: 'completed',
        updated_at: '2026-01-01T00:00:00Z',
      },
      {
        id: '2',
        repo_id: 'a/b',
        filename: 'model2.gguf',
        download_url: 'https://example.com/b',
        destination_dir: '/tmp',
        downloaded_bytes: 2,
        status: 'downloading',
        updated_at: '2026-01-02T00:00:00Z',
      },
    ],
    history: [],
  })),
  onDownloadSnapshotUpdate: vi.fn(async () => () => {}),
}));

vi.mock('$lib/services/local-models', () => ({
  LocalModelsService: serviceMocks,
}));

describe('download-manager store', () => {
  it('filters active downloads and clears error on successful refresh', async () => {
    await ensureDownloadManager();
    await refreshDownloadSnapshot();

    const active = get(activeDownloads) as Array<{ id: string }>;
    expect(active.map((item) => item.id)).toEqual(['2']);
    expect(get(downloadManagerError)).toBeNull();
  });
});
