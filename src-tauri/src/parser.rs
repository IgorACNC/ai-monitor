//! Converte linhas dos `.jsonl` do Claude Code em eventos que o indexador entende.

use chrono::DateTime;
use serde_json::Value;

/// Uso de tokens de uma resposta do modelo.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UsageRecord {
    pub session_id: String,
    pub message_id: String,
    pub request_id: String,
    /// Unix epoch em milissegundos (UTC).
    pub ts: i64,
    pub model: String,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
    pub is_sidechain: bool,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_5m_tokens: i64,
    pub cache_write_1h_tokens: i64,
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Usage(UsageRecord),
    AiTitle {
        session_id: String,
        title: String,
    },
    CustomTitle {
        session_id: String,
        title: String,
    },
    UserPrompt {
        session_id: String,
        ts: i64,
        text: String,
        cwd: Option<String>,
    },
}

const PROMPT_MAX_CHARS: usize = 200;

pub fn parse_line(line: &str) -> Option<Event> {
    let v: Value = serde_json::from_str(line).ok()?;
    let session_id = v.get("sessionId")?.as_str()?.to_string();
    match v.get("type")?.as_str()? {
        "assistant" => parse_assistant(&v, session_id),
        "user" => parse_user(&v, session_id),
        "ai-title" => Some(Event::AiTitle {
            session_id,
            title: str_field(&v, "aiTitle")?,
        }),
        "custom-title" => Some(Event::CustomTitle {
            session_id,
            title: str_field(&v, "customTitle")?,
        }),
        _ => None,
    }
}

fn parse_assistant(v: &Value, session_id: String) -> Option<Event> {
    let msg = v.get("message")?;
    let usage = msg.get("usage")?;
    let model = msg.get("model")?.as_str()?.to_string();
    // Respostas sintéticas (erros locais, interrupções) não consomem tokens reais.
    if model == "<synthetic>" {
        return None;
    }
    let num = |o: &Value, k: &str| o.get(k).and_then(Value::as_i64).unwrap_or(0);

    let cache_write_total = num(usage, "cache_creation_input_tokens");
    let (cache_write_5m_tokens, cache_write_1h_tokens) = match usage.get("cache_creation") {
        Some(cc) => (
            num(cc, "ephemeral_5m_input_tokens"),
            num(cc, "ephemeral_1h_input_tokens"),
        ),
        None => (cache_write_total, 0),
    };

    let tools = msg
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_use"))
                .filter_map(|b| b.get("name").and_then(Value::as_str).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Some(Event::Usage(UsageRecord {
        session_id,
        message_id: msg.get("id")?.as_str()?.to_string(),
        request_id: str_field(v, "requestId").unwrap_or_default(),
        ts: parse_ts(v)?,
        model,
        cwd: str_field(v, "cwd"),
        git_branch: str_field(v, "gitBranch").filter(|b| !b.is_empty()),
        is_sidechain: v
            .get("isSidechain")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        input_tokens: num(usage, "input_tokens"),
        output_tokens: num(usage, "output_tokens"),
        cache_read_tokens: num(usage, "cache_read_input_tokens"),
        cache_write_5m_tokens,
        cache_write_1h_tokens,
        tools,
    }))
}

/// Só interessa o texto digitado pelo usuário: ignora resultados de ferramentas,
/// mensagens de sistema (`isMeta`) e subagentes.
fn parse_user(v: &Value, session_id: String) -> Option<Event> {
    if v.get("isMeta").and_then(Value::as_bool) == Some(true)
        || v.get("isSidechain").and_then(Value::as_bool) == Some(true)
    {
        return None;
    }
    let content = v.get("message")?.get("content")?;
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .find(|b| b.get("type").and_then(Value::as_str) == Some("text"))?
            .get("text")?
            .as_str()?
            .to_string(),
        _ => return None,
    };
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    Some(Event::UserPrompt {
        session_id,
        ts: parse_ts(v)?,
        text: text.chars().take(PROMPT_MAX_CHARS).collect(),
        cwd: str_field(v, "cwd"),
    })
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key)?.as_str().map(String::from)
}

fn parse_ts(v: &Value) -> Option<i64> {
    let s = v.get("timestamp")?.as_str()?;
    Some(DateTime::parse_from_rfc3339(s).ok()?.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASSISTANT: &str = r#"{"parentUuid":"p","isSidechain":false,"message":{"model":"claude-opus-4-7","id":"msg_1","type":"message","role":"assistant","content":[{"type":"text","text":"oi"},{"type":"tool_use","id":"t1","name":"Bash","input":{}}],"usage":{"input_tokens":2,"cache_creation_input_tokens":100,"cache_read_input_tokens":300,"output_tokens":50,"cache_creation":{"ephemeral_1h_input_tokens":100,"ephemeral_5m_input_tokens":0}}},"requestId":"req_1","type":"assistant","uuid":"u","timestamp":"2026-10-21T00:45:01.694Z","cwd":"C:\\proj","sessionId":"s1","gitBranch":"main"}"#;

    #[test]
    fn parses_assistant_usage_and_tools() {
        let Some(Event::Usage(u)) = parse_line(ASSISTANT) else {
            panic!("esperava Usage")
        };
        assert_eq!(u.session_id, "s1");
        assert_eq!(u.message_id, "msg_1");
        assert_eq!(u.request_id, "req_1");
        assert_eq!(u.model, "claude-opus-4-7");
        assert_eq!(u.ts, 1792543501694);
        assert_eq!(
            (u.input_tokens, u.output_tokens, u.cache_read_tokens),
            (2, 50, 300)
        );
        assert_eq!((u.cache_write_5m_tokens, u.cache_write_1h_tokens), (0, 100));
        assert_eq!(u.tools, vec!["Bash"]);
        assert_eq!(u.cwd.as_deref(), Some("C:\\proj"));
        assert_eq!(u.git_branch.as_deref(), Some("main"));
        assert!(!u.is_sidechain);
    }

    #[test]
    fn cache_write_without_breakdown_counts_as_5m() {
        let line = ASSISTANT.replace(
            r#","cache_creation":{"ephemeral_1h_input_tokens":100,"ephemeral_5m_input_tokens":0}"#,
            "",
        );
        let Some(Event::Usage(u)) = parse_line(&line) else {
            panic!()
        };
        assert_eq!((u.cache_write_5m_tokens, u.cache_write_1h_tokens), (100, 0));
    }

    #[test]
    fn ignores_synthetic_messages() {
        let line = ASSISTANT.replace("claude-opus-4-7", "<synthetic>");
        assert_eq!(parse_line(&line), None);
    }

    #[test]
    fn parses_titles() {
        assert_eq!(
            parse_line(r#"{"type":"ai-title","aiTitle":"Título IA","sessionId":"s1"}"#),
            Some(Event::AiTitle {
                session_id: "s1".into(),
                title: "Título IA".into()
            })
        );
        assert_eq!(
            parse_line(r#"{"type":"custom-title","customTitle":"meu","sessionId":"s1"}"#),
            Some(Event::CustomTitle {
                session_id: "s1".into(),
                title: "meu".into()
            })
        );
    }

    #[test]
    fn parses_user_prompt_string_and_blocks() {
        let s = r#"{"type":"user","message":{"role":"user","content":"  faça X  "},"timestamp":"2026-10-21T00:00:00Z","sessionId":"s1","cwd":"C:\\p"}"#;
        assert_eq!(
            parse_line(s),
            Some(Event::UserPrompt {
                session_id: "s1".into(),
                ts: 1792540800000,
                text: "faça X".into(),
                cwd: Some("C:\\p".into())
            })
        );
        let b = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"bloco"}]},"timestamp":"2026-10-21T00:00:00Z","sessionId":"s1"}"#;
        assert!(matches!(parse_line(b), Some(Event::UserPrompt { text, .. }) if text == "bloco"));
    }

    #[test]
    fn ignores_tool_results_meta_and_garbage() {
        let tool_result = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"ok"}]},"timestamp":"2026-10-21T00:00:00Z","sessionId":"s1"}"#;
        let meta = r#"{"type":"user","isMeta":true,"message":{"role":"user","content":"x"},"timestamp":"2026-10-21T00:00:00Z","sessionId":"s1"}"#;
        assert_eq!(parse_line(tool_result), None);
        assert_eq!(parse_line(meta), None);
        assert_eq!(
            parse_line(r#"{"type":"mode","mode":"normal","sessionId":"s1"}"#),
            None
        );
        assert_eq!(parse_line("{quebrado"), None);
    }
}
