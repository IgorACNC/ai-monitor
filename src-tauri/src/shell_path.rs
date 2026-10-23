//! Mac e Linux: um app aberto pelo Finder ou pelo menu não herda o PATH do terminal, e aí não
//! acharia `gh`, `git`, `claude` nem `opencode`. No início, o app pega o PATH do shell do usuário
//! e soma as pastas onde esses programas costumam ficar. No Windows nada disso é necessário.

#![cfg_attr(windows, allow(dead_code))]

use std::path::{Path, PathBuf};

/// Pastas comuns de instalação, caso o shell não responda.
fn common_dirs(home: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"].iter().map(PathBuf::from).collect();
    for sub in [".local/bin", ".npm-global/bin", ".bun/bin", ".opencode/bin", ".cargo/bin"] {
        dirs.push(home.join(sub));
    }
    dirs
}

/// PATH final: o do shell primeiro, depois o atual, depois as pastas comuns, sem repetir.
fn merge(shell: Option<&str>, current: Option<&str>, home: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let parts = [shell, current]
        .into_iter()
        .flatten()
        .flat_map(|p| p.split(':'))
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .chain(common_dirs(home));
    for dir in parts {
        if !out.contains(&dir) {
            out.push(dir);
        }
    }
    out
}

/// O texto entre dois marcadores: o shell interativo pode imprimir avisos antes ou depois.
fn between_marks(output: &str, mark: &str) -> Option<String> {
    let start = output.find(mark)? + mark.len();
    let end = start + output[start..].find(mark)?;
    Some(output[start..end].to_string())
}

#[cfg(not(windows))]
fn shell_path() -> Option<String> {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    const MARK: &str = "__AI_MONITOR_PATH__";
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let mut child = Command::new(shell)
        .args(["-ilc", &format!("printf '{MARK}%s{MARK}' \"$PATH\"")])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // Um shell travado (ex.: esperando algo no .zshrc) não pode segurar a abertura do app.
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < Duration::from_secs(3) => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let output = child.wait_with_output().ok()?;
    between_marks(&String::from_utf8_lossy(&output.stdout), MARK)
}

/// Ajusta o PATH do próprio app. Chamar no começo, antes de abrir outras threads.
#[cfg(not(windows))]
pub fn fix() {
    let home = dirs::home_dir().unwrap_or_default();
    let current = std::env::var("PATH").ok();
    let merged = merge(shell_path().as_deref(), current.as_deref(), &home);
    if let Ok(path) = std::env::join_paths(merged) {
        // SAFETY: roda no início de `run`, antes de o Tauri criar qualquer thread.
        unsafe { std::env::set_var("PATH", path) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_path_comes_first_without_repeats() {
        let home = Path::new("/home/igor");
        let merged = merge(Some("/opt/homebrew/bin:/home/igor/.local/bin"), Some("/usr/bin:/bin:/usr/bin"), home);
        assert_eq!(merged[0], PathBuf::from("/opt/homebrew/bin"));
        assert_eq!(merged[1], PathBuf::from("/home/igor/.local/bin"));
        assert_eq!(merged.iter().filter(|d| **d == PathBuf::from("/usr/bin")).count(), 1);
        assert!(merged.contains(&home.join(".bun/bin")));
    }

    #[test]
    fn falls_back_to_common_dirs_when_shell_fails() {
        let home = Path::new("/Users/igor");
        let merged = merge(None, None, home);
        assert!(merged.contains(&PathBuf::from("/opt/homebrew/bin")));
        assert!(merged.contains(&home.join(".local/bin")));
    }

    #[test]
    fn reads_path_between_marks() {
        assert_eq!(between_marks("aviso do zsh\nM/a:/bM\n", "M").as_deref(), Some("/a:/b"));
        assert_eq!(between_marks("sem marcas", "M"), None);
    }
}
