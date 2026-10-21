//! Barrinha de uso: janela pequena, sempre visível, que aparece quando a janela principal
//! é minimizada ou escondida (se o usuário ligou a opção).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{json, Value};
use tauri::{AppHandle, Manager, PhysicalPosition};

pub const LABEL: &str = "widget";
const MARGIN: i32 = 16;

pub struct WidgetState {
    file: PathBuf,
    enabled: AtomicBool,
    /// Posiciona na borda da tela só na primeira vez; depois respeita onde o usuário arrastou.
    placed: AtomicBool,
}

impl WidgetState {
    pub fn load(data_dir: &Path) -> Self {
        let file = data_dir.join("ui.json");
        let enabled = read_enabled(&file);
        Self { file, enabled: AtomicBool::new(enabled), placed: AtomicBool::new(false) }
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<(), String> {
        write_enabled(&self.file, enabled)?;
        self.enabled.store(enabled, Ordering::Relaxed);
        Ok(())
    }
}

fn read_enabled(file: &Path) -> bool {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("widget").and_then(Value::as_bool))
        .unwrap_or(false)
}

fn write_enabled(file: &Path, enabled: bool) -> Result<(), String> {
    let mut v = std::fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    v["widget"] = Value::Bool(enabled);
    std::fs::write(file, v.to_string()).map_err(|e| format!("Não foi possível salvar a preferência: {e}"))
}

/// Mostra a barrinha quando a janela principal não está visível, e a esconde caso contrário.
pub fn sync(app: &AppHandle) {
    let (Some(main), Some(widget), Some(state)) =
        (app.get_webview_window("main"), app.get_webview_window(LABEL), app.try_state::<WidgetState>())
    else {
        return;
    };
    let main_visible = main.is_visible().unwrap_or(true) && !main.is_minimized().unwrap_or(false);
    if !state.enabled() || main_visible {
        let _ = widget.hide();
        return;
    }
    if !state.placed.swap(true, Ordering::Relaxed) {
        place_at_right_edge(&widget);
    }
    let _ = widget.show();
}

fn place_at_right_edge(widget: &tauri::WebviewWindow) {
    let (Ok(Some(monitor)), Ok(size)) = (widget.primary_monitor(), widget.outer_size()) else { return };
    let area = monitor.work_area();
    let x = area.position.x + area.size.width as i32 - size.width as i32 - MARGIN;
    let y = area.position.y + (area.size.height as i32 - size.height as i32) / 2;
    let _ = widget.set_position(PhysicalPosition::new(x, y));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preference_is_off_by_default_and_persists() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!WidgetState::load(tmp.path()).enabled());

        WidgetState::load(tmp.path()).set_enabled(true).unwrap();
        assert!(WidgetState::load(tmp.path()).enabled());

        WidgetState::load(tmp.path()).set_enabled(false).unwrap();
        assert!(!WidgetState::load(tmp.path()).enabled());
    }

    #[test]
    fn keeps_other_keys_and_survives_broken_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("ui.json");
        std::fs::write(&file, r#"{"outra":1}"#).unwrap();
        WidgetState::load(tmp.path()).set_enabled(true).unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(v, json!({"outra": 1, "widget": true}));

        std::fs::write(&file, "quebrado").unwrap();
        assert!(!WidgetState::load(tmp.path()).enabled());
    }
}
