//! Uso do opencode, lido do banco dele (`~/.local/share/opencode/opencode.db`) só para leitura.
//! O opencode não informa % de plano; o que dá para mostrar é o contexto de cada sessão (como
//! a tela dele faz: tokens da última resposta / limite do modelo) e o gasto em dólares.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::Value;

/// Sessão com resposta nos últimos minutos conta como ativa.
const ACTIVE_WINDOW_MS: i64 = 10 * 60 * 1000;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

pub fn db_path(home: &Path) -> PathBuf {
    home.join(".local").join("share").join("opencode").join("opencode.db")
}

pub fn models_path(home: &Path) -> PathBuf {
    home.join(".cache").join("opencode").join("models.json")
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OpencodeSession {
    pub id: String,
    pub title: String,
    pub directory: String,
    /// "provedor/modelo" da última resposta.
    pub model: Option<String>,
    /// A última resposta ainda está sendo escrita.
    pub busy: bool,
    pub started_at: i64,
    pub last_activity: i64,
    pub responses: i64,
    pub cost: f64,
    pub context_tokens: i64,
    pub context_limit: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Period {
    pub sessions: i64,
    pub responses: i64,
    pub cost: f64,
    pub top_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OpencodeUsage {
    pub installed: bool,
    pub active: Vec<OpencodeSession>,
    pub today: Period,
    pub week: Period,
    pub error: Option<String>,
}

/// Limite de contexto por "provedor/modelo", da lista de modelos que o opencode guarda.
pub fn context_limits(models_json: &str) -> HashMap<String, i64> {
    let Ok(v) = serde_json::from_str::<Value>(models_json) else { return HashMap::new() };
    let mut out = HashMap::new();
    for (provider, p) in v.as_object().into_iter().flatten() {
        for (model, m) in p.get("models").and_then(Value::as_object).into_iter().flatten() {
            if let Some(limit) = m.pointer("/limit/context").and_then(Value::as_i64).filter(|l| *l > 0) {
                out.insert(format!("{provider}/{model}"), limit);
            }
        }
    }
    out
}

/// Uma resposta (mensagem do assistente) lida do JSON do opencode.
struct Response {
    session_id: String,
    created: i64,
    /// Última vez que o opencode gravou nesta resposta (muda enquanto ela é escrita).
    updated: i64,
    completed: Option<i64>,
    model: Option<String>,
    cost: f64,
    tokens: i64,
}

fn parse_response(session_id: String, updated: i64, data: &str) -> Option<Response> {
    let v: Value = serde_json::from_str(data).ok()?;
    if v.get("role").and_then(Value::as_str) != Some("assistant") {
        return None;
    }
    let t = |p: &str| v.pointer(p).and_then(Value::as_i64).unwrap_or(0);
    let model = match (v.get("providerID").and_then(Value::as_str), v.get("modelID").and_then(Value::as_str)) {
        (Some(p), Some(m)) => Some(format!("{p}/{m}")),
        _ => None,
    };
    Some(Response {
        session_id,
        created: t("/time/created"),
        updated,
        completed: v.pointer("/time/completed").and_then(Value::as_i64),
        model,
        cost: v.get("cost").and_then(Value::as_f64).unwrap_or(0.0),
        // Mesma conta da tela do opencode para o contexto.
        tokens: t("/tokens/input") + t("/tokens/output") + t("/tokens/reasoning") + t("/tokens/cache/read") + t("/tokens/cache/write"),
    })
}

fn period(responses: &[&Response]) -> Period {
    let mut sessions: Vec<&str> = responses.iter().map(|r| r.session_id.as_str()).collect();
    sessions.sort_unstable();
    sessions.dedup();
    let mut by_model: HashMap<&str, i64> = HashMap::new();
    for r in responses {
        if let Some(m) = &r.model {
            *by_model.entry(m).or_default() += 1;
        }
    }
    Period {
        sessions: sessions.len() as i64,
        responses: responses.len() as i64,
        cost: responses.iter().map(|r| r.cost).sum(),
        top_model: by_model.into_iter().max_by_key(|(m, n)| (*n, std::cmp::Reverse(*m))).map(|(m, _)| m.to_string()),
    }
}

/// `today_start`: meia-noite local de hoje, em ms.
pub fn read(db: &Path, limits: &HashMap<String, i64>, now: i64, today_start: i64) -> rusqlite::Result<OpencodeUsage> {
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    let since = (now - 7 * DAY_MS).min(today_start);

    // Subagentes (sessões filhas) contam na sessão principal.
    let mut root_of: HashMap<String, String> = HashMap::new();
    let mut sessions: HashMap<String, (String, String, i64)> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT id, parent_id, title, directory, time_created FROM session")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?;
        let mut parents = HashMap::new();
        for row in rows {
            let (id, parent, title, dir, created) = row?;
            if let Some(p) = parent {
                parents.insert(id.clone(), p);
            }
            sessions.insert(id, (title, dir, created));
        }
        for id in sessions.keys() {
            let mut root = id.clone();
            // Limite de profundidade contra ciclos num banco corrompido.
            for _ in 0..16 {
                match parents.get(&root) {
                    Some(p) if sessions.contains_key(p) => root = p.clone(),
                    _ => break,
                }
            }
            root_of.insert(id.clone(), root);
        }
    }

    let mut stmt = conn.prepare("SELECT session_id, time_updated, data FROM message WHERE time_created >= ?1 ORDER BY time_created")?;
    let responses: Vec<Response> = stmt
        .query_map([since], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?)))?
        .filter_map(|row| row.ok())
        .filter_map(|(sid, updated, data)| parse_response(root_of.get(&sid).cloned().unwrap_or(sid), updated, &data))
        .collect();

    let today: Vec<&Response> = responses.iter().filter(|r| r.created >= today_start).collect();
    let week: Vec<&Response> = responses.iter().filter(|r| r.created >= now - 7 * DAY_MS).collect();

    let mut by_session: HashMap<&str, Vec<&Response>> = HashMap::new();
    for r in &responses {
        by_session.entry(&r.session_id).or_default().push(r);
    }
    let mut active: Vec<OpencodeSession> = by_session
        .into_iter()
        .filter_map(|(id, list)| {
            let last = *list.last()?;
            // Uma resposta interrompida (opencode fechado no meio) não fica "ativa" para sempre.
            let last_activity = list.iter().map(|r| r.updated).max()?;
            if now - last_activity > ACTIVE_WINDOW_MS {
                return None;
            }
            let (title, directory, started_at) = sessions.get(id).cloned().unwrap_or_default();
            Some(OpencodeSession {
                id: id.to_string(),
                title,
                directory,
                model: last.model.clone(),
                busy: last.completed.is_none(),
                started_at,
                last_activity,
                responses: list.len() as i64,
                cost: list.iter().map(|r| r.cost).sum(),
                context_tokens: last.tokens,
                context_limit: last.model.as_ref().and_then(|m| limits.get(m).copied()),
            })
        })
        .collect();
    active.sort_by_key(|s| std::cmp::Reverse(s.last_activity));

    Ok(OpencodeUsage { installed: true, active, today: period(&today), week: period(&week), error: None })
}

pub fn usage(home: &Path, now: i64, today_start: i64) -> OpencodeUsage {
    let db = db_path(home);
    if !db.is_file() {
        return OpencodeUsage {
            installed: false,
            active: vec![],
            today: Period::default(),
            week: Period::default(),
            error: None,
        };
    }
    let limits = std::fs::read_to_string(models_path(home)).map(|s| context_limits(&s)).unwrap_or_default();
    read(&db, &limits, now, today_start).unwrap_or_else(|e| OpencodeUsage {
        installed: true,
        active: vec![],
        today: Period::default(),
        week: Period::default(),
        error: Some(format!("Não foi possível ler o banco do opencode: {e}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000_000;
    const TODAY: i64 = NOW - 3 * 60 * 60 * 1000;

    fn fixture() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let conn = Connection::open(tmp.path().join("opencode.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE session (id TEXT PRIMARY KEY, parent_id TEXT, title TEXT NOT NULL, directory TEXT NOT NULL, time_created INTEGER NOT NULL);
             CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL, data TEXT NOT NULL);",
        )
        .unwrap();
        let session = |id: &str, parent: Option<&str>, title: &str, created: i64| {
            conn.execute("INSERT INTO session VALUES (?1, ?2, ?3, 'A:/proj', ?4)", rusqlite::params![id, parent, title, created])
                .unwrap();
        };
        let msg = |id: &str, sid: &str, created: i64, data: String| {
            conn.execute("INSERT INTO message VALUES (?1, ?2, ?3, ?3, ?4)", rusqlite::params![id, sid, created, data]).unwrap();
        };
        let answer = |created: i64, completed: Option<i64>, model: &str, cost: f64, input: i64| {
            let completed = completed.map_or("null".to_string(), |c| c.to_string());
            format!(
                r#"{{"role":"assistant","providerID":"opencode","modelID":"{model}","cost":{cost},
                "tokens":{{"input":{input},"output":10,"reasoning":5,"cache":{{"read":100,"write":0}}}},
                "time":{{"created":{created},"completed":{completed}}}}}"#
            )
        };
        session("ativa", None, "Corrige login", NOW - 3_600_000);
        session("filha", Some("ativa"), "subagente", NOW - 3_000_000);
        session("velha", None, "Ontem", NOW - 30 * 3_600_000);
        msg("u1", "ativa", NOW - 120_000, r#"{"role":"user","time":{"created":1}}"#.into());
        msg("a1", "ativa", NOW - 110_000, answer(NOW - 110_000, Some(NOW - 100_000), "free", 0.5, 1000));
        msg("a2", "filha", NOW - 90_000, answer(NOW - 90_000, Some(NOW - 80_000), "pro", 0.25, 50));
        msg("a3", "ativa", NOW - 60_000, answer(NOW - 60_000, None, "free", 0.0, 2000));
        msg("a4", "velha", NOW - 29 * 3_600_000, answer(NOW - 29 * 3_600_000, Some(NOW - 29 * 3_600_000), "free", 1.0, 10));
        // Resposta que ficou pela metade há 2 dias: não deixa a sessão "ativa".
        msg("a6", "velha", NOW - 48 * 3_600_000, answer(NOW - 48 * 3_600_000, None, "free", 0.0, 10));
        msg("a5", "velha", NOW - 9 * 24 * 3_600_000, answer(NOW - 9 * 24 * 3_600_000, Some(1), "free", 9.0, 10));
        tmp
    }

    #[test]
    fn reads_active_sessions_with_context_and_children() {
        let tmp = fixture();
        let limits = HashMap::from([("opencode/free".to_string(), 10_000)]);
        let u = read(&tmp.path().join("opencode.db"), &limits, NOW, TODAY).unwrap();
        assert_eq!(u.active.len(), 1, "a sessão de ontem não está ativa");
        let s = &u.active[0];
        assert_eq!((s.id.as_str(), s.title.as_str()), ("ativa", "Corrige login"));
        assert!(s.busy, "a última resposta ainda não terminou");
        assert_eq!(s.responses, 3, "inclui a resposta do subagente");
        assert_eq!(s.cost, 0.75);
        assert_eq!(s.context_tokens, 2000 + 10 + 5 + 100);
        assert_eq!(s.context_limit, Some(10_000));
        assert_eq!(s.model.as_deref(), Some("opencode/free"));
    }

    #[test]
    fn sums_today_and_last_seven_days() {
        let tmp = fixture();
        let u = read(&tmp.path().join("opencode.db"), &HashMap::new(), NOW, TODAY).unwrap();
        assert_eq!((u.today.sessions, u.today.responses), (1, 3));
        assert_eq!(u.today.cost, 0.75);
        assert_eq!(u.today.top_model.as_deref(), Some("opencode/free"));
        assert_eq!((u.week.sessions, u.week.responses), (2, 5), "a resposta de 9 dias atrás fica de fora");
        assert_eq!(u.week.cost, 1.75);
    }

    #[test]
    fn reads_context_limits_from_model_list() {
        let json = r#"{"opencode":{"models":{"free":{"limit":{"context":1000000}},"x":{}}},"anthropic":{"models":{"sonnet":{"limit":{"context":200000}}}}}"#;
        let l = context_limits(json);
        assert_eq!(l.get("opencode/free"), Some(&1_000_000));
        assert_eq!(l.get("anthropic/sonnet"), Some(&200_000));
        assert_eq!(l.len(), 2);
    }

    #[test]
    fn missing_opencode_is_not_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let u = usage(tmp.path(), NOW, TODAY);
        assert!(!u.installed);
        assert_eq!(u.error, None);
    }
}
