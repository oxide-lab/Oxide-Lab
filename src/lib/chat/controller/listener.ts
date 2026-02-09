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
import type { RetrievalSource } from '$lib/chat/types';

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

export function createStreamListener(ctx: ChatControllerCtx) {
    let unlistenStart: (() => void) | null = null;
    let unlistenMessage: (() => void) | null = null;
    let unlistenDone: (() => void) | null = null;
    let unlistenRetrievalContext: (() => void) | null = null;
    let unlistenRetrievalWarning: (() => void) | null = null;
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
            });
            ctx.messages = msgs;
        } else {
            last.thinking = '';
            last.isThinking = false;
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
        if (rafId !== null) {
            cancelAnimationFrame(rafId);
            rafId = null;
        }
        pendingSources = [];
        pendingWarnings = [];
    }

    return { ensureListener, destroy };
}
