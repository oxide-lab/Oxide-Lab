/**
 * Chat message types
 */

export type Role = 'user' | 'assistant';

export type RetrievalUrlCandidateEvent = {
    urls?: string[];
};

export type McpPermissionDecision = 'allow_once' | 'allow_this_session' | 'allow_this_server' | 'deny';

export type McpToolPermissionRequestEvent = {
    request_id: string;
    server_id: string;
    tool_name: string;
    arguments?: Record<string, unknown> | null;
};

export type McpToolCallStartedEvent = {
    call_id: string;
    server_id: string;
    tool_name: string;
    arguments?: Record<string, unknown> | null;
};

export type McpToolCallFinishedEvent = {
    call_id: string;
    server_id: string;
    tool_name: string;
    result?: unknown;
};

export type McpToolCallErrorEvent = {
    call_id: string;
    server_id: string;
    tool_name: string;
    error?: string;
};

export type McpToolUiState =
    | 'input-streaming'
    | 'input-available'
    | 'output-available'
    | 'output-error';

export type McpToolCallView = {
    call_id: string;
    server_id: string;
    tool_name: string;
    state: McpToolUiState;
    input?: Record<string, unknown> | null;
    output?: unknown;
    errorText?: string;
};

export type RetrievalSource = {
    source_type: string;
    title: string;
    url?: string | null;
    path?: string | null;
    snippet: string;
    score?: number | null;
};

/**
 * File attachment for messages
 */
export type Attachment = {
    filename: string;
    data: string;  // base64 encoded data
    mimeType: string;
    path?: string;
    kind?: string;
    size?: number;
};

/**
 * Utility to check if file is an image
 */
export function isImageFile(filename: string): boolean {
    const imageExtensions = ['.jpg', '.jpeg', '.png', '.gif', '.webp', '.svg', '.bmp', '.ico'];
    const lowerFilename = filename.toLowerCase();
    return imageExtensions.some(ext => lowerFilename.endsWith(ext));
}

/**
 * Utility to check if mimetype is an image
 */
export function isImageMimeType(mimeType: string): boolean {
    return mimeType.startsWith('image/');
}

export type ChatMessage = {
    role: Role;
    content: string;
    html?: string;
    thinking?: string;
    isThinking?: boolean;
    attachments?: Attachment[];
    sources?: RetrievalSource[];
    retrievalWarnings?: string[];
    mcpToolCalls?: McpToolCallView[];
};
