import type { DownloadJob, RemoteGGUFFile, RemoteModelInfo } from '$lib/types/local-models';

const VERIFIED_AUTHORS = new Set([
  'meta-llama',
  'mistralai',
  'microsoft',
  'google',
  'qwen',
  'deepseek-ai',
]);

function normalize(value: string): string {
  return value.trim().toLowerCase();
}

export function getPrimaryFile(model: RemoteModelInfo): RemoteGGUFFile | null {
  const files = Array.isArray(model?.gguf_files) ? model.gguf_files : [];
  if (!files.length) return null;

  const withSize = files.filter((file) => Number.isFinite(file.size) && file.size > 0);
  if (!withSize.length) return files[0] ?? null;

  return withSize.reduce((best, next) => (next.size < best.size ? next : best), withSize[0]);
}

export function extractParameterLabel(model: RemoteModelInfo): string {
  if (model.parameter_count?.trim()) return model.parameter_count;
  return '—';
}

export function isVerifiedAuthor(model: RemoteModelInfo): boolean {
  const author = normalize(model.author ?? model.repo_id.split('/')[0] ?? '');
  return VERIFIED_AUTHORS.has(author);
}

export function getModelJob(jobs: DownloadJob[], repoId: string): DownloadJob | null {
  return jobs.find((job) => job.repo_id === repoId) ?? null;
}

export function getFileJob(
  jobs: DownloadJob[],
  repoId: string,
  filename: string,
): DownloadJob | null {
  const normalizedFilename = normalize(filename);
  return (
    jobs.find(
      (job) =>
        job.repo_id === repoId && normalize(job.filename) === normalizedFilename,
    ) ?? null
  );
}

export function getDownloadProgress(job: DownloadJob | null): number {
  if (!job?.total_bytes || job.total_bytes <= 0) return 0;
  return Math.max(0, Math.min(100, (job.downloaded_bytes / job.total_bytes) * 100));
}

export function formatSpeedLabel(speedBytesPerSec?: number): string {
  if (!speedBytesPerSec || speedBytesPerSec <= 0) return '—';

  const units = ['B', 'KB', 'MB', 'GB'];
  let value = speedBytesPerSec;
  let index = 0;
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024;
    index += 1;
  }

  return `${value.toFixed(index === 0 ? 0 : 1)} ${units[index]}/s`;
}

export function formatEtaLabel(seconds?: number): string {
  if (!seconds || seconds <= 0) return '—';
  if (seconds < 60) return `${Math.round(seconds)}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  return `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`;
}
