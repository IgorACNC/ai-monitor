//! O que os agentes (Claude Code, opencode, Codex, Gemini) têm em comum: quais o usuário
//! conectou e o formato do uso que a tela mostra para opencode, Codex e Gemini.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::statusline::Limits;

/// Sessão com resposta nos últimos minutos conta como ativa.
pub const ACTIVE_WINDOW_MS: i64 = 10 * 60 * 1000;
pub const DAY_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Claude,
    Opencode,
    Codex,
    Gemini,
}

pub const ALL: [Agent; 4] = [Agent::Claude, Agent::Opencode, Agent::Codex, Agent::Gemini];

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    pub id: Agent,
    pub connected: bool,
    /// Há sinal dele neste computador (pasta de dados ou programa no PATH).
    pub found: bool,
}

/// Algum dos nomes está numa pasta do PATH (no Windows, também como `.exe` ou `.cmd` do npm).
fn on_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&path)
        .any(|dir| [name.to_string(), format!("{name}.exe"), format!("{name}.cmd")].iter().any(|n| dir.join(n).is_file()))
}

pub fn found(agent: Agent, home: &Path, claude_dir: Option<&Path>) -> bool {
    match agent {
        Agent::Claude => claude_dir.is_some() || on_path("claude"),
        Agent::Opencode => crate::opencode::db_path(home).is_file() || on_path("opencode"),
        Agent::Codex => crate::codex::sessions_dir(home).is_dir() || on_path("codex"),
        Agent::Gemini => crate::gemini::tmp_dir(home).is_dir() || on_path("gemini"),
    }
}

const KEY: &str = "agents";

/// Agentes conectados, na ordem em que foram conectados. Na primeira vez (nada salvo ainda),
/// conecta os que forem encontrados no computador e salva a escolha.
pub fn connected(data_dir: &Path, found: impl Fn(Agent) -> bool) -> Vec<Agent> {
    if let Some(saved) = crate::prefs::get(data_dir, KEY) {
        return serde_json::from_value::<Vec<Value>>(saved)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();
    }
    let list: Vec<Agent> = ALL.into_iter().filter(|a| found(*a)).collect();
    // Sem conseguir salvar, detecta de novo na próxima abertura.
    if let Err(e) = save(data_dir, &list) {
        eprintln!("{e}");
    }
    list
}

pub fn save(data_dir: &Path, list: &[Agent]) -> Result<(), String> {
    crate::prefs::set(data_dir, KEY, serde_json::to_value(list).unwrap_or_default())
}

pub fn set_connected(list: &mut Vec<Agent>, agent: Agent, connected: bool) {
    list.retain(|a| *a != agent);
    if connected {
        list.push(agent);
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSession {
    pub id: String,
    pub title: String,
    pub directory: String,
    pub model: Option<String>,
    /// A última resposta ainda está sendo escrita.
    pub busy: bool,
    pub started_at: i64,
    pub last_activity: i64,
    pub responses: i64,
    /// Em dólares; só o opencode registra.
    pub cost: Option<f64>,
    pub context_tokens: i64,
    pub context_limit: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Period {
    pub sessions: i64,
    pub responses: i64,
    pub cost: Option<f64>,
    pub top_model: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentUsage {
    pub installed: bool,
    pub active: Vec<AgentSession>,
    pub today: Period,
    pub week: Period,
    /// % do plano; só o Codex informa.
    pub limits: Option<Limits>,
    pub error: Option<String>,
}

/// Uma resposta do agente, para os resumos de hoje e da semana.
pub struct Reply<'a> {
    pub session_id: &'a str,
    pub model: Option<&'a str>,
    pub cost: Option<f64>,
}

pub fn period<'a>(replies: impl IntoIterator<Item = Reply<'a>>) -> Period {
    let mut sessions: Vec<&str> = vec![];
    let mut by_model: HashMap<&str, i64> = HashMap::new();
    let mut responses = 0;
    let mut cost: Option<f64> = None;
    for r in replies {
        responses += 1;
        sessions.push(r.session_id);
        if let Some(m) = r.model {
            *by_model.entry(m).or_default() += 1;
        }
        if let Some(c) = r.cost {
            *cost.get_or_insert(0.0) += c;
        }
    }
    sessions.sort_unstable();
    sessions.dedup();
    Period {
        sessions: sessions.len() as i64,
        responses,
        cost,
        top_model: by_model.into_iter().max_by_key(|(m, n)| (*n, std::cmp::Reverse(*m))).map(|(m, _)| m.to_string()),
    }
}

/// Arquivo lido: data de modificação e tamanho de quando foi lido, e o resultado.
type Cached<T> = (SystemTime, u64, Arc<T>);

/// Lê cada arquivo de histórico só quando ele muda: a tela pede o uso a cada poucos segundos.
pub struct FileCache<T> {
    files: Mutex<HashMap<PathBuf, Cached<T>>>,
}

impl<T> Default for FileCache<T> {
    fn default() -> Self {
        Self { files: Mutex::new(HashMap::new()) }
    }
}

impl<T> FileCache<T> {
    /// Lê os arquivos `paths` (os de antes que sumiram da lista são esquecidos).
    pub fn read_all(&self, paths: Vec<PathBuf>, parse: impl Fn(&Path) -> Option<T>) -> Vec<Arc<T>> {
        let mut files = self.files.lock().unwrap();
        let mut out = Vec::with_capacity(paths.len());
        let mut seen = HashMap::with_capacity(paths.len());
        for path in paths {
            let Ok(meta) = std::fs::metadata(&path) else { continue };
            let stamp = (meta.modified().unwrap_or(SystemTime::UNIX_EPOCH), meta.len());
            let parsed = match files.remove(&path) {
                Some((m, l, p)) if (m, l) == stamp => Some(p),
                _ => parse(&path).map(Arc::new),
            };
            if let Some(p) = parsed {
                out.push(p.clone());
                seen.insert(path, (stamp.0, stamp.1, p));
            }
        }
        *files = seen;
        out
    }
}

/// Arquivos com a extensão `ext` dentro de `dir`, descendo `depth` níveis de pastas.
pub fn files_in(dir: &Path, ext: &str, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            if depth > 0 {
                files_in(&path, ext, depth - 1, out);
            }
        } else if path.extension().is_some_and(|x| x == ext) {
            out.push(path);
        }
    }
}

/// Texto ISO (ex.: `2026-10-23T10:00:00.000Z`) em ms.
pub fn iso_ms(v: Option<&Value>) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(v?.as_str()?).ok().map(|d| d.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_run_connects_what_is_found_then_keeps_the_choice() {
        let tmp = tempfile::tempdir().unwrap();
        let found = |a: Agent| matches!(a, Agent::Claude | Agent::Codex);
        assert_eq!(connected(tmp.path(), found), vec![Agent::Claude, Agent::Codex]);

        let mut list = connected(tmp.path(), |_| true);
        assert_eq!(list, vec![Agent::Claude, Agent::Codex], "depois da primeira vez, não detecta de novo");
        set_connected(&mut list, Agent::Claude, false);
        set_connected(&mut list, Agent::Gemini, true);
        save(tmp.path(), &list).unwrap();
        assert_eq!(connected(tmp.path(), |_| true), vec![Agent::Codex, Agent::Gemini]);

        save(tmp.path(), &[]).unwrap();
        assert_eq!(connected(tmp.path(), |_| true), vec![], "nenhum conectado também é uma escolha");
    }

    #[test]
    fn period_counts_sessions_and_sums_cost_only_when_known() {
        let p = period([
            Reply { session_id: "a", model: Some("m1"), cost: None },
            Reply { session_id: "a", model: Some("m2"), cost: None },
            Reply { session_id: "b", model: Some("m2"), cost: None },
        ]);
        assert_eq!((p.sessions, p.responses, p.cost, p.top_model.as_deref()), (2, 3, None, Some("m2")));
        let p = period([Reply { session_id: "a", model: None, cost: Some(0.5) }]);
        assert_eq!(p.cost, Some(0.5));
    }

    #[test]
    fn cache_rereads_only_changed_files() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("a.jsonl");
        std::fs::write(&f, "1").unwrap();
        let cache = FileCache::default();
        let reads = std::cell::Cell::new(0);
        let parse = |p: &Path| {
            reads.set(reads.get() + 1);
            std::fs::read_to_string(p).ok()
        };
        assert_eq!(*cache.read_all(vec![f.clone()], parse)[0], "1");
        assert_eq!(*cache.read_all(vec![f.clone()], parse)[0], "1");
        assert_eq!(reads.get(), 1);
        std::fs::write(&f, "22").unwrap();
        assert_eq!(*cache.read_all(vec![f.clone()], parse)[0], "22");
        assert_eq!(reads.get(), 2);
    }
}
