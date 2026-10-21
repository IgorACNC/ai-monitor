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
const FILETIME_UNIX_OFFSET_SECS: u64 = 11_644_473_600;

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
        proc_start_secs: s("procStart")
            .and_then(|p| p.parse::<u64>().ok())
            .map(|filetime| (filetime / 10_000_000).saturating_sub(FILETIME_UNIX_OFFSET_SECS)),
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

    let mut cmd = std::process::Command::new("taskkill");
    cmd.args(["/PID", &entry.pid.to_string(), "/T", "/F"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd
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
        assert_eq!(e.proc_start_secs, Some(1792545874));
    }

    #[test]
    fn rejects_incomplete_files() {
        assert_eq!(parse_entry(r#"{"sessionId":"abc"}"#), None);
        assert_eq!(parse_entry("lixo"), None);
    }

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
