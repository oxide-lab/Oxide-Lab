use crate::core::prompt::{
    ChatMessage, PromptBuilder, configure_chat_template_environment, normalize_and_validate,
};
use crate::{log_template, log_template_error};
use minijinja::{Environment, Value, context};

pub fn render_prompt(
    chat_template: &Option<String>,
    messages: Vec<super::ChatMsgDto>,
) -> Result<String, String> {
    let tpl = match chat_template {
        Some(s) if !s.trim().is_empty() => s.clone(),
        _ => return Err("chat_template not available".into()),
    };
    let tpl = match normalize_and_validate(&tpl) {
        Ok(n) => n,
        Err(e) => {
            log_template_error!(
                "normalize/validate failed: {}; head=<<<{}>>>; fallback formatter used",
                e,
                tpl.chars().take(180).collect::<String>()
            );
            // Переходим на простой форматтер без попытки рендера
            let chat_messages: Vec<ChatMessage> = messages
                .iter()
                .map(|m| ChatMessage {
                    role: m.role.clone(),
                    content: m.content.clone(),
                })
                .collect();
            let builder = PromptBuilder::new(None);
            return Ok(builder.build_fallback_prompt(chat_messages));
        }
    };

    // Лог на вход
    log_template!("render: msgs={}, tpl_len={}", messages.len(), tpl.len());

    // Конвертируем DTO в общую структуру для возможного fallback
    let chat_messages: Vec<ChatMessage> = messages
        .iter()
        .map(|m| ChatMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();

    // Основной путь: рендер MiniJinja
    let render_result = (|| {
        let mut env = Environment::new();
        configure_chat_template_environment(&mut env);
        env.add_template("tpl", &tpl).map_err(|e| e.to_string())?;
        let tmpl = env.get_template("tpl").map_err(|e| e.to_string())?;

        // minijinja контекст
        let msgs_val: Vec<Value> = chat_messages.iter().map(Value::from_serialize).collect();
        tmpl.render(context! { messages => msgs_val, add_generation_prompt => true, tools => Vec::<String>::new() })
            .map_err(|e| e.to_string())
    })();

    match render_result {
        Ok(rendered) => {
            log_template!(
                "render ok, prefix=<<<{}>>>",
                rendered.chars().take(120).collect::<String>()
            );
            Ok(rendered)
        }
        Err(err) => {
            log_template_error!(
                "render failed: {}; head=<<<{}>>>; falling back to simple formatter",
                err,
                tpl.chars().take(180).collect::<String>()
            );
            let builder = PromptBuilder::new(Some(tpl));
            Ok(builder.build_fallback_prompt(chat_messages))
        }
    }
}
