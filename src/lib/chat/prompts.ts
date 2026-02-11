/**
 * Chat Prompt Builder
 * 
 * Builds prompts with chat template support from the backend.
 */

import { sanitizeForPrompt } from '$lib/chat/sanitize';
import type { ChatMessage } from '$lib/chat/types';
import { invoke } from '@tauri-apps/api/core';

/**
 * Build prompt with chat template from backend if available.
 * Supports /think and /no_think control commands.
 */
export async function buildPromptWithChatTemplate(history: ChatMessage[]): Promise<string> {
    // Detect control prefix (/think or /no_think) from last user message
    let control: 'think' | 'no_think' | null = null;
    const lastUser = [...history].reverse().find((m) => m.role === 'user');
    if (lastUser) {
        const t = lastUser.content.trim();
        if (/^\s*\/no_think\b/i.test(t)) control = 'no_think';
        else if (/^\s*\/think\b/i.test(t)) control = 'think';
    }

    // TODO: Integrate with Tauri backend
    // Command: invoke('get_chat_template')
    // Command: invoke('render_prompt', { messages: hist })

    let tpl: string | null = null;
    try {
        tpl = await invoke<string | null>('get_chat_template').catch(() => null);
    } catch {
        tpl = null;
    }

    console.log('[template] requested from backend, present=', !!tpl);

    if (tpl && typeof tpl === 'string' && tpl.trim().length > 0) {
        // Render via backend using minijinja for native template support
        const hist = history.map((m) => ({ role: m.role, content: sanitizeForPrompt(m.content) }));
        try {
            let out = await invoke<string>('render_prompt', { messages: hist });
            console.log('[template] applied (backend render), prefix=', out.slice(0, 160));

            // Add empty think block if /no_think and not present in template
            if (control === 'no_think' && !(out.includes('<think>') && out.includes('</think>'))) {
                out = out + '\n<think>\n\n</think>\n\n';
            }
            return out;
        } catch (e) {
            console.warn('Template render failed, using fallback:', e);
        }
    }

    // Fallback: model-agnostic transcript format.
    // Avoid model-specific tokens when backend template is unavailable.
    let text = '';

    for (const m of history) {
        const clean = sanitizeForPrompt(m.content);
        if (m.role === 'user') {
            let payload = clean;
            // Remove only control command prefix, preserve rest of user text
            payload = payload.replace(/^\s*\/(?:no_think|think)\b[ \t]*/i, '').trim();
            text += `User: ${payload}\n`;
        } else {
            text += `Assistant: ${clean}\n`;
        }
    }

    text += 'Assistant: ';

    // Official way to disable thinking - insert empty <think>...</think> block
    if (control === 'no_think') {
        text += `<think>\n\n</think>\n\n`;
    }

    return text;
}
