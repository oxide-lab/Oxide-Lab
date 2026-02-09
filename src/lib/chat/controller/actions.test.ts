import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { chatState, getDefaultChatState } from '$lib/stores/chat';

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
    retrieval_web_mode: 'lite',
    retrieval_local_enabled: false,
  };
}

let createActions: typeof import('$lib/chat/controller/actions').createActions;

describe('chat controller unload', () => {
  beforeEach(async () => {
    if (!createActions) {
      ({ createActions } = await import('$lib/chat/controller/actions'));
    }
    invokeMock.mockClear();
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
});
