/**
 * Stream Listener
 *
 * Handles structured message events from the Tauri backend.
 * Backend handles <think> tag parsing, frontend only renders.
 */


import { get } from 'svelte/store';
import { chatHistory } from '$lib/stores/chat-history';
import type { ChatControllerCtx } from './types';
import type {
    ChatMessage,
    McpToolCallErrorEvent,
    McpToolCallFinishedEvent,
    McpToolCallStartedEvent,
    McpToolCallView,
    McpToolPermissionRequestEvent,
    RetrievalSource,
} from '$lib/chat/types';
import {
    clearMcpPendingPermission,
    setMcpPendingPermission,
} from '$lib/stores/mcp-tooling';

/** Structured message from backend */
interface StreamMessage {
    thinking: string;
    content: string;
}

interface RetrievalContextEvent {
    sources?: RetrievalSource[];
}

interface RetrievalWarningEvent {
    message?: string;
}

interface RetrievalUrlCandidatesEvent {
    urls?: string[];
}

interface ToolingLogEvent {
    category?: string;
    message?: string;
}

export function createStreamListener(ctx: ChatControllerCtx) {
    const state = ctx.state;
    const listeners: Array<() => void> = [];
    let rafId: number | null = null;
    let pendingThinking = '';
    let pendingContent = '';
    let pendingSources: RetrievalSource[] = [];
    let pendingWarnings: string[] = [];

    function updateLastAssistantMessage(
        updater: (message: ChatMessage) => ChatMessage | null,
    ): boolean {
        const messages = state.messages;
        const lastIndex = messages.length - 1;
        if (lastIndex < 0) return false;

        const last = messages[lastIndex];
        if (!last || last.role !== 'assistant') return false;

        const updated = updater(last);
        if (!updated) return false;

        const nextMessages = messages.slice();
        nextMessages[lastIndex] = updated;
        state.messages = nextMessages;
        return true;
    }

    function scheduleUpdate() {
        if (rafId !== null) return;
        rafId = requestAnimationFrame(() => {
            rafId = null;
            applyPendingUpdates();
        });
    }

    function applyPendingUpdates() {
        if (pendingThinking === '' && pendingContent === '') return;

        const thinkingChunk = pendingThinking;
        const contentChunk = pendingContent;
        let consumedThinking = false;
        let consumedContent = false;

        const changed = updateLastAssistantMessage((last) => {
            let next = last;

            if (thinkingChunk) {
                next = {
                    ...next,
                    thinking: `${next.thinking ?? ''}${thinkingChunk}`,
                    isThinking: true,
                };
                consumedThinking = true;
            }

            if (contentChunk) {
                next = {
                    ...next,
                    content: `${next.content}${contentChunk}`,
                    isThinking: false,
                };
                consumedContent = true;
            }

            if (!consumedThinking && !consumedContent) {
                return null;
            }
            return next;
        });

        if (changed) {
            if (consumedThinking) pendingThinking = '';
            if (consumedContent) pendingContent = '';
        }
    }

    function handleStreamMessage(msg: StreamMessage) {
        if (msg.thinking) {
            pendingThinking += msg.thinking;
        }
        if (msg.content) {
            pendingContent += msg.content;
        }
        scheduleUpdate();
    }

    function applyRetrievalContextToLastMessage() {
        updateLastAssistantMessage((last) => ({
            ...last,
            sources: [...pendingSources],
            retrievalWarnings: [...pendingWarnings],
        }));
    }

    function handleRetrievalContext(event: RetrievalContextEvent) {
        pendingSources = Array.isArray(event.sources) ? event.sources : [];
        applyRetrievalContextToLastMessage();
    }

    function handleRetrievalWarning(event: RetrievalWarningEvent) {
        const message = event.message?.trim();
        if (!message) return;
        pendingWarnings = [...pendingWarnings, message];
        applyRetrievalContextToLastMessage();
    }

    function handleRetrievalUrlCandidates(event: RetrievalUrlCandidatesEvent) {
        const urls = (event.urls ?? []).filter(Boolean);
        if (urls.length === 0) return;
        if (state.retrieval_urls.length === 0) {
            state.retrieval_urls = urls;
        }
        if (!state.retrieval_url_enabled) {
            state.retrieval_url_enabled = true;
        }
        const preview = urls.slice(0, 3).join(', ');
        const suffix = urls.length > 3 ? ` (+${urls.length - 3})` : '';
        pendingWarnings = [
            ...pendingWarnings,
            `URL candidates found: ${preview}${suffix}. Confirm them in composer before sending.`,
        ];
        applyRetrievalContextToLastMessage();
    }

    function handleToolingLog(event: ToolingLogEvent) {
        const message = event.message?.trim();
        if (!message) return;
        const category = event.category?.trim();
        if (category === 'MCP_DEBUG') {
            // MCP progress is streamed directly by the backend agent loop.
            // Ignore duplicated tooling_log payloads here.
            return;
        }
    }

    function handleMcpPermissionRequest(event: McpToolPermissionRequestEvent) {
        if (!event?.request_id) return;
        setMcpPendingPermission(event);
    }

    function upsertToolCallOnLastAssistant(
        event: McpToolCallStartedEvent | McpToolCallFinishedEvent | McpToolCallErrorEvent,
    ) {
        updateLastAssistantMessage((last) => {
            const existing = Array.isArray(last.mcpToolCalls) ? [...last.mcpToolCalls] : [];
            const idx = existing.findIndex((item) => item.call_id === event.call_id);
            const base: McpToolCallView = idx >= 0
                ? { ...existing[idx] }
                : {
                    call_id: event.call_id,
                    server_id: event.server_id,
                    tool_name: event.tool_name,
                    state: 'input-streaming',
                    input: null,
                };

            base.server_id = event.server_id;
            base.tool_name = event.tool_name;

            if ('result' in event) {
                base.state = 'output-available';
                base.output = event.result;
                base.errorText = undefined;
            } else if ('error' in event) {
                base.state = 'output-error';
                base.output = undefined;
                base.errorText = event.error ?? 'Tool call failed';
            } else {
                const startedEvent = event as McpToolCallStartedEvent;
                base.state = 'input-available';
                base.input = startedEvent.arguments ?? base.input ?? null;
                base.output = undefined;
                base.errorText = undefined;
            }

            if (idx >= 0) {
                existing[idx] = base;
            } else {
                existing.push(base);
            }

            return {
                ...last,
                mcpToolCalls: existing,
            };
        });
    }

    function handleMcpCallStarted(event: McpToolCallStartedEvent) {
        if (!event?.call_id) return;
        upsertToolCallOnLastAssistant(event);
    }

    function handleMcpCallFinished(event: McpToolCallFinishedEvent) {
        if (!event?.call_id) return;
        upsertToolCallOnLastAssistant(event);
    }

    function handleMcpCallError(event: McpToolCallErrorEvent) {
        if (!event?.call_id) return;
        upsertToolCallOnLastAssistant(event);
    }

    function initNewStream() {
        const last = state.messages[state.messages.length - 1];

        if (!last || last.role !== 'assistant' || last.content !== '') {
            state.messages = [
                ...state.messages,
                {
                    role: 'assistant',
                    content: '',
                    html: '',
                    thinking: '',
                    isThinking: false,
                    sources: [...pendingSources],
                    retrievalWarnings: [...pendingWarnings],
                    mcpToolCalls: [],
                },
            ];
        } else {
            updateLastAssistantMessage((message) => ({
                ...message,
                thinking: '',
                isThinking: false,
                mcpToolCalls: [],
                sources: pendingSources.length > 0 ? [...pendingSources] : message.sources,
                retrievalWarnings: pendingWarnings.length > 0 ? [...pendingWarnings] : message.retrievalWarnings,
            }));
        }

        if (rafId !== null) {
            cancelAnimationFrame(rafId);
            rafId = null;
        }
        pendingThinking = '';
        pendingContent = '';
    }

    async function finalizeStream() {
        if (rafId !== null) {
            cancelAnimationFrame(rafId);
            rafId = null;
        }

        // Apply any remaining pending updates
        applyPendingUpdates();

        if (state.messages.length > 0) {
            const last = state.messages[state.messages.length - 1];

            // Ensure thinking state is finalized
            if (last && last.role === 'assistant' && last.isThinking) {
                updateLastAssistantMessage((message) => ({
                    ...message,
                    isThinking: false,
                }));
            }

            // Persist the complete assistant message to SQLite
            const historyState = get(chatHistory);
            const latestMessage = state.messages[state.messages.length - 1];
            if (historyState.currentSessionId && latestMessage?.role === 'assistant') {
                await chatHistory.saveAssistantMessage(
                    historyState.currentSessionId,
                    latestMessage.content,
                    latestMessage.thinking,
                    latestMessage.sources ?? [],
                    latestMessage.retrievalWarnings ?? [],
                );
            }
        }

        pendingSources = [];
        pendingWarnings = [];
    }

    async function ensureListener() {
        if (listeners.length > 0) return;

        const { listen } = await import('@tauri-apps/api/event');

        // Helper to push and track listeners
        const add = async (event: string, handler: (e: any) => void) => {
            listeners.push(await listen(event, handler));
        };

        await add('message_start', () => initNewStream());
        await add('message', (event) => {
            if (event.payload) handleStreamMessage(event.payload);
        });
        await add('message_done', () => void finalizeStream());
        await add('retrieval_context', (event) => handleRetrievalContext(event.payload ?? {}));
        await add('retrieval_warning', (event) => handleRetrievalWarning(event.payload ?? {}));
        await add('retrieval_url_candidates', (event) => handleRetrievalUrlCandidates(event.payload ?? {}));
        await add('tooling_log', (event) => handleToolingLog(event.payload ?? {}));
        await add('mcp_tool_permission_request', (event) => handleMcpPermissionRequest(event.payload));
        await add('mcp_tool_call_started', (event) => handleMcpCallStarted(event.payload));
        await add('mcp_tool_call_finished', (event) => handleMcpCallFinished(event.payload));
        await add('mcp_tool_call_error', (event) => handleMcpCallError(event.payload));
    }

    function destroy() {
        for (const unlisten of listeners) {
            try {
                unlisten();
            } catch { /* ignore */ }
        }
        listeners.length = 0;

        clearMcpPendingPermission();
        if (rafId !== null) {
            cancelAnimationFrame(rafId);
            rafId = null;
        }
        pendingSources = [];
        pendingWarnings = [];
    }

    function reset() {
        if (rafId !== null) {
            cancelAnimationFrame(rafId);
            rafId = null;
        }
        pendingThinking = '';
        pendingContent = '';
        pendingSources = [];
        pendingWarnings = [];
    }

    return { ensureListener, destroy, reset };
}

