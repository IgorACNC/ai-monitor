//! Avisa assim que um repositório ganha commit, push ou troca de branch, observando as
//! referências dentro da pasta `.git`, para a tela não depender só da leitura periódica.

use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};

/// Só interessam mudanças de referências: `refs/`, `logs/` (reflog), `HEAD` e `packed-refs`.
/// Objetos e o índice mudam o tempo todo e não indicam commit novo.
pub fn is_ref_change(git_dir: &Path, path: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(git_dir) else {
        return false;
    };
    let mut parts = rel.components().filter_map(|c| match c {
        Component::Normal(s) => s.to_str(),
        _ => None,
    });
    match parts.next() {
        Some("refs" | "logs" | "packed-refs" | "HEAD") => true,
        // Worktrees têm HEAD e reflog próprios em `.git/worktrees/<nome>/`.
        Some("worktrees") => matches!(parts.nth(1), Some("HEAD" | "logs")),
        _ => false,
    }
}

pub struct CommitWatcher {
    debouncer: Mutex<Debouncer<RecommendedWatcher>>,
    /// Pastas `.git` observadas; compartilhada com o callback para filtrar os eventos.
    git_dirs: Arc<Mutex<Vec<PathBuf>>>,
}

impl CommitWatcher {
    pub fn new(on_change: impl Fn() + Send + 'static) -> Result<Self, String> {
        let git_dirs: Arc<Mutex<Vec<PathBuf>>> = Arc::default();
        let dirs = git_dirs.clone();
        let debouncer = new_debouncer(Duration::from_millis(800), move |res: DebounceEventResult| {
            let Ok(events) = res else { return };
            let dirs = dirs.lock().unwrap();
            if events.iter().any(|e| dirs.iter().any(|d| is_ref_change(d, &e.path))) {
                on_change();
            }
        })
        .map_err(|e| e.to_string())?;
        Ok(Self { debouncer: Mutex::new(debouncer), git_dirs })
    }

    /// Passa a observar as pastas `.git` que ainda não estavam sendo observadas.
    pub fn watch(&self, git_dirs: &[PathBuf]) {
        let mut known = self.git_dirs.lock().unwrap();
        let mut debouncer = self.debouncer.lock().unwrap();
        for dir in git_dirs {
            if known.contains(dir) {
                continue;
            }
            match debouncer.watcher().watch(dir, RecursiveMode::Recursive) {
                Ok(()) => known.push(dir.clone()),
                Err(e) => eprintln!("não foi possível observar {}: {e}", dir.display()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::mpsc;

    #[test]
    fn only_reference_changes_count() {
        let g = Path::new("C:/repo/.git");
        for yes in ["refs/heads/main", "logs/HEAD", "HEAD", "packed-refs", "refs/remotes/origin/main", "worktrees/wt/HEAD", "worktrees/wt/logs/HEAD"] {
            assert!(is_ref_change(g, &g.join(yes)), "{yes}");
        }
        for no in ["objects/ab/cdef", "index", "worktrees/wt/index", "COMMIT_EDITMSG"] {
            assert!(!is_ref_change(g, &g.join(no)), "{no}");
        }
        assert!(!is_ref_change(g, Path::new("C:/outro/.git/HEAD")));
    }

    #[test]
    fn notifies_after_a_real_commit() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let git = |args: &[&str]| {
            let ok = Command::new("git").arg("-C").arg(repo).args(args).output().unwrap().status.success();
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q", "-b", "main"]);
        let commit = |msg: &str| git(&["-c", "user.email=a@b", "-c", "user.name=T", "commit", "-q", "--allow-empty", "-m", msg]);
        commit("primeiro");

        let (tx, rx) = mpsc::channel();
        let watcher = CommitWatcher::new(move || {
            let _ = tx.send(());
        })
        .unwrap();
        let dirs = crate::commits::git_dirs(&[repo.to_path_buf()]);
        assert_eq!(dirs.len(), 1);
        watcher.watch(&dirs);
        watcher.watch(&dirs); // observar de novo não duplica nada

        commit("segundo");
        assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok(), "nenhum aviso após o commit");
    }
}
