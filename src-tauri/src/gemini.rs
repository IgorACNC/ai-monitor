//! Uso do Gemini CLI, lido das conversas que ele grava em
//! `~/.gemini/tmp/<projeto>/chats/session-*.jsonl`, só para leitura. O arquivo é um registro em
//! que só se acrescenta: mensagens novas, trocas de dados da sessão (`$set`), voltas no tempo
//! (`$rewindTo`) e correções (`$patch`), reaplicados aqui na ordem.
//! O Gemini não guarda % do plano no computador; isso fica de fora.

use std::collections::HashMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::agents::{ACTIVE_WINDOW_MS, AgentSession, AgentUsage, DAY_MS, FileCache, Reply, files_in, iso_ms};

/// Os modelos Gemini atuais têm 1 milhão de tokens de contexto.
const GEMINI_CONTEXT: i64 = 1_048_576;

pub fn tmp_dir(home: &Path) -> PathBuf {
    home.join(".gemini").join("tmp")
}

struct Message {
    id: String,
    /// "user", "gemini", "info", "error"...
    kind: String,
    at: i64,
    model: Option<String>,
    tokens: Option<i64>,
    text: Option<String>,
}

fn message(v: &Value) -> Option<Message> {
    let content = v.get("content");
    let text = match content {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Array(parts)) => Some(parts.iter().filter_map(|p| p.get("text")?.as_str()).collect()),
        _ => None,
    };
    Some(Message {
        id: v.get("id")?.as_str()?.to_string(),
        kind: v.get("type").and_then(Value::as_str).unwrap_or_default().to_string(),
        at: iso_ms(v.get("timestamp")).unwrap_or(0),
        model: v.get("model").and_then(Value::as_str).map(str::to_string),
        tokens: v.pointer("/tokens/total").and_then(Value::as_i64),
        text,
    })
}

/// Uma conversa, depois de reaplicadas as mudanças gravadas no arquivo.
pub struct Chat {
    session_id: String,
    /// Id curto do projeto (nome da pasta em `tmp`).
    project: Option<String>,
    /// Conversa principal, se este arquivo for de um subagente (fica numa pasta com o id dela).
    parent: Option<String>,
    started_at: i64,
    last_updated: i64,
    summary: Option<String>,
    directory: Option<String>,
    messages: Vec<Message>,
}

fn put(messages: &mut Vec<Message>, m: Message) {
    match messages.iter_mut().find(|x| x.id == m.id) {
        Some(old) => *old = m,
        None => messages.push(m),
    }
}

fn apply_metadata(chat: &mut Chat, v: &Value) {
    if let Some(id) = v.get("sessionId").and_then(Value::as_str) {
        chat.session_id = id.to_string();
    }
    if let Some(t) = iso_ms(v.get("startTime")) {
        chat.started_at = t;
    }
    if let Some(t) = iso_ms(v.get("lastUpdated")) {
        chat.last_updated = t;
    }
    if let Some(s) = v.get("summary").and_then(Value::as_str) {
        chat.summary = Some(s.to_string());
    }
    if let Some(d) = v.pointer("/directories/0").and_then(Value::as_str) {
        chat.directory = Some(d.to_string());
    }
    if let Some(list) = v.get("messages").and_then(Value::as_array) {
        chat.messages = list.iter().filter_map(message).collect();
    }
}

fn parse(path: &Path) -> Option<Chat> {
    let file = std::fs::File::open(path).ok()?;
    let name = |d: Option<&Path>| d.and_then(Path::file_name).map(|n| n.to_string_lossy().to_string());
    let dir = path.parent();
    let (project, parent) = if name(dir).as_deref() == Some("chats") {
        (name(dir.and_then(Path::parent)), None)
    } else {
        (name(dir.and_then(Path::parent).and_then(Path::parent)), name(dir))
    };
    let mut chat = Chat {
        session_id: String::new(),
        project,
        parent,
        started_at: 0,
        last_updated: 0,
        summary: None,
        directory: None,
        messages: vec![],
    };
    for line in std::io::BufReader::new(file).lines() {
        let Ok(line) = line else { break };
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        if let Some(id) = v.get("$rewindTo").and_then(Value::as_str) {
            // Volta para antes dessa mensagem; sem ela, a conversa recomeça do zero.
            let keep = chat.messages.iter().position(|m| m.id == id).unwrap_or(0);
            chat.messages.truncate(keep);
        } else if let Some(patch) = v.get("$patch") {
            for id in patch.get("removeIds").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                chat.messages.retain(|m| m.id != id);
            }
        } else if let Some(set) = v.get("$set") {
            apply_metadata(&mut chat, set);
        } else if v.get("id").is_some_and(Value::is_string) {
            if let Some(m) = message(&v) {
                put(&mut chat.messages, m);
            }
        } else {
            apply_metadata(&mut chat, &v);
        }
    }
    (!chat.session_id.is_empty()).then_some(chat)
}

/// Pasta do projeto de cada id curto, pela lista que o Gemini mantém (`projects.json`).
fn project_dirs(home: &Path) -> HashMap<String, String> {
    let Ok(text) = std::fs::read_to_string(home.join(".gemini").join("projects.json")) else { return HashMap::new() };
    let Ok(v) = serde_json::from_str::<Value>(&text) else { return HashMap::new() };
    v.get("projects")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(dir, id)| Some((id.as_str()?.to_string(), dir.clone())))
        .collect()
}

fn short(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    match line.char_indices().nth(80) {
        Some((i, _)) => format!("{}…", &line[..i]),
        None => line.to_string(),
    }
}

/// `dirs`: pasta de cada projeto pelo id curto. Subagentes contam na conversa principal.
pub fn read(chats: &[&Chat], dirs: &HashMap<String, String>, now: i64, today_start: i64) -> AgentUsage {
    let roots: Vec<&str> = chats.iter().map(|c| c.parent.as_deref().unwrap_or(&c.session_id)).collect();
    let replies = |since: i64| {
        crate::agents::period(chats.iter().zip(&roots).flat_map(|(c, root)| {
            c.messages
                .iter()
                .filter(move |m| m.kind == "gemini" && m.at >= since)
                .map(|m| Reply { session_id: root, model: m.model.as_deref(), cost: None })
        }))
    };

    let mut groups: HashMap<&str, Vec<&Chat>> = HashMap::new();
    for (c, root) in chats.iter().zip(&roots) {
        groups.entry(root).or_default().push(c);
    }
    let last = |c: &Chat| c.messages.iter().map(|m| m.at).max().unwrap_or(0).max(c.last_updated);
    // Pergunta feita e ainda sem resposta.
    let waiting = |c: &Chat| {
        c.messages.iter().rev().find(|m| m.kind == "user" || m.kind == "gemini").is_some_and(|m| m.kind == "user")
    };
    let mut active: Vec<AgentSession> = groups
        .into_iter()
        .filter_map(|(root, list)| {
            let last_activity = list.iter().map(|c| last(c)).max()?;
            if now - last_activity > ACTIVE_WINDOW_MS {
                return None;
            }
            // Sem o arquivo da conversa principal (subagente sozinho), usa o mais antigo.
            let main = list.iter().find(|c| c.session_id == root).or_else(|| list.iter().min_by_key(|c| c.started_at))?;
            let answer = main.messages.iter().rev().find(|m| m.kind == "gemini");
            let model = answer.and_then(|m| m.model.clone());
            Some(AgentSession {
                id: root.to_string(),
                title: main
                    .summary
                    .clone()
                    .or_else(|| main.messages.iter().find(|m| m.kind == "user").and_then(|m| m.text.as_deref()).map(short))
                    .unwrap_or_default(),
                directory: main
                    .project
                    .as_ref()
                    .and_then(|p| dirs.get(p).cloned())
                    .or_else(|| main.directory.clone())
                    .unwrap_or_default(),
                busy: list.iter().any(|c| waiting(c)),
                started_at: main.started_at,
                last_activity,
                responses: list.iter().map(|c| c.messages.iter().filter(|m| m.kind == "gemini").count() as i64).sum(),
                cost: None,
                context_tokens: answer.and_then(|m| m.tokens).unwrap_or(0),
                context_limit: model.as_deref().filter(|m| m.starts_with("gemini-")).map(|_| GEMINI_CONTEXT),
                model,
            })
        })
        .collect();
    active.sort_by_key(|s| std::cmp::Reverse(s.last_activity));

    AgentUsage {
        installed: true,
        active,
        today: replies(today_start),
        week: replies(now - 7 * DAY_MS),
        limits: None,
        error: None,
    }
}

pub fn usage(home: &Path, cache: &FileCache<Chat>, now: i64, today_start: i64) -> AgentUsage {
    let tmp = tmp_dir(home);
    if !tmp.is_dir() {
        return AgentUsage::default();
    }
    let since = (now - 7 * DAY_MS).min(today_start);
    // tmp/<projeto>/chats/*.jsonl e, de subagentes, tmp/<projeto>/chats/<conversa principal>/*.jsonl.
    let mut paths = vec![];
    for project in std::fs::read_dir(&tmp).into_iter().flatten().flatten() {
        files_in(&project.path().join("chats"), "jsonl", 1, &mut paths);
    }
    paths.retain(|p| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .is_ok_and(|t| chrono::DateTime::<chrono::Utc>::from(t).timestamp_millis() >= since)
    });
    let chats = cache.read_all(paths, parse);
    let refs: Vec<&Chat> = chats.iter().map(|c| c.as_ref()).collect();
    read(&refs, &project_dirs(home), now, today_start)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000_000;
    const TODAY: i64 = NOW - 3 * 60 * 60 * 1000;

    fn iso(ms: i64) -> String {
        chrono::DateTime::from_timestamp_millis(ms).unwrap().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    }

    fn user(id: &str, at: i64, text: &str) -> String {
        format!(r#"{{"id":"{id}","timestamp":"{}","type":"user","content":[{{"text":"{text}"}}]}}"#, iso(at)) + "\n"
    }

    fn answer(id: &str, at: i64, model: &str, total: i64) -> String {
        format!(
            r#"{{"id":"{id}","timestamp":"{}","type":"gemini","content":"ok","model":"{model}","tokens":{{"input":1,"output":1,"cached":0,"total":{total}}}}}"#,
            iso(at)
        ) + "\n"
    }

    fn meta(id: &str, at: i64) -> String {
        format!(r#"{{"sessionId":"{id}","projectHash":"h","startTime":"{}","lastUpdated":"{}"}}"#, iso(at), iso(at)) + "\n"
    }

    fn fixture() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let chats = tmp.path().join(".gemini/tmp/meu-proj/chats");
        std::fs::create_dir_all(chats.join("s1")).unwrap();
        std::fs::write(tmp.path().join(".gemini/projects.json"), r#"{"projects":{"A:/meu-proj":"meu-proj"}}"#).unwrap();

        let t = NOW - 20 * 60_000;
        let mut main = meta("s1", t);
        main += &user("u1", t, "Explique o projeto\\noutra linha");
        main += &answer("g1", t + 1000, "gemini-3-pro", 5000);
        // Volta no tempo: a pergunta u2 e a resposta g2 são desfeitas.
        main += &user("u2", t + 2000, "e os testes?");
        main += &answer("g2", t + 3000, "gemini-3-pro", 9000);
        main += "{\"$rewindTo\":\"u2\"}\n";
        main += &user("u3", NOW - 120_000, "e os testes, agora sim?");
        main += &answer("g3", NOW - 110_000, "gemini-3-flash", 12000);
        main += &format!(r#"{{"$set":{{"lastUpdated":"{}","summary":"Entender o projeto"}}}}"#, iso(NOW - 110_000));
        main += "\n";
        main += &user("u4", NOW - 60_000, "mais uma");
        std::fs::write(chats.join("session-a-s1.jsonl"), main).unwrap();

        let mut sub = meta("sub1", NOW - 100_000);
        sub += &answer("x1", NOW - 90_000, "gemini-3-flash", 100);
        sub += "{\"$patch\":{\"removeIds\":[\"nao-existe\"]}}\n";
        std::fs::write(chats.join("s1").join("session-b-sub1.jsonl"), sub).unwrap();

        let old = NOW - 3 * DAY_MS;
        let mut prev = meta("s0", old);
        prev += &user("u1", old, "antiga");
        prev += &answer("g1", old + 1000, "gemini-2.5-pro", 10);
        std::fs::write(chats.join("session-c-s0.jsonl"), prev).unwrap();
        tmp
    }

    fn read_fixture(tmp: &Path) -> AgentUsage {
        let mut paths = vec![];
        files_in(&tmp.join(".gemini/tmp/meu-proj/chats"), "jsonl", 1, &mut paths);
        let parsed: Vec<Chat> = paths.iter().filter_map(|p| parse(p)).collect();
        let refs: Vec<&Chat> = parsed.iter().collect();
        read(&refs, &project_dirs(tmp), NOW, TODAY)
    }

    #[test]
    fn reads_active_session_after_rewind_and_with_subagent() {
        let tmp = fixture();
        let u = read_fixture(tmp.path());
        assert_eq!(u.active.len(), 1);
        let s = &u.active[0];
        assert_eq!((s.id.as_str(), s.title.as_str(), s.directory.as_str()), ("s1", "Entender o projeto", "A:/meu-proj"));
        assert!(s.busy, "a última pergunta ainda não tem resposta");
        assert_eq!(s.responses, 3, "g1 e g3, mais a do subagente; g2 foi desfeita");
        assert_eq!((s.context_tokens, s.context_limit), (12000, Some(GEMINI_CONTEXT)));
        assert_eq!(s.model.as_deref(), Some("gemini-3-flash"));
        assert_eq!(u.limits, None);
    }

    #[test]
    fn sums_today_and_week() {
        let tmp = fixture();
        let u = read_fixture(tmp.path());
        assert_eq!((u.today.sessions, u.today.responses), (1, 3));
        assert_eq!((u.week.sessions, u.week.responses), (2, 4));
        assert_eq!(u.week.cost, None);
    }

    #[test]
    fn title_falls_back_to_first_question() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("chats").join("x.jsonl");
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, meta("s", NOW) + &user("u", NOW, "Oi\\nlinha 2")).unwrap();
        let c = parse(&f).unwrap();
        let u = read(&[&c], &HashMap::new(), NOW, TODAY);
        assert_eq!(u.active[0].title, "Oi");
        assert!(u.active[0].busy);
    }

    #[test]
    fn missing_gemini_is_not_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let u = usage(tmp.path(), &FileCache::default(), NOW, TODAY);
        assert!(!u.installed);
        assert_eq!(u.error, None);
    }
}
