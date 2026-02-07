use crate::core::prompt::PromptBuilder;
use crate::core::types::ChatMessage;
use tokenizers::Tokenizer;

#[derive(Debug, Clone)]
pub struct ContextSlice {
    pub encoded_len: usize,
    pub base_context_len: usize,
    pub effective_context_tokens: Vec<u32>,
}

impl ContextSlice {
    pub fn new(full_context_tokens: Vec<u32>, limit: usize) -> Self {
        let encoded_len = full_context_tokens.len();
        let effective_context_tokens = if encoded_len > limit && limit > 0 {
            // Efficient truncation: skip first N tokens, collect rest
            // This is a fallback if smart_truncate fails or wasn't used
            let skip = encoded_len - limit;
            full_context_tokens.into_iter().skip(skip).collect()
        } else {
            // No truncation needed, consume vec directly
            full_context_tokens
        };
        let base_context_len = effective_context_tokens.len();
        Self {
            encoded_len,
            base_context_len,
            effective_context_tokens,
        }
    }
}

/// Truncates the conversation to fit within the context limit.
///
/// Strategy:
/// 1. Always keep System Prompt.
/// 2. Always keep the Last User Message (if possible, otherwise it will be hard-truncated by ContextSlice).
/// 3. Fill the remaining space with history (newest to oldest).
///
/// Optimization:
/// Instead of O(N^2) loop (build+tokenize for every suffix), we estimate
/// token counts per message and find a safe start index, then refine.
pub fn smart_truncate(
    tokenizer: &Tokenizer,
    chat_template: &Option<String>,
    messages: &[ChatMessage],
    bos_token: Option<String>,
    limit: usize,
) -> Result<String, String> {
    if messages.is_empty() {
        return Ok(String::new());
    }

    let builder = PromptBuilder::new(chat_template.clone()).with_bos(bos_token.clone());

    // Connect core::types::ChatMessage to core::prompt::ChatMessage
    // They are identical in structure, but distinct types.
    let map_msg = |m: &ChatMessage| crate::core::prompt::ChatMessage {
        role: m.role.clone(),
        content: m.content.clone(),
    };

    // 1. Separate System and Other messages
    let system_msgs: Vec<crate::core::prompt::ChatMessage> = messages
        .iter()
        .filter(|m| m.role == "system")
        .map(map_msg)
        .collect();

    let other_msgs: Vec<crate::core::types::ChatMessage> = messages
        .iter()
        .filter(|m| m.role != "system")
        .cloned()
        .collect();

    // If no other messages, just return system prompt (or empty)
    if other_msgs.is_empty() {
        return builder.render_prompt(system_msgs);
    }

    // 2. Measure System Prompt Overhead
    // We build a prompt with JUST system messages to see how much it takes.
    // Note: Some templates might add BOS/Extra for empty user msgs, but this is a good baseline.
    let sys_prompt_str = builder.render_prompt(system_msgs.clone())?;
    let sys_tokens = tokenizer
        .encode(sys_prompt_str.clone(), true)
        .map_err(|e| e.to_string())?
        .get_ids()
        .len();

    if sys_tokens >= limit {
        // Edge case: System prompt alone exceeds limit.
        // We return it anyway; ContextSlice calls later will handle hard clip.
        return Ok(sys_prompt_str); // Or maybe standard prompt building
    }

    let remaining_budget = limit.saturating_sub(sys_tokens);

    // 3. Estimate "Other" messages length from BACK to FRONT
    // We count raw tokens + safety margin (for template tags like <|im_start|>)
    // Typical template overhead per msg is ~5-10 tokens. Let's say 20 for safety.
    const TEMPLATE_OVERHEAD_PER_MSG: usize = 20;

    let mut used_tokens = 0;
    let mut include_count = 0;
    let n = other_msgs.len();

    // Always include the last message at least (if we can)
    // We iterate backwards
    for (i, msg) in other_msgs.iter().rev().enumerate() {
        // Approximate token count: encode just the content
        // (This ignores template structure but is fast substitute)
        let content_len = tokenizer
            .encode(msg.content.clone(), false) // false = no special tokens added yet
            .map(|t| t.len())
            .unwrap_or(0);

        let estimated_cost = content_len + TEMPLATE_OVERHEAD_PER_MSG;

        if i == 0 {
            // Always try to include the very last message (which is first in rev iter)
            used_tokens += estimated_cost;
            include_count += 1;
        } else {
            // For historical messages, check budget
            if used_tokens + estimated_cost > remaining_budget {
                // Stop adding history
                break;
            }
            used_tokens += estimated_cost;
            include_count += 1;
        }
    }

    // include_count is how many messages from the END we want to keep.
    // So we skip (n - include_count).
    let start_index = n.saturating_sub(include_count);

    // 4. Refine (Double Check)
    // Because our estimate was loose, we might be slightly over or under.
    // Since we want to fit as much as possible, we can try to add one more if we were conservative,
    // or remove one if we were optimistic.
    // However, safest is to verify "start_index". If it fits, try "start_index - 1" provided it fits.
    // If "start_index" doesn't fit, try "start_index + 1".

    // Let's optimize: Verify safe start_index.
    // If it's too big, shrink (increment index).
    // If it fits and start_index > 0, maybe try expanding? (Skip for now to prefer speed/safety)

    let mut current_start = start_index;

    loop {
        // Construct candidate
        let subset_others = &other_msgs[current_start..];
        let mut candidate_msgs = system_msgs.clone();
        let mapped_subset: Vec<crate::core::prompt::ChatMessage> =
            subset_others.iter().map(map_msg).collect();
        candidate_msgs.extend(mapped_subset);

        let p = builder.render_prompt(candidate_msgs)?;
        let encoded_len = tokenizer
            .encode(p.clone(), true)
            .map_err(|e| e.to_string())?
            .len();

        if encoded_len <= limit {
            // Fits!
            return Ok(p);
        } else {
            // Too big. Drop one more oldest message.
            if current_start < n - 1 {
                current_start += 1;
            } else {
                // If we are at the last message and it still doesn't fit...
                // We must return it (System + Last) and let hard truncation handle it.
                return Ok(p);
            }
        }
    }
}
