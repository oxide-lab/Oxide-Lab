import { describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';

const repositoryMocks = vi.hoisted(() => ({
  loadSessions: vi.fn(async () => []),
  createSession: vi.fn(async () => {}),
  insertMessage: vi.fn(async () => {}),
  updateSessionTitle: vi.fn(async () => {}),
  touchSession: vi.fn(async () => {}),
  loadSessionMessages: vi.fn(async () => []),
  updateLastMessage: vi.fn(async () => {}),
  truncateMessages: vi.fn(async () => {}),
  deleteSession: vi.fn(async () => []),
  clearAll: vi.fn(async () => []),
  cleanupAttachmentPaths: vi.fn(async () => {}),
}));

vi.mock('$lib/stores/chat-history-repository', () => ({
  chatHistoryRepository: repositoryMocks,
  dbMessageToChatMessage: (msg: any) => msg,
}));

describe('chatHistory.importSession', () => {
  it('imports valid exported session payload into store and repository', async () => {
    const { chatHistory } = await import('$lib/stores/chat-history');

    const payload = JSON.stringify({
      id: 'legacy-id',
      title: 'Imported session',
      modelPath: 'C:/models/demo.gguf',
      repoId: '',
      createdAt: 1700000000000,
      updatedAt: 1700000001000,
      messages: [
        { role: 'user', content: 'hello' },
        { role: 'assistant', content: 'world', thinking: '...' },
      ],
    });

    const ok = await chatHistory.importSession(payload);
    expect(ok).toBe(true);

    expect(repositoryMocks.createSession).toHaveBeenCalledTimes(1);
    expect(repositoryMocks.insertMessage).toHaveBeenCalledTimes(2);

    const state = get(chatHistory);
    expect(state.sessions.length).toBeGreaterThan(0);
    expect(state.sessions[0].title).toBe('Imported session');
    expect(state.sessions[0].messages.length).toBe(2);
  });
});
