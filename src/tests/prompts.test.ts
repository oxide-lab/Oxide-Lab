import { describe, expect, it, vi } from 'vitest';
import { buildPromptWithChatTemplate } from '$lib/chat/prompts';

const tauriMocks = vi.hoisted(() => {
  const invoke = vi.fn(async (command: string) => {
    if (command === 'get_chat_template') return null;
    throw new Error(`Unexpected command ${command}`);
  });
  return { invoke };
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: tauriMocks.invoke,
}));

describe('buildPromptWithChatTemplate', () => {
  it('uses model-agnostic fallback when backend template is missing', async () => {
    const prompt = await buildPromptWithChatTemplate([
      { role: 'user', content: 'Hello' },
      { role: 'assistant', content: 'Hi there' },
    ]);

    expect(prompt).toContain('User: Hello');
    expect(prompt).toContain('Assistant: Hi there');
    expect(prompt).toContain('Assistant: ');
    expect(prompt).not.toContain('<|im_start|>');
  });

  it('strips /no_think control command and appends empty think block', async () => {
    const prompt = await buildPromptWithChatTemplate([{ role: 'user', content: '/no_think Explain' }]);

    expect(prompt).toContain('User: Explain');
    expect(prompt).toContain('<think>');
    expect(prompt).toContain('</think>');
  });
});
