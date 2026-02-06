use crate::core::template_registry::TemplateEntry;

pub const TEMPLATE: TemplateEntry = TemplateEntry {
    name: "glm4",
    template: r#"{% for message in messages %}{{ '<|im_start|>' + message['role'] + '\n' }}{% if message['content'] is string %}{{ message['content'] }}{% elif message['content'] is iterable %}{% for item in message['content'] %}{% if item['type'] == 'text' %}{{ item['text'] }}{% endif %}{% endfor %}{% endif %}{{ '<|im_end|>\n' }}{% endfor %}{% if add_generation_prompt %}{{ '<|im_start|>assistant\n' }}{% endif %}"#,
    stop_tokens: &["<|im_end|>"],
    force_bos: false,
};

