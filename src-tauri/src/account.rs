//! Uso da conta direto da Anthropic, pela mesma consulta que o `/usage` do Claude Code faz.
//! Os limites do plano são da conta (navegador, app e Claude Code juntos), então este número
//! não depende de o Claude Code estar respondendo. A consulta não é documentada: se ela mudar
//! ou falhar, o app continua com os números da statusline.

use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use crate::prefs::Toggle;
use crate::statusline::{LimitWindow, Limits, WINDOWS};

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
pub const POLL: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProductShare {
    pub key: String,
    pub label: String,
    /// Parte do uso da semana que veio deste produto.
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccountUsage {
    pub limits: Limits,
    pub breakdown: Vec<ProductShare>,
}

pub fn parse_usage(json: &str, fetched_at: i64) -> Result<AccountUsage, String> {
    let v: Value = serde_json::from_str(json).map_err(|_| "Resposta inesperada da Anthropic.".to_string())?;
    let windows: Vec<LimitWindow> = WINDOWS
        .iter()
        .filter_map(|(id, label)| {
            let w = v.get(*id).filter(|w| w.is_object())?;
            Some(LimitWindow {
                id: id.to_string(),
                label: label.to_string(),
                used_percentage: w.get("utilization")?.as_f64()?,
                resets_at: w
                    .get("resets_at")
                    .and_then(Value::as_str)
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|d| d.timestamp_millis()),
            })
        })
        .collect();
    if windows.is_empty() {
        return Err("A Anthropic não devolveu os limites da conta.".into());
    }
    let breakdown = v
        .pointer("/seven_day_breakdown/rows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|r| {
            Some(ProductShare {
                key: r.get("key")?.as_str()?.to_string(),
                label: r.get("display_name")?.as_str()?.to_string(),
                percent: r.get("percent")?.as_f64()?,
            })
        })
        .filter(|p| p.percent > 0.0)
        .collect();
    Ok(AccountUsage { limits: Limits { captured_at: fetched_at, windows }, breakdown })
}

/// Token de login do Claude Code. O app só o lê; quem renova é o próprio Claude Code.
fn read_access_token(claude_dir: &Path, now_ms: i64) -> Result<String, String> {
    let text = std::fs::read_to_string(claude_dir.join(".credentials.json"))
        .map_err(|_| "Não achei o login do Claude Code neste computador.".to_string())?;
    let v: Value = serde_json::from_str(&text).map_err(|_| "O arquivo de login do Claude Code está ilegível.".to_string())?;
    let oauth = v.get("claudeAiOauth").ok_or("O Claude Code não está logado com uma conta do Claude.")?;
    if oauth.get("expiresAt").and_then(Value::as_i64).is_some_and(|exp| exp <= now_ms) {
        return Err(EXPIRED.into());
    }
    oauth
        .get("accessToken")
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or_else(|| "O Claude Code não está logado com uma conta do Claude.".into())
}

const EXPIRED: &str = "O login do Claude Code expirou. Use o Claude Code uma vez para renová-lo.";

pub fn fetch(claude_dir: &Path) -> Result<AccountUsage, String> {
    let now = chrono::Utc::now().timestamp_millis();
    let token = read_access_token(claude_dir, now)?;
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).build().into();
    let body = agent
        .get(USAGE_URL)
        .header("Authorization", &format!("Bearer {token}"))
        .header("anthropic-beta", "oauth-2025-04-20")
        .call()
        .map_err(|e| match e {
            ureq::Error::StatusCode(401 | 403) => EXPIRED.to_string(),
            ureq::Error::StatusCode(code) => format!("A Anthropic recusou a consulta de uso (HTTP {code})."),
            _ => "Sem conexão com a Anthropic.".to_string(),
        })?
        .body_mut()
        .read_to_string()
        .map_err(|_| "Resposta incompleta da Anthropic.".to_string())?;
    parse_usage(&body, now)
}

pub struct AccountState {
    pub enabled: Toggle,
    pub last: Mutex<Option<AccountUsage>>,
    pub error: Mutex<Option<String>>,
}

impl AccountState {
    pub fn load(data_dir: &Path) -> Self {
        Self { enabled: Toggle::load(data_dir, "accountUsage"), last: Mutex::new(None), error: Mutex::new(None) }
    }

    /// Consulta e guarda o resultado; num erro, mantém o último número bom.
    pub fn refresh(&self, claude_dir: &Path) {
        match fetch(claude_dir) {
            Ok(usage) => {
                *self.last.lock().unwrap() = Some(usage);
                *self.error.lock().unwrap() = None;
            }
            Err(e) => *self.error.lock().unwrap() = Some(e),
        }
    }
}

/// O mais recente entre a statusline (a cada resposta do Claude Code) e a consulta da conta.
pub fn newest(a: Option<Limits>, b: Option<Limits>) -> Option<Limits> {
    match (a, b) {
        (Some(a), Some(b)) => Some(if b.captured_at > a.captured_at { b } else { a }),
        (a, b) => a.or(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESPONSE: &str = r#"{
      "five_hour": {"utilization": 56.0, "resets_at": "2026-10-21T14:30:00.616524+00:00", "limit_dollars": null},
      "seven_day": {"utilization": 16.0, "resets_at": "2026-10-25T03:00:00.616543+00:00"},
      "seven_day_opus": null, "seven_day_sonnet": null, "iguana_necktie": {"utilization": 0},
      "seven_day_breakdown": {"rows": [
        {"key": "claude_code", "display_name": "Claude Code", "percent": 91},
        {"key": "chat", "display_name": "Chats", "percent": 2},
        {"key": "cowork", "display_name": "Cowork", "percent": 7},
        {"key": "other", "display_name": "Other", "percent": 0}]}
    }"#;

    #[test]
    fn parses_account_limits_and_product_breakdown() {
        let u = parse_usage(RESPONSE, 42).unwrap();
        assert_eq!(u.limits.captured_at, 42);
        let w: Vec<(&str, f64)> = u.limits.windows.iter().map(|w| (w.id.as_str(), w.used_percentage)).collect();
        assert_eq!(w, vec![("five_hour", 56.0), ("seven_day", 16.0)]);
        assert_eq!(u.limits.windows[0].resets_at, Some(1792593000616));
        let b: Vec<(&str, f64)> = u.breakdown.iter().map(|p| (p.label.as_str(), p.percent)).collect();
        assert_eq!(b, vec![("Claude Code", 91.0), ("Chats", 2.0), ("Cowork", 7.0)]);
    }

    #[test]
    fn rejects_responses_without_limits() {
        assert!(parse_usage("{}", 1).is_err());
        assert!(parse_usage("<html>", 1).is_err());
    }

    #[test]
    fn reads_token_and_detects_expiry() {
        let tmp = tempfile::tempdir().unwrap();
        let creds = tmp.path().join(".credentials.json");
        std::fs::write(&creds, r#"{"claudeAiOauth":{"accessToken":"tok","expiresAt":2000}}"#).unwrap();
        assert_eq!(read_access_token(tmp.path(), 1000).unwrap(), "tok");
        assert_eq!(read_access_token(tmp.path(), 3000).unwrap_err(), EXPIRED);
        std::fs::remove_file(&creds).unwrap();
        assert!(read_access_token(tmp.path(), 1000).is_err());
    }

    #[test]
    fn newest_picks_most_recent_source() {
        let l = |t| Some(Limits { captured_at: t, windows: vec![] });
        assert_eq!(newest(l(1), l(2)).unwrap().captured_at, 2);
        assert_eq!(newest(l(3), l(2)).unwrap().captured_at, 3);
        assert_eq!(newest(None, l(2)).unwrap().captured_at, 2);
        assert!(newest(None, None).is_none());
    }
}
