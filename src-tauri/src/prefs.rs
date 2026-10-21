//! Opções liga/desliga do app, guardadas em `ui.json` na pasta de dados.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{Value, json};

pub struct Toggle {
    file: PathBuf,
    key: &'static str,
    value: AtomicBool,
}

impl Toggle {
    /// Lê a opção `key`; desligada se o arquivo não existir ou estiver quebrado.
    pub fn load(data_dir: &Path, key: &'static str) -> Self {
        let file = data_dir.join("ui.json");
        let value = read_object(&file).get(key).and_then(Value::as_bool).unwrap_or(false);
        Self { file, key, value: AtomicBool::new(value) }
    }

    pub fn get(&self) -> bool {
        self.value.load(Ordering::Relaxed)
    }

    /// Salva no arquivo sem mexer nas outras opções guardadas nele.
    pub fn set(&self, value: bool) -> Result<(), String> {
        let mut v = read_object(&self.file);
        v[self.key] = Value::Bool(value);
        std::fs::write(&self.file, v.to_string())
            .map_err(|e| format!("Não foi possível salvar a preferência: {e}"))?;
        self.value.store(value, Ordering::Relaxed);
        Ok(())
    }
}

fn read_object(file: &Path) -> Value {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_by_default_and_persists() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!Toggle::load(tmp.path(), "widget").get());

        Toggle::load(tmp.path(), "widget").set(true).unwrap();
        assert!(Toggle::load(tmp.path(), "widget").get());

        Toggle::load(tmp.path(), "widget").set(false).unwrap();
        assert!(!Toggle::load(tmp.path(), "widget").get());
    }

    #[test]
    fn keys_are_independent_and_broken_file_is_off() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("ui.json");
        std::fs::write(&file, r#"{"outra":1}"#).unwrap();
        Toggle::load(tmp.path(), "widget").set(true).unwrap();
        Toggle::load(tmp.path(), "accountUsage").set(true).unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(v, json!({"outra": 1, "widget": true, "accountUsage": true}));

        std::fs::write(&file, "quebrado").unwrap();
        assert!(!Toggle::load(tmp.path(), "widget").get());
    }
}
