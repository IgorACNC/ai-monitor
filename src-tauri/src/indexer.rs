//! Lê os `.jsonl` de `~/.claude/projects` de forma incremental e grava as métricas no banco.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::{db, parser};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Pasta de configuração do Claude Code: `CLAUDE_CONFIG_DIR` ou `~/.claude`.
pub fn default_claude_dir() -> Option<PathBuf> {
    match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(dir) if !dir.is_empty() => Some(PathBuf::from(dir)),
        _ => dirs::home_dir().map(|h| h.join(".claude")),
    }
}

/// Indexa todos os `.jsonl` sob `projects_dir` (inclusive os de subagentes).
pub fn index_all(conn: &mut Connection, projects_dir: &Path) -> Result<()> {
    for path in jsonl_files(projects_dir) {
        index_file(conn, projects_dir, &path)?;
    }
    Ok(())
}

/// Processa apenas as linhas completas acrescentadas desde a última leitura.
pub fn index_file(conn: &mut Connection, projects_dir: &Path, path: &Path) -> Result<()> {
    let key = path.to_string_lossy().to_string();
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    let len = file.metadata()?.len();
    let mut offset = db::file_offset(conn, &key)?;
    if len < offset {
        // Arquivo foi reescrito: relê do início (a deduplicação evita contagem dupla).
        offset = 0;
    }
    if len == offset {
        return Ok(());
    }

    file.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::with_capacity((len - offset) as usize);
    file.read_to_end(&mut buf)?;
    // Uma linha sem '\n' ainda está sendo escrita; fica para a próxima leitura.
    let Some(last_newline) = buf.iter().rposition(|&b| b == b'\n') else {
        return Ok(());
    };
    let complete = &buf[..=last_newline];

    let project_dir = project_dir_of(projects_dir, path);
    let tx = conn.transaction()?;
    for line in String::from_utf8_lossy(complete).lines() {
        if let Some(event) = parser::parse_line(line) {
            db::apply_event(&tx, &project_dir, &event)?;
        }
    }
    db::set_file_offset(&tx, &key, offset + complete.len() as u64)?;
    tx.commit()?;
    Ok(())
}

pub fn is_jsonl(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "jsonl")
}

fn jsonl_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if is_jsonl(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// Nome da pasta do projeto: primeiro componente do caminho relativo a `projects_dir`.
fn project_dir_of(projects_dir: &Path, path: &Path) -> String {
    path.strip_prefix(projects_dir)
        .ok()
        .and_then(|rel| rel.components().next())
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn assistant(msg: &str, session: &str, output: i64) -> String {
        format!(
            r#"{{"type":"assistant","sessionId":"{session}","requestId":"r","timestamp":"2026-10-21T00:00:00Z","message":{{"model":"m","id":"{msg}","content":[],"usage":{{"input_tokens":1,"output_tokens":{output}}}}}}}"#
        )
    }

    fn setup() -> (tempfile::TempDir, PathBuf, Connection) {
        let tmp = tempfile::tempdir().unwrap();
        let projects = tmp.path().join("projects");
        std::fs::create_dir_all(projects.join("C--proj/s1/subagents")).unwrap();
        let conn = db::open(&tmp.path().join("t.db")).unwrap();
        (tmp, projects, conn)
    }

    fn output_total(conn: &Connection) -> i64 {
        db::totals_between(conn, 0, i64::MAX).unwrap().output_tokens
    }

    #[test]
    fn indexes_sessions_and_subagents_with_dedupe() {
        let (_tmp, projects, mut conn) = setup();
        let main = projects.join("C--proj/s1.jsonl");
        // Mesma resposta em duas linhas + outra resposta.
        std::fs::write(
            &main,
            format!(
                "{}\n{}\n{}\n",
                assistant("a", "s1", 5),
                assistant("a", "s1", 5),
                assistant("b", "s1", 3)
            ),
        )
        .unwrap();
        std::fs::write(
            projects.join("C--proj/s1/subagents/agent-x.jsonl"),
            format!("{}\n", assistant("c", "s1", 10)),
        )
        .unwrap();

        index_all(&mut conn, &projects).unwrap();
        assert_eq!(output_total(&conn), 18);
        let project: String = conn
            .query_row("SELECT project_dir FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(project, "C--proj");

        // Reindexar sem mudanças não altera nada.
        index_all(&mut conn, &projects).unwrap();
        assert_eq!(output_total(&conn), 18);
    }

    #[test]
    fn reads_only_appended_complete_lines() {
        let (_tmp, projects, mut conn) = setup();
        let path = projects.join("C--proj/s1.jsonl");
        let mut f = File::create(&path).unwrap();
        write!(
            f,
            "{}\n{}",
            assistant("a", "s1", 5),
            &assistant("b", "s1", 3)[..20]
        )
        .unwrap();
        f.flush().unwrap();

        index_file(&mut conn, &projects, &path).unwrap();
        assert_eq!(output_total(&conn), 5);

        // Termina a linha incompleta: agora "b" entra.
        std::fs::write(
            &path,
            format!("{}\n{}\n", assistant("a", "s1", 5), assistant("b", "s1", 3)),
        )
        .unwrap();
        index_file(&mut conn, &projects, &path).unwrap();
        assert_eq!(output_total(&conn), 8);
    }

    #[test]
    fn missing_file_is_ignored() {
        let (_tmp, projects, mut conn) = setup();
        index_file(&mut conn, &projects, &projects.join("nao-existe.jsonl")).unwrap();
    }
}
