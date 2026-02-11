/**
 * Chat History Store
 *
 * Manages chat sessions with SQLite persistence through tauri-plugin-sql.
 * Provides reactive Svelte stores for sessions and messages.
 */

import { writable, derived, get } from 'svelte/store';
import type { ChatMessage } from './chat';
import type { RetrievalSource } from '$lib/chat/types';
import {
    chatHistoryRepository,
    dbMessageToChatMessage,
    type DbSession,
} from './chat-history-repository';

export type ChatSession = {
    id: string;
    title: string;
    messages: ChatMessage[];
    createdAt: number;
    updatedAt: number;
    modelPath?: string;
    repoId?: string;
};

export type ChatHistoryState = {
    sessions: ChatSession[];
    currentSessionId: string | null;
    isInitialized: boolean;
};

function isRecord(value: unknown): value is Record<string, unknown> {
    return typeof value === 'object' && value !== null;
}

function normalizeImportedMessages(value: unknown): ChatMessage[] {
    if (!Array.isArray(value)) return [];
    const normalized: ChatMessage[] = [];
    for (const raw of value) {
        if (!isRecord(raw)) continue;
        const role = raw.role;
        if (role !== 'user' && role !== 'assistant') continue;
        const content = typeof raw.content === 'string' ? raw.content : '';
        const message: ChatMessage = { role, content };
        if (typeof raw.thinking === 'string') {
            message.thinking = raw.thinking;
        }
        if (Array.isArray(raw.sources)) {
            message.sources = raw.sources as RetrievalSource[];
        }
        if (Array.isArray(raw.attachments)) {
            message.attachments = raw.attachments as ChatMessage['attachments'];
        }
        if (Array.isArray(raw.retrievalWarnings)) {
            message.retrievalWarnings = raw.retrievalWarnings.filter(
                (item): item is string => typeof item === 'string',
            );
        }
        normalized.push(message);
    }
    return normalized;
}


function createChatHistoryStore() {
    const { subscribe, update, set } = writable<ChatHistoryState>({
        sessions: [],
        currentSessionId: null,
        isInitialized: false,
    });

    return {
        subscribe,

        init: async () => {
            try {
                const rows = await chatHistoryRepository.loadSessions();

                const sessions: ChatSession[] = rows.map((row) => ({
                    id: row.id,
                    title: row.title,
                    modelPath: row.model_path ?? undefined,
                    repoId: row.repo_id ?? undefined,
                    createdAt: row.created_at,
                    updatedAt: row.updated_at,
                    messages: [],
                }));

                update((s) => ({
                    ...s,
                    sessions,
                    isInitialized: true,
                }));
            } catch (err) {
                console.error('Failed to init chat database:', err);
                update((s) => ({ ...s, isInitialized: true }));
            }
        },

        createSession: async (modelPath?: string, repoId?: string): Promise<string> => {
            const id = crypto.randomUUID();
            const title = 'New chat';
            const now = Date.now();

            try {
                await chatHistoryRepository.createSession(id, title, modelPath, repoId, now);
            } catch (err) {
                console.error('Failed to create session in DB:', err);
            }

            update((s) => ({
                ...s,
                sessions: [
                    {
                        id,
                        title,
                        modelPath,
                        repoId,
                        createdAt: now,
                        updatedAt: now,
                        messages: [],
                    },
                    ...s.sessions,
                ],
                currentSessionId: id,
            }));

            return id;
        },

        loadSession: async (sessionId: string) => {
            update((s) => ({ ...s, currentSessionId: sessionId }));

            try {
                const messages = await chatHistoryRepository.loadSessionMessages(sessionId);

                update((s) => {
                    const sessions = s.sessions.map((sess) =>
                        sess.id === sessionId
                            ? { ...sess, messages: messages.map(dbMessageToChatMessage) }
                            : sess,
                    );
                    return { ...s, sessions };
                });
            } catch (err) {
                console.error('Failed to load session messages:', err);
            }
        },

        addMessage: async (message: ChatMessage, sessionId?: string) => {
            const now = Date.now();
            const state = get(chatHistory);
            const targetSessionId = sessionId ?? state.currentSessionId;

            if (!targetSessionId) return;

            try {
                await chatHistoryRepository.insertMessage(targetSessionId, message, now);
            } catch (err) {
                console.error('Failed to add message to DB:', err);
            }

            update((s) => {
                const sessions = s.sessions.map((sess) => {
                    if (sess.id !== targetSessionId) return sess;

                    const isFirstUserMessage = sess.messages.length === 0 && message.role === 'user';
                    const titleFromMessage = isFirstUserMessage
                        ? message.content.substring(0, 50).split('\n')[0]
                        : sess.title;

                    // Update title in DB if changed
                    if (isFirstUserMessage && titleFromMessage !== sess.title) {
                        chatHistoryRepository
                            .updateSessionTitle(targetSessionId, titleFromMessage, now)
                            .catch(console.error);
                    }

                    return {
                        ...sess,
                        title: titleFromMessage ?? sess.title,
                        messages: [...sess.messages, message],
                        updatedAt: now,
                    };
                });

                return { ...s, sessions, currentSessionId: s.currentSessionId ?? targetSessionId };
            });
        },

        updateLastMessage: async (
            sessionId: string,
            content: string,
            thinking?: string,
            sources?: RetrievalSource[],
        ) => {
            try {
                await chatHistoryRepository.updateLastMessage(
                    sessionId,
                    content,
                    thinking ?? '',
                    sources ?? [],
                );
            } catch (err) {
                console.error('Failed to update last message in DB:', err);
            }

            update((s) => {
                const sessions = s.sessions.map((sess) => {
                    if (sess.id === sessionId && sess.messages.length > 0) {
                        const msgs = [...sess.messages];
                        msgs[msgs.length - 1] = {
                            ...msgs[msgs.length - 1],
                            content,
                            thinking: thinking ?? msgs[msgs.length - 1].thinking,
                            sources: sources ?? msgs[msgs.length - 1].sources,
                        };
                        return { ...sess, messages: msgs };
                    }
                    return sess;
                });
                return { ...s, sessions };
            });
        },

        updateLastMessageOptimistic: (content: string, thinking?: string) => {
            update((s) => {
                if (!s.currentSessionId) return s;
                const sessions = s.sessions.map((sess) => {
                    if (sess.id === s.currentSessionId) {
                        const msgs = [...sess.messages];
                        if (msgs.length > 0) {
                            msgs[msgs.length - 1] = {
                                ...msgs[msgs.length - 1],
                                content,
                                thinking: thinking ?? msgs[msgs.length - 1].thinking,
                            };
                        }
                        return { ...sess, messages: msgs };
                    }
                    return sess;
                });
                return { ...s, sessions };
            });
        },

        saveAssistantMessage: async (
            sessionId: string,
            content: string,
            thinking?: string,
            sources?: RetrievalSource[],
            retrievalWarnings?: string[],
        ) => {
            try {
                const now = Date.now();
                await chatHistoryRepository.updateLastMessage(
                    sessionId,
                    content,
                    thinking ?? '',
                    sources ?? [],
                );
                await chatHistoryRepository.touchSession(sessionId, now);
            } catch (err) {
                console.error('Failed to save assistant message:', err);
            }

            update((s) => {
                const sessions = s.sessions.map((sess) => {
                    if (sess.id !== sessionId || sess.messages.length === 0) return sess;
                    const msgs = [...sess.messages];
                    const lastIndex = msgs.length - 1;
                    msgs[lastIndex] = {
                        ...msgs[lastIndex],
                        content,
                        thinking: thinking ?? msgs[lastIndex].thinking,
                        sources: sources ?? msgs[lastIndex].sources,
                        retrievalWarnings: retrievalWarnings ?? msgs[lastIndex].retrievalWarnings,
                    };
                    return { ...sess, messages: msgs, updatedAt: Date.now() };
                });
                return { ...s, sessions };
            });
        },

        truncateMessages: async (sessionId: string, keepCount: number) => {
            try {
                await chatHistoryRepository.truncateMessages(sessionId, keepCount);
            } catch (err) {
                console.error('Failed to truncate messages:', err);
            }

            update((s) => {
                const sessions = s.sessions.map((sess) => {
                    if (sess.id === sessionId) {
                        return { ...sess, messages: sess.messages.slice(0, keepCount) };
                    }
                    return sess;
                });
                return { ...s, sessions };
            });
        },

        deleteSession: async (sessionId: string) => {
            let deleted = false;
            let attachmentPaths: string[] = [];
            try {
                attachmentPaths = await chatHistoryRepository.deleteSession(sessionId);
                deleted = true;
            } catch (err) {
                console.error('Failed to delete session from DB:', err);
            }
            if (!deleted) return;
            await chatHistoryRepository.cleanupAttachmentPaths(attachmentPaths).catch((err) => {
                console.warn('Failed to cleanup attachment files:', err);
            });

            update((s) => {
                const sessions = s.sessions.filter((sess) => sess.id !== sessionId);
                let current = s.currentSessionId;
                if (current === sessionId) {
                    current = sessions.length > 0 ? sessions[0].id : null;
                }
                return { ...s, sessions, currentSessionId: current };
            });
        },

        renameSession: async (sessionId: string, title: string) => {
            const now = Date.now();

            try {
                await chatHistoryRepository.updateSessionTitle(sessionId, title, now);
            } catch (err) {
                console.error('Failed to rename session in DB:', err);
            }

            update((s) => ({
                ...s,
                sessions: s.sessions.map((sess) =>
                    sess.id === sessionId ? { ...sess, title, updatedAt: now } : sess,
                ),
            }));
        },

        clearAll: async () => {
            let deleted = false;
            let attachmentPaths: string[] = [];
            try {
                attachmentPaths = await chatHistoryRepository.clearAll();
                deleted = true;
            } catch (err) {
                console.error('Failed to clear chat history:', err);
            }
            if (!deleted) return;
            await chatHistoryRepository.cleanupAttachmentPaths(attachmentPaths).catch((err) => {
                console.warn('Failed to cleanup attachment files:', err);
            });

            update((s) => ({ ...s, sessions: [], currentSessionId: null }));
        },

        exportSession: (sessionId: string) => {
            const state = get({ subscribe });
            const session = state.sessions.find((s) => s.id === sessionId);
            return session ? JSON.stringify(session, null, 2) : null;
        },

        importSession: async (json: string) => {
            try {
                const parsed = JSON.parse(json) as unknown;
                if (!isRecord(parsed)) return false;

                const title = typeof parsed.title === 'string' && parsed.title.trim()
                    ? parsed.title.trim()
                    : 'Imported chat';
                const modelPath = typeof parsed.modelPath === 'string' && parsed.modelPath.trim()
                    ? parsed.modelPath
                    : undefined;
                const repoId = typeof parsed.repoId === 'string' && parsed.repoId.trim()
                    ? parsed.repoId
                    : undefined;
                const createdAt = typeof parsed.createdAt === 'number' && Number.isFinite(parsed.createdAt)
                    ? parsed.createdAt
                    : Date.now();
                const updatedAt = typeof parsed.updatedAt === 'number' && Number.isFinite(parsed.updatedAt)
                    ? Math.max(parsed.updatedAt, createdAt)
                    : createdAt;
                const messages = normalizeImportedMessages(parsed.messages);

                const id = crypto.randomUUID();
                await chatHistoryRepository.createSession(id, title, modelPath, repoId, createdAt);

                for (let i = 0; i < messages.length; i += 1) {
                    await chatHistoryRepository.insertMessage(id, messages[i], createdAt + i);
                }
                if (updatedAt !== createdAt) {
                    await chatHistoryRepository.touchSession(id, updatedAt);
                }

                update((s) => ({
                    ...s,
                    sessions: [
                        {
                            id,
                            title,
                            modelPath,
                            repoId,
                            createdAt,
                            updatedAt,
                            messages,
                        },
                        ...s.sessions,
                    ],
                    currentSessionId: id,
                }));

                return true;
            } catch (err) {
                console.error('Failed to import session:', err);
                return false;
            }
        },
    };
}

export const chatHistory = createChatHistoryStore();

export const currentSession = derived(chatHistory, ($h) => {
    if (!$h.currentSessionId) return null;
    return $h.sessions.find((s) => s.id === $h.currentSessionId) || null;
});

export const sortedSessions = derived(chatHistory, ($h) => {
    return [...$h.sessions].sort((a, b) => b.updatedAt - a.updatedAt);
});

export function groupSessionsByDate(sessions: ChatSession[], now = Date.now()) {
    const todayStart = new Date().setHours(0, 0, 0, 0);
    const weekAgo = now - 7 * 24 * 60 * 60 * 1000;

    const groups = {
        today: [] as ChatSession[],
        thisWeek: [] as ChatSession[],
        older: [] as ChatSession[],
    };

    for (const session of sessions) {
        if (session.updatedAt >= todayStart) {
            groups.today.push(session);
        } else if (session.updatedAt >= weekAgo) {
            groups.thisWeek.push(session);
        } else {
            groups.older.push(session);
        }
    }

    return groups;
}

// Group sessions by time period (like Ollama)
export const groupedSessions = derived(sortedSessions, ($sessions) => {
    return groupSessionsByDate($sessions);
});

// Auto-initialize on client side
if (typeof window !== 'undefined' && !import.meta.env?.VITEST) {
    chatHistory.init();
}
