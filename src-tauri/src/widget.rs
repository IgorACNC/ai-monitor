//! Barrinha de uso: janela pequena, sempre visível, que aparece quando a janela principal
//! é minimizada ou escondida (se o usuário ligou a opção).

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager, PhysicalPosition};

use crate::prefs::Toggle;

pub const LABEL: &str = "widget";
const MARGIN: i32 = 16;

pub struct WidgetState {
    enabled: Toggle,
    /// Posiciona na borda da tela só na primeira vez; depois respeita onde o usuário arrastou.
    placed: AtomicBool,
}

impl WidgetState {
    pub fn load(data_dir: &Path) -> Self {
        Self { enabled: Toggle::load(data_dir, "widget"), placed: AtomicBool::new(false) }
    }

    pub fn enabled(&self) -> bool {
        self.enabled.get()
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<(), String> {
        self.enabled.set(enabled)
    }
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
