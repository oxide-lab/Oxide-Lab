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
    const state = ctx.state;
    const stream = createStreamListener(ctx);

    function toBackendMessages(messages: typeof state.messages) {
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

    function parseStopSequences(text: string): string[] {
        return Array.from(
            new Set(
                String(text ?? '')
                    .split(/\r?\n/g)
                    .map((item) => item.trim())
                    .filter(Boolean),
            ),
        );
    }

    function latestUserAttachments(messages: typeof state.messages): BackendAttachment[] {
        for (let i = messages.length - 1; i >= 0; i -= 1) {
            const message = messages[i];
            if (message.role !== 'user') continue;
            return uiToBackendAttachments(message.attachments);
        }
        return [];
    }

    function normalizedRetrievalUrls(): string[] {
        const dedup = new Set<string>();
        for (const raw of state.retrieval_urls ?? []) {
            const url = String(raw ?? '').trim();
            if (!url) continue;
            dedup.add(url);
        }
        return Array.from(dedup);
    }

    async function resolveUrlCandidatesBeforeSend(text: string): Promise<boolean> {
        if (!state.retrieval_url_enabled) return true;

        const existing = normalizedRetrievalUrls();
        if (existing.length > 0) {
            if (existing.length !== (state.retrieval_urls ?? []).length) {
                state.retrieval_urls = existing;
            }
            return true;
        }

        try {
            const { invoke } = await import('@tauri-apps/api/core');
            const candidates = await invoke<string[]>('extract_url_candidates', {
                messages: toBackendMessages(state.messages),
                prompt: text,
            });
            const normalized = Array.from(
                new Set((candidates ?? []).map((value) => String(value ?? '').trim()).filter(Boolean)),
            );
            if (normalized.length === 0) return true;
            state.retrieval_urls = normalized;

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
    let modalityUnlisten: (() => void) | null = null;

    async function refreshModalitySupport() {
        try {
            const { invoke } = await import('@tauri-apps/api/core');
            const m = await invoke<{ text?: boolean; image?: boolean; audio?: boolean; video?: boolean }>(
                'get_modality_support',
            );
            if (typeof m?.text === 'boolean') state.supports_text = m.text;
            if (typeof m?.image === 'boolean') state.supports_image = m.image;
            if (typeof m?.audio === 'boolean') state.supports_audio = m.audio;
            if (typeof m?.video === 'boolean') state.supports_video = m.video;
        } catch {
            // Ignore; modality events will still update state when available.
        }
    }

    async function ensureLoadProgressListener() {
        if (loadUnlisten && modalityUnlisten) return;
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
                    state.loadingProgress = Math.max(0, Math.min(100, Math.floor(p.progress)));
                if (typeof p.stage === 'string') state.loadingStage = p.stage;
                if (p.message) state.errorText = '';
                if (p.error) state.errorText = String(p.error);

                // If start stage, ensure loading indicators are on
                if (p.stage === 'start') {
                    state.isLoadingModel = true;
                    state.busy = true;
                    state.isLoaded = false;
                }
                if (p.done) {
                    state.isLoadingModel = false;
                    state.busy = false;
                    if (!p.error) {
                        state.isLoaded = true;
                        state.loadingProgress = 100;
                        await refreshModalitySupport();
                    }
                }
            });

            // Additional channel: early modality signal from backend
            modalityUnlisten = await listen<{ text?: boolean; image?: boolean; audio?: boolean; video?: boolean }>('modality_support', (e) => {
                const m = e.payload || {};
                if (typeof m.text === 'boolean') state.supports_text = m.text;
                if (typeof m.image === 'boolean') state.supports_image = m.image;
                if (typeof m.audio === 'boolean') state.supports_audio = m.audio;
                if (typeof m.video === 'boolean') state.supports_video = m.video;
            });

            await refreshModalitySupport();
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

            state.cuda_build = !!info?.cuda_build;
            state.cuda_available = !!info?.cuda_available;
            state.current_device = String(info?.current ?? 'CPU');
            state.avx = !!info?.avx;
            state.neon = !!info?.neon;
            state.simd128 = !!info?.simd128;
            state.f16c = !!info?.f16c;
            state.use_gpu = state.cuda_available && state.current_device === 'CUDA';
        } catch { /* ignore */ }
    }

    async function setDeviceByToggle(desired?: boolean) {
        try {
            const { invoke } = await import('@tauri-apps/api/core');
            if (typeof desired !== 'undefined') {
                state.use_gpu = !!desired;
            }
            if (state.use_gpu) {
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
    void ensureLoadProgressListener();

    function cancelLoading() {
        state.isCancelling = true;
        state.loadingStage = 'cancelling';
        try {
            import('@tauri-apps/api/core').then(({ invoke }) => {
                void invoke('cancel_model_loading');
            });
        } catch { /* ignore */ }
    }

    async function loadGGUF() {
        state.isLoadingModel = true;
        state.loadingProgress = 0;
        state.loadingStage = 'start';
        state.busy = true;
        state.isLoaded = false;
        state.errorText = '';

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

            const context_length = Math.max(1, Math.floor(state.ctx_limit_value));
            console.log('[load] frontend params', { context_length, format: state.format });

            if (state.isCancelling) return;

            // TODO: Integrate with Tauri backend
            // Command: invoke('load_model', { req: {...} })

            if (state.format === 'gguf') {
                if (!state.modelPath) {
                    await message('Укажите путь к .gguf', { title: 'Загрузка модели', kind: 'warning' });
                    return;
                }
                await invoke('load_model', {
                    req: {
                        format: 'gguf',
                        model_path: state.modelPath,
                        tokenizer_path: null,
                        context_length,
                    },
                });
            } else if (state.format === 'hub_gguf') {
                if (!state.repoId || !state.hubGgufFilename) {
                    await message('Укажите repoId и имя файла .gguf', {
                        title: 'Загрузка из HF Hub',
                        kind: 'warning',
                    });
                    return;
                }
                await invoke('load_model', {
                    req: {
                        format: 'hub_gguf',
                        repo_id: state.repoId,
                        revision: state.revision || null,
                        filename: state.hubGgufFilename,
                        context_length,
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
            if (state.isCancelling) return;
        } catch (e) {
            const err = String(e ?? 'Unknown error');
            state.errorText = err;
            try {
                const { message } = await import('@tauri-apps/plugin-dialog');
                await message(err, { title: get(t)('chat.errors.loadFailed'), kind: 'error' });
            } catch { /* ignore */ }
        } finally {
            if (stallTimer) clearInterval(stallTimer);
        }
    }

    async function unloadGGUF() {
        if (state.busy || !state.isLoaded) return;
        state.isUnloadingModel = true;
        state.unloadingProgress = 0;
        state.busy = true;
        state.errorText = '';

        try {
            const unloadInterval = setInterval(() => {
                if (state.unloadingProgress < 80) state.unloadingProgress += Math.random() * 15 + 5;
            }, 100);

            const { invoke } = await import('@tauri-apps/api/core');
            await invoke('unload_model');

            state.unloadingProgress = 100;
            clearInterval(unloadInterval);
            await new Promise((r) => setTimeout(r, 300));
            state.isLoaded = false;
            state.modelPath = '';
            state.messages = [];
            state.errorText = get(t)('chat.loading.unloadSuccess');

            const unloadSuccessText = get(t)('chat.loading.unloadSuccess');
            setTimeout(() => {
                if (state.errorText === unloadSuccessText) state.errorText = '';
            }, 3000);
        } catch (e) {
            const err = String(e ?? 'Unknown error');
            state.errorText = err;
        } finally {
            state.isUnloadingModel = false;
            state.unloadingProgress = 0;
            state.busy = false;
        }
    }

    async function handleSend(payload?: SendPayload) {
        const text = (payload?.text ?? state.prompt).trim();
        const hasFiles = (payload?.files ?? []).some((file) => file.filename && file.url);
        if ((!text && !hasFiles) || state.busy) return;

        const isModelLoaded = state.isLoaded;
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
        let historyState = get(chatHistory);
        if (!historyState.currentSessionId) {
            await chatHistory.createSession(state.modelPath, state.repoId);
            historyState = get(chatHistory);
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
            if (!historyState.currentSessionId) {
                throw new Error('Missing current session id for attachment persistence');
            }
            persistedAttachments = await invoke<BackendAttachment[]>('persist_chat_attachments', {
                sessionId: historyState.currentSessionId,
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
        state.messages = [
            ...state.messages,
            { role: 'user', content: text, attachments: uiAttachments },
            {
                role: 'assistant',
                content: '',
                thinking: '',
                isThinking: false,
                sources: [],
                retrievalWarnings: [],
                mcpToolCalls: [],
            },
        ];

        state.prompt = '';
        await generateFromHistory();
    }

    async function generateFromHistory(editIndex?: number) {
        state.busy = true;
        try {
            await stream.ensureListener();

            const msgs = state.messages;
            let hist =
                msgs[msgs.length - 1]?.role === 'assistant' && msgs[msgs.length - 1]?.content === ''
                    ? msgs.slice(0, -1)
                    : msgs.slice();

            const chatPrompt = await buildPromptWithChatTemplate(hist);
            const attachments = latestUserAttachments(hist);
            const stopSequences = parseStopSequences(state.stop_sequences_text);
            const maxNewTokens = state.max_new_tokens_enabled
                ? Math.max(1, Math.floor(state.max_new_tokens_value))
                : null;
            const seedValue = state.seed_enabled ? Math.max(0, Math.floor(state.seed_value)) : null;

            console.log('[infer] frontend params', {
                use_custom_params: state.use_custom_params,
                temperature: state.use_custom_params && state.temperature_enabled ? state.temperature : null,
                top_k: state.use_custom_params && state.top_k_enabled ? Math.max(1, Math.floor(state.top_k_value)) : null,
                top_p: state.use_custom_params && state.top_p_enabled
                    ? state.top_p_value > 0 && state.top_p_value <= 1 ? state.top_p_value : 0.9
                    : null,
                min_p: state.use_custom_params && state.min_p_enabled
                    ? state.min_p_value > 0 && state.min_p_value <= 1 ? state.min_p_value : 0.05
                    : null,
                repeat_penalty: state.use_custom_params && state.repeat_penalty_enabled ? state.repeat_penalty_value : null,
                max_new_tokens: maxNewTokens,
                seed: seedValue,
                stop_sequences: stopSequences,
                reasoning_parse_enabled: state.reasoning_parse_enabled !== false,
                structured_output_enabled: !!state.structured_output_enabled,
            });

            const { invoke } = await import('@tauri-apps/api/core');
            await invoke('generate_stream', {
                req: {
                    prompt: chatPrompt,
                    messages: toBackendMessages(hist),
                    attachments: attachments.length > 0 ? attachments : null,
                    use_custom_params: state.use_custom_params,
                    temperature: state.use_custom_params && state.temperature_enabled ? state.temperature : null,
                    top_p: state.use_custom_params && state.top_p_enabled
                        ? state.top_p_value > 0 && state.top_p_value <= 1 ? state.top_p_value : 0.9
                        : null,
                    top_k: state.use_custom_params && state.top_k_enabled
                        ? Math.max(1, Math.floor(state.top_k_value))
                        : null,
                    min_p: state.use_custom_params && state.min_p_enabled
                        ? state.min_p_value > 0 && state.min_p_value <= 1 ? state.min_p_value : 0.05
                        : null,
                    repeat_penalty: state.use_custom_params && state.repeat_penalty_enabled ? state.repeat_penalty_value : null,
                    max_new_tokens: maxNewTokens,
                    seed: seedValue,
                    repeat_last_n: 64,
                    stop_sequences: stopSequences.length > 0 ? stopSequences : null,
                    reasoning_parse_enabled: state.reasoning_parse_enabled !== false,
                    reasoning_start_tag: state.reasoning_start_tag || '<think>',
                    reasoning_end_tag: state.reasoning_end_tag || '</think>',
                    structured_output_enabled: !!state.structured_output_enabled,
                    split_prompt: !!state.split_prompt,
                    verbose_prompt: !!state.verbose_prompt,
                    tracing: !!state.tracing,
                    edit_index: editIndex,
                    retrieval: {
                        web: {
                            enabled: state.retrieval_url_enabled,
                            urls: normalizedRetrievalUrls(),
                        },
                        local: {
                            enabled: state.retrieval_local_enabled,
                        },
                    },
                    mcp: {
                        enabled: state.mcp_enabled,
                    },
                },
            });
        } catch (e) {
            const err = String(e ?? 'Unknown error');
            const lastIndex = state.messages.length - 1;
            const last = state.messages[lastIndex];
            if (lastIndex >= 0 && last && last.role === 'assistant' && last.content === '') {
                const nextMessages = state.messages.slice();
                nextMessages[lastIndex] = {
                    ...last,
                    content: `${get(t)('chat.errors.generationFailed')}: ${err}`,
                };
                state.messages = nextMessages;
            }
            try {
                const { message } = await import('@tauri-apps/plugin-dialog');
                await message(err, { title: get(t)('chat.errors.generationFailed'), kind: 'error' });
            } catch { /* ignore */ }
        } finally {
            state.busy = false;
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
            const historyState = get(chatHistory);
            if (historyState.currentSessionId) {
                const msgs = state.messages;
                const last = msgs[msgs.length - 1];
                if (last && last.role === 'assistant' && last.content) {
                    await chatHistory.saveAssistantMessage(
                        historyState.currentSessionId,
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
        if (state.busy) return;

        const isModelLoaded = state.isLoaded;
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
        const msgs = state.messages.slice(0, editIndex + 1);
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
        state.messages = msgs;

        await generateFromHistory(editIndex);
    }

    /**
     * Regenerate the last assistant response.
     * Finds the last user message and regenerates from that point.
     */
    async function handleRegenerate(messageIndex: number) {
        if (state.busy) return;

        const isModelLoaded = state.isLoaded;
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
        if (state.messages[messageIndex]?.role === 'assistant') {
            userIndex = messageIndex - 1;
        }

        if (userIndex < 0 || state.messages[userIndex]?.role !== 'user') {
            console.warn('[regenerate] Could not find user message to regenerate from');
            return;
        }

        const { chatHistory } = await import('$lib/stores/chat-history');
        const historyState = get(chatHistory);

        // Truncate to include the user message, remove the assistant response
        const msgs = state.messages.slice(0, userIndex + 1);

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
        state.messages = msgs;

        await generateFromHistory(userIndex);
    }

    async function pickModel() {
        const { open, message } = await import('@tauri-apps/plugin-dialog');

        if (state.format === 'gguf') {
            const selected = await open({
                multiple: false,
                filters: [{ name: 'GGUF', extensions: ['gguf'] }],
            });
            if (typeof selected === 'string') state.modelPath = selected;
        } else {
            await message(
                'Для загрузки из HF Hub заполните repoId, revision (по желанию) и, для GGUF, имя файла.',
                { title: 'HF Hub', kind: 'info' },
            );
        }
    }

    function destroy() {
        try {
            loadUnlisten?.();
        } catch {
            // ignore
        }
        try {
            modalityUnlisten?.();
        } catch {
            // ignore
        }
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
        resetStreamState: stream.reset,
    };
}
