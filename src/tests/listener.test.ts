import { describe, expect, it, vi } from 'vitest';
import { createStreamListener } from '$lib/chat/controller/listener';
import type { ChatControllerCtx } from '$lib/chat/controller/types';
import { getDefaultChatState } from '$lib/stores/chat';

const eventMocks = vi.hoisted(() => {
  const handlers = new Map<string, (event: { payload?: any }) => void>();
  return {
    handlers,
    listen: vi.fn(async (eventName: string, handler: (event: { payload?: any }) => void) => {
      handlers.set(eventName, handler);
      return () => handlers.delete(eventName);
    }),
  };
});

const mcpMocks = vi.hoisted(() => ({
  setMcpPendingPermission: vi.fn(),
  clearMcpPendingPermission: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: eventMocks.listen,
}));

vi.mock('$lib/stores/mcp-tooling', () => ({
  setMcpPendingPermission: mcpMocks.setMcpPendingPermission,
  clearMcpPendingPermission: mcpMocks.clearMcpPendingPermission,
}));

vi.mock('$lib/stores/chat-history', () => ({
  chatHistory: {
    subscribe: (run: (v: { currentSessionId: null }) => void) => {
      run({ currentSessionId: null });
      return () => {};
    },
    saveAssistantMessage: vi.fn(),
  },
}));

function makeCtx(): ChatControllerCtx {
  return {
    state: {
      ...getDefaultChatState(),
      isLoaded: true,
      supports_text: true,
      supports_image: false,
      supports_audio: false,
      supports_video: false,
    },
  };
}

describe('createStreamListener', () => {
  it('streams assistant message chunks and handles MCP permission event', async () => {
    const ctx = makeCtx();
    const listener = createStreamListener(ctx);
    await listener.ensureListener();

    eventMocks.handlers.get('message_start')?.({ payload: undefined });
    const afterStartRef = ctx.state.messages;
    eventMocks.handlers.get('message')?.({ payload: { thinking: 't', content: 'hello' } });

    await new Promise((resolve) => requestAnimationFrame(() => resolve(undefined)));
    expect(ctx.state.messages).not.toBe(afterStartRef);
    expect(ctx.state.messages.length).toBe(1);
    expect(ctx.state.messages[0].role).toBe('assistant');
    expect(ctx.state.messages[0].content).toBe('hello');
    expect(ctx.state.messages[0].thinking).toContain('t');
    expect(eventMocks.handlers.has('mcp_tool_permission_request')).toBe(true);

    eventMocks.handlers.get('mcp_tool_permission_request')?.({
      payload: { request_id: 'req-1' },
    });
    expect(mcpMocks.setMcpPendingPermission).toHaveBeenCalled();

    listener.destroy();
  });
});
