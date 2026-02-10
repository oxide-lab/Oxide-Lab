/**
 * Chat History Store
 *
 * Manages chat sessions with SQLite persistence through tauri-plugin-sql.
 * Provides reactive Svelte stores for sessions and messages.
 */

import { writable, derived, get } from 'svelte/store';
import type { ChatMessage } from './chat';
import type { RetrievalSource } from '$lib/chat/types';

const DB_NAME = 'sqlite:chat_history.db';

// Types matching the SQLite schema
export type DbSession = {
    id: string;
    title: string;
    model_path: string | null;
    repo_id: string | null;
    created_at: number;
    updated_at: number;
};

export type DbMessage = {
    id: number;
    session_id: string;
    role: string;
    content: string;
    thinking: string;
    sources_json: string;
    attachments_json: string;
    created_at: number;
};

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

// Lazy-load Database to avoid SSR issues
let Database: typeof import('@tauri-apps/plugin-sql').default | null = null;
let dbInstance: Awaited<ReturnType<typeof import('@tauri-apps/plugin-sql').default.load>> | null =
    null;

async function getDb() {
    if (dbInstance) return dbInstance;

    if (!Database) {
        const mod = await import('@tauri-apps/plugin-sql');
        Database = mod.default;
    }

    dbInstance = await Database.load(DB_NAME);
    return dbInstance;
}

type AttachmentPathRow = {
    attachments_json: string;
};

async function collectAttachmentPathsForSession(sessionId: string): Promise<string[]> {
    const db = await getDb();
    const rows = await db.select<AttachmentPathRow[]>(
        'SELECT attachments_json FROM messages WHERE session_id = ?',
        [sessionId],
    );
    const paths: string[] = [];
    for (const row of rows) {
        try {
            const parsed = JSON.parse(row.attachments_json || '[]');
            if (!Array.isArray(parsed)) continue;
            for (const item of parsed) {
                if (item && typeof item.path === 'string' && item.path.trim()) {
                    paths.push(item.path);
                }
            }
        } catch {
            // Ignore malformed rows.
        }
    }
    return Array.from(new Set(paths));
}

async function collectAttachmentPathsForAllSessions(): Promise<string[]> {
    const db = await getDb();
    const rows = await db.select<AttachmentPathRow[]>(
        'SELECT attachments_json FROM messages',
        [],
    );
    const paths: string[] = [];
    for (const row of rows) {
        try {
            const parsed = JSON.parse(row.attachments_json || '[]');
            if (!Array.isArray(parsed)) continue;
            for (const item of parsed) {
                if (item && typeof item.path === 'string' && item.path.trim()) {
                    paths.push(item.path);
                }
            }
        } catch {
            // Ignore malformed rows.
        }
    }
    return Array.from(new Set(paths));
}

async function cleanupAttachmentPaths(paths: string[]) {
    if (!paths.length) return;
    try {
        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('delete_chat_attachment_files', { paths });
    } catch (err) {
        console.warn('Failed to cleanup attachment files:', err);
    }
}

function dbMessageToChatMessage(msg: DbMessage): ChatMessage {
    let sources: RetrievalSource[] = [];
    try {
        const parsed = JSON.parse(msg.sources_json || '[]');
        if (Array.isArray(parsed)) {
            sources = parsed as RetrievalSource[];
        }
    } catch {
        sources = [];
    }

    let attachments: ChatMessage['attachments'] = [];
    try {
        const parsed = JSON.parse(msg.attachments_json || '[]');
        if (Array.isArray(parsed)) {
            attachments = parsed as ChatMessage['attachments'];
        }
    } catch {
        attachments = [];
    }

    return {
        role: msg.role as 'user' | 'assistant',
        content: msg.content,
        thinking: msg.thinking || undefined,
        sources,
        attachments,
    };
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
                const db = await getDb();

                // Load all sessions
                const rows = await db.select<DbSession[]>(
                    'SELECT id, title, model_path, repo_id, created_at, updated_at FROM sessions ORDER BY updated_at DESC',
                );

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
                const db = await getDb();
                await db.execute(
                    'INSERT INTO sessions (id, title, model_path, repo_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)',
                    [id, title, modelPath ?? null, repoId ?? null, now, now],
                );
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
                const db = await getDb();
                const messages = await db.select<DbMessage[]>(
                    'SELECT id, session_id, role, content, thinking, sources_json, attachments_json, created_at FROM messages WHERE session_id = ? ORDER BY id ASC',
                    [sessionId],
                );

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
            const state = get({ subscribe });
            const targetSessionId = sessionId ?? state.currentSessionId;

            if (!targetSessionId) return;

            try {
                const db = await getDb();
                await db.execute(
                    'INSERT INTO messages (session_id, role, content, thinking, sources_json, attachments_json, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)',
                    [
                        targetSessionId,
                        message.role,
                        message.content,
                        message.thinking ?? '',
                        JSON.stringify(message.sources ?? []),
                        JSON.stringify(message.attachments ?? []),
                        now,
                    ],
                );
                await db.execute('UPDATE sessions SET updated_at = ? WHERE id = ?', [
                    now,
                    targetSessionId,
                ]);
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
                        getDb()
                            .then((db) =>
                                db.execute('UPDATE sessions SET title = ? WHERE id = ?', [
                                    titleFromMessage,
                                    targetSessionId,
                                ]),
                            )
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
                const db = await getDb();
                await db.execute(
                    `UPDATE messages SET content = ?, thinking = ?, sources_json = ?
                     WHERE id = (SELECT MAX(id) FROM messages WHERE session_id = ?)`,
                    [content, thinking ?? '', JSON.stringify(sources ?? []), sessionId],
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
                const db = await getDb();
                const now = Date.now();
                await db.execute(
                    `UPDATE messages SET content = ?, thinking = ?, sources_json = ?
                     WHERE id = (SELECT MAX(id) FROM messages WHERE session_id = ?)`,
                    [content, thinking ?? '', JSON.stringify(sources ?? []), sessionId],
                );
                await db.execute('UPDATE sessions SET updated_at = ? WHERE id = ?', [now, sessionId]);
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
                const db = await getDb();
                await db.execute(
                    `DELETE FROM messages 
                     WHERE session_id = ? 
                     AND id NOT IN (
                         SELECT id FROM messages 
                         WHERE session_id = ? 
                         ORDER BY id ASC 
                         LIMIT ?
                     )`,
                    [sessionId, sessionId, keepCount],
                );
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
            const attachmentPaths = await collectAttachmentPathsForSession(sessionId).catch(() => []);
            try {
                const db = await getDb();
                await db.execute('DELETE FROM sessions WHERE id = ?', [sessionId]);
                deleted = true;
            } catch (err) {
                console.error('Failed to delete session from DB:', err);
            }
            if (!deleted) return;
            await cleanupAttachmentPaths(attachmentPaths);

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
                const db = await getDb();
                await db.execute('UPDATE sessions SET title = ?, updated_at = ? WHERE id = ?', [
                    title,
                    now,
                    sessionId,
                ]);
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
            const attachmentPaths = await collectAttachmentPathsForAllSessions().catch(() => []);
            try {
                const db = await getDb();
                await db.execute('DELETE FROM sessions');
                deleted = true;
            } catch (err) {
                console.error('Failed to clear chat history:', err);
            }
            if (!deleted) return;
            await cleanupAttachmentPaths(attachmentPaths);

            update((s) => ({ ...s, sessions: [], currentSessionId: null }));
        },

        exportSession: (sessionId: string) => {
            const state = get({ subscribe });
            const session = state.sessions.find((s) => s.id === sessionId);
            return session ? JSON.stringify(session, null, 2) : null;
        },

        importSession: async (_json: string) => {
            // TODO: Implement session import
            return false;
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

// Group sessions by time period (like Ollama)
export const groupedSessions = derived(sortedSessions, ($sessions) => {
    const now = Date.now();
    const todayStart = new Date().setHours(0, 0, 0, 0);
    const weekAgo = now - 7 * 24 * 60 * 60 * 1000;

    const groups = {
        today: [] as ChatSession[],
        thisWeek: [] as ChatSession[],
        older: [] as ChatSession[],
    };

    for (const session of $sessions) {
        if (session.updatedAt >= todayStart) {
            groups.today.push(session);
        } else if (session.updatedAt >= weekAgo) {
            groups.thisWeek.push(session);
        } else {
            groups.older.push(session);
        }
    }

    return groups;
});

// Auto-initialize on client side
if (typeof window !== 'undefined') {
    chatHistory.init();
}
