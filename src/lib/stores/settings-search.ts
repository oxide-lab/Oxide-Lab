import { writable } from 'svelte/store';
import type { SettingsSearchItem, SettingsSearchResult } from '$lib/types/settings-v2';
import { searchSettingsV2 } from '$lib/services/settings-v2';

const SEARCH_HISTORY_KEY = 'settings.search.history.v2';

type RegistryDef = {
  id: string;
  section: SettingsSearchItem['section'];
  title?: string;
  description?: string;
  keywords?: string[];
  devOnly?: boolean;
};

const REGISTRY_DEFS: RegistryDef[] = [
  { id: 'general.locale', section: 'general', title: 'Language' },
  { id: 'general.theme', section: 'general', title: 'Theme' },
  { id: 'general.expert_mode', section: 'general', title: 'Expert Mode' },
  { id: 'general.developer_mode', section: 'general', title: 'Developer Mode' },
  { id: 'general.search_history_enabled', section: 'general', title: 'Search History' },
  { id: 'models_storage.models_dir', section: 'models_storage', title: 'Models Directory' },
  { id: 'models_storage.cache_dir', section: 'models_storage', title: 'Cache Directory' },
  {
    id: 'models_storage.model_selector_search',
    section: 'models_storage',
    title: 'Model Search',
    description: 'Show search in model selector',
    keywords: ['model search', 'selector search'],
  },
  { id: 'performance.ctx_size', section: 'performance', title: 'Context Size' },
  {
    id: 'performance.hardware.gpu_offload',
    section: 'hardware',
    title: 'Hardware GPU Offload',
  },
  {
    id: 'performance.hardware.gpu_selection',
    section: 'hardware',
    title: 'Hardware GPU Selection',
  },
  {
    id: 'performance.hardware.cpu_threads',
    section: 'hardware',
    title: 'Hardware CPU Threads',
  },
  { id: 'performance.hardware.memory_mapping', section: 'hardware', title: 'Memory Mapping' },
  { id: 'performance.hardware.split_gpus', section: 'hardware', title: 'Split Across GPUs' },
  { id: 'performance.hardware.batch_size', section: 'hardware', title: 'Hardware Batch Size' },
  { id: 'performance.hardware.memory_mode', section: 'hardware', title: 'Hardware Memory Mode' },
  { id: 'chat_presets.default_preset', section: 'chat_presets', title: 'Default Preset' },
  { id: 'chat_presets.temperature', section: 'chat_presets', title: 'Temperature' },
  { id: 'privacy_data.export', section: 'privacy_data', title: 'Export Data' },
  {
    id: 'web_rag.url_fetch.enabled_by_default',
    section: 'web_rag',
    title: 'URL Fetch',
  },
  {
    id: 'web_rag.embeddings_provider.base_url',
    section: 'web_rag',
    title: 'Embeddings API URL',
  },
  { id: 'web_rag.local_rag.beta_enabled', section: 'web_rag', title: 'Local RAG' },
  { id: 'web_rag.mcp.enabled', section: 'web_rag', title: 'MCP Tools' },
  {
    id: 'web_rag.mcp.max_tool_rounds',
    section: 'web_rag',
    title: 'MCP Max Tool Rounds',
  },
  {
    id: 'developer.openai_server',
    section: 'developer',
    title: 'OpenAI Server',
    devOnly: true,
  },
  { id: 'developer.auth_required', section: 'developer', title: 'Auth Required', devOnly: true },
  { id: 'developer.cors', section: 'developer', title: 'CORS Mode', devOnly: true },
];

function idToTitle(id: string): string {
  const tail = id.split('.').at(-1) ?? id;
  return tail
    .split('_')
    .map((word) => (word ? `${word[0].toUpperCase()}${word.slice(1)}` : word))
    .join(' ');
}

const registry: SettingsSearchItem[] = REGISTRY_DEFS.map((item) => ({
  id: item.id,
  section: item.section,
  title: item.title ?? idToTitle(item.id),
  description: item.description ?? '',
  keywords: item.keywords ?? [],
  synonyms: [],
  devOnly: item.devOnly,
}));

function norm(value: string): string {
  return value.toLowerCase().trim();
}

const query = writable('');
const debouncedQuery = writable('');
const results = writable<SettingsSearchResult[]>([]);
const searchHistory = writable<string[]>(loadHistory());
let debounceHandle: ReturnType<typeof setTimeout> | null = null;
let searchToken = 0;

function loadHistory(): string[] {
  if (typeof window === 'undefined') return [];
  try {
    const raw = localStorage.getItem(SEARCH_HISTORY_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((entry): entry is string => typeof entry === 'string').slice(0, 8);
  } catch {
    return [];
  }
}

function persistHistory(items: string[]) {
  if (typeof window === 'undefined') return;
  try {
    localStorage.setItem(SEARCH_HISTORY_KEY, JSON.stringify(items));
  } catch {
    // ignore storage write errors
  }
}

function setQuery(next: string) {
  query.set(next);
  if (debounceHandle) {
    clearTimeout(debounceHandle);
  }
  debounceHandle = setTimeout(() => {
    const normalized = norm(next);
    debouncedQuery.set(normalized);
    void runSearch(normalized);
  }, 180);
}

function pushHistory(value: string, enabled: boolean) {
  const cleaned = norm(value);
  if (!enabled || cleaned.length < 2) return;
  searchHistory.update((prev) => {
    const next = [cleaned, ...prev.filter((item) => item !== cleaned)];
    const limited = next.slice(0, 8);
    persistHistory(limited);
    return limited;
  });
}

async function runSearch(normalizedQuery: string) {
  const token = ++searchToken;
  if (!normalizedQuery) {
    results.set([]);
    return;
  }

  try {
    const rows = await searchSettingsV2(normalizedQuery);
    if (token !== searchToken) return;
    results.set(rows);
  } catch (error) {
    if (token !== searchToken) return;
    console.warn('Settings search failed', error);
    results.set([]);
  }
}

export const settingsSearchStore = {
  registry,
  query: { subscribe: query.subscribe },
  debouncedQuery: { subscribe: debouncedQuery.subscribe },
  results,
  searchHistory: { subscribe: searchHistory.subscribe },
  setQuery,
  clear: () => {
    searchToken += 1;
    query.set('');
    debouncedQuery.set('');
    results.set([]);
  },
  pushHistory,
  clearHistory: () => {
    searchHistory.set([]);
    persistHistory([]);
  },
};
