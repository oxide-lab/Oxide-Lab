/**
 * Chat Controller Actions
 * 
 * Implements all chat-related actions for model loading, inference, and device management.
 */

import type { ChatControllerCtx } from './types';
import { createStreamListener } from './listener';
import { buildPromptWithChatTemplate } from '$lib/chat/prompts';
import { get } from 'svelte/store';
import { t } from '$lib/i18n';
import { chatState } from '$lib/stores/chat';
import type { Attachment as UiAttachment } from '$lib/chat/types';

type ComposerFile = {
    url?: string;
    mediaType?: string;
    filename?: string;
};

type SendPayload = {
    text?: string;
    files?: ComposerFile[];
};

type BackendAttachment = {
    kind?: string | null;
    mime?: string | null;
    name?: string | null;
    path?: string | null;
    bytes_b64?: string | null;
    size?: number | null;
};

export function createActions(ctx: ChatControllerCtx) {
    const stream = createStreamListener(ctx);

    function toBackendMessages(messages: typeof ctx.messages) {
        return messages
            .filter((m) => m.role === 'user' || m.role === 'assistant')
            .map((m) => ({
                role: m.role,
                content: m.content ?? '',
            }));
    }

    function uiToBackendAttachments(attachments: UiAttachment[] | undefined): BackendAttachment[] {
        if (!attachments || attachments.length === 0) return [];
        return attachments.map((attachment) => ({
            kind: attachment.kind ?? null,
            mime: attachment.mimeType ?? null,
            name: attachment.filename ?? null,
            path: attachment.path ?? null,
            bytes_b64: attachment.data ?? null,
            size: attachment.size ?? null,
        }));
    }

    function backendToUiAttachments(attachments: BackendAttachment[] | undefined): UiAttachment[] {
        if (!attachments || attachments.length === 0) return [];
        return attachments.map((attachment) => ({
            filename: attachment.name ?? 'attachment',
            data: attachment.bytes_b64 ?? '',
            mimeType: attachment.mime ?? 'application/octet-stream',
            path: attachment.path ?? undefined,
            kind: attachment.kind ?? undefined,
            size: typeof attachment.size === 'number' ? attachment.size : undefined,
        }));
    }

    function latestUserAttachments(messages: typeof ctx.messages): BackendAttachment[] {
        for (let i = messages.length - 1; i >= 0; i -= 1) {
            const message = messages[i];
            if (message.role !== 'user') continue;
            return uiToBackendAttachments(message.attachments);
        }
        return [];
    }

    function normalizedRetrievalUrls(): string[] {
        const dedup = new Set<string>();
        for (const raw of ctx.retrieval_urls ?? []) {
            const url = String(raw ?? '').trim();
            if (!url) continue;
            dedup.add(url);
        }
        return Array.from(dedup);
    }

    async function resolveUrlCandidatesBeforeSend(text: string): Promise<boolean> {
        if (!ctx.retrieval_url_enabled) return true;

        const existing = normalizedRetrievalUrls();
        if (existing.length > 0) {
            if (existing.length !== (ctx.retrieval_urls ?? []).length) {
                ctx.retrieval_urls = existing;
            }
            return true;
        }

        try {
            const { invoke } = await import('@tauri-apps/api/core');
            const candidates = await invoke<string[]>('extract_url_candidates', {
                messages: toBackendMessages(ctx.messages),
                prompt: text,
            });
            const normalized = Array.from(
                new Set((candidates ?? []).map((value) => String(value ?? '').trim()).filter(Boolean)),
            );
            if (normalized.length === 0) return true;
            ctx.retrieval_urls = normalized;

            const { ask } = await import('@tauri-apps/plugin-dialog');
            const preview = normalized.slice(0, 6).map((url) => `- ${url}`).join('\n');
            const tail =
                normalized.length > 6
                    ? `\n...and ${normalized.length - 6} more`
                    : '';
            const approved = await ask(
                `Found URL candidates for retrieval:\n\n${preview}${tail}\n\nUse them for this request?`,
                {
                    title: 'Confirm URL retrieval',
                    kind: 'info',
                    okLabel: 'Use URLs',
                    cancelLabel: 'Cancel',
                },
            );
            return approved === true;
        } catch (err) {
            console.warn('[retrieval] failed to extract URL candidates', err);
            return true;
        }
    }

    function isModelLoadDebugEnabled() {
        try {
            return localStorage.getItem('oxide.debugModelLoad') === '1';
        } catch {
            return false;
        }
    }

    // Load progress event type
    type LoadProgressEvent = {
        stage: string;
        progress: number;
        message?: string;
        done?: boolean;
        error?: string;
    };

    let loadUnlisten: (() => void) | null = null;
    let lastLoadProgressAt = 0;

    async function ensureLoadProgressListener() {
        if (loadUnlisten) return;
        try {
            // TODO: Integrate with Tauri backend
            // Command: listen('load_progress', callback)
            const { listen } = await import('@tauri-apps/api/event');

            loadUnlisten = await listen<LoadProgressEvent>('load_progress', async (e) => {
                const p = e.payload || ({} as LoadProgressEvent);
                if (isModelLoadDebugEnabled()) {
                    const now = performance.now();
                    const delta = lastLoadProgressAt ? Math.round(now - lastLoadProgressAt) : 0;
                    lastLoadProgressAt = now;
                    console.debug('[load_progress]', { deltaMs: delta, ...p });
                }
                if (typeof p.progress === 'number')
                    ctx.loadingProgress = Math.max(0, Math.min(100, Math.floor(p.progress)));
                if (typeof p.stage === 'string') ctx.loadingStage = p.stage;
                if (p.message) ctx.errorText = '';
                if (p.error) ctx.errorText = String(p.error);

                // If start stage, ensure loading indicators are on
                if (p.stage === 'start') {
                    ctx.isLoadingModel = true;
                    ctx.busy = true;
                    ctx.isLoaded = false;
                    chatState.update(s => ({ ...s, isLoaded: false, isLoadingModel: true, busy: true }));
                }
                if (p.done) {
                    ctx.isLoadingModel = false;
                    ctx.busy = false;
                    if (!p.error) {
                        ctx.isLoaded = true;
                        ctx.loadingProgress = 100;
                        chatState.update(s => ({ ...s, isLoaded: true, isLoadingModel: false, busy: false, loadingProgress: 100 }));
                    }
                }
            });

            // Additional channel: early modality signal from backend
            await listen<{ text?: boolean; image?: boolean; audio?: boolean; video?: boolean }>('modality_support', (e) => {
                const m = e.payload || {};
                if (typeof m.text === 'boolean') ctx.supports_text = m.text;
                if (typeof m.image === 'boolean') ctx.supports_image = m.image;
                if (typeof m.audio === 'boolean') ctx.supports_audio = m.audio;
                if (typeof m.video === 'boolean') ctx.supports_video = m.video;
            });
        } catch (err) {
            console.warn('failed to attach load_progress listener', err);
        }
    }

    async function refreshDeviceInfo() {
        try {
            // TODO: Integrate with Tauri backend
            // Command: invoke('get_device_info')
            const { invoke } = await import('@tauri-apps/api/core');
            const info = await invoke<{
                cuda_build?: boolean;
                cuda_available?: boolean;
                current?: string;
                avx?: boolean;
                neon?: boolean;
                simd128?: boolean;
                f16c?: boolean;
            }>('get_device_info');

            ctx.cuda_build = !!info?.cuda_build;
            ctx.cuda_available = !!info?.cuda_available;
            ctx.current_device = String(info?.current ?? 'CPU');
            ctx.avx = !!info?.avx;
            ctx.neon = !!info?.neon;
            ctx.simd128 = !!info?.simd128;
            ctx.f16c = !!info?.f16c;
            ctx.use_gpu = ctx.cuda_available && ctx.current_device === 'CUDA';
        } catch { /* ignore */ }
    }

    async function setDeviceByToggle(desired?: boolean) {
        try {
            const { invoke } = await import('@tauri-apps/api/core');
            if (typeof desired !== 'undefined') {
                ctx.use_gpu = !!desired;
            }
            if (ctx.use_gpu) {
                await invoke('set_device', { pref: { kind: 'cuda', index: 0 } });
            } else {
                await invoke('set_device', { pref: { kind: 'cpu' } });
            }
            await refreshDeviceInfo();
        } catch (e) {
            console.warn('[device] toggle switch failed', e);
        }
    }

    // Initialize device info on start
    void refreshDeviceInfo();

    function cancelLoading() {
        ctx.isCancelling = true;
        ctx.loadingStage = 'cancelling';
        try {
            import('@tauri-apps/api/core').then(({ invoke }) => {
                void invoke('cancel_model_loading');
            });
        } catch { /* ignore */ }
    }

    async function loadGGUF() {
        ctx.isLoadingModel = true;
        ctx.loadingProgress = 0;
        ctx.loadingStage = 'start';
        ctx.busy = true;
        ctx.isLoaded = false;
        ctx.errorText = '';

        let stallTimer: ReturnType<typeof setInterval> | null = null;
        if (isModelLoadDebugEnabled()) {
            const intervalMs = 250;
            let expected = performance.now() + intervalMs;
            stallTimer = setInterval(() => {
                const now = performance.now();
                const drift = now - expected;
                expected = now + intervalMs;
                if (drift > 500) {
                    console.warn('[ui-stall] event loop blocked', { driftMs: Math.round(drift) });
                }
            }, intervalMs);
        }

        try {
            await ensureLoadProgressListener();
            await stream.ensureListener();

            const { invoke } = await import('@tauri-apps/api/core');
            const { message } = await import('@tauri-apps/plugin-dialog');

            const context_length = Math.max(1, Math.floor(ctx.ctx_limit_value));
            console.log('[load] frontend params', { context_length, format: ctx.format });

            if (ctx.isCancelling) return;

            // TODO: Integrate with Tauri backend
            // Command: invoke('load_model', { req: {...} })

            if (ctx.format === 'gguf') {
                if (!ctx.modelPath) {
                    await message('Укажите путь к .gguf', { title: 'Загрузка модели', kind: 'warning' });
                    return;
                }
                await invoke('load_model', {
                    req: {
                        format: 'gguf',
                        model_path: ctx.modelPath,
                        tokenizer_path: null,
                        mmproj_path: ctx.mmprojPath?.trim() ? ctx.mmprojPath.trim() : null,
                        context_length,
                        device: ctx.use_gpu ? { kind: 'cuda', index: 0 } : { kind: 'cpu' },
                    },
                });
            } else if (ctx.format === 'hub_gguf') {
                if (!ctx.repoId || !ctx.hubGgufFilename) {
                    await message('Укажите repoId и имя файла .gguf', {
                        title: 'Загрузка из HF Hub',
                        kind: 'warning',
                    });
                    return;
                }
                await invoke('load_model', {
                    req: {
                        format: 'hub_gguf',
                        repo_id: ctx.repoId,
                        revision: ctx.revision || null,
                        filename: ctx.hubGgufFilename,
                        mmproj_path: ctx.mmprojPath?.trim() ? ctx.mmprojPath.trim() : null,
                        context_length,
                        device: ctx.use_gpu ? { kind: 'cuda', index: 0 } : { kind: 'cpu' },
                    },
                });
            } else {
                await message('Поддерживаются только форматы GGUF и Hub GGUF.', {
                    title: 'Неподдерживаемый формат',
                    kind: 'warning',
                });
                return;
            }

            await refreshDeviceInfo();
            if (ctx.isCancelling) return;
        } catch (e) {
            const err = String(e ?? 'Unknown error');
            ctx.errorText = err;
            try {
                const { message } = await import('@tauri-apps/plugin-dialog');
                await message(err, { title: get(t)('chat.errors.loadFailed'), kind: 'error' });
            } catch { /* ignore */ }
        } finally {
            if (stallTimer) clearInterval(stallTimer);
        }
    }

    async function unloadGGUF() {
        if (ctx.busy || !ctx.isLoaded) return;
        ctx.isUnloadingModel = true;
        ctx.unloadingProgress = 0;
        ctx.busy = true;
        ctx.errorText = '';
        chatState.update((s) => ({
            ...s,
            busy: true,
            isUnloadingModel: true,
            unloadingProgress: 0,
            errorText: '',
        }));

        try {
            const unloadInterval = setInterval(() => {
                if (ctx.unloadingProgress < 80) ctx.unloadingProgress += Math.random() * 15 + 5;
            }, 100);

            const { invoke } = await import('@tauri-apps/api/core');
            await invoke('unload_model');

            ctx.unloadingProgress = 100;
            clearInterval(unloadInterval);
            await new Promise((r) => setTimeout(r, 300));
            ctx.isLoaded = false;
            ctx.modelPath = '';
            ctx.messages = [];
            ctx.errorText = get(t)('chat.loading.unloadSuccess');
            chatState.update((s) => ({
                ...s,
                isLoaded: false,
                modelPath: '',
                busy: true,
                isUnloadingModel: true,
                unloadingProgress: 100,
                errorText: ctx.errorText,
            }));

            const unloadSuccessText = get(t)('chat.loading.unloadSuccess');
            setTimeout(() => {
                if (ctx.errorText === unloadSuccessText) ctx.errorText = '';
            }, 3000);
        } catch (e) {
            const err = String(e ?? 'Unknown error');
            ctx.errorText = err;
        } finally {
            ctx.isUnloadingModel = false;
            ctx.unloadingProgress = 0;
            ctx.busy = false;
            chatState.update((s) => ({
                ...s,
                busy: false,
                isLoaded: ctx.isLoaded,
                modelPath: ctx.modelPath,
                isUnloadingModel: false,
                unloadingProgress: 0,
                errorText: ctx.errorText,
            }));
        }
    }

    async function handleSend(payload?: SendPayload) {
        const text = (payload?.text ?? ctx.prompt).trim();
        const hasFiles = (payload?.files ?? []).some((file) => file.filename && file.url);
        if ((!text && !hasFiles) || ctx.busy) return;

        // Check both ctx.isLoaded and chatState for model loaded status
        // (ctx.isLoaded may not update due to Svelte 5 reactivity issues with getter/setter pattern)
        const storeState = get(chatState);
        const isModelLoaded = ctx.isLoaded || storeState.isLoaded;
        if (!isModelLoaded) {
            const { message } = await import('@tauri-apps/plugin-dialog');
            await message(get(t)('chat.errors.loadModelFirst'), {
                title: get(t)('chat.errors.modelNotLoaded'),
                kind: 'warning',
            });
            return;
        }

        if (text) {
            const canContinue = await resolveUrlCandidatesBeforeSend(text);
            if (!canContinue) return;
        }

        // Add user message to database
        const { chatHistory } = await import('$lib/stores/chat-history');
        const { invoke } = await import('@tauri-apps/api/core');

        // Create session if none exists
        let state = get(chatHistory);
        if (!state.currentSessionId) {
            await chatHistory.createSession(ctx.modelPath, ctx.repoId);
            state = get(chatHistory);
        }

        const rawAttachments: BackendAttachment[] = (payload?.files ?? [])
            .filter((file) => file.filename && file.url)
            .map((file) => ({
                name: file.filename ?? null,
                mime: file.mediaType ?? null,
                bytes_b64: file.url ?? null,
            }));

        let persistedAttachments: BackendAttachment[] = [];
        if (rawAttachments.length > 0) {
            if (!state.currentSessionId) {
                throw new Error('Missing current session id for attachment persistence');
            }
            persistedAttachments = await invoke<BackendAttachment[]>('persist_chat_attachments', {
                sessionId: state.currentSessionId,
                attachments: rawAttachments,
            });
        }
        const uiAttachments = backendToUiAttachments(persistedAttachments);

        // Save user message to DB
        await chatHistory.addMessage({ role: 'user', content: text, attachments: uiAttachments });

        // Add empty assistant message to DB (will be updated by listener on stream end)
        await chatHistory.addMessage({
            role: 'assistant',
            content: '',
            thinking: '',
            sources: [],
            retrievalWarnings: [],
        });

        // Update local context
        const msgs = ctx.messages;
        msgs.push({ role: 'user', content: text, attachments: uiAttachments });
        msgs.push({
            role: 'assistant',
            content: '',
            thinking: '',
            isThinking: false,
            sources: [],
            retrievalWarnings: [],
            mcpToolCalls: [],
        });
        ctx.messages = msgs;

        ctx.prompt = '';
        await generateFromHistory();
    }

    async function generateFromHistory() {
        ctx.busy = true;
        chatState.update(s => ({ ...s, busy: true }));
        try {
            await stream.ensureListener();

            const msgs = ctx.messages;
            let hist =
                msgs[msgs.length - 1]?.role === 'assistant' && msgs[msgs.length - 1]?.content === ''
                    ? msgs.slice(0, -1)
                    : msgs.slice();

            const chatPrompt = await buildPromptWithChatTemplate(hist);
            const attachments = latestUserAttachments(hist);

            console.log('[infer] frontend params', {
                use_custom_params: ctx.use_custom_params,
                temperature: ctx.use_custom_params && ctx.temperature_enabled ? ctx.temperature : null,
                top_k: ctx.use_custom_params && ctx.top_k_enabled ? Math.max(1, Math.floor(ctx.top_k_value)) : null,
                top_p: ctx.use_custom_params && ctx.top_p_enabled
                    ? ctx.top_p_value > 0 && ctx.top_p_value <= 1 ? ctx.top_p_value : 0.9
                    : null,
                min_p: ctx.use_custom_params && ctx.min_p_enabled
                    ? ctx.min_p_value > 0 && ctx.min_p_value <= 1 ? ctx.min_p_value : 0.05
                    : null,
                repeat_penalty: ctx.use_custom_params && ctx.repeat_penalty_enabled ? ctx.repeat_penalty_value : null,
            });

            const { invoke } = await import('@tauri-apps/api/core');
            await invoke('generate_stream', {
                req: {
                    prompt: chatPrompt,
                    messages: toBackendMessages(hist),
                    attachments: attachments.length > 0 ? attachments : null,
                    use_custom_params: ctx.use_custom_params,
                    temperature: ctx.use_custom_params && ctx.temperature_enabled ? ctx.temperature : null,
                    top_p: ctx.use_custom_params && ctx.top_p_enabled
                        ? ctx.top_p_value > 0 && ctx.top_p_value <= 1 ? ctx.top_p_value : 0.9
                        : null,
                    top_k: ctx.use_custom_params && ctx.top_k_enabled
                        ? Math.max(1, Math.floor(ctx.top_k_value))
                        : null,
                    min_p: ctx.use_custom_params && ctx.min_p_enabled
                        ? ctx.min_p_value > 0 && ctx.min_p_value <= 1 ? ctx.min_p_value : 0.05
                        : null,
                    repeat_penalty: ctx.use_custom_params && ctx.repeat_penalty_enabled ? ctx.repeat_penalty_value : null,
                    repeat_last_n: 64,
                    split_prompt: !!ctx.split_prompt,
                    verbose_prompt: !!ctx.verbose_prompt,
                    tracing: !!ctx.tracing,
                    retrieval: {
                        web: {
                            enabled: ctx.retrieval_url_enabled,
                            urls: normalizedRetrievalUrls(),
                        },
                        local: {
                            enabled: ctx.retrieval_local_enabled,
                        },
                    },
                    mcp: {
                        enabled: ctx.mcp_enabled,
                    },
                },
            });
        } catch (e) {
            const err = String(e ?? 'Unknown error');
            const msgs = ctx.messages;
            const last = msgs[msgs.length - 1];
            if (last && last.role === 'assistant' && last.content === '') {
                last.content = `${get(t)('chat.errors.generationFailed')}: ${err}`;
                ctx.messages = msgs;
            }
            try {
                const { message } = await import('@tauri-apps/plugin-dialog');
                await message(err, { title: get(t)('chat.errors.generationFailed'), kind: 'error' });
            } catch { /* ignore */ }
        } finally {
            ctx.busy = false;
            chatState.update(s => ({ ...s, busy: false }));
        }
    }

    async function stopGenerate() {
        console.log('[stopGenerate] called');
        try {
            const { invoke } = await import('@tauri-apps/api/core');
            console.log('[stopGenerate] invoking cancel_generation');
            await invoke('cancel_generation');
            console.log('[stopGenerate] cancel_generation completed');

            // Save partial generation
            const { chatHistory } = await import('$lib/stores/chat-history');
            const state = get(chatHistory);
            if (state.currentSessionId) {
                const msgs = ctx.messages;
                const last = msgs[msgs.length - 1];
                if (last && last.role === 'assistant' && last.content) {
                    await chatHistory.saveAssistantMessage(
                        state.currentSessionId,
                        last.content,
                        last.thinking,
                        last.sources ?? [],
                        last.retrievalWarnings ?? [],
                    );
                }
            }
        } catch (err) {
            console.error('[stopGenerate] error:', err);
        }
    }

    /**
     * Edit a message at the given index and regenerate from that point.
     * Truncates history at editIndex, updates the message content, and regenerates.
     */
    async function handleEdit(editIndex: number, newContent: string) {
        if (ctx.busy) return;

        const storeState = get(chatState);
        const isModelLoaded = ctx.isLoaded || storeState.isLoaded;
        if (!isModelLoaded) {
            const { message } = await import('@tauri-apps/plugin-dialog');
            await message(get(t)('chat.errors.loadModelFirst'), {
                title: get(t)('chat.errors.modelNotLoaded'),
                kind: 'warning',
            });
            return;
        }

        const { chatHistory } = await import('$lib/stores/chat-history');
        const historyState = get(chatHistory);

        // Truncate messages to editIndex (inclusive) and update content
        const msgs = ctx.messages.slice(0, editIndex + 1);
        if (msgs[editIndex]) {
            msgs[editIndex].content = newContent;
        }

        // Sync with database
        if (historyState.currentSessionId) {
            // Truncate in DB (keep messages up to editIndex + 1)
            await chatHistory.truncateMessages(historyState.currentSessionId, editIndex + 1);
            // Update the edited message content
            await chatHistory.updateLastMessage(historyState.currentSessionId, newContent);
            // Add new empty assistant message
            await chatHistory.addMessage({
                role: 'assistant',
                content: '',
                thinking: '',
                sources: [],
                retrievalWarnings: [],
            });
        }

        // Add empty assistant message for the new response
        msgs.push({
            role: 'assistant',
            content: '',
            html: '',
            thinking: '',
            isThinking: false,
            sources: [],
            retrievalWarnings: [],
            mcpToolCalls: [],
        });
        ctx.messages = msgs;

        await generateFromHistoryWithIndex(editIndex);
    }

    /**
     * Regenerate the last assistant response.
     * Finds the last user message and regenerates from that point.
     */
    async function handleRegenerate(messageIndex: number) {
        if (ctx.busy) return;

        const storeState = get(chatState);
        const isModelLoaded = ctx.isLoaded || storeState.isLoaded;
        if (!isModelLoaded) {
            const { message } = await import('@tauri-apps/plugin-dialog');
            await message(get(t)('chat.errors.loadModelFirst'), {
                title: get(t)('chat.errors.modelNotLoaded'),
                kind: 'warning',
            });
            return;
        }

        // Find the user message before this assistant message
        let userIndex = messageIndex;
        if (ctx.messages[messageIndex]?.role === 'assistant') {
            userIndex = messageIndex - 1;
        }

        if (userIndex < 0 || ctx.messages[userIndex]?.role !== 'user') {
            console.warn('[regenerate] Could not find user message to regenerate from');
            return;
        }

        const { chatHistory } = await import('$lib/stores/chat-history');
        const historyState = get(chatHistory);

        // Truncate to include the user message, remove the assistant response
        const msgs = ctx.messages.slice(0, userIndex + 1);

        // Sync with database
        if (historyState.currentSessionId) {
            // Truncate in DB (keep messages up to userIndex + 1)
            await chatHistory.truncateMessages(historyState.currentSessionId, userIndex + 1);
            // Add new empty assistant message
            await chatHistory.addMessage({
                role: 'assistant',
                content: '',
                thinking: '',
                sources: [],
                retrievalWarnings: [],
            });
        }

        // Add empty assistant message for the new response
        msgs.push({
            role: 'assistant',
            content: '',
            html: '',
            thinking: '',
            isThinking: false,
            sources: [],
            retrievalWarnings: [],
            mcpToolCalls: [],
        });
        ctx.messages = msgs;

        await generateFromHistoryWithIndex(userIndex);
    }

    /**
     * Generate from history with edit_index for truncating on backend if needed.
     */
    async function generateFromHistoryWithIndex(editIndex?: number) {
        ctx.busy = true;
        chatState.update(s => ({ ...s, busy: true }));
        try {
            await stream.ensureListener();

            const msgs = ctx.messages;
            let hist =
                msgs[msgs.length - 1]?.role === 'assistant' && msgs[msgs.length - 1]?.content === ''
                    ? msgs.slice(0, -1)
                    : msgs.slice();

            const chatPrompt = await buildPromptWithChatTemplate(hist);
            const attachments = latestUserAttachments(hist);

            const { invoke } = await import('@tauri-apps/api/core');
            await invoke('generate_stream', {
                req: {
                    prompt: chatPrompt,
                    messages: toBackendMessages(hist),
                    attachments: attachments.length > 0 ? attachments : null,
                    use_custom_params: ctx.use_custom_params,
                    temperature: ctx.use_custom_params && ctx.temperature_enabled ? ctx.temperature : null,
                    top_p: ctx.use_custom_params && ctx.top_p_enabled
                        ? ctx.top_p_value > 0 && ctx.top_p_value <= 1 ? ctx.top_p_value : 0.9
                        : null,
                    top_k: ctx.use_custom_params && ctx.top_k_enabled
                        ? Math.max(1, Math.floor(ctx.top_k_value))
                        : null,
                    min_p: ctx.use_custom_params && ctx.min_p_enabled
                        ? ctx.min_p_value > 0 && ctx.min_p_value <= 1 ? ctx.min_p_value : 0.05
                        : null,
                    repeat_penalty: ctx.use_custom_params && ctx.repeat_penalty_enabled ? ctx.repeat_penalty_value : null,
                    repeat_last_n: 64,
                    split_prompt: !!ctx.split_prompt,
                    verbose_prompt: !!ctx.verbose_prompt,
                    tracing: !!ctx.tracing,
                    edit_index: editIndex,
                    retrieval: {
                        web: {
                            enabled: ctx.retrieval_url_enabled,
                            urls: normalizedRetrievalUrls(),
                        },
                        local: {
                            enabled: ctx.retrieval_local_enabled,
                        },
                    },
                    mcp: {
                        enabled: ctx.mcp_enabled,
                    },
                },
            });
        } catch (e) {
            const err = String(e ?? 'Unknown error');
            const msgs = ctx.messages;
            const last = msgs[msgs.length - 1];
            if (last && last.role === 'assistant' && last.content === '') {
                last.content = `${get(t)('chat.errors.generationFailed')}: ${err}`;
                ctx.messages = msgs;
            }
            try {
                const { message } = await import('@tauri-apps/plugin-dialog');
                await message(err, { title: get(t)('chat.errors.generationFailed'), kind: 'error' });
            } catch { /* ignore */ }
        } finally {
            ctx.busy = false;
            chatState.update(s => ({ ...s, busy: false }));
        }
    }

    async function pickModel() {
        const { open, message } = await import('@tauri-apps/plugin-dialog');

        if (ctx.format === 'gguf') {
            const selected = await open({
                multiple: false,
                filters: [{ name: 'GGUF', extensions: ['gguf'] }],
            });
            if (typeof selected === 'string') ctx.modelPath = selected;
        } else {
            await message(
                'Для загрузки из HF Hub заполните repoId, revision (по желанию) и, для GGUF, имя файла.',
                { title: 'HF Hub', kind: 'info' },
            );
        }
    }

    function destroy() {
        stream.destroy();
    }

    return {
        cancelLoading,
        loadGGUF,
        unloadGGUF,
        handleSend,
        handleEdit,
        handleRegenerate,
        generateFromHistory,
        stopGenerate,
        pickModel,
        destroy,
        refreshDeviceInfo,
        setDeviceByToggle,
        ensureStreamListener: stream.ensureListener,
    };
}
