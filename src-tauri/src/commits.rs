//! Commits do usuário nos repositórios do computador, lidos com `git log`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub sha: String,
    pub repo: String,
    pub branch: Option<String>,
    /// Unix epoch em milissegundos.
    pub ts: i64,
    pub subject: String,
    /// `None` quando o repositório não tem remoto (só existe no computador).
    pub pushed: Option<bool>,
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir).args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

fn repo_root(dir: &Path) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    git(dir, &["rev-parse", "--show-toplevel"]).map(|s| PathBuf::from(s.trim()))
}

const SEP: char = '\u{1f}';

/// Commit lido do repositório, ainda com o autor, antes de saber se é do usuário.
struct RawCommit {
    author_name: String,
    author_email: String,
    commit: Commit,
}

/// Nome e e-mail do git configurados para o repositório (incluindo os globais).
struct Identity {
    name: Option<String>,
    email: Option<String>,
}

fn config(root: &Path, key: &str) -> Option<String> {
    git(root, &["config", key])
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
}

/// Todos os commits dos últimos `days` dias nos branches locais do repositório.
fn read_repo(root: &Path, days: u32) -> (Identity, Vec<RawCommit>) {
    let identity = Identity {
        name: config(root, "user.name"),
        email: config(root, "user.email"),
    };
    let since = format!("--since={days}.days");
    let has_remote = git(root, &["remote"]).is_some_and(|r| !r.trim().is_empty());
    let pushed: HashSet<String> = if has_remote {
        git(root, &["log", "--remotes", &since, "--format=%H"])
            .map(|out| out.lines().map(String::from).collect())
            .unwrap_or_default()
    } else {
        HashSet::new()
    };
    let repo = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let format = format!("--format=%H{SEP}%ct{SEP}%an{SEP}%ae{SEP}%S{SEP}%s");
    let Some(out) = git(
        root,
        &[
            "log",
            "--branches",
            "--source",
            "--no-merges",
            &since,
            &format,
        ],
    ) else {
        return (identity, Vec::new());
    };
    let commits = out
        .lines()
        .filter_map(|line| {
            let mut f = line.splitn(6, SEP);
            let (sha, ts, name, email, source, subject) = (
                f.next()?,
                f.next()?,
                f.next()?,
                f.next()?,
                f.next()?,
                f.next()?,
            );
            Some(RawCommit {
                author_name: name.trim().to_lowercase(),
                author_email: email.trim().to_lowercase(),
                commit: Commit {
                    sha: sha.to_string(),
                    repo: repo.clone(),
                    branch: Some(
                        source
                            .strip_prefix("refs/heads/")
                            .unwrap_or(source)
                            .to_string(),
                    )
                    .filter(|b| !b.is_empty()),
                    ts: ts.parse::<i64>().ok()? * 1000,
                    subject: subject.to_string(),
                    pushed: has_remote.then(|| pushed.contains(sha)),
                },
            })
        })
        .collect();
    (identity, commits)
}

/// E-mails do usuário: os configurados no git e os usados em commits assinados com o nome
/// configurado. Assim um e-mail antigo (de outro computador, por exemplo) também conta.
fn my_emails(identities: &[Identity], commits: &[RawCommit]) -> HashSet<String> {
    let names: HashSet<&str> = identities
        .iter()
        .filter_map(|i| i.name.as_deref())
        .collect();
    let mut emails: HashSet<String> = identities.iter().filter_map(|i| i.email.clone()).collect();
    emails.extend(
        commits
            .iter()
            .filter(|c| names.contains(c.author_name.as_str()))
            .map(|c| c.author_email.clone()),
    );
    emails
}

/// Junta os commits do usuário nos repositórios que contêm as pastas dadas, sem repetir commits
/// (worktrees do mesmo repositório compartilham o histórico), do mais novo para o mais antigo.
pub fn collect(dirs: &[PathBuf], days: u32) -> Vec<Commit> {
    let mut roots: Vec<PathBuf> = dirs.iter().filter_map(|d| repo_root(d)).collect();
    roots.sort();
    roots.dedup();
    let (identities, raw): (Vec<Identity>, Vec<Vec<RawCommit>>) =
        roots.iter().map(|r| read_repo(r, days)).unzip();
    let raw: Vec<RawCommit> = raw.into_iter().flatten().collect();
    let emails = my_emails(&identities, &raw);

    let mut seen = HashSet::new();
    let mut all: Vec<Commit> = raw
        .into_iter()
        .filter(|c| emails.contains(&c.author_email))
        .map(|c| c.commit)
        .filter(|c| seen.insert(c.sha.clone()))
        .collect();
    all.sort_by_key(|c| std::cmp::Reverse(c.ts));
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?} falhou");
    }

    fn commit(dir: &Path, email: &str, msg: &str) {
        run(
            dir,
            &[
                "-c",
                &format!("user.email={email}"),
                "-c",
                "user.name=T",
                "commit",
                "--allow-empty",
                "-m",
                msg,
            ],
        );
    }

    #[test]
    fn lists_only_my_commits_and_marks_pushed() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("meu-repo");
        let remote = tmp.path().join("remoto.git");
        std::fs::create_dir(&repo).unwrap();
        run(&repo, &["init", "-q", "-b", "main"]);
        run(&repo, &["config", "user.email", "Eu@Exemplo.com"]);
        run(tmp.path(), &["init", "-q", "--bare", "remoto.git"]);
        run(
            &repo,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );

        commit(&repo, "eu@exemplo.com", "primeiro");
        commit(&repo, "outra@pessoa.com", "de outra pessoa");
        run(&repo, &["push", "-q", "origin", "main"]);
        commit(&repo, "eu@exemplo.com", "ainda não enviado");

        let sub = repo.join("src");
        std::fs::create_dir(&sub).unwrap();
        let commits = collect(&[sub, repo.clone(), tmp.path().join("nao-existe")], 30);
        let summary: Vec<(&str, Option<bool>)> = commits
            .iter()
            .map(|c| (c.subject.as_str(), c.pushed))
            .collect();
        assert_eq!(
            summary,
            vec![("ainda não enviado", Some(false)), ("primeiro", Some(true))]
        );
        assert!(
            commits
                .iter()
                .all(|c| c.repo == "meu-repo" && c.branch.as_deref() == Some("main"))
        );
    }

    #[test]
    fn recognizes_my_other_emails_through_my_name() {
        let tmp = tempfile::tempdir().unwrap();
        run(tmp.path(), &["init", "-q", "-b", "main"]);
        run(tmp.path(), &["config", "user.email", "eu@gmail.com"]);
        run(tmp.path(), &["config", "user.name", "Igor Couto"]);
        let commit_as = |name: &str, email: &str, msg: &str| {
            let (n, e) = (format!("user.name={name}"), format!("user.email={email}"));
            run(
                tmp.path(),
                &["-c", &n, "-c", &e, "commit", "--allow-empty", "-m", msg],
            );
        };
        commit_as("Igor Couto", "eu@hotmail.com", "com o e-mail antigo");
        commit_as(
            "Igor A. C. N. Couto",
            "eu@hotmail.com",
            "nome diferente, e-mail meu",
        );
        commit_as("Outra Pessoa", "outra@pessoa.com", "de outra pessoa");
        commit_as("Igor Couto", "eu@gmail.com", "com o e-mail atual");

        let subjects: Vec<String> = collect(&[tmp.path().to_path_buf()], 30)
            .into_iter()
            .map(|c| c.subject)
            .collect();
        assert_eq!(subjects.len(), 3);
        assert!(!subjects.contains(&"de outra pessoa".to_string()));
    }

    #[test]
    fn repo_without_remote_has_unknown_push_state() {
        let tmp = tempfile::tempdir().unwrap();
        run(tmp.path(), &["init", "-q", "-b", "main"]);
        run(tmp.path(), &["config", "user.email", "eu@exemplo.com"]);
        commit(tmp.path(), "eu@exemplo.com", "local");
        let commits = collect(&[tmp.path().to_path_buf()], 30);
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].pushed, None);
    }
}
