/**
 * Chat Utilities
 */

/**
 * Removes thinking tags from the text
 */
export function cleanThinking(text: string): string {
    return text.replace(/<\/?think>/gi, '').trim();
}

/**
 * Splits inline thinking from content if present
 */
export function splitInlineThinking(content: string): { thinking: string; content: string } {
    const source = content ?? '';
    const closeTag = '</think>';
    const openTag = '<think>';
    const lower = source.toLowerCase();
    const openIdx = lower.indexOf(openTag);
    const closeIdx = lower.indexOf(closeTag);

    if (openIdx >= 0 && closeIdx > openIdx) {
        const thinking = source.slice(openIdx + openTag.length, closeIdx).trim();
        const contentWithoutThinking = `${source.slice(0, openIdx)}${source.slice(closeIdx + closeTag.length)}`.trimStart();
        return { thinking, content: contentWithoutThinking };
    }

    if (openIdx < 0 && closeIdx >= 0) {
        const thinking = source.slice(0, closeIdx).trim();
        const contentWithoutThinking = source.slice(closeIdx + closeTag.length).trimStart();
        return { thinking, content: contentWithoutThinking };
    }

    return { thinking: '', content: source };
}
