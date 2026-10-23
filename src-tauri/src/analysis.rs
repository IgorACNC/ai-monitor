//! "Analisar com Claude/opencode": um agente lê o diff do PR (e, se o repositório estiver neste
//! computador, o código em volta) e devolve um parecer. Roda só com ferramentas de leitura.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::commits;

/// Uma análise longa com Opus no máximo leva alguns minutos; depois disso o processo é encerrado.
const TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// Diffs enormes são cortados: o agente vê o começo e é avisado do corte.
const MAX_DIFF_BYTES: usize = 150_000;

const MODELS: [&str; 3] = ["haiku", "sonnet", "opus"];
const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "agent", rename_all = "camelCase")]
pub enum Analyzer {
    Claude { model: String, effort: String },
    Opencode,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    pub repo: String,
    pub number: i64,
    /// Commit do PR que foi analisado; se o PR andou, a análise está desatualizada.
    pub head_sha: String,
    pub analyzer: Analyzer,
    /// "pronto", "ajustes" ou "arriscado", quando o agente seguiu o formato pedido.
    pub verdict: Option<String>,
    pub text: String,
    /// Se o agente pôde ler o repositório deste computador além do diff.
    pub local_repo: bool,
    pub created_at: i64,
}

#[derive(Debug, PartialEq)]
struct PrInfo {
    title: String,
    head: String,
    base: String,
    head_sha: String,
}

fn no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

fn find_exe(name: &str) -> Option<PathBuf> {
    find_in(std::env::split_paths(&std::env::var_os("PATH")?), name)
}

/// Caminho do `.exe` de um programa. Instalações pelo npm só deixam um `.cmd` que chama
/// o `.exe`; o app chama o `.exe` direto, para o prompt não passar pelo `cmd.exe`.
#[cfg(windows)]
fn find_in(dirs: impl Iterator<Item = PathBuf>, name: &str) -> Option<PathBuf> {
    for dir in dirs {
        let exe = dir.join(format!("{name}.exe"));
        if exe.is_file() {
            return Some(exe);
        }
        if let Ok(text) = std::fs::read_to_string(dir.join(format!("{name}.cmd"))) {
            if let Some(exe) = exe_in_npm_shim(&text, &dir) {
                return Some(exe);
            }
        }
    }
    None
}

/// Mac e Linux: o programa fica no PATH sem extensão.
#[cfg(not(windows))]
fn find_in(mut dirs: impl Iterator<Item = PathBuf>, name: &str) -> Option<PathBuf> {
    dirs.find_map(|dir| Some(dir.join(name)).filter(|exe| exe.is_file()))
}

#[cfg(windows)]
fn exe_in_npm_shim(text: &str, dir: &Path) -> Option<PathBuf> {
    let line = text.lines().find(|l| l.contains(".exe\""))?;
    let start = line.find("\"%dp0%\\")? + "\"%dp0%\\".len();
    let end = start + line[start..].find('"')?;
    let exe = dir.join(&line[start..end]);
    exe.is_file().then_some(exe)
}

fn gh(args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("gh");
    cmd.args(args);
    no_window(&mut cmd);
    let out = cmd.output().map_err(|_| "O GitHub CLI (gh) não está instalado.".to_string())?;
    if !out.status.success() {
        return Err(format!("O GitHub recusou o pedido: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn pr_info(repo: &str, number: i64) -> Result<PrInfo, String> {
    let json = gh(&["pr", "view", &number.to_string(), "--repo", repo, "--json", "title,headRefName,baseRefName,headRefOid"])?;
    let v: Value = serde_json::from_str(&json).map_err(|_| "Resposta inesperada do GitHub.".to_string())?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    Ok(PrInfo { title: s("title"), head: s("headRefName"), base: s("baseRefName"), head_sha: s("headRefOid") })
}

/// Pasta do repositório neste computador cujo remoto aponta para `repo` ("dono/nome").
pub fn find_local_repo(dirs: &[PathBuf], repo: &str) -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = dirs.iter().filter_map(|d| commits::repo_root(d)).collect();
    roots.sort();
    roots.dedup();
    roots.into_iter().find(|root| {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(root).args(["remote", "-v"]);
        no_window(&mut cmd);
        cmd.output()
            .ok()
            .is_some_and(|out| remote_matches(&String::from_utf8_lossy(&out.stdout), repo))
    })
}

fn remote_matches(remotes: &str, repo: &str) -> bool {
    let repo = repo.to_ascii_lowercase();
    remotes.lines().filter_map(|l| l.split_whitespace().nth(1)).any(|url| {
        let url = url.to_ascii_lowercase();
        let path = url
            .split_once("github.com/")
            .or_else(|| url.split_once("github.com:"))
            .map(|(_, p)| p.trim_end_matches('/').trim_end_matches(".git"));
        path == Some(repo.as_str())
    })
}

fn prompt(repo: &str, number: i64, info: &PrInfo, local_repo: bool, diff_place: &str) -> String {
    let local = if local_repo {
        "Você está dentro de uma cópia local do repositório e pode ler os arquivos para entender o contexto, \
         mas ela pode estar em outra branch: o diff é a fonte da verdade. "
    } else {
        ""
    };
    format!(
        "Revise o pull request {repo}#{number} \"{title}\" (branch {head} para {base}). O diff completo está {diff_place}. \
         {local}Não altere nada, só analise.\n\n\
         Responda em português, em texto simples, sem tabelas nem títulos com #, exatamente neste formato:\n\
         VEREDITO: PRONTO, AJUSTES ou ARRISCADO\n\
         Resumo: duas ou três frases sobre o que o PR faz.\n\
         Problemas:\n- bugs ou riscos concretos, citando arquivo e linha (ou \"- nenhum encontrado\")\n\
         Sugestões:\n- melhorias opcionais, no máximo cinco\n",
        title = info.title,
        head = info.head,
        base = info.base,
    )
}

const OPENCODE_READ_ONLY: &str = r#"{"permission":{"edit":"deny","bash":"deny","webfetch":"deny"}}"#;

/// Resposta final do `opencode run --format json`: o texto da última etapa que escreveu algo
/// (etapas anteriores são o agente falando enquanto lê arquivos).
fn opencode_answer(events: &str) -> Result<String, String> {
    let mut steps: Vec<String> = vec![String::new()];
    for line in events.lines() {
        let Ok(e) = serde_json::from_str::<Value>(line) else { continue };
        match e.get("type").and_then(Value::as_str) {
            Some("step_start") => steps.push(String::new()),
            Some("text") => {
                if let Some(t) = e.pointer("/part/text").and_then(Value::as_str) {
                    steps.last_mut().expect("começa com uma etapa").push_str(t);
                }
            }
            Some("error") => {
                let msg = e.pointer("/error/data/message").or_else(|| e.pointer("/error/message")).and_then(Value::as_str);
                return Err(format!("O opencode não conseguiu analisar: {}", msg.unwrap_or("erro desconhecido")));
            }
            _ => {}
        }
    }
    steps
        .into_iter()
        .rev()
        .find(|t| !t.trim().is_empty())
        .ok_or_else(|| "O opencode terminou sem responder.".to_string())
}

fn cut_diff(diff: String) -> String {
    if diff.len() <= MAX_DIFF_BYTES {
        return diff;
    }
    let mut end = MAX_DIFF_BYTES;
    while !diff.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n\n[diff cortado: o PR é grande demais; só o começo foi incluído]\n", &diff[..end])
}

/// Separa a linha "VEREDITO: X" do resto do texto.
fn split_verdict(output: &str) -> (Option<String>, String) {
    let mut verdict = None;
    let mut rest = Vec::new();
    for line in output.trim().lines() {
        let clean = line.trim().trim_matches('*').trim();
        if verdict.is_none() {
            if let Some(v) = clean.strip_prefix("VEREDITO:").or_else(|| clean.strip_prefix("Veredito:")) {
                let v = v.trim().trim_matches('*').trim().to_lowercase();
                verdict = ["pronto", "ajustes", "arriscado"].into_iter().find(|k| v.starts_with(k)).map(String::from);
                if verdict.is_some() {
                    continue;
                }
            }
        }
        rest.push(line);
    }
    (verdict, rest.join("\n").trim().to_string())
}

/// Roda o agente com tempo máximo; devolve a saída padrão.
fn run_agent(mut cmd: Command, stdin: Option<String>, who: &str) -> Result<String, String> {
    no_window(&mut cmd);
    cmd.stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("Não foi possível iniciar o {who}: {e}"))?;
    if let Some(text) = stdin {
        let mut pipe = child.stdin.take().expect("stdin configurado acima");
        std::thread::spawn(move || {
            let _ = pipe.write_all(text.as_bytes());
        });
    }
    let read_all = |mut r: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = r.read_to_end(&mut buf);
            String::from_utf8_lossy(&buf).into_owned()
        })
    };
    let out = read_all(Box::new(child.stdout.take().expect("stdout configurado acima")));
    let err = read_all(Box::new(child.stderr.take().expect("stderr configurado acima")));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if started.elapsed() > TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("O {who} passou de {} minutos e foi interrompido.", TIMEOUT.as_secs() / 60));
        }
        std::thread::sleep(Duration::from_millis(300));
    };
    let out = out.join().unwrap_or_default();
    let err = err.join().unwrap_or_default();
    if !status.success() || out.trim().is_empty() {
        let detail = if err.trim().is_empty() { out.trim() } else { err.trim() };
        let detail: String = detail.chars().take(400).collect();
        return Err(format!("O {who} não conseguiu analisar: {detail}"));
    }
    Ok(out)
}

/// Pasta vazia onde o agente roda quando o repositório não está no computador.
fn empty_dir(data_dir: &Path) -> Result<PathBuf, String> {
    let dir = data_dir.join("analysis-empty");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn analyze(
    repo: &str,
    number: i64,
    analyzer: Analyzer,
    session_dirs: &[PathBuf],
    data_dir: &Path,
    now_ms: i64,
) -> Result<Analysis, String> {
    if let Analyzer::Claude { model, effort } = &analyzer {
        if !MODELS.contains(&model.as_str()) || !EFFORTS.contains(&effort.as_str()) {
            return Err("Modelo ou desempenho inválido.".into());
        }
    }
    let info = pr_info(repo, number)?;
    let diff = cut_diff(gh(&["pr", "diff", &number.to_string(), "--repo", repo])?);
    let local = find_local_repo(session_dirs, repo);
    let cwd = match &local {
        Some(dir) => dir.clone(),
        None => empty_dir(data_dir)?,
    };

    let output = match &analyzer {
        Analyzer::Claude { model, effort } => {
            let exe = find_exe("claude").ok_or("O Claude Code não foi encontrado neste computador.")?;
            let mut cmd = Command::new(exe);
            cmd.current_dir(&cwd).args([
                "-p",
                "--model",
                model,
                "--effort",
                effort,
                "--no-session-persistence",
                "--tools",
                // Sem o repositório, não há o que ler além do diff.
                if local.is_some() { "Read,Grep,Glob" } else { "" },
            ]);
            let text = format!("{}\n\n<diff>\n{diff}\n</diff>\n", prompt(repo, number, &info, local.is_some(), "abaixo, entre <diff> e </diff>"));
            run_agent(cmd, Some(text), "Claude Code")?
        }
        Analyzer::Opencode => {
            let exe = find_exe("opencode").ok_or("O opencode não foi encontrado neste computador.")?;
            let diff_file = data_dir.join(format!("analysis-{}.diff", now_ms));
            std::fs::write(&diff_file, &diff).map_err(|e| e.to_string())?;
            let mut cmd = Command::new(exe);
            // Agente "plan" e, por cima, comandos, edição e internet negados: só ler arquivos.
            // A configuração extra soma à do usuário (modelo, provedores) sem mudar o arquivo dele.
            cmd.current_dir(&cwd)
                .env("OPENCODE_CONFIG_CONTENT", OPENCODE_READ_ONLY)
                .arg("run")
                .arg(prompt(repo, number, &info, local.is_some(), "no arquivo anexado"))
                // O opencode não usa a pasta do processo (pega a do terminal que abriu o app).
                .arg("--dir")
                .arg(&cwd)
                .arg("--title")
                .arg(format!("AI Monitor: análise de {repo}#{number}"))
                .args(["--agent", "plan", "--format", "json", "-f"])
                .arg(&diff_file);
            let result = run_agent(cmd, None, "opencode");
            let _ = std::fs::remove_file(&diff_file);
            opencode_answer(&result?)?
        }
    };

    let (verdict, text) = split_verdict(&output);
    Ok(Analysis {
        repo: repo.to_string(),
        number,
        head_sha: info.head_sha,
        analyzer,
        verdict,
        text,
        local_repo: local.is_some(),
        created_at: now_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn finds_exe_behind_npm_shim() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp.path().join("node_modules").join("x").join("bin");
        std::fs::create_dir_all(&exe).unwrap();
        std::fs::write(exe.join("claude.exe"), b"").unwrap();
        let shim = "@ECHO off\r\nSETLOCAL\r\nCALL :find_dp0\r\n\"%dp0%\\node_modules\\x\\bin\\claude.exe\"   %*\r\n";
        assert_eq!(exe_in_npm_shim(shim, tmp.path()), Some(tmp.path().join("node_modules\\x\\bin\\claude.exe")));
        assert_eq!(exe_in_npm_shim("@ECHO off\r\nnode x.js %*", tmp.path()), None);
    }

    #[test]
    fn finds_program_in_path_dirs() {
        let empty = tempfile::tempdir().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let file = if cfg!(windows) { "opencode.exe" } else { "opencode" };
        std::fs::write(tmp.path().join(file), b"").unwrap();
        let dirs = || [empty.path().to_path_buf(), tmp.path().to_path_buf()].into_iter();
        assert_eq!(find_in(dirs(), "opencode"), Some(tmp.path().join(file)));
        assert_eq!(find_in(dirs(), "claude"), None);
    }

    #[test]
    fn matches_github_remotes() {
        let remotes = "origin\thttps://github.com/IgorACNC/ai-monitor.git (fetch)\norigin\thttps://github.com/IgorACNC/ai-monitor.git (push)\n";
        assert!(remote_matches(remotes, "igoracnc/AI-monitor"));
        assert!(remote_matches("o\tgit@github.com:a/b.git (fetch)", "a/b"));
        assert!(!remote_matches("o\tgit@github.com:a/bc.git (fetch)", "a/b"));
        assert!(!remote_matches("o\thttps://gitlab.com/a/b.git (fetch)", "a/b"));
    }

    #[test]
    fn splits_verdict_from_text() {
        let (v, t) = split_verdict("**VEREDITO: Ajustes necessários**\nResumo: corrige o login.\nProblemas:\n- nenhum encontrado");
        assert_eq!(v.as_deref(), Some("ajustes"));
        assert_eq!(t, "Resumo: corrige o login.\nProblemas:\n- nenhum encontrado");
        let (v, t) = split_verdict("Texto livre sem formato.");
        assert_eq!(v, None);
        assert_eq!(t, "Texto livre sem formato.");
    }

    #[test]
    fn takes_the_last_written_step_from_opencode_events() {
        let events = r#"{"type":"step_start","part":{}}
{"type":"text","part":{"type":"text","text":"Vou ler o arquivo."}}
{"type":"tool_use","part":{"type":"tool"}}
{"type":"step_finish","part":{}}
{"type":"step_start","part":{}}
{"type":"text","part":{"type":"text","text":"VEREDITO: PRONTO\nResumo: ok."}}
{"type":"step_finish","part":{}}
"#;
        assert_eq!(opencode_answer(events).unwrap(), "VEREDITO: PRONTO\nResumo: ok.");
        assert!(opencode_answer(r#"{"type":"error","error":{"data":{"message":"sem créditos"}}}"#).unwrap_err().contains("sem créditos"));
        assert!(opencode_answer("").is_err());
    }

    #[test]
    fn cuts_huge_diffs_on_char_boundary() {
        let diff = "á".repeat(MAX_DIFF_BYTES);
        let cut = cut_diff(diff);
        assert!(cut.len() < MAX_DIFF_BYTES + 200);
        assert!(cut.ends_with("incluído]\n"));
        assert_eq!(cut_diff("pequeno".into()), "pequeno");
    }

    #[test]
    fn analyzer_comes_from_the_screen_as_tagged_json() {
        let a: Analyzer = serde_json::from_str(r#"{"agent":"claude","model":"sonnet","effort":"medium"}"#).unwrap();
        assert_eq!(a, Analyzer::Claude { model: "sonnet".into(), effort: "medium".into() });
        let o: Analyzer = serde_json::from_str(r#"{"agent":"opencode"}"#).unwrap();
        assert_eq!(o, Analyzer::Opencode);
    }
}
