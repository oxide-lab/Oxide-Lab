/**
 * Model Icons Utility
 * 
 * Maps model families to icon names from @lobehub/icons-static-svg
 */

const FAMILY_ICON_MAP: Record<string, string> = {
  // Major providers
  'llama': 'meta',
  'llama2': 'meta',
  'llama3': 'meta',
  'meta-llama': 'meta',
  'codellama': 'meta',

  // Mistral family
  'mistral': 'mistral',
  'mixtral': 'mistral',

  // Qwen family
  'qwen': 'qwen',
  'qwen2': 'qwen',
  'qwen2.5': 'qwen',

  // Google
  'google': 'google',
  'gemini': 'gemini',
  'gemma': 'gemini',
  'gemma2': 'gemini',

  // Microsoft
  'microsoft': 'microsoft',
  'phi': 'microsoft',
  'phi2': 'microsoft',
  'phi3': 'microsoft',

  // DeepSeek
  'deepseek': 'deepseek',
  'deepseek-coder': 'deepseek',
  'deepseek-v2': 'deepseek',

  // Anthropic
  'claude': 'anthropic',

  // OpenAI
  'gpt': 'openai',
  'chatgpt': 'openai',

  // Stability
  'stable': 'stability',
  'stablelm': 'stability',

  // Yi
  'yi': 'yi',

  // Cohere
  'cohere': 'cohere',
  'command': 'cohere',
  'command-r': 'cohere',

  // Yandex
  'yandex': 'yandex',

  // Others
  'falcon': 'falcon',
  'vicuna': 'vicuna',
  'wizardlm': 'wizardlm',
  'starcoder': 'huggingface',
  'codestral': 'mistral',
};

// Known icon names from lobe-hub that should pass through directly
// These are icon names that may be passed from getModelIconFamily in RemoteModelsPanel
const DIRECT_ICON_NAMES = new Set([
  'qwen', 'alibaba', 'google', 'microsoft', 'mistral', 'cohere', 'yandex',
  'deepseek', 'meta', 'tii', 'claude', 'openai', 'stability', 'nvidia', 'ibm',
  'baichuan', 'chatglm', 'baidu', 'bytedance', 'zai', 'yi', 'tencent', 'huawei',
  'minimax', 'internlm', 'groq', 'together', 'fireworks', 'replicate', 'anyscale',
  'perplexity', 'nousresearch', 'rwkv', 'cerebras', 'sambanova', 'huggingface',
  'gemini', 'anthropic', 'falcon', 'apple', 'ai21', 'ai2', 'ai360', 'arcee',
  'bfl', 'snowflake', 'upstage', 'xai', 'intel', 'zhipu', 'moonshot', 'sensenova',
  'cloudflare', 'lmstudio', 'jina', 'vllm', 'elevenlabs', 'stepfun', 'infinigence',
  'leptonai', 'deepinfra', 'novita', 'ollama', 'openrouter', 'liquid', 'lightricks',
  'gradient', 'openchat'
]);

/**
 * Get the icon name for a model family
 * 
 * If family is already a valid icon name, it's returned as-is.
 * Otherwise, tries to match in FAMILY_ICON_MAP for aliases.
 */
export function getModelIconName(family: string | null | undefined): string {
  if (!family) return 'huggingface';

  const normalized = family.toLowerCase().trim();

  // If already a valid icon name, return it directly
  if (DIRECT_ICON_NAMES.has(normalized)) {
    return normalized;
  }

  // Direct match in alias map
  if (FAMILY_ICON_MAP[normalized]) {
    return FAMILY_ICON_MAP[normalized];
  }

  // Partial match
  for (const [key, icon] of Object.entries(FAMILY_ICON_MAP)) {
    if (normalized.includes(key) || key.includes(normalized)) {
      return icon;
    }
  }

  // Default
  return 'huggingface';
}
