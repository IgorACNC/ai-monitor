mod account;
mod analysis;
mod commit_watch;
mod commits;
mod db;
mod github;
mod indexer;
mod live;
mod opencode;
mod parser;
mod pets;
mod prefs;
mod shell_path;
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
fn get_limits(state: State<AppState>, account: State<account::AccountState>) -> Option<statusline::Limits> {
    let from_account = account.last.lock().unwrap().as_ref().map(|u| u.limits.clone());
    account::newest(statusline::read_limits(&state.data_dir), from_account)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountStatus {
    enabled: bool,
    breakdown: Vec<account::ProductShare>,
    fetched_at: Option<i64>,
    error: Option<String>,
}

fn account_status(account: &account::AccountState) -> AccountStatus {
    let last = account.last.lock().unwrap();
    AccountStatus {
        enabled: account.enabled.get(),
        breakdown: last.as_ref().map(|u| u.breakdown.clone()).unwrap_or_default(),
        fetched_at: last.as_ref().map(|u| u.limits.captured_at),
        error: account.error.lock().unwrap().clone(),
    }
}

#[tauri::command]
fn get_account_usage(account: State<account::AccountState>) -> AccountStatus {
    account_status(&account)
}

/// Liga/desliga a consulta do uso da conta; ao ligar, já consulta uma vez.
#[tauri::command]
async fn set_account_usage(app: AppHandle, enabled: bool) -> Result<AccountStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let account = app.state::<account::AccountState>();
        account.enabled.set(enabled)?;
        if enabled {
            if let Some(dir) = &app.state::<AppState>().claude_dir {
                account.refresh(dir);
            }
        } else {
            *account.last.lock().unwrap() = None;
            *account.error.lock().unwrap() = None;
        }
        Ok(account_status(&account))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Consulta o uso da conta a cada poucos minutos, com a janela aberta ou não.
fn start_account_polling(app: AppHandle) {
    std::thread::spawn(move || loop {
        let account = app.state::<account::AccountState>();
        if account.enabled.get() {
            if let Some(dir) = &app.state::<AppState>().claude_dir {
                account.refresh(dir);
            }
        }
        std::thread::sleep(account::POLL);
    });
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
        let exe = app_exe().map_err(|e| e.to_string())?;
        statusline::enable(&settings, &state.data_dir, &exe)?;
    } else {
        statusline::disable(&settings, &state.data_dir)?;
    }
    Ok(statusline::status(&settings))
}

const COMMIT_DAYS: u32 = 90;

/// Roda `git` várias vezes; fica fora da thread principal para não travar a janela.
#[tauri::command]
async fn get_commits(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<commits::Commit>, String> {
    let dirs: Vec<PathBuf> = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        db::session_dirs(&conn)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(PathBuf::from)
            .collect()
    };
    tauri::async_runtime::spawn_blocking(move || {
        // Repositórios novos (de sessões novas) passam a ser observados também.
        if let Some(watcher) = app.try_state::<commit_watch::CommitWatcher>() {
            watcher.watch(&commits::git_dirs(&dirs));
        }
        commits::collect(&dirs, COMMIT_DAYS)
    })
    .await
    .map_err(|e| e.to_string())
}

/// Consulta a internet pelo `gh`; fica fora da thread principal para não travar a janela.
#[tauri::command]
async fn get_github_calendar() -> Result<github::Calendar, String> {
    tauri::async_runtime::spawn_blocking(github::fetch_calendar)
        .await
        .map_err(|e| e.to_string())?
}

/// PRs abertos que esperam você ou que você abriu; consulta a internet pelo `gh`.
#[tauri::command]
async fn get_pull_requests() -> Result<github::PullRequests, String> {
    tauri::async_runtime::spawn_blocking(github::fetch_pull_requests)
        .await
        .map_err(|e| e.to_string())?
}

/// Aprova ou pede mudanças num PR. A confirmação é feita na tela antes de chamar.
#[tauri::command]
async fn review_pull_request(repo: String, number: u64, action: github::ReviewAction, body: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || github::review(&repo, number, action, &body))
        .await
        .map_err(|e| e.to_string())?
}

/// Faz o merge de um PR, só se ele ainda estiver no commit `head_sha` que a pessoa viu.
#[tauri::command]
async fn merge_pull_request(repo: String, number: u64, method: github::MergeMethod, head_sha: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || github::merge(&repo, number, method, &head_sha))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn get_pr_analyses(state: State<AppState>) -> Result<Vec<analysis::Analysis>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    db::analyses(&conn).map_err(|e| e.to_string())
}

/// Roda o Claude Code ou o opencode sobre o PR (pode levar minutos) e guarda o parecer.
#[tauri::command]
async fn analyze_pull_request(
    app: AppHandle,
    repo: String,
    number: i64,
    analyzer: analysis::Analyzer,
) -> Result<analysis::Analysis, String> {
    let (dirs, data_dir) = {
        let state = app.state::<AppState>();
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        let dirs: Vec<PathBuf> = db::session_dirs(&conn).map_err(|e| e.to_string())?.into_iter().map(PathBuf::from).collect();
        (dirs, state.data_dir.clone())
    };
    tauri::async_runtime::spawn_blocking(move || {
        let now = chrono::Utc::now().timestamp_millis();
        let result = analysis::analyze(&repo, number, analyzer, &dirs, &data_dir, now)?;
        let state = app.state::<AppState>();
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        db::save_analysis(&conn, &result).map_err(|e| e.to_string())?;
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Sessões ativas e resumo de hoje e da semana do opencode (lê o banco dele, sem alterar).
#[tauri::command]
fn get_opencode_usage() -> opencode::OpencodeUsage {
    let home = dirs::home_dir().unwrap_or_default();
    let now = chrono::Local::now();
    let today_start = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|d| d.and_local_timezone(chrono::Local).earliest())
        .map_or(now.timestamp_millis(), |d| d.timestamp_millis());
    opencode::usage(&home, now.timestamp_millis(), today_start)
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

/// Caminho que a statusline do Claude Code chama. No Linux, o AppImage roda de uma pasta
/// temporária que muda a cada abertura; o caminho fixo é o do próprio arquivo `.AppImage`.
fn app_exe() -> std::io::Result<PathBuf> {
    match std::env::var_os("APPIMAGE") {
        Some(appimage) if !appimage.is_empty() => Ok(PathBuf::from(appimage)),
        _ => std::env::current_exe(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(not(windows))]
    shell_path::fix();

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
            if let (Some(dir), Ok(exe)) = (&claude_dir, app_exe()) {
                if let Err(e) = statusline::repoint(&dir.join("settings.json"), &data_dir, &exe) {
                    eprintln!("não foi possível atualizar a statusline: {e}");
                }
            }
            app.manage(widget::WidgetState::load(&data_dir));
            app.manage(account::AccountState::load(&data_dir));
            let handle = app.handle().clone();
            match commit_watch::CommitWatcher::new(move || {
                let _ = handle.emit("commits-changed", ());
            }) {
                Ok(watcher) => {
                    app.manage(watcher);
                }
                Err(e) => eprintln!("não foi possível observar os repositórios: {e}"),
            }
            app.manage(AppState {
                conn,
                claude_dir,
                pets_dir: data_dir.join("pets"),
                data_dir,
            });

            start_account_polling(app.handle().clone());
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
            get_account_usage,
            set_account_usage,
            get_capture_status,
            set_capture,
            get_commits,
            get_github_calendar,
            get_pull_requests,
            review_pull_request,
            merge_pull_request,
            get_pr_analyses,
            analyze_pull_request,
            get_opencode_usage,
            kill_session,
            show_main,
            get_widget_enabled,
            set_widget_enabled,
            list_pets,
            read_pet,
            add_pet,
            remove_pet
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_app, _event| {
            // Mac: clicar no ícone do Dock com a janela escondida traz ela de volta.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = _event {
                show_main_window(_app);
            }
        });
}
