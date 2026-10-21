//! Banco local com métricas extraídas dos `.jsonl`. Nunca guarda o conteúdo das conversas.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::parser::{Event, UsageRecord};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS messages (
    message_id            TEXT NOT NULL,
    request_id            TEXT NOT NULL,
    session_id            TEXT NOT NULL,
    ts                    INTEGER NOT NULL,
    model                 TEXT NOT NULL,
    is_sidechain          INTEGER NOT NULL,
    input_tokens          INTEGER NOT NULL,
    output_tokens         INTEGER NOT NULL,
    cache_read_tokens     INTEGER NOT NULL,
    cache_write_5m_tokens INTEGER NOT NULL,
    cache_write_1h_tokens INTEGER NOT NULL,
    PRIMARY KEY (message_id, request_id)
);
CREATE INDEX IF NOT EXISTS idx_messages_ts ON messages(ts);
CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id);

CREATE TABLE IF NOT EXISTS message_tools (
    message_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    tool       TEXT NOT NULL,
    PRIMARY KEY (message_id, request_id, tool)
);

CREATE TABLE IF NOT EXISTS sessions (
    session_id   TEXT PRIMARY KEY,
    project_dir  TEXT,
    cwd          TEXT,
    git_branch   TEXT,
    ai_title     TEXT,
    custom_title TEXT,
    first_prompt TEXT,
    first_prompt_ts INTEGER,
    last_prompt  TEXT,
    last_prompt_ts INTEGER,
    first_ts     INTEGER,
    last_ts      INTEGER
);

CREATE TABLE IF NOT EXISTS files (
    path   TEXT PRIMARY KEY,
    offset INTEGER NOT NULL
);

-- Toda pasta em que o Claude Code trabalhou. Uma sessão pode passar por várias
-- (ex.: começa numa worktree e segue em outra pasta), por isso não basta sessions.cwd.
CREATE TABLE IF NOT EXISTS dirs (
    cwd TEXT PRIMARY KEY
);
";

/// Versão do esquema. Ao subir, os `.jsonl` são relidos do início para preencher
/// colunas novas (a gravação é idempotente, então nada é contado duas vezes).
const SCHEMA_VERSION: i64 = 2;

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    init(&conn)?;
    Ok(conn)
}

fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(SCHEMA)?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version < 1 {
        // Bancos criados no M1 não têm o último prompt.
        let has_last_prompt: bool = conn
            .prepare("SELECT 1 FROM pragma_table_info('sessions') WHERE name = 'last_prompt'")?
            .exists([])?;
        if !has_last_prompt {
            conn.execute_batch(
                "ALTER TABLE sessions ADD COLUMN last_prompt TEXT;
                 ALTER TABLE sessions ADD COLUMN last_prompt_ts INTEGER;",
            )?;
        }
    }
    if version < SCHEMA_VERSION {
        // v1: último prompt; v2: tabela `dirs`. Ambas são preenchidas relendo os `.jsonl`.
        conn.execute("DELETE FROM files", [])?;
    }
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

/// Grava um evento. `project_dir` é o nome da pasta em `~/.claude/projects` de onde veio a linha.
pub fn apply_event(conn: &Connection, project_dir: &str, event: &Event) -> rusqlite::Result<()> {
    match event {
        Event::Usage(u) => apply_usage(conn, project_dir, u),
        Event::AiTitle { session_id, title } => {
            ensure_session(conn, session_id, project_dir)?;
            conn.execute(
                "UPDATE sessions SET ai_title = ?2 WHERE session_id = ?1",
                params![session_id, title],
            )?;
            Ok(())
        }
        Event::CustomTitle { session_id, title } => {
            ensure_session(conn, session_id, project_dir)?;
            conn.execute(
                "UPDATE sessions SET custom_title = ?2 WHERE session_id = ?1",
                params![session_id, title],
            )?;
            Ok(())
        }
        Event::UserPrompt {
            session_id,
            ts,
            text,
            cwd,
        } => {
            ensure_session(conn, session_id, project_dir)?;
            conn.execute(
                "UPDATE sessions SET
                    first_prompt    = CASE WHEN first_prompt_ts IS NULL OR ?2 < first_prompt_ts THEN ?3 ELSE first_prompt END,
                    first_prompt_ts = MIN(COALESCE(first_prompt_ts, ?2), ?2),
                    last_prompt     = CASE WHEN last_prompt_ts IS NULL OR ?2 >= last_prompt_ts THEN ?3 ELSE last_prompt END,
                    last_prompt_ts  = MAX(COALESCE(last_prompt_ts, ?2), ?2),
                    cwd = COALESCE(cwd, ?4)
                 WHERE session_id = ?1",
                params![session_id, ts, text, cwd],
            )?;
            record_dir(conn, cwd.as_deref())?;
            touch_session(conn, session_id, *ts)
        }
    }
}

fn apply_usage(conn: &Connection, project_dir: &str, u: &UsageRecord) -> rusqlite::Result<()> {
    // A mesma resposta aparece em várias linhas (uma por bloco de conteúdo);
    // guardamos uma vez só, com o maior valor visto de cada contador.
    conn.execute(
        "INSERT INTO messages VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT (message_id, request_id) DO UPDATE SET
            input_tokens          = MAX(input_tokens, excluded.input_tokens),
            output_tokens         = MAX(output_tokens, excluded.output_tokens),
            cache_read_tokens     = MAX(cache_read_tokens, excluded.cache_read_tokens),
            cache_write_5m_tokens = MAX(cache_write_5m_tokens, excluded.cache_write_5m_tokens),
            cache_write_1h_tokens = MAX(cache_write_1h_tokens, excluded.cache_write_1h_tokens)",
        params![
            u.message_id,
            u.request_id,
            u.session_id,
            u.ts,
            u.model,
            u.is_sidechain,
            u.input_tokens,
            u.output_tokens,
            u.cache_read_tokens,
            u.cache_write_5m_tokens,
            u.cache_write_1h_tokens,
        ],
    )?;
    for tool in &u.tools {
        conn.execute(
            "INSERT OR IGNORE INTO message_tools VALUES (?1, ?2, ?3)",
            params![u.message_id, u.request_id, tool],
        )?;
    }
    ensure_session(conn, &u.session_id, project_dir)?;
    conn.execute(
        "UPDATE sessions SET cwd = COALESCE(cwd, ?2), git_branch = COALESCE(?3, git_branch) WHERE session_id = ?1",
        params![u.session_id, u.cwd, u.git_branch],
    )?;
    record_dir(conn, u.cwd.as_deref())?;
    touch_session(conn, &u.session_id, u.ts)
}

fn record_dir(conn: &Connection, cwd: Option<&str>) -> rusqlite::Result<()> {
    if let Some(cwd) = cwd.filter(|c| !c.is_empty()) {
        conn.execute("INSERT OR IGNORE INTO dirs VALUES (?1)", [cwd])?;
    }
    Ok(())
}

fn ensure_session(conn: &Connection, session_id: &str, project_dir: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO sessions (session_id, project_dir) VALUES (?1, ?2)",
        params![session_id, project_dir],
    )?;
    Ok(())
}

fn touch_session(conn: &Connection, session_id: &str, ts: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sessions SET
            first_ts = MIN(COALESCE(first_ts, ?2), ?2),
            last_ts  = MAX(COALESCE(last_ts, ?2), ?2)
         WHERE session_id = ?1",
        params![session_id, ts],
    )?;
    Ok(())
}

pub fn file_offset(conn: &Connection, path: &str) -> rusqlite::Result<u64> {
    let offset: Option<i64> = conn
        .query_row("SELECT offset FROM files WHERE path = ?1", [path], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(offset.unwrap_or(0) as u64)
}

pub fn set_file_offset(conn: &Connection, path: &str, offset: u64) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO files VALUES (?1, ?2) ON CONFLICT (path) DO UPDATE SET offset = excluded.offset",
        params![path, offset as i64],
    )?;
    Ok(())
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageTotals {
    pub messages: i64,
    pub sessions: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
}

/// Soma o uso no intervalo `[from_ms, to_ms)`.
#[allow(dead_code)] // a tela deixou de mostrar tokens; volta a ser usada no custo estimado (M4)
pub fn totals_between(
    conn: &Connection,
    from_ms: i64,
    to_ms: i64,
) -> rusqlite::Result<UsageTotals> {
    conn.query_row(
        "SELECT COUNT(*), COUNT(DISTINCT session_id),
                COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0),
                COALESCE(SUM(cache_read_tokens), 0),
                COALESCE(SUM(cache_write_5m_tokens + cache_write_1h_tokens), 0)
         FROM messages WHERE ts >= ?1 AND ts < ?2",
        params![from_ms, to_ms],
        |r| {
            Ok(UsageTotals {
                messages: r.get(0)?,
                sessions: r.get(1)?,
                input_tokens: r.get(2)?,
                output_tokens: r.get(3)?,
                cache_read_tokens: r.get(4)?,
                cache_write_tokens: r.get(5)?,
            })
        },
    )
}

/// Pastas em que o Claude Code já trabalhou.
pub fn session_dirs(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    conn.prepare("SELECT cwd FROM dirs")?
        .query_map([], |r| r.get(0))?
        .collect()
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolCount {
    pub tool: String,
    pub count: i64,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub title: Option<String>,
    pub project_dir: Option<String>,
    pub git_branch: Option<String>,
    /// Modelo da resposta mais recente da conversa principal.
    pub model: Option<String>,
    pub totals: UsageTotals,
    /// Parte de `totals` que veio de subagentes.
    pub subagent_totals: UsageTotals,
    /// Tokens que a última resposta da conversa principal recebeu de entrada: o tamanho do contexto atual.
    pub context_tokens: i64,
    pub top_tools: Vec<ToolCount>,
    pub last_prompt: Option<String>,
}

const TOP_TOOLS: i64 = 5;

pub fn session_detail(conn: &Connection, session_id: &str) -> rusqlite::Result<SessionDetail> {
    let mut detail = conn
        .query_row(
            "SELECT COALESCE(custom_title, ai_title, first_prompt), project_dir, git_branch, last_prompt
             FROM sessions WHERE session_id = ?1",
            [session_id],
            |r| {
                Ok(SessionDetail {
                    title: r.get(0)?,
                    project_dir: r.get(1)?,
                    git_branch: r.get(2)?,
                    last_prompt: r.get(3)?,
                    ..Default::default()
                })
            },
        )
        .optional()?
        .unwrap_or_default();

    detail.totals = session_totals(conn, session_id, false)?;
    detail.subagent_totals = session_totals(conn, session_id, true)?;

    if let Some((model, context)) = conn
        .query_row(
            "SELECT model, input_tokens + cache_read_tokens + cache_write_5m_tokens + cache_write_1h_tokens
             FROM messages WHERE session_id = ?1 AND is_sidechain = 0
             ORDER BY ts DESC LIMIT 1",
            [session_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
        )
        .optional()?
    {
        detail.model = Some(model);
        detail.context_tokens = context;
    }

    detail.top_tools = conn
        .prepare(
            "SELECT t.tool, COUNT(*) AS n FROM message_tools t
             JOIN messages m ON m.message_id = t.message_id AND m.request_id = t.request_id
             WHERE m.session_id = ?1
             GROUP BY t.tool ORDER BY n DESC, t.tool LIMIT ?2",
        )?
        .query_map(params![session_id, TOP_TOOLS], |r| {
            Ok(ToolCount {
                tool: r.get(0)?,
                count: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    Ok(detail)
}

/// Totais da sessão inteira, ou só dos subagentes quando `only_subagents`.
fn session_totals(
    conn: &Connection,
    session_id: &str,
    only_subagents: bool,
) -> rusqlite::Result<UsageTotals> {
    conn.query_row(
        "SELECT COUNT(*), COUNT(DISTINCT session_id),
                COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0),
                COALESCE(SUM(cache_read_tokens), 0),
                COALESCE(SUM(cache_write_5m_tokens + cache_write_1h_tokens), 0)
         FROM messages WHERE session_id = ?1 AND (?2 = 0 OR is_sidechain = 1)",
        params![session_id, only_subagents],
        |r| {
            Ok(UsageTotals {
                messages: r.get(0)?,
                sessions: r.get(1)?,
                input_tokens: r.get(2)?,
                output_tokens: r.get(3)?,
                cache_read_tokens: r.get(4)?,
                cache_write_tokens: r.get(5)?,
            })
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::UsageRecord;

    fn usage(msg: &str, ts: i64, output: i64) -> Event {
        Event::Usage(UsageRecord {
            session_id: "s1".into(),
            message_id: msg.into(),
            request_id: "r".into(),
            ts,
            model: "m".into(),
            input_tokens: 1,
            output_tokens: output,
            tools: vec!["Bash".into()],
            ..Default::default()
        })
    }

    fn mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        conn
    }

    #[test]
    fn duplicate_lines_of_same_response_count_once() {
        let conn = mem();
        apply_event(&conn, "p", &usage("a", 10, 5)).unwrap();
        apply_event(&conn, "p", &usage("a", 10, 7)).unwrap();
        apply_event(&conn, "p", &usage("b", 20, 1)).unwrap();
        let t = totals_between(&conn, 0, 100).unwrap();
        assert_eq!(
            (t.messages, t.sessions, t.input_tokens, t.output_tokens),
            (2, 1, 2, 8)
        );
    }

    #[test]
    fn totals_respect_interval() {
        let conn = mem();
        apply_event(&conn, "p", &usage("a", 10, 5)).unwrap();
        apply_event(&conn, "p", &usage("b", 20, 1)).unwrap();
        assert_eq!(totals_between(&conn, 15, 100).unwrap().output_tokens, 1);
        assert_eq!(
            totals_between(&conn, 0, 10).unwrap(),
            UsageTotals::default()
        );
    }

    #[test]
    fn session_keeps_earliest_prompt_and_titles() {
        let conn = mem();
        let prompt = |ts, text: &str| Event::UserPrompt {
            session_id: "s1".into(),
            ts,
            text: text.into(),
            cwd: None,
        };
        apply_event(&conn, "p", &prompt(50, "depois")).unwrap();
        apply_event(&conn, "p", &prompt(10, "primeiro")).unwrap();
        apply_event(
            &conn,
            "p",
            &Event::AiTitle {
                session_id: "s1".into(),
                title: "T".into(),
            },
        )
        .unwrap();
        let (first, title, first_ts, last_ts): (String, String, i64, i64) = conn
            .query_row(
                "SELECT first_prompt, ai_title, first_ts, last_ts FROM sessions",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            (first.as_str(), title.as_str(), first_ts, last_ts),
            ("primeiro", "T", 10, 50)
        );
    }

    #[test]
    fn session_detail_splits_subagents_and_reads_context() {
        let conn = mem();
        let rec = |msg: &str, ts, side: bool, model: &str, input, cache_read, tools: Vec<&str>| {
            Event::Usage(UsageRecord {
                session_id: "s1".into(),
                message_id: msg.into(),
                request_id: "r".into(),
                ts,
                model: model.into(),
                is_sidechain: side,
                input_tokens: input,
                output_tokens: 10,
                cache_read_tokens: cache_read,
                tools: tools.into_iter().map(String::from).collect(),
                ..Default::default()
            })
        };
        apply_event(
            &conn,
            "p",
            &rec("a", 10, false, "opus", 5, 1000, vec!["Bash", "Read"]),
        )
        .unwrap();
        apply_event(
            &conn,
            "p",
            &rec("b", 20, true, "haiku", 3, 50, vec!["Bash"]),
        )
        .unwrap();
        apply_event(
            &conn,
            "p",
            &rec("c", 30, false, "sonnet", 7, 2000, vec!["Bash"]),
        )
        .unwrap();
        let prompt = |ts, text: &str| Event::UserPrompt {
            session_id: "s1".into(),
            ts,
            text: text.into(),
            cwd: None,
        };
        apply_event(&conn, "p", &prompt(25, "último")).unwrap();
        apply_event(&conn, "p", &prompt(5, "primeiro")).unwrap();

        let d = session_detail(&conn, "s1").unwrap();
        assert_eq!(d.title.as_deref(), Some("primeiro"));
        assert_eq!(d.last_prompt.as_deref(), Some("último"));
        assert_eq!(d.model.as_deref(), Some("sonnet"));
        assert_eq!(d.context_tokens, 2007);
        assert_eq!((d.totals.messages, d.totals.output_tokens), (3, 30));
        assert_eq!(
            (d.subagent_totals.messages, d.subagent_totals.input_tokens),
            (1, 3)
        );
        assert_eq!(
            d.top_tools,
            vec![
                ToolCount {
                    tool: "Bash".into(),
                    count: 3
                },
                ToolCount {
                    tool: "Read".into(),
                    count: 1
                }
            ]
        );
    }

    #[test]
    fn unknown_session_has_empty_detail() {
        assert_eq!(
            session_detail(&mem(), "nada").unwrap(),
            SessionDetail::default()
        );
    }

    #[test]
    fn session_dirs_include_every_folder_a_session_moved_through() {
        let conn = mem();
        let usage = |msg: &str, cwd: &str| {
            Event::Usage(UsageRecord {
                session_id: "s1".into(),
                message_id: msg.into(),
                request_id: "r".into(),
                cwd: Some(cwd.into()),
                ..Default::default()
            })
        };
        // A sessão começa numa worktree e depois passa a trabalhar em outra pasta.
        apply_event(&conn, "p", &usage("a", "C:/worktree")).unwrap();
        apply_event(&conn, "p", &usage("b", "A:/projeto")).unwrap();
        let prompt = Event::UserPrompt { session_id: "s2".into(), ts: 1, text: "x".into(), cwd: Some("A:/outro".into()) };
        apply_event(&conn, "p", &prompt).unwrap();

        let mut dirs = session_dirs(&conn).unwrap();
        dirs.sort();
        assert_eq!(dirs, vec!["A:/outro", "A:/projeto", "C:/worktree"]);
    }

    #[test]
    fn upgrading_m1_database_adds_columns_and_forces_reindex() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            &SCHEMA
                .replace("last_prompt  TEXT,", "")
                .replace("last_prompt_ts INTEGER,", ""),
        )
        .unwrap();
        set_file_offset(&conn, "a.jsonl", 99).unwrap();
        init(&conn).unwrap();
        assert_eq!(file_offset(&conn, "a.jsonl").unwrap(), 0);
        conn.execute("UPDATE sessions SET last_prompt = 'x'", [])
            .unwrap();
        // Abrir de novo não apaga o progresso.
        set_file_offset(&conn, "a.jsonl", 99).unwrap();
        init(&conn).unwrap();
        assert_eq!(file_offset(&conn, "a.jsonl").unwrap(), 99);
    }
}
