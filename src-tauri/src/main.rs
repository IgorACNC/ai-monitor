// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Chamado pelo Claude Code como statusline: grava as porcentagens e sai, sem abrir janela.
    if let Some(code) = ai_monitor_lib::statusline::run_from_args() {
        std::process::exit(code);
    }
    ai_monitor_lib::run()
}
