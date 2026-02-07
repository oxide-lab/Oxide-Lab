use crate::core::template_registry::TemplateEntry;

pub const TEMPLATE: TemplateEntry = TemplateEntry {
    name: "phi3",
    template: r#"{% for message in messages %}{% if message['role'] == 'system' and message['content'] %}{{'<|system|>
' + message['content'] + '<|end|>
'}}{% elif message['role'] == 'user' %}{{'<|user|>
' + message['content'] + '<|end|>
'}}{% elif message['role'] == 'assistant' %}{{'<|assistant|>
' + message['content'] + '<|end|>
'}}{% endif %}{% endfor %}{% if add_generation_prompt %}{{ '<|assistant|>
' }}{% else %}{{ eos_token }}{% endif %}"#,
    stop_tokens: &["<|end|>", "<|system|>", "<|user|>", "<|assistant|>"],
    force_bos: false,
};
