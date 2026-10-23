//! Uso do Codex (OpenAI), lido do histórico que ele grava em `~/.codex/sessions/AAAA/MM/DD/*.jsonl`
//! (ou em `$CODEX_HOME`), só para leitura. Cada resposta grava um `token_count` com o contexto
//! e as % do plano (janelas de 5 h e semanal), como a statusline do Claude Code.
//! Arquivos com mais de 7 dias o Codex compacta (`.jsonl.zst`); esses ficam de fora.

use std::collections::HashMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::agents::{ACTIVE_WINDOW_MS, AgentSession, AgentUsage, DAY_MS, FileCache, Reply, files_in, iso_ms};
use crate::statusline::{LimitWindow, Limits};

pub fn codex_home(home: &Path) -> PathBuf {
    match std::env::var_os("CODEX_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => home.join(".codex"),
    }
}

pub fn sessions_dir(home: &Path) -> PathBuf {
    codex_home(home).join("sessions")
}

/// Uma vez (pedido do usuário) dentro de uma conversa.
struct Turn {
    started: i64,
    model: Option<String>,
}

/// O que interessa de um arquivo de conversa (uma "thread"; subagentes têm arquivo próprio).
pub struct Rollout {
    id: String,
    /// Conversa principal; igual a `id` fora de subagentes.
    root: String,
    cwd: String,
    started_at: i64,
    first_prompt: Option<String>,
    turns: Vec<Turn>,
    last_activity: i64,
    busy: bool,
    context: Option<(i64, Option<i64>)>,
    /// Últimas % do plano gravadas neste arquivo, com o horário.
    rate_limits: Option<(i64, Value)>,
}

fn parse(path: &Path) -> Option<Rollout> {
    let file = std::fs::File::open(path).ok()?;
    let mut r = Rollout {
        id: String::new(),
        root: String::new(),
        cwd: String::new(),
        started_at: 0,
        first_prompt: None,
        turns: vec![],
        last_activity: 0,
        busy: false,
        context: None,
        rate_limits: None,
    };
    let mut model: Option<String> = None;
    for line in std::io::BufReader::new(file).lines() {
        let Ok(line) = line else { break };
        // As respostas em si são a maior parte do arquivo e não interessam aqui.
        if line.get(..120).unwrap_or(&line).contains(r#""type":"response_item""#) {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        let Some(ts) = iso_ms(v.get("timestamp")) else { continue };
        r.last_activity = r.last_activity.max(ts);
        let p = v.get("payload").unwrap_or(&Value::Null);
        let text = |k: &str| p.get(k).and_then(Value::as_str).map(str::to_string);
        match v.get("type").and_then(Value::as_str) {
            Some("session_meta") if r.id.is_empty() => {
                r.id = text("id").unwrap_or_default();
                r.root = text("session_id").unwrap_or_else(|| r.id.clone());
                r.cwd = text("cwd").unwrap_or_default();
                r.started_at = iso_ms(p.get("timestamp")).unwrap_or(ts);
            }
            Some("turn_context") => {
                model = text("model");
                if let Some(t) = r.turns.last_mut().filter(|t| t.model.is_none()) {
                    t.model = model.clone();
                }
            }
            Some("event_msg") => match p.get("type").and_then(Value::as_str) {
                Some("task_started" | "turn_started") => {
                    r.turns.push(Turn { started: ts, model: model.clone() });
                    r.busy = true;
                }
                Some("task_complete" | "turn_complete" | "turn_aborted") => r.busy = false,
                Some("user_message") if r.first_prompt.is_none() => r.first_prompt = text("message"),
                Some("token_count") => {
                    if let Some(info) = p.get("info").filter(|i| !i.is_null()) {
                        let used = info.pointer("/last_token_usage/total_tokens").and_then(Value::as_i64).unwrap_or(0);
                        r.context = Some((used, info.get("model_context_window").and_then(Value::as_i64)));
                    }
                    let limits = p.get("rate_limits").filter(|l| !l.is_null());
                    // Há limites à parte para alguns modelos; o do plano não tem id ou é "codex".
                    if let Some(l) = limits.filter(|l| l.get("limit_id").and_then(Value::as_str).is_none_or(|id| id == "codex")) {
                        r.rate_limits = Some((ts, l.clone()));
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    (!r.id.is_empty()).then_some(r)
}

/// "Sessão de 5 h", "Semana"... pelo tamanho da janela.
fn window_label(minutes: Option<i64>) -> String {
    match minutes {
        Some(300) => "Sessão de 5 h".into(),
        Some(10080) => "Semana".into(),
        Some(m) if m % 1440 == 0 => format!("Janela de {} dias", m / 1440),
        Some(m) if m % 60 == 0 => format!("Janela de {} h", m / 60),
        Some(m) => format!("Janela de {m} min"),
        None => "Limite".into(),
    }
}

/// Converte as % gravadas pelo Codex. Janela que já reiniciou desde então volta a 0%.
fn limits(captured_at: i64, v: &Value, now: i64) -> Limits {
    let windows = ["primary", "secondary"]
        .into_iter()
        .filter_map(|id| {
            let w = v.get(id).filter(|w| !w.is_null())?;
            let resets_at = w.get("resets_at").and_then(Value::as_i64).map(|s| s * 1000);
            let passed = resets_at.is_some_and(|r| r <= now);
            Some(LimitWindow {
                id: id.to_string(),
                label: window_label(w.get("window_minutes").and_then(Value::as_i64)),
                used_percentage: if passed { 0.0 } else { w.get("used_percent")?.as_f64()? },
                resets_at: if passed { None } else { resets_at },
            })
        })
        .collect();
    Limits { captured_at, windows }
}

/// Título: o nome dado à conversa no Codex (`session_index.jsonl`) ou o primeiro pedido.
fn thread_names(codex_home: &Path) -> HashMap<String, String> {
    let Ok(text) = std::fs::read_to_string(codex_home.join("session_index.jsonl")) else { return HashMap::new() };
    // O nome mais recente de cada conversa é o que vale.
    text.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|v| Some((v.get("id")?.as_str()?.to_string(), v.get("thread_name")?.as_str()?.to_string())))
        .collect()
}

fn short(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    match line.char_indices().nth(80) {
        Some((i, _)) => format!("{}…", &line[..i]),
        None => line.to_string(),
    }
}

pub fn read(rollouts: &[&Rollout], names: &HashMap<String, String>, now: i64, today_start: i64) -> AgentUsage {
    let week_start = now - 7 * DAY_MS;
    let replies = |since: i64| {
        crate::agents::period(rollouts.iter().flat_map(|r| {
            r.turns
                .iter()
                .filter(move |t| t.started >= since)
                .map(|t| Reply { session_id: &r.root, model: t.model.as_deref(), cost: None })
        }))
    };

    let mut groups: HashMap<&str, Vec<&Rollout>> = HashMap::new();
    for r in rollouts {
        groups.entry(&r.root).or_default().push(r);
    }
    let mut active: Vec<AgentSession> = groups
        .into_iter()
        .filter_map(|(root, list)| {
            let last_activity = list.iter().map(|r| r.last_activity).max()?;
            if now - last_activity > ACTIVE_WINDOW_MS {
                return None;
            }
            // Sem o arquivo da conversa principal (subagente sozinho), usa o mais antigo.
            let main = list.iter().find(|r| r.id == root).or_else(|| list.iter().min_by_key(|r| r.started_at))?;
            let model = main.turns.last().and_then(|t| t.model.clone());
            Some(AgentSession {
                id: root.to_string(),
                title: names
                    .get(root)
                    .cloned()
                    .or_else(|| main.first_prompt.as_deref().map(short))
                    .unwrap_or_default(),
                directory: main.cwd.clone(),
                model,
                busy: list.iter().any(|r| r.busy),
                started_at: main.started_at,
                last_activity,
                responses: list.iter().map(|r| r.turns.len() as i64).sum(),
                cost: None,
                context_tokens: main.context.map_or(0, |c| c.0),
                context_limit: main.context.and_then(|c| c.1),
            })
        })
        .collect();
    active.sort_by_key(|s| std::cmp::Reverse(s.last_activity));

    let limits = rollouts
        .iter()
        .filter_map(|r| r.rate_limits.as_ref())
        .max_by_key(|(ts, _)| *ts)
        .map(|(ts, v)| limits(*ts, v, now));

    AgentUsage {
        installed: true,
        active,
        today: replies(today_start),
        week: replies(week_start),
        limits,
        error: None,
    }
}

pub fn usage(home: &Path, cache: &FileCache<Rollout>, now: i64, today_start: i64) -> AgentUsage {
    let dir = sessions_dir(home);
    if !dir.is_dir() {
        return AgentUsage::default();
    }
    let since = (now - 7 * DAY_MS).min(today_start);
    let mut paths = vec![];
    files_in(&dir, "jsonl", 3, &mut paths);
    // Só conversas mexidas no período; as % do plano vêm da mais recente delas.
    paths.retain(|p| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .is_ok_and(|t| chrono::DateTime::<chrono::Utc>::from(t).timestamp_millis() >= since)
    });
    let rollouts = cache.read_all(paths, parse);
    let refs: Vec<&Rollout> = rollouts.iter().map(|r| r.as_ref()).collect();
    read(&refs, &thread_names(&codex_home(home)), now, today_start)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000_000;
    const TODAY: i64 = NOW - 3 * 60 * 60 * 1000;

    fn iso(ms: i64) -> String {
        chrono::DateTime::from_timestamp_millis(ms).unwrap().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    }

    fn line(ms: i64, kind: &str, payload: &str) -> String {
        format!(r#"{{"timestamp":"{}","type":"{kind}","payload":{payload}}}"#, iso(ms)) + "\n"
    }

    fn token_count(used: i64, five_hour: f64, resets_s: i64) -> String {
        format!(
            r#"{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1,"reasoning_output_tokens":0,"total_tokens":2}},
            "last_token_usage":{{"input_tokens":{used},"cached_input_tokens":0,"output_tokens":0,"reasoning_output_tokens":0,"total_tokens":{used}}},"model_context_window":200000}},
            "rate_limits":{{"limit_id":"codex","primary":{{"used_percent":{five_hour},"window_minutes":300,"resets_at":{resets_s}}},
            "secondary":{{"used_percent":40.0,"window_minutes":10080,"resets_at":{}}},"credits":null,"plan_type":"plus"}}}}"#,
            NOW / 1000 + 86_400
        )
        .replace('\n', "")
    }

    /// Conversa ativa (com um subagente) hoje e outra de 2 dias atrás.
    fn fixture() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let day = tmp.path().join(".codex/sessions/2027/01/15");
        std::fs::create_dir_all(&day).unwrap();
        let t = NOW - 30 * 60_000;
        let mut main = line(t, "session_meta", r#"{"id":"main","timestamp":"2027-01-15T10:00:00Z","cwd":"A:/proj","cli_version":"1"}"#);
        main += &line(t, "event_msg", r#"{"type":"task_started","turn_id":"1"}"#);
        main += &line(t, "turn_context", r#"{"cwd":"A:/proj","model":"gpt-5.5"}"#);
        main += &line(t, "event_msg", r#"{"type":"user_message","message":"Corrija o login\ncom cuidado"}"#);
        main += &line(t + 1000, "response_item", r#"{"type":"message","role":"assistant","content":[]}"#);
        main += &line(t + 2000, "event_msg", &token_count(1000, 10.0, NOW / 1000 + 3600));
        main += &line(t + 3000, "event_msg", r#"{"type":"task_complete","turn_id":"1"}"#);
        main += &line(NOW - 60_000, "event_msg", r#"{"type":"task_started","turn_id":"2"}"#);
        main += &line(NOW - 60_000, "turn_context", r#"{"cwd":"A:/proj","model":"gpt-5.5"}"#);
        main += &line(NOW - 50_000, "event_msg", &token_count(50_000, 25.5, NOW / 1000 + 3600));
        std::fs::write(day.join("rollout-a-main.jsonl"), main).unwrap();

        let mut sub = line(NOW - 40_000, "session_meta", r#"{"id":"sub","session_id":"main","parent_thread_id":"main","timestamp":"2027-01-15T10:00:00Z","cwd":"A:/proj"}"#);
        sub += &line(NOW - 40_000, "event_msg", r#"{"type":"task_started","turn_id":"s"}"#);
        sub += &line(NOW - 40_000, "turn_context", r#"{"cwd":"A:/proj","model":"gpt-5.5-mini"}"#);
        sub += &line(NOW - 30_000, "event_msg", r#"{"type":"task_complete","turn_id":"s"}"#);
        std::fs::write(day.join("rollout-b-sub.jsonl"), sub).unwrap();

        let old = NOW - 2 * DAY_MS;
        let mut prev = line(old, "session_meta", r#"{"id":"old","timestamp":"2027-01-13T10:00:00Z","cwd":"A:/outro"}"#);
        prev += &line(old, "event_msg", r#"{"type":"task_started","turn_id":"1"}"#);
        prev += &line(old, "turn_context", r#"{"cwd":"A:/outro","model":"o3"}"#);
        // % mais antigas que as de hoje: não podem ganhar.
        prev += &line(old + 1, "event_msg", &token_count(10, 99.0, NOW / 1000 + 3600));
        std::fs::write(day.join("rollout-c-old.jsonl"), prev).unwrap();
        tmp
    }

    fn read_fixture(tmp: &Path) -> AgentUsage {
        let mut paths = vec![];
        files_in(&tmp.join(".codex/sessions"), "jsonl", 3, &mut paths);
        let parsed: Vec<Rollout> = paths.iter().filter_map(|p| parse(p)).collect();
        let refs: Vec<&Rollout> = parsed.iter().collect();
        let names = HashMap::from([("old".to_string(), "Nome dado".to_string())]);
        read(&refs, &names, NOW, TODAY)
    }

    #[test]
    fn reads_active_session_with_context_and_subagent() {
        let tmp = fixture();
        let u = read_fixture(tmp.path());
        assert_eq!(u.active.len(), 1, "a conversa de 2 dias atrás não está ativa");
        let s = &u.active[0];
        assert_eq!((s.id.as_str(), s.title.as_str(), s.directory.as_str()), ("main", "Corrija o login", "A:/proj"));
        assert!(s.busy, "a segunda vez ainda não terminou");
        assert_eq!(s.responses, 3, "inclui a do subagente");
        assert_eq!((s.context_tokens, s.context_limit), (50_000, Some(200_000)));
        assert_eq!(s.model.as_deref(), Some("gpt-5.5"));
        assert_eq!(s.cost, None);
    }

    #[test]
    fn sums_today_and_week() {
        let tmp = fixture();
        let u = read_fixture(tmp.path());
        assert_eq!((u.today.sessions, u.today.responses), (1, 3));
        assert_eq!(u.today.top_model.as_deref(), Some("gpt-5.5"));
        assert_eq!((u.week.sessions, u.week.responses), (2, 4));
    }

    #[test]
    fn plan_limits_come_from_the_newest_record() {
        let tmp = fixture();
        let l = read_fixture(tmp.path()).limits.unwrap();
        assert_eq!(l.captured_at, NOW - 50_000);
        assert_eq!(l.windows.len(), 2);
        assert_eq!((l.windows[0].label.as_str(), l.windows[0].used_percentage), ("Sessão de 5 h", 25.5));
        assert_eq!(l.windows[0].resets_at, Some(NOW + 3_600_000));
        assert_eq!((l.windows[1].label.as_str(), l.windows[1].used_percentage), ("Semana", 40.0));
    }

    #[test]
    fn window_already_reset_shows_zero() {
        let v: Value = serde_json::from_str(r#"{"primary":{"used_percent":80,"window_minutes":300,"resets_at":100}}"#).unwrap();
        let l = limits(0, &v, NOW);
        assert_eq!((l.windows[0].used_percentage, l.windows[0].resets_at), (0.0, None));
    }

    #[test]
    fn missing_codex_is_not_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let u = usage(tmp.path(), &FileCache::default(), NOW, TODAY);
        assert!(!u.installed);
        assert_eq!(u.error, None);
    }
}
