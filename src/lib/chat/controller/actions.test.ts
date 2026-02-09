import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { chatState, getDefaultChatState } from '$lib/stores/chat';

const askMock = vi.fn(async (_message?: string, _options?: unknown) => true);

const invokeMock = vi.fn(async (command: string, _payload?: unknown) => {
  if (command === 'get_device_info') {
    return {
      cuda_build: false,
      cuda_available: false,
      current: 'CPU',
      avx: true,
      neon: false,
      simd128: true,
      f16c: true,
    };
  }

  if (command === 'unload_model') {
    return null;
  }

  if (command === 'extract_url_candidates') {
    return ['https://example.com/a', 'https://example.com/b'];
  }

  if (command === 'generate_stream') {
    return null;
  }

  return null;
});

vi.mock('$lib/stores/chat-history', () => ({
  chatHistory: {
    subscribe(run: (value: { currentSessionId: string | null }) => void) {
      run({ currentSessionId: null });
      return () => {};
    },
    createSession: vi.fn(),
    addMessage: vi.fn(),
    saveAssistantMessage: vi.fn(),
    truncateMessages: vi.fn(),
    updateLastMessage: vi.fn(),
  },
}));

vi.mock('$lib/chat/prompts', () => ({
  buildPromptWithChatTemplate: vi.fn(async () => 'prompt'),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  ask: (message: string, options?: unknown) => askMock(message, options),
  message: vi.fn(async () => undefined),
}));

function makeCtx() {
  return {
    modelPath: 'C:/models/test.gguf',
    format: 'gguf' as const,
    repoId: '',
    revision: '',
    hubGgufFilename: '',
    prompt: '',
    messages: [{ role: 'assistant', content: 'hello' }],
    busy: false,
    isLoaded: true,
    errorText: '',
    isLoadingModel: false,
    loadingProgress: 0,
    loadingStage: '',
    isCancelling: false,
    isUnloadingModel: false,
    unloadingProgress: 0,
    temperature: 0.8,
    temperature_enabled: false,
    top_k_enabled: false,
    top_k_value: 40,
    top_p_enabled: false,
    top_p_value: 0.9,
    min_p_enabled: false,
    min_p_value: 0.05,
    repeat_penalty_enabled: false,
    repeat_penalty_value: 1.1,
    ctx_limit_value: 4096,
    use_custom_params: false,
    use_gpu: false,
    cuda_available: false,
    cuda_build: false,
    current_device: 'CPU',
    avx: true,
    neon: false,
    simd128: true,
    f16c: true,
    supports_text: true,
    supports_image: false,
    supports_audio: false,
    supports_video: false,
    split_prompt: false,
    verbose_prompt: false,
    tracing: false,
    retrieval_url_enabled: false,
    retrieval_urls: [],
    retrieval_local_enabled: false,
    mcp_enabled: false,
  };
}

let createActions: typeof import('$lib/chat/controller/actions').createActions;

describe('chat controller unload', () => {
  beforeEach(async () => {
    if (!createActions) {
      ({ createActions } = await import('$lib/chat/controller/actions'));
    }
    invokeMock.mockClear();
    askMock.mockReset();
    askMock.mockResolvedValue(true);
    (window as any).__TAURI_INTERNALS__ = {
      invoke: (command: string, payload?: unknown) => invokeMock(command, payload),
      transformCallback: vi.fn(() => 1),
      unregisterCallback: vi.fn(),
      convertFileSrc: vi.fn(),
    };
    chatState.set({
      ...getDefaultChatState(),
      modelPath: 'C:/models/test.gguf',
      isLoaded: true,
      busy: false,
    });
  });

  it('resets model selection and syncs chat store on unload', async () => {
    const ctx = makeCtx();
    const actions = createActions(ctx as any);
    expect(ctx.isLoaded).toBe(true);
    expect(ctx.busy).toBe(false);
    ctx.isLoaded = true;
    ctx.busy = false;

    await actions.unloadGGUF();

    const snapshot = get(chatState);
    expect(ctx.isLoaded).toBe(false);
    expect(ctx.modelPath).toBe('');
    expect(snapshot.isLoaded).toBe(false);
    expect(snapshot.modelPath).toBe('');
    expect(snapshot.busy).toBe(false);
  });

  it('extracts and confirms URL candidates before sending when URL retrieval is enabled', async () => {
    const ctx = makeCtx();
    ctx.retrieval_url_enabled = true;
    ctx.prompt = 'прочитай https://example.com/a и https://example.com/b';
    const actions = createActions(ctx as any);

    await actions.handleSend();

    expect(askMock).toHaveBeenCalledTimes(1);
    expect(ctx.retrieval_urls).toEqual(['https://example.com/a', 'https://example.com/b']);
    const generateCall = invokeMock.mock.calls.find((entry) => entry[0] === 'generate_stream');
    expect(generateCall).toBeTruthy();
    const payload = generateCall?.[1] as any;
    expect(payload?.req?.retrieval?.web?.urls).toEqual([
      'https://example.com/a',
      'https://example.com/b',
    ]);
  });

  it('does not send generation when URL candidates were denied by user', async () => {
    askMock.mockResolvedValue(false);
    const ctx = makeCtx();
    ctx.retrieval_url_enabled = true;
    ctx.prompt = 'check https://example.com/a';
    const actions = createActions(ctx as any);

    await actions.handleSend();

    expect(ctx.retrieval_urls).toEqual(['https://example.com/a', 'https://example.com/b']);
    expect(invokeMock.mock.calls.some((entry) => entry[0] === 'generate_stream')).toBe(false);
  });
});
