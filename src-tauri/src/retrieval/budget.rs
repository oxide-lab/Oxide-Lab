use crate::core::types::ChatMessage;
use crate::retrieval::types::RetrievalCandidate;

const RESERVED_GENERATION_TOKENS: usize = 512;
const APPROX_CHARS_PER_TOKEN: usize = 4;

fn estimate_tokens(text: &str) -> usize {
    let len = text.trim().chars().count();
    if len == 0 {
        0
    } else {
        len.div_ceil(APPROX_CHARS_PER_TOKEN)
    }
}

pub fn estimate_message_tokens(messages: &[ChatMessage]) -> usize {
    messages
        .iter()
        .map(|m| estimate_tokens(&m.content) + estimate_tokens(&m.role))
        .sum()
}

pub fn trim_by_budget(
    candidates: &[RetrievalCandidate],
    max_tokens: usize,
) -> (Vec<RetrievalCandidate>, bool) {
    if max_tokens == 0 {
        return (Vec::new(), !candidates.is_empty());
    }

    let mut out = Vec::new();
    let mut used = 0usize;
    let mut trimmed = false;
    for candidate in candidates {
        if used + candidate.estimated_tokens > max_tokens {
            trimmed = true;
            continue;
        }
        used += candidate.estimated_tokens;
        out.push(candidate.clone());
    }
    (out, trimmed)
}

pub fn compute_retrieval_budget(
    ctx_size: usize,
    history_messages: &[ChatMessage],
    reserve_tokens: Option<usize>,
) -> usize {
    let reserve = reserve_tokens.unwrap_or(RESERVED_GENERATION_TOKENS);
    let history_tokens = estimate_message_tokens(history_messages);
    ctx_size.saturating_sub(history_tokens.saturating_add(reserve))
}

pub fn trim_history_oldest_first(messages: &mut Vec<ChatMessage>, target_ctx_size: usize) -> bool {
    let mut trimmed = false;
    while estimate_message_tokens(messages) > target_ctx_size && !messages.is_empty() {
        let drop_index = messages
            .iter()
            .position(|m| m.role == "user" || m.role == "assistant")
            .unwrap_or(0);
        messages.remove(drop_index);
        trimmed = true;
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_decreases_when_history_grows() {
        let short = vec![ChatMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
        }];
        let long = vec![ChatMessage {
            role: "user".to_string(),
            content: "a".repeat(4000),
        }];
        let s = compute_retrieval_budget(4096, &short, Some(512));
        let l = compute_retrieval_budget(4096, &long, Some(512));
        assert!(s > l);
    }
}
