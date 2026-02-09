/**
 * Stream Listener
 *
 * Handles structured message events from the Tauri backend.
 * Backend handles <think> tag parsing, frontend only renders.
 */

import { finalizeStreaming } from '$lib/chat/stream_render';
import { get } from 'svelte/store';
import { chatHistory } from '$lib/stores/chat-history';
import type { ChatControllerCtx } from './types';
import type {
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
    let unlistenStart: (() => void) | null = null;
    let unlistenMessage: (() => void) | null = null;
    let unlistenDone: (() => void) | null = null;
    let unlistenRetrievalContext: (() => void) | null = null;
    let unlistenRetrievalWarning: (() => void) | null = null;
    let unlistenRetrievalUrlCandidates: (() => void) | null = null;
    let unlistenToolingLog: (() => void) | null = null;
    let unlistenMcpPermission: (() => void) | null = null;
    let unlistenMcpCallStarted: (() => void) | null = null;
    let unlistenMcpCallFinished: (() => void) | null = null;
    let unlistenMcpCallError: (() => void) | null = null;
    let rafId: number | null = null;
    let pendingThinking = '';
    let pendingContent = '';
    let pendingSources: RetrievalSource[] = [];
    let pendingWarnings: string[] = [];

    function scheduleUpdate() {
        if (rafId !== null) return;
        rafId = requestAnimationFrame(() => {
            rafId = null;
            applyPendingUpdates();
        });
    }

    function applyPendingUpdates() {
        if (pendingThinking === '' && pendingContent === '') return;

        const msgs = ctx.messages;
        const last = msgs[msgs.length - 1];
        if (last && last.role === 'assistant') {
            let hasUpdates = false;

            if (pendingThinking) {
                if (!last.thinking) last.thinking = '';
                last.thinking += pendingThinking;
                last.isThinking = true;
                pendingThinking = '';
                hasUpdates = true;
            }

            if (pendingContent) {
                last.content += pendingContent;
                // When content arrives, thinking phase is done
                if (last.isThinking) {
                    last.isThinking = false;
                }
                pendingContent = '';
                hasUpdates = true;
            }

            if (hasUpdates) {
                ctx.messages = msgs;
            }
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
        const msgs = ctx.messages;
        const last = msgs[msgs.length - 1];
        if (!last || last.role !== 'assistant') {
            return;
        }
        last.sources = [...pendingSources];
        last.retrievalWarnings = [...pendingWarnings];
        ctx.messages = msgs;
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
        if (ctx.retrieval_urls.length === 0) {
            ctx.retrieval_urls = urls;
        }
        if (!ctx.retrieval_url_enabled) {
            ctx.retrieval_url_enabled = true;
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
        const msgs = ctx.messages;
        const lastIndex = msgs.length - 1;
        if (lastIndex < 0) return;
        const last = msgs[lastIndex];
        if (!last || last.role !== 'assistant') return;

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

        last.mcpToolCalls = existing;
        ctx.messages = msgs;
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
        const msgs = ctx.messages;
        const last = msgs[msgs.length - 1];

        if (!last || last.role !== 'assistant' || last.content !== '') {
            msgs.push({
                role: 'assistant',
                content: '',
                html: '',
                thinking: '',
                isThinking: false,
                sources: [...pendingSources],
                retrievalWarnings: [...pendingWarnings],
                mcpToolCalls: [],
            });
            ctx.messages = msgs;
        } else {
            last.thinking = '';
            last.isThinking = false;
            last.mcpToolCalls = [];
            if (pendingSources.length > 0) {
                last.sources = [...pendingSources];
            }
            if (pendingWarnings.length > 0) {
                last.retrievalWarnings = [...pendingWarnings];
            }
            ctx.messages = msgs;
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

        const msgs = ctx.messages;
        if (msgs.length > 0) {
            const idx = msgs.length - 1;
            const last = msgs[idx];

            // Ensure thinking state is finalized
            if (last && last.isThinking) {
                last.isThinking = false;
                ctx.messages = msgs;
            }

            finalizeStreaming(idx);

            // Persist the complete assistant message to SQLite
            const state = get(chatHistory);
            if (state.currentSessionId) {
                if (last && last.role === 'assistant') {
                    await chatHistory.saveAssistantMessage(
                        state.currentSessionId,
                        last.content,
                        last.thinking,
                        last.sources ?? [],
                        last.retrievalWarnings ?? [],
                    );
                }
            }
        }

        pendingSources = [];
        pendingWarnings = [];
    }

    async function ensureListener() {
        if (unlistenMessage) return;

        const { listen } = await import('@tauri-apps/api/event');

        // Stream start signal - creates assistant message
        unlistenStart = await listen('message_start', () => {
            initNewStream();
        });

        // Primary: structured message events from backend
        unlistenMessage = await listen<StreamMessage>('message', (event) => {
            const msg = event.payload;
            if (msg) {
                handleStreamMessage(msg);
            }
        });

        // Stream completion signal
        unlistenDone = await listen('message_done', () => {
            void finalizeStream();
        });

        unlistenRetrievalContext = await listen<RetrievalContextEvent>('retrieval_context', (event) => {
            handleRetrievalContext(event.payload ?? {});
        });

        unlistenRetrievalWarning = await listen<RetrievalWarningEvent>('retrieval_warning', (event) => {
            handleRetrievalWarning(event.payload ?? {});
        });
        unlistenRetrievalUrlCandidates = await listen<RetrievalUrlCandidatesEvent>(
            'retrieval_url_candidates',
            (event) => {
                handleRetrievalUrlCandidates(event.payload ?? {});
            },
        );
        unlistenToolingLog = await listen<ToolingLogEvent>('tooling_log', (event) => {
            handleToolingLog(event.payload ?? {});
        });
        unlistenMcpPermission = await listen<McpToolPermissionRequestEvent>(
            'mcp_tool_permission_request',
            (event) => {
                handleMcpPermissionRequest(event.payload);
            },
        );
        unlistenMcpCallStarted = await listen<McpToolCallStartedEvent>('mcp_tool_call_started', (event) => {
            handleMcpCallStarted(event.payload);
        });
        unlistenMcpCallFinished = await listen<McpToolCallFinishedEvent>(
            'mcp_tool_call_finished',
            (event) => {
                handleMcpCallFinished(event.payload);
            },
        );
        unlistenMcpCallError = await listen<McpToolCallErrorEvent>('mcp_tool_call_error', (event) => {
            handleMcpCallError(event.payload);
        });
    }

    function destroy() {
        if (unlistenStart) {
            try {
                unlistenStart();
            } catch {
                /* ignore */
            }
            unlistenStart = null;
        }
        if (unlistenMessage) {
            try {
                unlistenMessage();
            } catch {
                /* ignore */
            }
            unlistenMessage = null;
        }
        if (unlistenDone) {
            try {
                unlistenDone();
            } catch {
                /* ignore */
            }
            unlistenDone = null;
        }
        if (unlistenRetrievalContext) {
            try {
                unlistenRetrievalContext();
            } catch {
                /* ignore */
            }
            unlistenRetrievalContext = null;
        }
        if (unlistenRetrievalWarning) {
            try {
                unlistenRetrievalWarning();
            } catch {
                /* ignore */
            }
            unlistenRetrievalWarning = null;
        }
        if (unlistenRetrievalUrlCandidates) {
            try {
                unlistenRetrievalUrlCandidates();
            } catch {
                /* ignore */
            }
            unlistenRetrievalUrlCandidates = null;
        }
        if (unlistenToolingLog) {
            try {
                unlistenToolingLog();
            } catch {
                /* ignore */
            }
            unlistenToolingLog = null;
        }
        if (unlistenMcpPermission) {
            try {
                unlistenMcpPermission();
            } catch {
                /* ignore */
            }
            unlistenMcpPermission = null;
        }
        if (unlistenMcpCallStarted) {
            try {
                unlistenMcpCallStarted();
            } catch {
                /* ignore */
            }
            unlistenMcpCallStarted = null;
        }
        if (unlistenMcpCallFinished) {
            try {
                unlistenMcpCallFinished();
            } catch {
                /* ignore */
            }
            unlistenMcpCallFinished = null;
        }
        if (unlistenMcpCallError) {
            try {
                unlistenMcpCallError();
            } catch {
                /* ignore */
            }
            unlistenMcpCallError = null;
        }
        clearMcpPendingPermission();
        if (rafId !== null) {
            cancelAnimationFrame(rafId);
            rafId = null;
        }
        pendingSources = [];
        pendingWarnings = [];
    }

    return { ensureListener, destroy };
}
