//! Consoles do Claude Code abertos agora, a partir de `~/.claude/sessions/<pid>.json`.

use std::path::Path;

use serde::Serialize;
use serde_json::Value;
use sysinfo::{Pid, ProcessesToUpdate, System};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveEntry {
    pub pid: u32,
    pub session_id: String,
    pub cwd: Option<String>,
    pub name: Option<String>,
    /// `busy` (trabalhando) ou `idle` (aguardando o usuário), como o Claude Code grava.
    pub status: Option<String>,
    /// Unix epoch em milissegundos.
    pub started_at: Option<i64>,
    /// Início do processo em segundos desde 1970, quando o arquivo informa.
    #[serde(skip)]
    pub proc_start_secs: Option<u64>,
}

/// Diferença entre as épocas do FILETIME do Windows (1601) e do Unix (1970), em segundos.
#[cfg(windows)]
const FILETIME_UNIX_OFFSET_SECS: u64 = 11_644_473_600;

/// Início do processo gravado pelo Claude Code. No Windows ele vem em FILETIME; nos outros
/// sistemas o formato não é conhecido, então a conferência fica pelo nome do processo.
#[cfg(windows)]
fn proc_start_secs(v: &Value) -> Option<u64> {
    let filetime = v.get("procStart")?.as_str()?.parse::<u64>().ok()?;
    Some((filetime / 10_000_000).saturating_sub(FILETIME_UNIX_OFFSET_SECS))
}

#[cfg(not(windows))]
fn proc_start_secs(_: &Value) -> Option<u64> {
    None
}

pub fn parse_entry(json: &str) -> Option<LiveEntry> {
    let v: Value = serde_json::from_str(json).ok()?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(String::from);
    Some(LiveEntry {
        pid: u32::try_from(v.get("pid")?.as_u64()?).ok()?,
        session_id: s("sessionId")?,
        cwd: s("cwd"),
        name: s("name"),
        status: s("status"),
        started_at: v.get("startedAt").and_then(Value::as_i64),
        proc_start_secs: proc_start_secs(&v),
    })
}

/// Lê os registros e mantém só os cujo processo ainda existe.
pub fn read_live(claude_dir: &Path) -> Vec<LiveEntry> {
    let Ok(dir) = std::fs::read_dir(claude_dir.join("sessions")) else {
        return Vec::new();
    };
    let entries: Vec<LiveEntry> = dir
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|json| parse_entry(&json))
        .collect();

    let mut sys = System::new();
    let pids: Vec<Pid> = entries.iter().map(|e| Pid::from_u32(e.pid)).collect();
    sys.refresh_processes(ProcessesToUpdate::Some(&pids), true);
    let mut alive: Vec<LiveEntry> = entries
        .into_iter()
        .filter(|e| is_same_process(&sys, e))
        .collect();
    alive.sort_by_key(|e| std::cmp::Reverse(e.started_at));
    alive
}

/// O PID pode ter sido reaproveitado por outro programa depois que o Claude fechou:
/// confere o horário de início quando ele existe; senão, o nome do executável.
fn is_same_process(sys: &System, entry: &LiveEntry) -> bool {
    let Some(process) = sys.process(Pid::from_u32(entry.pid)) else {
        return false;
    };
    match entry.proc_start_secs {
        Some(expected) => process.start_time().abs_diff(expected) <= 2,
        None => {
            let name = process.name().to_string_lossy().to_lowercase();
            name.starts_with("claude") || name.starts_with("node")
        }
    }
}

/// Encerra o console (e os processos filhos, como servidores MCP) da sessão indicada.
pub fn kill(claude_dir: &Path, pid: u32, session_id: &str) -> Result<(), String> {
    let entry = read_live(claude_dir)
        .into_iter()
        .find(|e| e.pid == pid && e.session_id == session_id)
        .ok_or("Essa sessão já foi encerrada.")?;

    kill_tree(entry.pid)
}

#[cfg(windows)]
fn kill_tree(pid: u32) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Não foi possível executar o taskkill: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "O Windows recusou o encerramento: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Mac e Linux: pede para fechar (SIGTERM) o console e os processos que ele abriu.
#[cfg(not(windows))]
fn kill_tree(pid: u32) -> Result<(), String> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let links: Vec<(u32, Option<u32>)> =
        sys.processes().iter().map(|(p, proc_)| (p.as_u32(), proc_.parent().map(|pp| pp.as_u32()))).collect();
    // Filhos antes do pai, para nenhum ficar órfão no caminho.
    for p in descendants(pid, &links).into_iter().rev().chain([pid]) {
        if let Some(process) = sys.process(Pid::from_u32(p)) {
            if process.kill_with(sysinfo::Signal::Term) == Some(false) && p == pid {
                return Err("O sistema recusou o encerramento do console.".into());
            }
        }
    }
    Ok(())
}

/// Todos os descendentes de `root` (filhos, netos...), a partir de pares (processo, pai).
#[cfg_attr(windows, allow(dead_code))]
fn descendants(root: u32, links: &[(u32, Option<u32>)]) -> Vec<u32> {
    let mut out = Vec::new();
    let mut frontier = vec![root];
    while let Some(parent) = frontier.pop() {
        for &(child, p) in links {
            if p == Some(parent) && child != root && !out.contains(&child) {
                out.push(child);
                frontier.push(child);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_session_registry_file() {
        let json = r#"{"pid":19992,"sessionId":"abc","cwd":"C:\\p","startedAt":1792545877594,"procStart":"134370194741009430","name":"lugworm-66","status":"busy"}"#;
        let e = parse_entry(json).unwrap();
        assert_eq!(e.pid, 19992);
        assert_eq!(e.session_id, "abc");
        assert_eq!(e.status.as_deref(), Some("busy"));
        assert_eq!(e.started_at, Some(1792545877594));
        // 134370194741009430 / 1e7 - 11644473600 = 1792545874 (3 s antes do startedAt).
        #[cfg(windows)]
        assert_eq!(e.proc_start_secs, Some(1792545874));
        #[cfg(not(windows))]
        assert_eq!(e.proc_start_secs, None, "fora do Windows o formato não é conhecido");
    }

    #[test]
    fn finds_every_descendant_once() {
        let links = [(1, None), (10, Some(1)), (11, Some(10)), (12, Some(10)), (20, Some(2)), (13, Some(11))];
        let mut d = descendants(1, &links);
        d.sort();
        assert_eq!(d, vec![10, 11, 12, 13]);
        assert!(descendants(20, &links).is_empty());
    }

    #[test]
    fn rejects_incomplete_files() {
        assert_eq!(parse_entry(r#"{"sessionId":"abc"}"#), None);
        assert_eq!(parse_entry("lixo"), None);
    }

    #[cfg(windows)]
    #[test]
    fn current_process_is_recognized_as_alive() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("sessions")).unwrap();
        let pid = std::process::id();
        let dead_pid = u32::MAX - 1;
        std::fs::write(
            tmp.path().join("sessions/b.json"),
            format!(r#"{{"pid":{dead_pid},"sessionId":"morto"}}"#),
        )
        .unwrap();

        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        let me = sys.process(Pid::from_u32(pid)).unwrap();
        // O teste não roda como "claude.exe", então confere pelo horário de início.
        let filetime = (me.start_time() + FILETIME_UNIX_OFFSET_SECS) * 10_000_000;
        std::fs::write(
            tmp.path().join("sessions/a.json"),
            format!(r#"{{"pid":{pid},"sessionId":"vivo","procStart":"{filetime}"}}"#),
        )
        .unwrap();

        let live = read_live(tmp.path());
        assert_eq!(
            live.iter()
                .map(|e| e.session_id.as_str())
                .collect::<Vec<_>>(),
            vec!["vivo"]
        );
    }
}
