import { SimSearch } from '$lib/utils/simsearch';
import type { RemoteGGUFFile, RemoteModelInfo } from '$lib/types/local-models';

export type RemoteResultView = 'grid' | 'list';
export type SizeBucket = 'any' | 'lt4gb' | '4to8gb' | 'gt8gb';

export interface RemoteSearchFilters {
  architectures: string[];
  format: 'gguf' | 'safetensors' | 'gptq' | 'awq' | 'any';
  quantization: string;
  tags: string[];
  license: string;
  pipelineTag: string;
  library: string;
  language: string;
  parameter: string;
  parameterMinB?: number | null;
  parameterMaxB?: number | null;
  sizeBucket: SizeBucket;
  minDownloads: number;
  sortBy: 'downloads' | 'likes' | 'updated' | 'file_size';
  sortOrder: 'asc' | 'desc';
  newThisWeek: boolean;
}

export interface ParameterBucket {
  value: string;
  label: string;
  minB?: number;
  maxB?: number;
}

export const PARAMETER_BUCKETS: ParameterBucket[] = [
  { value: 'lt1b', label: '<1B', maxB: 1 },
  { value: '1to3b', label: '1B-3B', minB: 1, maxB: 3 },
  { value: '3to9b', label: '3B-9B', minB: 3, maxB: 9 },
  { value: '9to12b', label: '9B-12B', minB: 9, maxB: 12 },
  { value: '12to27b', label: '12B-27B', minB: 12, maxB: 27 },
  { value: '27to81b', label: '27B-81B', minB: 27, maxB: 81 },
  { value: '81to243b', label: '81B-243B', minB: 81, maxB: 243 },
  { value: '243to500b', label: '243B-500B', minB: 243, maxB: 500 },
  { value: 'gt500b', label: '>500B', minB: 500 },
];

const KB = 1024;
const MB = 1024 * KB;
const GB = 1024 * MB;

const ARCH_ALIASES: Record<string, string> = {
  llama: 'llama',
  mistral: 'mistral',
  mixtral: 'mixtral',
  phi: 'phi',
  qwen: 'qwen',
  gemma: 'gemma',
  deepseek: 'deepseek',
};

function normalize(value: string): string {
  return value.trim().toLowerCase();
}

function normalizeTag(value: string): string {
  return value.replaceAll('_', '-').trim().toLowerCase();
}

function getModelPrimaryFile(model: RemoteModelInfo): RemoteGGUFFile | null {
  const files = Array.isArray(model?.gguf_files) ? model.gguf_files : [];
  if (!files.length) return null;
  const withSize = files.filter((file) => Number.isFinite(file.size) && file.size > 0);
  if (withSize.length === 0) return files[0] ?? null;
  return withSize.reduce((best, next) => (next.size < best.size ? next : best), withSize[0]);
}

function getModelPrimarySizeBytes(model: RemoteModelInfo): number {
  return getModelPrimaryFile(model)?.size ?? 0;
}

function getModelParameterLabel(model: RemoteModelInfo): string {
  const explicit = normalize(model.parameter_count ?? '');
  if (explicit) return explicit;
  return '';
}

function getModelLanguages(model: RemoteModelInfo): string[] {
  const set = new Set<string>();

  const parseLanguage = (raw: string): string | null => {
    const trimmed = raw.trim().toLowerCase();
    if (!trimmed) return null;

    const value = trimmed.startsWith('language:') ? trimmed.slice('language:'.length).trim() : trimmed;
    if (!/^[a-z-]+$/.test(value)) return null;
    if (/^[a-z]{2}$/.test(value)) return value;
    if (/^[a-z]{2}-[a-z]{2}$/.test(value)) return value;
    return null;
  };

  const languages = Array.isArray(model.languages) ? model.languages : [];
  for (const language of languages) {
    const parsed = parseLanguage(language);
    if (parsed) set.add(parsed);
  }

  const tags = Array.isArray(model.tags) ? model.tags : [];
  for (const tag of tags) {
    const parsed = parseLanguage(tag);
    if (parsed) set.add(parsed);
  }

  return Array.from(set);
}

function getModelLicenses(model: RemoteModelInfo): string[] {
  const set = new Set<string>();

  const license = normalize(model.license ?? '');
  if (license) set.add(license);

  const tags = Array.isArray(model.tags) ? model.tags : [];
  for (const tag of tags) {
    const normalizedTag = normalize(tag);
    if (!normalizedTag.startsWith('license:')) continue;
    const value = normalizedTag.slice('license:'.length).trim();
    if (value) set.add(value);
  }

  return Array.from(set);
}

function parseModelParameterBillions(model: RemoteModelInfo): number | null {
  const label = getModelParameterLabel(model);
  if (!label) return null;

  const match = label.match(/(\d+(?:\.\d+)?)\s*([bm])/i);
  if (!match) return null;

  const value = Number(match[1]);
  if (!Number.isFinite(value)) return null;
  const unit = match[2]?.toLowerCase();
  if (unit === 'm') return value / 1000;
  return value;
}

function getModelArchitectures(model: RemoteModelInfo): string[] {
  const items = new Set<string>();

  const architectures = Array.isArray(model?.architectures) ? model.architectures : [];
  for (const arch of architectures) {
    const normalized = normalize(arch);
    if (normalized) items.add(normalized);
  }

  const tags = Array.isArray(model?.tags) ? model.tags : [];
  const merged = `${model.repo_id} ${model.name} ${tags.join(' ')}`.toLowerCase();
  for (const [needle, architecture] of Object.entries(ARCH_ALIASES)) {
    if (merged.includes(needle)) {
      items.add(architecture);
    }
  }

  return Array.from(items);
}

function passesSizeBucket(model: RemoteModelInfo, bucket: SizeBucket): boolean {
  if (bucket === 'any') return true;
  const size = getModelPrimarySizeBytes(model);
  if (size <= 0) return false;
  if (bucket === 'lt4gb') return size <= 4 * GB;
  if (bucket === '4to8gb') return size > 4 * GB && size <= 8 * GB;
  return size > 8 * GB;
}

function extractTimestamp(iso?: string): number {
  if (!iso) return 0;
  const timestamp = Date.parse(iso);
  return Number.isFinite(timestamp) ? timestamp : 0;
}

function sortModels(
  models: RemoteModelInfo[],
  sortBy: RemoteSearchFilters['sortBy'],
  sortOrder: RemoteSearchFilters['sortOrder'],
): RemoteModelInfo[] {
  const sorted = [...models].sort((left, right) => {
    if (sortBy === 'downloads') return left.downloads - right.downloads;
    if (sortBy === 'likes') return left.likes - right.likes;
    if (sortBy === 'updated') return extractTimestamp(left.last_modified) - extractTimestamp(right.last_modified);
    return getModelPrimarySizeBytes(left) - getModelPrimarySizeBytes(right);
  });
  return sortOrder === 'asc' ? sorted : sorted.reverse();
}

export function applyRemoteFilters(
  models: RemoteModelInfo[],
  filters: RemoteSearchFilters,
): RemoteModelInfo[] {
  const architectureSet = new Set(filters.architectures.map(normalize).filter(Boolean));
  const tagSet = new Set(filters.tags.map(normalizeTag).filter(Boolean));
  const selectedQuant = normalize(filters.quantization);
  const selectedLicense = normalize(filters.license);
  const selectedPipelineTag = normalize(filters.pipelineTag);
  const selectedLanguage = normalize(filters.language);
  const minParameterB = typeof filters.parameterMinB === 'number' ? filters.parameterMinB : null;
  const maxParameterB = typeof filters.parameterMaxB === 'number' ? filters.parameterMaxB : null;

  const filtered = models.filter((model) => {
    const tags = Array.isArray(model?.tags) ? model.tags : [];
    const isStaticFirstPage = tags.some((tag) => normalizeTag(tag) === 'static:first-page');
    if (filters.format !== 'any' && filters.format !== 'gguf') return false;
    const files = Array.isArray(model?.gguf_files) ? model.gguf_files : [];
    if (!isStaticFirstPage && files.length === 0) return false;
    if (isStaticFirstPage && filters.sizeBucket !== 'any') return false;
    if (!isStaticFirstPage && !passesSizeBucket(model, filters.sizeBucket)) return false;
    if (model.downloads < filters.minDownloads) return false;

    if (architectureSet.size > 0) {
      const modelArchitectures = getModelArchitectures(model);
      if (!modelArchitectures.some((architecture) => architectureSet.has(architecture))) {
        return false;
      }
    }

    if (selectedQuant && selectedQuant !== 'any') {
      if (isStaticFirstPage) return false;
      const modelQuantizations = files
        .map((file) => normalize(file.quantization ?? ''))
        .filter(Boolean);
      if (!modelQuantizations.some((quantization) => quantization === selectedQuant)) {
        return false;
      }
    }

    if (selectedLicense && selectedLicense !== 'any') {
      const licenses = getModelLicenses(model);
      if (!licenses.some((license) => license === selectedLicense || license.includes(selectedLicense))) {
        return false;
      }
    }

    if (selectedPipelineTag && selectedPipelineTag !== 'any') {
      const pipelineTag = normalize(model.pipeline_tag ?? '');
      if (pipelineTag !== selectedPipelineTag) {
        return false;
      }
    }

    if (selectedLanguage && selectedLanguage !== 'any') {
      const modelLanguages = getModelLanguages(model);
      if (!modelLanguages.includes(selectedLanguage)) {
        return false;
      }
    }

    if (minParameterB !== null || maxParameterB !== null) {
      const parameterB = parseModelParameterBillions(model);
      if (parameterB === null) {
        return false;
      }
      if (minParameterB !== null && parameterB < minParameterB) return false;
      if (maxParameterB !== null && parameterB >= maxParameterB) return false;
    }

    if (tagSet.size > 0) {
      const modelTags = new Set(tags.map(normalizeTag));
      for (const requiredTag of tagSet) {
        if (!modelTags.has(requiredTag)) {
          return false;
        }
      }
    }

    // Note: newThisWeek is handled server-side via sort order (trending),
    // not as a client-side post-filter on paginated results.

    return true;
  });

  return sortModels(filtered, filters.sortBy, filters.sortOrder);
}

export function applyFuzzyFallback(
  models: RemoteModelInfo[],
  query: string,
  limit = 20,
): RemoteModelInfo[] {
  const normalized = normalize(query);
  if (!normalized) return models.slice(0, limit);
  if (models.length === 0) return [];

  const index = new SimSearch(
    models.map((model) => ({
      id: model.repo_id,
      text: [
        model.repo_id,
        model.name,
        model.author ?? '',
        model.description ?? '',
        (Array.isArray(model.languages) ? model.languages : []).join(' '),
        (Array.isArray(model?.tags) ? model.tags : []).join(' '),
        (Array.isArray(model?.quantizations) ? model.quantizations : []).join(' '),
      ].join(' '),
    })),
  );

  const ids = new Set(index.search(normalized, Math.max(limit, models.length)).map((item) => item.id));
  return models.filter((model) => ids.has(model.repo_id)).slice(0, limit);
}

export function updateSearchHistory(history: string[], query: string, maxItems = 10): string[] {
  const normalized = normalize(query);
  if (!normalized) return history.slice(0, maxItems);
  const next = [normalized, ...history.filter((item) => normalize(item) !== normalized)];
  return next.slice(0, maxItems);
}

export function extractRepoFromHfUrl(input: string): { repoId: string; filename?: string } | null {
  const value = input.trim();
  if (!value) return null;

  let url: URL;
  try {
    url = new URL(value);
  } catch {
    return null;
  }
  if (url.hostname !== 'huggingface.co') return null;

  const pathParts = url.pathname.split('/').filter(Boolean);
  if (pathParts.length < 2) return null;

  const repoId = `${pathParts[0]}/${pathParts[1]}`;
  if (pathParts.length <= 2) return { repoId };

  const modeIndex = pathParts.findIndex((part) => part === 'resolve' || part === 'blob');
  if (modeIndex >= 0 && pathParts.length > modeIndex + 2) {
    return {
      repoId,
      filename: decodeURIComponent(pathParts.slice(modeIndex + 2).join('/')),
    };
  }

  return { repoId };
}

export function estimateVramGb(fileSizeBytes: number, quantization?: string | null): number {
  const fileSizeGb = fileSizeBytes > 0 ? fileSizeBytes / GB : 0;
  if (!Number.isFinite(fileSizeGb) || fileSizeGb <= 0) return 0;

  const normalized = normalize(quantization ?? '');
  let multiplier = 1.2;
  if (normalized.startsWith('q2')) multiplier = 0.55;
  else if (normalized.startsWith('q3')) multiplier = 0.7;
  else if (normalized.startsWith('q4')) multiplier = 0.85;
  else if (normalized.startsWith('q5')) multiplier = 1;
  else if (normalized.startsWith('q6')) multiplier = 1.15;
  else if (normalized.startsWith('q8')) multiplier = 1.45;
  else if (normalized.includes('f16') || normalized.includes('bf16')) multiplier = 1.65;

  const estimate = Math.max(0.5, fileSizeGb * multiplier);
  return Math.round(estimate * 10) / 10;
}

export function getRelativeTimeLabel(isoDate?: string, now = new Date()): string {
  if (!isoDate) return 'Unknown';
  const date = new Date(isoDate);
  if (Number.isNaN(date.getTime())) return 'Unknown';

  const diffMs = Math.max(0, now.getTime() - date.getTime());
  const minute = 60_000;
  const hour = 60 * minute;
  const day = 24 * hour;
  const week = 7 * day;

  if (diffMs < minute) return 'just now';
  if (diffMs < hour) return `${Math.floor(diffMs / minute)} min ago`;
  if (diffMs < day) return `${Math.floor(diffMs / hour)} hours ago`;
  if (diffMs < week) return `${Math.floor(diffMs / day)} days ago`;
  return `${Math.floor(diffMs / week)} weeks ago`;
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '—';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  if (unitIndex === 0) {
    return `${Math.round(value)} ${units[unitIndex]}`;
  }
  return `${value.toFixed(1)} ${units[unitIndex]}`;
}
