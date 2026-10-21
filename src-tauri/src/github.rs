//! Gráfico de contribuições do GitHub, o mesmo do perfil, lido com o `gh` já autenticado
//! (o app não guarda token nenhum).

use std::process::Command;

use serde::Serialize;
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

pub fn fetch_calendar() -> Result<Calendar, String> {
    let mut cmd = Command::new("gh");
    cmd.args(["api", "graphql", "-f", &format!("query={QUERY}")]);
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
            format!("O GitHub recusou a consulta: {}", err.trim())
        });
    }
    parse_calendar(&String::from_utf8_lossy(&out.stdout))
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
}
