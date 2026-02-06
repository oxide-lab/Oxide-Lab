use oxide_lib::core::types::LlamaSessionKind;
use oxide_lib::inference::llamacpp::state::SessionKey;
use std::collections::HashSet;

#[test]
fn session_keys_are_isolated_by_kind() {
    let chat = SessionKey {
        model_id: "m".to_string(),
        kind: LlamaSessionKind::Chat,
    };
    let emb = SessionKey {
        model_id: "m".to_string(),
        kind: LlamaSessionKind::Embedding,
    };

    let mut set = HashSet::new();
    set.insert(chat);
    set.insert(emb);
    assert_eq!(set.len(), 2);
}
