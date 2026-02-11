/**
 * Chat Controller Types
 * 
 * Defines the context interface for the chat controller.
 */

import type { ChatMessage } from '$lib/chat/types';
import type { ChatPersistedState } from '$lib/stores/chat';

export type ChatControllerState = ChatPersistedState & {
    messages: ChatMessage[];
    supports_text: boolean;
    supports_image: boolean;
    supports_audio: boolean;
    supports_video: boolean;
};

export type ChatControllerCtx = {
    state: ChatControllerState;
};
