/**
 * Chat message types
 */

export type Role = 'user' | 'assistant';

export type RetrievalWebMode = 'off' | 'lite' | 'pro';

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
};
