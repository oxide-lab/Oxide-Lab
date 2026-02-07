import { writable } from 'svelte/store';
import type { SettingsSearchItem } from '$lib/types/settings-v2';
import type { SettingsSearchResult } from '$lib/types/settings-v2';
import { searchSettingsV2 } from '$lib/services/settings-v2';

const SEARCH_HISTORY_KEY = 'settings.search.history.v2';

const registry: SettingsSearchItem[] = [
  {
    id: 'general.locale',
    section: 'general',
    title: 'Language',
    description: 'Application locale',
    keywords: ['locale', 'language', 'i18n'],
    synonyms: ['idioma', 'язык', 'lingua', 'lang'],
  },
  {
    id: 'general.theme',
    section: 'general',
    title: 'Theme',
    description: 'Light, dark, or system mode',
    keywords: ['theme', 'appearance', 'dark', 'light'],
    synonyms: ['ui style', 'tema', 'тема'],
  },
  {
    id: 'general.expert_mode',
    section: 'general',
    title: 'Expert Mode',
    description: 'Show advanced settings',
    keywords: ['expert', 'advanced', 'basic'],
    synonyms: ['power mode', 'продвинутый', 'avancado'],
  },
  {
    id: 'general.developer_mode',
    section: 'general',
    title: 'Developer Mode',
    description: 'Reveal developer settings',
    keywords: ['developer', 'dev', 'network'],
    synonyms: ['unsafe', 'advanced dev', 'разработчик', 'desenvolvedor'],
  },
  {
    id: 'models_storage.models_dir',
    section: 'models_storage',
    title: 'Models Directory',
    description: 'Folder where models are stored',
    keywords: ['models', 'path', 'folder', 'directory'],
    synonyms: ['model path', 'путь к моделям', 'pasta modelos'],
  },
  {
    id: 'models_storage.cache_dir',
    section: 'models_storage',
    title: 'Cache Directory',
    description: 'Folder where caches are stored',
    keywords: ['cache', 'path', 'folder'],
    synonyms: ['kv cache path', 'кэш', 'cache pasta'],
  },
  {
    id: 'performance.n_gpu_layers',
    section: 'performance',
    title: 'GPU Layers',
    description: 'Number of offloaded layers',
    keywords: ['gpu', 'layers', 'offload', 'vram'],
    synonyms: ['ngl', 'слои gpu', 'camadas gpu'],
  },
  {
    id: 'performance.ctx_size',
    section: 'performance',
    title: 'Context Size',
    description: 'Maximum context window',
    keywords: ['context', 'ctx', 'tokens'],
    synonyms: ['window size', 'контекст', 'contexto'],
  },
  {
    id: 'performance.batch_size',
    section: 'performance',
    title: 'Batch Size',
    description: 'Batch token size',
    keywords: ['batch', 'ubatch', 'tokens'],
    synonyms: ['throughput', 'пакет', 'lote'],
  },
  {
    id: 'chat_presets.default_preset',
    section: 'chat_presets',
    title: 'Default Preset',
    description: 'Preset applied to new chats',
    keywords: ['preset', 'default', 'profile'],
    synonyms: ['template', 'mode', 'профиль', 'predefinicao'],
  },
  {
    id: 'chat_presets.temperature',
    section: 'chat_presets',
    title: 'Temperature',
    description: 'Sampling creativity level',
    keywords: ['temperature', 'sampling', 'randomness'],
    synonyms: ['creativity', 'температура', 'criatividade'],
  },
  {
    id: 'models_storage.model_selector_search',
    section: 'models_storage',
    title: 'Quantization',
    description: 'Model quantization metadata',
    keywords: ['quantization', 'gguf', 'bits'],
    synonyms: ['4bit', '8bit', 'квантизация', 'quantizacao'],
  },
  {
    id: 'general.search_history_enabled',
    section: 'general',
    title: 'Search History',
    description: 'Save search query history',
    keywords: ['search', 'history', 'privacy'],
    synonyms: ['query log', 'история поиска', 'historico busca'],
  },
  {
    id: 'privacy_data.export',
    section: 'privacy_data',
    title: 'Export Data',
    description: 'Export local user data',
    keywords: ['export', 'backup', 'data'],
    synonyms: ['archive', 'экспорт', 'exportar'],
  },
  {
    id: 'developer.openai_server',
    section: 'developer',
    title: 'OpenAI Server',
    description: 'Configure local OpenAI-compatible server',
    keywords: ['api server', 'openai', 'endpoint', 'port', 'host'],
    synonyms: ['localhost', 'lan', 'сервер', 'servidor'],
    devOnly: true,
  },
  {
    id: 'developer.auth_required',
    section: 'developer',
    title: 'Auth Required',
    description: 'Require API key for requests',
    keywords: ['auth', 'api key', 'security'],
    synonyms: ['token', 'авторизация', 'autenticacao'],
    devOnly: true,
  },
  {
    id: 'developer.cors',
    section: 'developer',
    title: 'CORS Mode',
    description: 'Cross-origin request policy',
    keywords: ['cors', 'origin', 'allowlist', 'any'],
    synonyms: ['cross origin', 'источник', 'origem'],
    devOnly: true,
  },
];

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
