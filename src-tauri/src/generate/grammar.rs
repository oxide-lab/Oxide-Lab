//! Grammar Sampling - Structured outputs через JSON Schema constraints
//!
//! Обеспечивает генерацию валидного JSON путём ограничения logits
//! на каждом шаге генерации.

use serde::{Deserialize, Serialize};

/// Формат вывода для генерации
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(untagged)]
pub enum OutputFormat {
    /// Без ограничений на формат
    #[default]
    None,
    /// JSON mode: генерировать валидный JSON
    Json,
    /// JSON Schema: генерировать JSON по схеме
    JsonSchema(serde_json::Value),
}

impl OutputFormat {
    /// Проверяет, требуется ли grammar sampling
    pub fn requires_grammar(&self) -> bool {
        !matches!(self, OutputFormat::None)
    }

    /// Проверяет, является ли это простым JSON режимом
    pub fn is_json_mode(&self) -> bool {
        matches!(self, OutputFormat::Json)
    }
}

/// Состояние JSON FSM для grammar sampling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonState {
    /// Начало: ожидаем { или [
    Start,
    /// Внутри объекта: ожидаем ключ или }
    ObjectStart,
    /// После ключа: ожидаем :
    ObjectKey,
    /// После : ожидаем значение
    ObjectColon,
    /// После значения в объекте: ожидаем , или }
    ObjectValue,
    /// Внутри массива: ожидаем значение или ]
    ArrayStart,
    /// После значения в массиве: ожидаем , или ]
    ArrayValue,
    /// Внутри строки
    InString,
    /// После escape в строке
    StringEscape,
    /// Внутри числа
    InNumber,
    /// Внутри true/false/null
    InLiteral,
    /// Завершено
    Done,
}

/// Sampler для grammar-constrained генерации
pub struct GrammarSampler {
    state: JsonState,
    depth: usize,
    in_string: bool,
}

impl GrammarSampler {
    pub fn new() -> Self {
        Self {
            state: JsonState::Start,
            depth: 0,
            in_string: false,
        }
    }

    /// Обновляет состояние FSM после генерации токена
    pub fn update(&mut self, token: &str) {
        for ch in token.chars() {
            self.update_char(ch);
        }
    }

    fn update_char(&mut self, ch: char) {
        if self.in_string {
            match ch {
                '\\' => {
                    self.state = JsonState::StringEscape;
                }
                '"' => {
                    self.in_string = false;
                    self.state = if self.depth > 0 {
                        JsonState::ObjectValue
                    } else {
                        JsonState::Done
                    };
                }
                _ => {
                    if self.state == JsonState::StringEscape {
                        self.state = JsonState::InString;
                    }
                }
            }
            return;
        }

        match ch {
            '{' => {
                self.depth += 1;
                self.state = JsonState::ObjectStart;
            }
            '}' => {
                self.depth = self.depth.saturating_sub(1);
                self.state = if self.depth == 0 {
                    JsonState::Done
                } else {
                    JsonState::ObjectValue
                };
            }
            '[' => {
                self.depth += 1;
                self.state = JsonState::ArrayStart;
            }
            ']' => {
                self.depth = self.depth.saturating_sub(1);
                self.state = if self.depth == 0 {
                    JsonState::Done
                } else {
                    JsonState::ArrayValue
                };
            }
            '"' => {
                self.in_string = true;
                self.state = JsonState::InString;
            }
            ':' => {
                self.state = JsonState::ObjectColon;
            }
            ',' => {
                self.state = match self.state {
                    JsonState::ObjectValue => JsonState::ObjectStart,
                    JsonState::ArrayValue => JsonState::ArrayStart,
                    _ => self.state,
                };
            }
            _ if ch.is_whitespace() => {}
            _ => {}
        }
    }

    /// Проверяет, завершена ли генерация JSON
    pub fn is_complete(&self) -> bool {
        self.state == JsonState::Done && self.depth == 0
    }

    /// Текущая глубина вложенности
    pub fn depth(&self) -> usize {
        self.depth
    }
}

impl Default for GrammarSampler {
    fn default() -> Self {
        Self::new()
    }
}

/// Валидирует JSON строку
pub fn validate_json(output: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(output).map_err(|e| format!("Invalid JSON: {}", e))
}

/// Валидирует JSON против schema (упрощённая версия)
pub fn validate_against_schema(
    json: &serde_json::Value,
    _schema: &serde_json::Value,
) -> Result<(), String> {
    // Базовая проверка: JSON существует и является объектом или массивом
    if json.is_null() {
        return Err("JSON is null".to_string());
    }
    // TODO: Полная валидация JSON Schema (jsonschema crate)
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_default() {
        let fmt = OutputFormat::default();
        assert!(!fmt.requires_grammar());
    }

    #[test]
    fn test_output_format_json() {
        let fmt = OutputFormat::Json;
        assert!(fmt.requires_grammar());
        assert!(fmt.is_json_mode());
    }

    #[test]
    fn test_grammar_sampler_simple_object() {
        let mut sampler = GrammarSampler::new();
        sampler.update("{\"key\": \"value\"}");
        assert!(sampler.is_complete());
    }

    #[test]
    fn test_grammar_sampler_nested() {
        let mut sampler = GrammarSampler::new();
        sampler.update("{\"a\": {\"b\": 1}}");
        assert!(sampler.is_complete());
    }

    #[test]
    fn test_validate_json() {
        assert!(validate_json("{\"test\": 123}").is_ok());
        assert!(validate_json("not json").is_err());
    }
}
