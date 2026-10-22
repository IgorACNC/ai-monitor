//! Gráfico de contribuições do GitHub, o mesmo do perfil, lido com o `gh` já autenticado
//! (o app não guarda token nenhum).

use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContributionDay {
    /// "2026-10-21", como o GitHub mostra no perfil.
    pub date: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Calendar {
    pub login: String,
    pub total: i64,
    pub days: Vec<ContributionDay>,
}

const QUERY: &str = "query { viewer { login contributionsCollection { contributionCalendar { \
    totalContributions weeks { contributionDays { date contributionCount } } } } } }";

/// Roda o `gh` sem abrir janela de console e devolve a saída, com erros em português.
fn gh(args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("gh");
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "O GitHub CLI (gh) não está instalado.".to_string()
        } else {
            format!("Não foi possível executar o gh: {e}")
        }
    })?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(if err.contains("gh auth login") {
            "O GitHub CLI não está conectado à sua conta. Rode `gh auth login` num terminal.".to_string()
        } else if err.contains("connecting") || err.contains("connection") {
            "Sem conexão com o GitHub.".to_string()
        } else {
            format!("O GitHub recusou o pedido: {}", err.trim())
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn graphql(query: &str) -> Result<String, String> {
    gh(&["api", "graphql", "-f", &format!("query={query}")])
}

pub fn fetch_calendar() -> Result<Calendar, String> {
    parse_calendar(&graphql(QUERY)?)
}

pub fn parse_calendar(json: &str) -> Result<Calendar, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("Resposta inválida do GitHub: {e}"))?;
    let viewer = v.pointer("/data/viewer").ok_or("Resposta do GitHub sem os dados da conta.")?;
    let cal = viewer
        .pointer("/contributionsCollection/contributionCalendar")
        .ok_or("Resposta do GitHub sem o gráfico de contribuições.")?;
    let days = cal
        .get("weeks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|w| w.get("contributionDays").and_then(Value::as_array))
        .flatten()
        .filter_map(|d| {
            Some(ContributionDay {
                date: d.get("date")?.as_str()?.to_string(),
                count: d.get("contributionCount")?.as_i64()?,
            })
        })
        .collect();
    Ok(Calendar {
        login: viewer.get("login").and_then(Value::as_str).unwrap_or_default().to_string(),
        total: cal.get("totalContributions").and_then(Value::as_i64).unwrap_or(0),
        days,
    })
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CheckRun {
    pub name: String,
    /// "success", "failure", "pending" ou "neutral".
    pub outcome: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    pub additions: i64,
    pub deletions: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub repo: String,
    pub number: i64,
    pub title: String,
    pub url: String,
    pub author: String,
    pub draft: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub body: String,
    pub head: String,
    pub base: String,
    /// "APPROVED", "CHANGES_REQUESTED", "REVIEW_REQUIRED" ou `None` (sem revisão exigida).
    pub review_decision: Option<String>,
    pub head_sha: String,
    /// Resumo dos checks: "success", "failure", "pending" ou `None` quando o PR não tem checks.
    pub checks: Option<&'static str>,
    pub check_runs: Vec<CheckRun>,
    pub additions: i64,
    pub deletions: i64,
    pub changed_files: i64,
    pub files: Vec<ChangedFile>,
    /// "MERGEABLE", "CONFLICTING" ou "UNKNOWN" (o GitHub ainda está calculando).
    pub mergeable: String,
    /// Forma de merge usada pelo app: a primeira que o repositório permite (merge, squash, rebase).
    pub merge_method: Option<MergeMethod>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PullRequests {
    /// PRs de outras pessoas que pediram a sua revisão.
    pub review_requested: Vec<PullRequest>,
    /// PRs abertos por você.
    pub mine: Vec<PullRequest>,
}

const PR_QUERY: &str = "query { \
    review: search(type: ISSUE, first: 30, query: \"is:open is:pr archived:false review-requested:@me sort:updated-desc\") { nodes { ...pr } } \
    mine: search(type: ISSUE, first: 30, query: \"is:open is:pr archived:false author:@me sort:updated-desc\") { nodes { ...pr } } } \
    fragment pr on PullRequest { number title url isDraft createdAt updatedAt bodyText additions deletions changedFiles \
    headRefName baseRefName reviewDecision mergeable author { login } \
    repository { nameWithOwner mergeCommitAllowed squashMergeAllowed rebaseMergeAllowed } \
    files(first: 100) { nodes { path additions deletions } } \
    commits(last: 1) { nodes { commit { oid statusCheckRollup { state contexts(first: 50) { nodes { \
    __typename ... on CheckRun { name status conclusion } ... on StatusContext { context state } } } } } } } }";

pub fn fetch_pull_requests() -> Result<PullRequests, String> {
    parse_pull_requests(&graphql(PR_QUERY)?)
}

fn rollup(state: &str) -> &'static str {
    match state {
        "SUCCESS" => "success",
        "FAILURE" | "ERROR" => "failure",
        _ => "pending",
    }
}

fn check_run(node: &Value) -> Option<CheckRun> {
    let s = |k: &str| node.get(k).and_then(Value::as_str);
    if let Some(name) = s("name") {
        let outcome = match (s("status")?, s("conclusion")) {
            (st, _) if st != "COMPLETED" => "pending",
            (_, Some("SUCCESS")) => "success",
            (_, Some("FAILURE" | "TIMED_OUT" | "STARTUP_FAILURE" | "ACTION_REQUIRED" | "CANCELLED")) => "failure",
            _ => "neutral",
        };
        return Some(CheckRun { name: name.to_string(), outcome });
    }
    Some(CheckRun { name: s("context")?.to_string(), outcome: rollup(s("state")?) })
}

fn timestamp(v: &Value, key: &str) -> i64 {
    v.get(key)
        .and_then(Value::as_str)
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.timestamp_millis())
        .unwrap_or(0)
}

fn parse_pr(v: &Value) -> Option<PullRequest> {
    let str_of = |p: &str| v.pointer(p).and_then(Value::as_str).unwrap_or_default().to_string();
    let int_of = |k: &str| v.get(k).and_then(Value::as_i64).unwrap_or(0);
    let commit = v.pointer("/commits/nodes/0/commit");
    let rollup_v = commit.and_then(|c| c.get("statusCheckRollup")).filter(|r| r.is_object());
    Some(PullRequest {
        repo: v.pointer("/repository/nameWithOwner")?.as_str()?.to_string(),
        number: v.get("number")?.as_i64()?,
        title: str_of("/title"),
        url: str_of("/url"),
        // Conta apagada aparece sem autor.
        author: v.pointer("/author/login").and_then(Value::as_str).unwrap_or("ghost").to_string(),
        draft: v.get("isDraft").and_then(Value::as_bool).unwrap_or(false),
        created_at: timestamp(v, "createdAt"),
        updated_at: timestamp(v, "updatedAt"),
        body: str_of("/bodyText"),
        head: str_of("/headRefName"),
        base: str_of("/baseRefName"),
        review_decision: v.get("reviewDecision").and_then(Value::as_str).map(String::from),
        head_sha: commit.and_then(|c| c.get("oid")).and_then(Value::as_str).unwrap_or_default().to_string(),
        checks: rollup_v.and_then(|r| r.get("state")).and_then(Value::as_str).map(rollup),
        check_runs: rollup_v
            .and_then(|r| r.pointer("/contexts/nodes"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(check_run)
            .collect(),
        additions: int_of("additions"),
        deletions: int_of("deletions"),
        changed_files: int_of("changedFiles"),
        mergeable: str_of("/mergeable"),
        merge_method: [
            ("mergeCommitAllowed", MergeMethod::Merge),
            ("squashMergeAllowed", MergeMethod::Squash),
            ("rebaseMergeAllowed", MergeMethod::Rebase),
        ]
        .into_iter()
        .find(|(k, _)| v.pointer(&format!("/repository/{k}")).and_then(Value::as_bool) == Some(true))
        .map(|(_, m)| m),
        files: v
            .pointer("/files/nodes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|f| {
                Some(ChangedFile {
                    path: f.get("path")?.as_str()?.to_string(),
                    additions: f.get("additions").and_then(Value::as_i64).unwrap_or(0),
                    deletions: f.get("deletions").and_then(Value::as_i64).unwrap_or(0),
                })
            })
            .collect(),
    })
}

pub fn parse_pull_requests(json: &str) -> Result<PullRequests, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("Resposta inválida do GitHub: {e}"))?;
    let list = |key: &str| -> Result<Vec<PullRequest>, String> {
        let nodes = v
            .pointer(&format!("/data/{key}/nodes"))
            .and_then(Value::as_array)
            .ok_or("Resposta do GitHub sem a lista de pull requests.")?;
        Ok(nodes.iter().filter_map(parse_pr).collect())
    };
    Ok(PullRequests { review_requested: list("review")?, mine: list("mine")? })
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ReviewAction {
    Approve,
    RequestChanges,
}

/// "dono/repositório" com os caracteres que o GitHub aceita; barra valores que o `gh` leria como opção.
fn check_repo(repo: &str) -> Result<(), String> {
    let ok_part = |p: &str| !p.is_empty() && !p.starts_with('-') && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    match repo.split_once('/') {
        Some((owner, name)) if ok_part(owner) && ok_part(name) => Ok(()),
        _ => Err(format!("Repositório inválido: {repo}")),
    }
}

fn review_args(repo: &str, number: u64, action: ReviewAction, body: &str) -> Result<Vec<String>, String> {
    check_repo(repo)?;
    let body = body.trim();
    if action == ReviewAction::RequestChanges && body.is_empty() {
        return Err("Escreva o que precisa mudar.".into());
    }
    let mut args = vec!["pr".into(), "review".into(), number.to_string(), "--repo".into(), repo.into()];
    args.push(match action {
        ReviewAction::Approve => "--approve".into(),
        ReviewAction::RequestChanges => "--request-changes".into(),
    });
    if !body.is_empty() {
        // `--body=` junto: um texto começando com "-" não vira opção.
        args.push(format!("--body={body}"));
    }
    Ok(args)
}

fn merge_args(repo: &str, number: u64, method: MergeMethod, head_sha: &str) -> Result<Vec<String>, String> {
    check_repo(repo)?;
    if head_sha.len() != 40 || !head_sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Commit do PR inválido; atualize a lista.".into());
    }
    let method = match method {
        MergeMethod::Merge => "--merge",
        MergeMethod::Squash => "--squash",
        MergeMethod::Rebase => "--rebase",
    };
    Ok(vec![
        "pr".into(),
        "merge".into(),
        number.to_string(),
        "--repo".into(),
        repo.into(),
        method.into(),
        // Só junta se o PR ainda estiver no commit que a pessoa viu no app.
        format!("--match-head-commit={head_sha}"),
    ])
}

fn run(args: Vec<String>) -> Result<(), String> {
    gh(&args.iter().map(String::as_str).collect::<Vec<_>>()).map(|_| ())
}

pub fn review(repo: &str, number: u64, action: ReviewAction, body: &str) -> Result<(), String> {
    run(review_args(repo, number, action, body)?)
}

pub fn merge(repo: &str, number: u64, method: MergeMethod, head_sha: &str) -> Result<(), String> {
    run(merge_args(repo, number, method, head_sha)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_contribution_calendar() {
        let json = r#"{"data":{"viewer":{"login":"IgorACNC","contributionsCollection":{"contributionCalendar":{
            "totalContributions":9,"weeks":[
              {"contributionDays":[{"date":"2026-10-19","contributionCount":0},{"date":"2026-10-20","contributionCount":3}]},
              {"contributionDays":[{"date":"2026-10-21","contributionCount":6}]}]}}}}}"#;
        let cal = parse_calendar(json).unwrap();
        assert_eq!(cal.login, "IgorACNC");
        assert_eq!(cal.total, 9);
        assert_eq!(cal.days.len(), 3);
        assert_eq!(cal.days[2], ContributionDay { date: "2026-10-21".into(), count: 6 });
    }

    #[test]
    fn explains_unexpected_responses() {
        assert!(parse_calendar("não é json").is_err());
        assert!(parse_calendar(r#"{"errors":[{"message":"x"}]}"#).is_err());
    }

    #[test]
    fn parses_pull_requests_with_checks_and_files() {
        let json = r#"{"data":{"review":{"nodes":[{"number":7,"title":"Corrige login","url":"https://github.com/a/b/pull/7",
            "isDraft":false,"createdAt":"2026-10-20T10:00:00Z","updatedAt":"2026-10-21T10:00:00Z","bodyText":"Explica",
            "additions":10,"deletions":2,"changedFiles":1,"headRefName":"fix","baseRefName":"main",
            "reviewDecision":"REVIEW_REQUIRED","author":null,"repository":{"nameWithOwner":"a/b"},
            "files":{"nodes":[{"path":"src/x.rs","additions":10,"deletions":2}]},
            "commits":{"nodes":[{"commit":{"oid":"abc","statusCheckRollup":{"state":"FAILURE","contexts":{"nodes":[
              {"__typename":"CheckRun","name":"test","status":"COMPLETED","conclusion":"FAILURE"},
              {"__typename":"CheckRun","name":"lint","status":"IN_PROGRESS","conclusion":null},
              {"__typename":"StatusContext","context":"ci/old","state":"SUCCESS"}]}}}}]}}]},
          "mine":{"nodes":[{"number":4,"title":"Uso da conta","url":"u","isDraft":true,"createdAt":"2026-10-21T09:00:00Z",
            "updatedAt":"2026-10-21T09:00:00Z","bodyText":"","additions":1,"deletions":0,"changedFiles":1,
            "headRefName":"feat","baseRefName":"main","reviewDecision":null,"author":{"login":"IgorACNC"},
            "repository":{"nameWithOwner":"IgorACNC/ai-monitor"},"files":{"nodes":[]},
            "commits":{"nodes":[{"commit":{"oid":"def","statusCheckRollup":null}}]}}, {}]}}}"#;
        let prs = parse_pull_requests(json).unwrap();
        let r = &prs.review_requested[0];
        assert_eq!((r.repo.as_str(), r.number, r.author.as_str()), ("a/b", 7, "ghost"));
        assert_eq!(r.checks, Some("failure"));
        let runs: Vec<(&str, &str)> = r.check_runs.iter().map(|c| (c.name.as_str(), c.outcome)).collect();
        assert_eq!(runs, vec![("test", "failure"), ("lint", "pending"), ("ci/old", "success")]);
        assert_eq!(r.files, vec![ChangedFile { path: "src/x.rs".into(), additions: 10, deletions: 2 }]);
        assert_eq!(r.head_sha, "abc");
        // O item vazio (resultado de busca que não é PR) é ignorado.
        assert_eq!(prs.mine.len(), 1);
        assert!(prs.mine[0].draft);
        assert_eq!(prs.mine[0].checks, None);
        assert_eq!(prs.mine[0].review_decision, None);
    }

    #[test]
    fn rejects_pull_request_responses_without_lists() {
        assert!(parse_pull_requests(r#"{"errors":[{"message":"x"}]}"#).is_err());
    }

    #[test]
    fn picks_first_merge_method_the_repo_allows() {
        let pr = |repo: &str| {
            let json = format!(r#"{{"data":{{"review":{{"nodes":[]}},"mine":{{"nodes":[{{"number":1,"mergeable":"MERGEABLE",
                "repository":{{"nameWithOwner":"a/b",{repo}}}}}]}}}}}}"#);
            parse_pull_requests(&json).unwrap().mine.remove(0)
        };
        let p = pr(r#""mergeCommitAllowed":false,"squashMergeAllowed":true,"rebaseMergeAllowed":true"#);
        assert_eq!(p.merge_method, Some(MergeMethod::Squash));
        assert_eq!(p.mergeable, "MERGEABLE");
        assert_eq!(pr(r#""mergeCommitAllowed":true"#).merge_method, Some(MergeMethod::Merge));
        assert_eq!(pr(r#""x":1"#).merge_method, None);
    }

    #[test]
    fn builds_review_commands() {
        let a = review_args("a/b", 7, ReviewAction::Approve, "  ").unwrap();
        assert_eq!(a, ["pr", "review", "7", "--repo", "a/b", "--approve"]);
        let a = review_args("a/b", 7, ReviewAction::RequestChanges, "-x quebra o login").unwrap();
        assert_eq!(a.last().unwrap(), "--body=-x quebra o login");
        assert!(review_args("a/b", 7, ReviewAction::RequestChanges, "").is_err());
        assert!(review_args("--repo/x", 7, ReviewAction::Approve, "").is_err());
        assert!(review_args("semdono", 7, ReviewAction::Approve, "").is_err());
    }

    #[test]
    fn merge_is_pinned_to_the_commit_seen() {
        let sha = "d9509f03f9041094c12b577a17910f159176292d";
        let a = merge_args("IgorACNC/ai-monitor", 4, MergeMethod::Squash, sha).unwrap();
        assert_eq!(a[5], "--squash");
        assert_eq!(a[6], format!("--match-head-commit={sha}"));
        assert!(merge_args("IgorACNC/ai-monitor", 4, MergeMethod::Merge, "abc").is_err());
    }
}
