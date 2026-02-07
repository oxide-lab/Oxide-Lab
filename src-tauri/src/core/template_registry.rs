use once_cell::sync::Lazy;
use strsim::normalized_levenshtein;

#[derive(Debug, Clone)]
pub struct TemplateEntry {
    pub name: &'static str,
    pub template: &'static str,
    pub stop_tokens: &'static [&'static str],
    /// Если true, то BOS токен должен быть принудительно добавлен, даже если его нет
    pub force_bos: bool,
}

static TEMPLATE_REGISTRY: Lazy<Vec<TemplateEntry>> = Lazy::new(|| {
    // Dynamically load templates from separate files
    crate::core::templates::get_all()
});

/// Найти эталонный шаблон, наиболее похожий на входной.
/// Возвращает None, если совпадения слишком низкого качества.
pub fn match_template(raw_template: &str) -> Option<&'static TemplateEntry> {
    // 1. Агрессивная нормализация: убираем все пробельные символы для сравнения структуры
    let sample_clean: String = raw_template
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();

    if sample_clean.is_empty() {
        return None;
    }

    let mut best_match: Option<&TemplateEntry> = None;
    let mut best_score = 0.0;

    // Порог схожести.
    let threshold = 0.85;

    for entry in TEMPLATE_REGISTRY.iter() {
        // Чистим эталон так же
        let entry_clean: String = entry
            .template
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();

        // Считаем similarity на "сжатых" строках
        let score = normalized_levenshtein(&sample_clean, &entry_clean);

        // Дополнительная эвристика: если шаблон содержит ключевые маркеры
        let markers_score = check_markers(raw_template, entry.name);

        // Бонусная система: маркеры добавляют вес к Levenshtein, а не перекрывают его.
        // Это позволяет отличить qwen3 от chatml, даже если оба имеют <|im_start|>.
        let final_score = (score + markers_score).min(1.0);

        if final_score > best_score {
            best_score = final_score;
            best_match = Some(entry);
        }
    }

    if best_score >= threshold {
        log::info!(
            "Template Fuzzy Match: found '{}' with score {:.2}",
            best_match.unwrap().name,
            best_score
        );
        best_match
    } else {
        log::debug!(
            "Template Fuzzy Match: no match found (best was {:.2})",
            best_score
        );
        None
    }
}

/// Простая эвристика по маркерам, если Levenshtein сбоит из-за форматирования
fn check_markers(content: &str, name: &str) -> f64 {
    match name {
        "llama3" | "llama31" | "llama32" | "llama33" => {
            // Уникальные маркеры Llama 3 — сильный сигнал распознавания
            if content.contains("<|start_header_id|>") && content.contains("<|eot_id|>") {
                return 0.85;
            }
        }
        "qwen3" | "qwen3coder" => {
            if content.contains("<|im_start|>") && content.contains("<|im_end|>") {
                let mut score = 0.5;
                if content.contains("<tool_call>") {
                    score += 0.2;
                }
                if content.contains("<think>") {
                    score += 0.2;
                }
                return score;
            }
        }
        "chatml" => {
            if content.contains("<|im_start|>") && content.contains("<|im_end|>") {
                return 0.4; // Чуть ниже чем qwen3, чтобы qwen3 выигрывал при прочих равных
            }
        }
        "deepseekv3" => {
            // Уникальные full-width маркеры — сильный сигнал распознавания
            if content.contains("<｜User｜>") || content.contains("<｜Assistant｜>") {
                return 0.85;
            }
        }
        "mistral-instruct" => {
            if content.contains("[INST]") && content.contains("[/INST]") {
                return 0.5;
            }
        }
        "gemma-instruct" => {
            if content.contains("<start_of_turn>") && content.contains("<end_of_turn>") {
                return 0.5;
            }
        }
        _ => {}
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_match_llama3() {
        // Симулируем "битый" или слегка отличающийся шаблон из GGUF
        let raw = r"{{- bos_token }}
{%- if custom_tools is defined %}
    {%- set tools = custom_tools %}
{%- endif %}
<|start_header_id|>system<|end_header_id|>

{{ system_message }}<|eot_id|>";

        // Он должен сматчится с одним из Llama 3 вариантов (llama3, llama31, llama32, llama33)
        let matched = match_template(raw);
        assert!(matched.is_some());
        assert!(
            matched.unwrap().name.starts_with("llama3"),
            "Expected llama3* template"
        );
    }

    /*
        #[test]
        fn test_fuzzy_match_mistral() {
            let raw = "{{ bos_token }}{% for message in messages %}[INST] {{ message['content'] }} [/INST]{% endfor %}";
            let matched = match_template(raw);
            assert!(matched.is_some());
            assert_eq!(matched.unwrap().name, "mistral-instruct");
        }
    */

    #[test]
    fn test_no_match_garbage() {
        let raw = "Some random text that is definitely not a template";
        let matched = match_template(raw);
        assert!(matched.is_none());
    }

    #[test]
    fn test_fuzzy_match_qwen3() {
        // Шаблон Qwen3 с маркерами инструментов и размышлений
        let raw = "<|im_start|>system\n<tool_call>\n<think>\n<|im_end|>";
        let matched = match_template(raw);
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().name, "qwen3");
    }

    #[test]
    fn test_fuzzy_match_deepseekv3() {
        let raw = "<｜User｜>Hello<｜Assistant｜>";
        let matched = match_template(raw);
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().name, "deepseekv3");
    }

    #[test]
    fn test_all_templates_syntax() {
        use minijinja::Environment;
        use serde::Serialize;

        #[derive(Serialize)]
        struct Message {
            role: String,
            content: String,
        }

        #[derive(Serialize)]
        struct Context {
            messages: Vec<Message>,
            bos_token: String,
            eos_token: String,
            add_generation_prompt: bool,
        }

        let mut env = Environment::new();
        // Register standard filters if needed, though most are built-in (trim, expected by some templates)
        env.add_function(
            "raise_exception",
            |msg: String| -> Result<String, minijinja::Error> {
                Err(minijinja::Error::new(
                    minijinja::ErrorKind::InvalidOperation,
                    msg,
                ))
            },
        );

        let ctx = Context {
            messages: vec![
                Message {
                    role: "user".into(),
                    content: "Hello".into(),
                },
                Message {
                    role: "assistant".into(),
                    content: "Hi there".into(),
                },
            ],
            bos_token: "<s>".into(),
            eos_token: "</s>".into(),
            add_generation_prompt: true,
        };

        let mut failed = 0;
        for entry in TEMPLATE_REGISTRY.iter() {
            println!("Checking template: {}", entry.name);

            // 1. Check syntax compilation
            let tmpl_res = env.template_from_str(entry.template);
            if let Err(e) = tmpl_res {
                println!("❌ Template '{}' failed to compile: {}", entry.name, e);
                failed += 1;
                continue;
            }

            // 2. Check rendering with dummy context
            let tmpl = tmpl_res.unwrap();
            let render_res = tmpl.render(&ctx);

            if let Err(e) = render_res {
                println!(
                    "⚠️ Template '{}' warning (render failed): {}",
                    entry.name, e
                );
            } else {
                println!("✅ Template '{}' compiled and rendered OK", entry.name);
            }
        }
        assert_eq!(failed, 0, "Some templates failed to compile");
    }
}
