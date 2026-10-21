//! Porcentagens de uso (limites de 5 h e semanal, contexto da sessão) capturadas pela
//! statusline do Claude Code. O próprio executável do app é chamado como comando de
//! statusline: grava os números e repassa a entrada para a statusline que já existia.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Value, json};

/// Marca que identifica a nossa statusline dentro do `settings.json`.
pub const FLAG: &str = "--ai-monitor-statusline";
const IDENTIFIER: &str = "com.igoracnc.aimonitor";

/// Janelas que o Claude Code informa, na ordem em que aparecem no app.
const WINDOWS: [(&str, &str); 4] = [
    ("five_hour", "Sessão de 5 h"),
    ("seven_day", "Semana"),
    ("seven_day_opus", "Opus na semana"),
    ("seven_day_sonnet", "Sonnet na semana"),
];

/// Pasta de dados do app (a mesma que o Tauri usa), acessível também no modo statusline.
pub fn data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(IDENTIFIER))
}

fn capture_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("statusline")
}

fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

// ---------- Modo statusline (executado pelo Claude Code) ----------

/// Se o app foi chamado como statusline, processa a entrada e devolve o código de saída.
pub fn run_from_args() -> Option<i32> {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some(FLAG) {
        return None;
    }
    let tee = args.next().as_deref() == Some("tee");
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    if let Some(dir) = data_dir() {
        let _ = capture(&input, &capture_dir(&dir), now_ms());
    }
    let out = if tee {
        input.clone()
    } else {
        summary_line(&input)
    };
    let _ = std::io::stdout().write_all(out.as_bytes());
    Some(0)
}

/// Guarda os limites da conta e o contexto da sessão que vieram na entrada da statusline.
pub fn capture(payload: &str, dir: &Path, captured_at: i64) -> std::io::Result<()> {
    let Ok(v) = serde_json::from_str::<Value>(payload) else {
        return Ok(());
    };
    if let Some(limits) = v.get("rate_limits").filter(|l| l.is_object()) {
        let doc = json!({ "capturedAt": captured_at, "rateLimits": limits });
        write_atomic(&dir.join("limits.json"), &doc.to_string())?;
    }
    let session = v
        .get("session_id")
        .and_then(Value::as_str)
        .filter(|s| is_safe_id(s));
    if let (Some(session), Some(context)) = (session, v.get("context_window")) {
        let doc = json!({ "capturedAt": captured_at, "contextWindow": context });
        write_atomic(
            &dir.join("sessions").join(format!("{session}.json")),
            &doc.to_string(),
        )?;
    }
    Ok(())
}

fn is_safe_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Linha mostrada no terminal quando não havia statusline antes da nossa.
fn summary_line(payload: &str) -> String {
    let Ok(v) = serde_json::from_str::<Value>(payload) else {
        return String::new();
    };
    let pct = |w: &str| {
        v.pointer(&format!("/rate_limits/{w}/used_percentage"))
            .and_then(Value::as_f64)
            .map(|p| format!("{p:.0}%"))
    };
    let mut parts = Vec::new();
    if let Some(p) = pct("five_hour") {
        parts.push(format!("5 h {p}"));
    }
    if let Some(p) = pct("seven_day") {
        parts.push(format!("semana {p}"));
    }
    if let Some(p) = v
        .pointer("/context_window/used_percentage")
        .and_then(Value::as_f64)
    {
        parts.push(format!("contexto {p:.0}%"));
    }
    parts.join(" | ")
}

// ---------- Leitura pelo app ----------

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LimitWindow {
    pub id: String,
    pub label: String,
    pub used_percentage: f64,
    /// Unix epoch em milissegundos.
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    pub captured_at: i64,
    pub windows: Vec<LimitWindow>,
}

/// `resets_at` chega como segundos, milissegundos ou texto ISO, dependendo do campo.
fn parse_reset(v: &Value) -> Option<i64> {
    if let Some(n) = v.as_f64() {
        return Some(if n < 1e12 {
            (n * 1000.0) as i64
        } else {
            n as i64
        });
    }
    chrono::DateTime::parse_from_rfc3339(v.as_str()?)
        .ok()
        .map(|d| d.timestamp_millis())
}

pub fn read_limits(data_dir: &Path) -> Option<Limits> {
    let text = std::fs::read_to_string(capture_dir(data_dir).join("limits.json")).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let limits = v.get("rateLimits")?;
    let windows = WINDOWS
        .iter()
        .filter_map(|(id, label)| {
            let w = limits.get(*id)?;
            Some(LimitWindow {
                id: id.to_string(),
                label: label.to_string(),
                used_percentage: w.get("used_percentage")?.as_f64()?,
                resets_at: w.get("resets_at").and_then(parse_reset),
            })
        })
        .collect();
    Some(Limits {
        captured_at: v.get("capturedAt")?.as_i64()?,
        windows,
    })
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContextUsage {
    pub percent: f64,
    /// Tokens ocupando o contexto agora.
    pub used_tokens: Option<i64>,
    /// Tamanho da janela de contexto do modelo da sessão.
    pub window_size: Option<i64>,
}

/// Contexto ocupado na sessão, segundo a última statusline dela.
pub fn read_context(data_dir: &Path, session_id: &str) -> Option<ContextUsage> {
    if !is_safe_id(session_id) {
        return None;
    }
    let path = capture_dir(data_dir)
        .join("sessions")
        .join(format!("{session_id}.json"));
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let ctx = v.get("contextWindow")?;
    let percent = ctx.get("used_percentage")?.as_f64()?;
    let window_size = ctx
        .get("context_window_size")
        .and_then(Value::as_i64)
        .filter(|n| *n > 0);
    // O Claude Code calcula a porcentagem a partir de `current_usage`; sem ele, deriva do tamanho.
    let used_tokens = ctx
        .get("current_usage")
        .filter(|u| u.is_object())
        .map(|u| {
            [
                "input_tokens",
                "cache_creation_input_tokens",
                "cache_read_input_tokens",
            ]
            .iter()
            .filter_map(|k| u.get(*k).and_then(Value::as_i64))
            .sum()
        })
        .or_else(|| window_size.map(|size| (size as f64 * percent / 100.0).round() as i64));
    Some(ContextUsage {
        percent,
        used_tokens,
        window_size,
    })
}

// ---------- Ligar e desligar no settings.json ----------

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStatus {
    pub enabled: bool,
}

fn read_settings(path: &Path) -> Result<(String, Value), String> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let v = serde_json::from_str(&text)
                .map_err(|e| format!("O settings.json do Claude Code não é um JSON válido: {e}"))?;
            Ok((text, v))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok((String::new(), json!({}))),
        Err(e) => Err(format!("Não foi possível ler o settings.json: {e}")),
    }
}

fn write_settings(path: &Path, v: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(v).map_err(|e| e.to_string())? + "\n";
    write_atomic(path, &text).map_err(|e| format!("Não foi possível salvar o settings.json: {e}"))
}

fn is_ours(status_line: Option<&Value>) -> bool {
    status_line
        .and_then(|s| s.get("command"))
        .and_then(Value::as_str)
        .is_some_and(|c| c.contains(FLAG))
}

/// Comando que chama o app e, se havia statusline antes, repassa a entrada para ela.
pub fn compose_command(exe: &Path, original: Option<&str>) -> String {
    let exe = exe.to_string_lossy().replace('\\', "/");
    match original.map(str::trim).filter(|c| !c.is_empty()) {
        Some(cmd) => format!("\"{exe}\" {FLAG} tee | ( {cmd} )"),
        None => format!("\"{exe}\" {FLAG}"),
    }
}

pub fn status(settings_path: &Path) -> CaptureStatus {
    let enabled = read_settings(settings_path).is_ok_and(|(_, v)| is_ours(v.get("statusLine")));
    CaptureStatus { enabled }
}

pub fn enable(settings_path: &Path, data_dir: &Path, exe: &Path) -> Result<(), String> {
    let (text, mut v) = read_settings(settings_path)?;
    if is_ours(v.get("statusLine")) {
        return Ok(());
    }
    if !text.is_empty() {
        let backup = data_dir
            .join("backups")
            .join(format!("settings-{}.json", now_ms()));
        write_atomic(&backup, &text)
            .map_err(|e| format!("Não foi possível fazer o backup: {e}"))?;
    }
    let original = v.get("statusLine").cloned().unwrap_or(Value::Null);
    write_atomic(
        &capture_dir(data_dir).join("original.json"),
        &json!({ "statusLine": original }).to_string(),
    )
    .map_err(|e| e.to_string())?;

    let original_cmd = original.get("command").and_then(Value::as_str);
    let mut status_line =
        json!({ "type": "command", "command": compose_command(exe, original_cmd) });
    if let Some(padding) = original.get("padding") {
        status_line["padding"] = padding.clone();
    }
    v.as_object_mut()
        .ok_or("O settings.json não é um objeto JSON.")?
        .insert("statusLine".into(), status_line);
    write_settings(settings_path, &v)
}

/// Se a captura está ligada mas aponta para outro executável (por exemplo, a versão de
/// desenvolvimento), passa a apontar para `exe`, mantendo a statusline original encadeada.
pub fn repoint(settings_path: &Path, data_dir: &Path, exe: &Path) -> Result<(), String> {
    let (_, mut v) = read_settings(settings_path)?;
    let Some(current) = v.get("statusLine").filter(|s| is_ours(Some(s))) else {
        return Ok(());
    };
    let original = std::fs::read_to_string(capture_dir(data_dir).join("original.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|o| o.get("statusLine").cloned())
        .unwrap_or(Value::Null);
    let command = compose_command(exe, original.get("command").and_then(Value::as_str));
    if current.get("command").and_then(Value::as_str) == Some(command.as_str()) {
        return Ok(());
    }
    v["statusLine"]["command"] = Value::String(command);
    write_settings(settings_path, &v)
}

pub fn disable(settings_path: &Path, data_dir: &Path) -> Result<(), String> {
    let original_path = capture_dir(data_dir).join("original.json");
    let (_, mut v) = read_settings(settings_path)?;
    if is_ours(v.get("statusLine")) {
        let original = std::fs::read_to_string(&original_path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|o| o.get("statusLine").cloned())
            .unwrap_or(Value::Null);
        let obj = v
            .as_object_mut()
            .ok_or("O settings.json não é um objeto JSON.")?;
        if original.is_null() {
            obj.remove("statusLine");
        } else {
            obj.insert("statusLine".into(), original);
        }
        write_settings(settings_path, &v)?;
    }
    let _ = std::fs::remove_file(original_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAYLOAD: &str = r#"{"session_id":"abc-123","rate_limits":{"five_hour":{"used_percentage":23.5,"resets_at":1792545600},"seven_day":{"used_percentage":41,"resets_at":"2026-10-24T14:00:00Z"},"seven_day_opus":null},"context_window":{"used_percentage":34,"context_window_size":1000000,"current_usage":{"input_tokens":10,"cache_creation_input_tokens":40000,"cache_read_input_tokens":299990,"output_tokens":5}}}"#;

    #[test]
    fn captures_limits_and_session_context() {
        let tmp = tempfile::tempdir().unwrap();
        capture(PAYLOAD, &capture_dir(tmp.path()), 1000).unwrap();

        let limits = read_limits(tmp.path()).unwrap();
        assert_eq!(limits.captured_at, 1000);
        assert_eq!(
            limits.windows,
            vec![
                LimitWindow {
                    id: "five_hour".into(),
                    label: "Sessão de 5 h".into(),
                    used_percentage: 23.5,
                    resets_at: Some(1792545600000)
                },
                LimitWindow {
                    id: "seven_day".into(),
                    label: "Semana".into(),
                    used_percentage: 41.0,
                    resets_at: Some(1792850400000)
                },
            ]
        );
        assert_eq!(
            read_context(tmp.path(), "abc-123"),
            Some(ContextUsage {
                percent: 34.0,
                used_tokens: Some(340_000),
                window_size: Some(1_000_000)
            })
        );
        assert_eq!(read_context(tmp.path(), "outra"), None);
    }

    #[test]
    fn context_without_current_usage_is_derived_from_window_size() {
        let tmp = tempfile::tempdir().unwrap();
        let payload = r#"{"session_id":"s","context_window":{"used_percentage":25,"context_window_size":200000,"current_usage":null}}"#;
        capture(payload, &capture_dir(tmp.path()), 1).unwrap();
        assert_eq!(
            read_context(tmp.path(), "s"),
            Some(ContextUsage {
                percent: 25.0,
                used_tokens: Some(50_000),
                window_size: Some(200_000)
            })
        );
    }

    #[test]
    fn ignores_garbage_and_unsafe_session_ids() {
        let tmp = tempfile::tempdir().unwrap();
        capture("não é json", &capture_dir(tmp.path()), 1).unwrap();
        capture(
            r#"{"session_id":"../x","context_window":{"used_percentage":1}}"#,
            &capture_dir(tmp.path()),
            1,
        )
        .unwrap();
        assert!(read_limits(tmp.path()).is_none());
        assert_eq!(read_context(tmp.path(), "../x"), None);
    }

    #[test]
    fn summary_line_shows_percentages() {
        assert_eq!(summary_line(PAYLOAD), "5 h 24% | semana 41% | contexto 34%");
        assert_eq!(summary_line("{}"), "");
    }

    #[test]
    fn compose_chains_previous_statusline() {
        let exe = Path::new("A:\\app\\ai-monitor.exe");
        assert_eq!(
            compose_command(exe, None),
            "\"A:/app/ai-monitor.exe\" --ai-monitor-statusline"
        );
        assert_eq!(
            compose_command(exe, Some("orca-statusline.cmd")),
            "\"A:/app/ai-monitor.exe\" --ai-monitor-statusline tee | ( orca-statusline.cmd )"
        );
    }

    #[test]
    fn enable_then_disable_restores_original_statusline() {
        let tmp = tempfile::tempdir().unwrap();
        let settings = tmp.path().join("settings.json");
        let original =
            r#"{"theme":"dark","statusLine":{"type":"command","command":"orca.cmd","padding":0}}"#;
        std::fs::write(&settings, original).unwrap();
        let exe = Path::new("C:/app.exe");

        enable(&settings, tmp.path(), exe).unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(
            v["statusLine"]["command"],
            "\"C:/app.exe\" --ai-monitor-statusline tee | ( orca.cmd )"
        );
        assert_eq!(v["statusLine"]["padding"], 0);
        assert_eq!(v["theme"], "dark");
        assert!(status(&settings).enabled);
        assert_eq!(
            std::fs::read_dir(tmp.path().join("backups"))
                .unwrap()
                .count(),
            1
        );

        // Ativar de novo não encadeia a nossa statusline nela mesma.
        enable(&settings, tmp.path(), exe).unwrap();
        assert_eq!(
            std::fs::read_dir(tmp.path().join("backups"))
                .unwrap()
                .count(),
            1
        );

        disable(&settings, tmp.path()).unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(
            v["statusLine"],
            json!({"type":"command","command":"orca.cmd","padding":0})
        );
        assert!(!status(&settings).enabled);
    }

    #[test]
    fn repoint_moves_capture_to_new_exe_and_keeps_chain() {
        let tmp = tempfile::tempdir().unwrap();
        let settings = tmp.path().join("settings.json");
        std::fs::write(
            &settings,
            r#"{"statusLine":{"type":"command","command":"orca.cmd"}}"#,
        )
        .unwrap();
        enable(&settings, tmp.path(), Path::new("C:/dev/app.exe")).unwrap();

        repoint(
            &settings,
            tmp.path(),
            Path::new("C:/Programas/AI Monitor/app.exe"),
        )
        .unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(
            v["statusLine"]["command"],
            "\"C:/Programas/AI Monitor/app.exe\" --ai-monitor-statusline tee | ( orca.cmd )"
        );

        // Desativar continua devolvendo a statusline original.
        disable(&settings, tmp.path()).unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(v["statusLine"]["command"], "orca.cmd");
    }

    #[test]
    fn repoint_does_nothing_when_capture_is_off() {
        let tmp = tempfile::tempdir().unwrap();
        let settings = tmp.path().join("settings.json");
        let text = r#"{"statusLine":{"type":"command","command":"orca.cmd"}}"#;
        std::fs::write(&settings, text).unwrap();
        repoint(&settings, tmp.path(), Path::new("C:/app.exe")).unwrap();
        assert_eq!(std::fs::read_to_string(&settings).unwrap(), text);
    }

    #[test]
    fn disable_removes_statusline_when_there_was_none() {
        let tmp = tempfile::tempdir().unwrap();
        let settings = tmp.path().join("settings.json");
        std::fs::write(&settings, r#"{"theme":"dark"}"#).unwrap();
        enable(&settings, tmp.path(), Path::new("C:/app.exe")).unwrap();
        disable(&settings, tmp.path()).unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(v, json!({"theme":"dark"}));
    }
}
