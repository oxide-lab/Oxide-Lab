/**
 * Chat State Store
 * 
 * Manages the main chat/inference state including model loading, messages, and inference params.
 */

import { writable, type Writable } from 'svelte/store';
import type { ChatMessage } from '$lib/chat/types';

export type Role = 'user' | 'assistant';
export type { ChatMessage };

export type ChatPersistedState = {
    // Model selection
    modelPath: string;
    repoId: string;
    revision: string;
    hubGgufFilename: string;
    mmprojPath: string;
    format: 'gguf' | 'hub_gguf';
    pendingModelPath: string;
    pendingFormat: 'gguf' | 'hub_gguf';

    // Chat state
    prompt: string;
    messages: ChatMessage[];
    busy: boolean;
    isLoaded: boolean;
    errorText: string;

    // Loading / unloading state
    isLoadingModel: boolean;
    loadingProgress: number;
    loadingStage: string; // 'model' | 'tokenizer' | 'complete' | ''
    isCancelling: boolean;
    isUnloadingModel: boolean;
    unloadingProgress: number;

    // Inference params
    temperature: number;
    temperature_enabled: boolean;
    top_k_enabled: boolean;
    top_k_value: number;
    top_p_enabled: boolean;
    top_p_value: number;
    min_p_enabled: boolean;
    min_p_value: number;
    repeat_penalty_enabled: boolean;
    repeat_penalty_value: number;
    max_new_tokens_enabled: boolean;
    max_new_tokens_value: number;
    seed_enabled: boolean;
    seed_value: number;
    stop_sequences_text: string;
    reasoning_parse_enabled: boolean;
    reasoning_start_tag: string;
    reasoning_end_tag: string;
    structured_output_enabled: boolean;
    ctx_limit_value: number;
    use_custom_params: boolean;

    // Device state
    use_gpu: boolean;
    cuda_available: boolean;
    cuda_build: boolean;
    current_device: string;
    // CPU capability flags
    avx: boolean;
    neon: boolean;
    simd128: boolean;
    f16c: boolean;
    // Prompt/control flags
    split_prompt: boolean;
    verbose_prompt: boolean;
    tracing: boolean;
    retrieval_url_enabled: boolean;
    retrieval_urls: string[];
    retrieval_local_enabled: boolean;
    mcp_enabled: boolean;

    // Presets
    preset_id: string | null;
};

export function getDefaultChatState(): ChatPersistedState {
    return {
        modelPath: '',
        repoId: '',
        revision: '',
        hubGgufFilename: '',
        mmprojPath: '',
        format: 'gguf',
        pendingModelPath: '',
        pendingFormat: 'gguf',

        prompt: '',
        messages: [],
        busy: false,
        isLoaded: false,
        errorText: '',

        isLoadingModel: false,
        loadingProgress: 0,
        loadingStage: '',
        isCancelling: false,
        isUnloadingModel: false,
        unloadingProgress: 0,

        temperature: 0.8,
        temperature_enabled: false,
        top_k_enabled: false,
        top_k_value: 40,
        top_p_enabled: false,
        top_p_value: 0.9,
        min_p_enabled: false,
        min_p_value: 0.05,
        repeat_penalty_enabled: false,
        repeat_penalty_value: 1.1,
        max_new_tokens_enabled: false,
        max_new_tokens_value: 1024,
        seed_enabled: false,
        seed_value: 42,
        stop_sequences_text: '',
        reasoning_parse_enabled: true,
        reasoning_start_tag: '<think>',
        reasoning_end_tag: '</think>',
        structured_output_enabled: false,
        ctx_limit_value: 4096,
        use_custom_params: false,

        use_gpu: false,
        cuda_available: false,
        cuda_build: false,
        current_device: 'CPU',
        avx: false,
        neon: false,
        simd128: false,
        f16c: false,
        split_prompt: false,
        verbose_prompt: false,
        tracing: false,
        retrieval_url_enabled: false,
        retrieval_urls: [],
        retrieval_local_enabled: false,
        mcp_enabled: false,

        preset_id: null,
    };
}

export const chatState: Writable<ChatPersistedState> = writable(getDefaultChatState());

// Indicates whether the Chat UI component is currently mounted.
export const chatUiMounted = writable(false);
