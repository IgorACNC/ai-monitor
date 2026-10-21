mod commits;
mod db;
mod indexer;
mod live;
mod parser;
mod pets;
pub mod statusline;
mod widget;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify_debouncer_mini::{DebounceEventResult, new_debouncer, notify::RecursiveMode};
use rusqlite::Connection;
use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};

struct AppState {
    conn: Arc<Mutex<Connection>>,
    claude_dir: Option<PathBuf>,
    pets_dir: PathBuf,
    data_dir: PathBuf,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveSession {
    #[serde(flatten)]
    entry: live::LiveEntry,
    #[serde(flatten)]
    detail: db::SessionDetail,
    /// Contexto da sessão, segundo a última statusline dela (precisa da captura ativa).
    context: Option<statusline::ContextUsage>,
}

#[tauri::command]
fn get_live_sessions(state: State<AppState>) -> Result<Vec<LiveSession>, String> {
    let Some(claude_dir) = &state.claude_dir else {
        return Ok(Vec::new());
    };
    let entries = live::read_live(claude_dir);
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    entries
        .into_iter()
        .map(|entry| {
            let detail = db::session_detail(&conn, &entry.session_id).map_err(|e| e.to_string())?;
            let context = statusline::read_context(&state.data_dir, &entry.session_id);
            Ok(LiveSession {
                entry,
                detail,
                context,
            })
        })
        .collect()
}

#[tauri::command]
fn kill_session(state: State<AppState>, pid: u32, session_id: String) -> Result<(), String> {
    let claude_dir = state
        .claude_dir
        .as_ref()
        .ok_or("Pasta do Claude Code não encontrada.")?;
    live::kill(claude_dir, pid, &session_id)
}

#[tauri::command]
fn get_limits(state: State<AppState>) -> Option<statusline::Limits> {
    statusline::read_limits(&state.data_dir)
}

fn settings_path(state: &AppState) -> Result<PathBuf, String> {
    let dir = state
        .claude_dir
        .as_ref()
        .ok_or("Pasta do Claude Code não encontrada.")?;
    Ok(dir.join("settings.json"))
}

#[tauri::command]
fn get_capture_status(state: State<AppState>) -> Result<statusline::CaptureStatus, String> {
    Ok(statusline::status(&settings_path(&state)?))
}

#[tauri::command]
fn set_capture(state: State<AppState>, enabled: bool) -> Result<statusline::CaptureStatus, String> {
    let settings = settings_path(&state)?;
    if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        statusline::enable(&settings, &state.data_dir, &exe)?;
    } else {
        statusline::disable(&settings, &state.data_dir)?;
    }
    Ok(statusline::status(&settings))
}

const COMMIT_DAYS: u32 = 90;

/// Roda `git` várias vezes; fica fora da thread principal para não travar a janela.
#[tauri::command]
async fn get_commits(state: State<'_, AppState>) -> Result<Vec<commits::Commit>, String> {
    let dirs: Vec<PathBuf> = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        db::session_dirs(&conn)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(PathBuf::from)
            .collect()
    };
    tauri::async_runtime::spawn_blocking(move || commits::collect(&dirs, COMMIT_DAYS))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_pets(state: State<AppState>) -> Vec<String> {
    pets::list(&state.pets_dir)
}

/// Devolve os bytes do GIF direto, sem passar por JSON.
#[tauri::command]
fn read_pet(state: State<AppState>, name: String) -> Result<tauri::ipc::Response, String> {
    pets::read(&state.pets_dir, &name).map(tauri::ipc::Response::new)
}

#[tauri::command]
fn add_pet(state: State<AppState>, path: String) -> Result<String, String> {
    pets::add(&state.pets_dir, std::path::Path::new(&path))
}

#[tauri::command]
fn remove_pet(state: State<AppState>, name: String) -> Result<(), String> {
    pets::remove(&state.pets_dir, &name)
}

/// Indexa tudo uma vez e depois reindexa cada `.jsonl` que mudar, avisando a interface.
fn start_indexing(app: AppHandle, conn: Arc<Mutex<Connection>>, projects_dir: PathBuf) {
    std::thread::spawn(move || {
        if let Err(e) = indexer::index_all(&mut conn.lock().unwrap(), &projects_dir) {
            eprintln!("falha na indexação inicial: {e}");
        }
        let _ = app.emit("usage-updated", ());

        let watch_conn = conn.clone();
        let watch_dir = projects_dir.clone();
        let watch_app = app.clone();
        let debouncer = new_debouncer(
            Duration::from_millis(500),
            move |res: DebounceEventResult| {
                let Ok(events) = res else { return };
                let mut changed = false;
                for event in events.iter().filter(|e| indexer::is_jsonl(&e.path)) {
                    match indexer::index_file(
                        &mut watch_conn.lock().unwrap(),
                        &watch_dir,
                        &event.path,
                    ) {
                        Ok(()) => changed = true,
                        Err(e) => eprintln!("falha ao indexar {}: {e}", event.path.display()),
                    }
                }
                if changed {
                    let _ = watch_app.emit("usage-updated", ());
                }
            },
        );
        match debouncer {
            Ok(mut debouncer) => {
                if let Err(e) = debouncer
                    .watcher()
                    .watch(&projects_dir, RecursiveMode::Recursive)
                {
                    eprintln!("não foi possível observar {}: {e}", projects_dir.display());
                }
                // O watcher precisa viver enquanto o app estiver aberto.
                app.manage(debouncer);
            }
            Err(e) => eprintln!("não foi possível iniciar o watcher: {e}"),
        }
    });
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    widget::sync(app);
}

#[tauri::command]
fn show_main(app: AppHandle) {
    show_main_window(&app);
}

#[tauri::command]
fn get_widget_enabled(state: State<widget::WidgetState>) -> bool {
    state.enabled()
}

#[tauri::command]
fn set_widget_enabled(app: AppHandle, state: State<widget::WidgetState>, enabled: bool) -> Result<(), String> {
    state.set_enabled(enabled)?;
    widget::sync(&app);
    Ok(())
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Abrir", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("AI Monitor")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Abrir o app de novo (pelo atalho, por exemplo) só traz de volta a janela já aberta.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main_window(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let conn = Arc::new(Mutex::new(db::open(&data_dir.join("ai-monitor.db"))?));

            let claude_dir = indexer::default_claude_dir().filter(|d| d.is_dir());
            let projects_dir = claude_dir
                .as_ref()
                .map(|d| d.join("projects"))
                .filter(|d| d.is_dir());
            if let Some(dir) = &projects_dir {
                start_indexing(app.handle().clone(), conn.clone(), dir.clone());
            }
            // O app instalado assume a captura das porcentagens, mesmo que ela tenha sido
            // ligada pela versão de desenvolvimento (cuja pasta pode ser apagada).
            #[cfg(not(debug_assertions))]
            if let (Some(dir), Ok(exe)) = (&claude_dir, std::env::current_exe()) {
                if let Err(e) = statusline::repoint(&dir.join("settings.json"), &data_dir, &exe) {
                    eprintln!("não foi possível atualizar a statusline: {e}");
                }
            }
            app.manage(widget::WidgetState::load(&data_dir));
            app.manage(AppState {
                conn,
                claude_dir,
                pets_dir: data_dir.join("pets"),
                data_dir,
            });

            setup_tray(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            match event {
                // Fechar a janela só a esconde: o monitoramento continua na bandeja.
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window.hide();
                    widget::sync(window.app_handle());
                }
                // Minimizar e restaurar chegam como mudança de tamanho.
                WindowEvent::Resized(_) | WindowEvent::Focused(true) => widget::sync(window.app_handle()),
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_live_sessions,
            get_limits,
            get_capture_status,
            set_capture,
            get_commits,
            kill_session,
            show_main,
            get_widget_enabled,
            set_widget_enabled,
            list_pets,
            read_pet,
            add_pet,
            remove_pet
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
